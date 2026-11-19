//! `LootFabTileEntity`: one tick of the Loot Fabricator, and the selections its menu buttons make. A prediction, FE
//! and a chosen drop give that drop of the prediction's model every 60 ticks.
use crate::{
    config::Config,
    model::{Model, Models, PREDICTION},
    sim::{Ctx, Redstone, Stack},
};
use std::collections::BTreeMap;

pub const PREDICTION_IN: usize = 0;
/// The sixteen output slots are 1 to 16.
pub const SLOTS: usize = 17;
/// The ticks a drop takes, the energy being there.
pub const RUNTIME: i32 = 60;

/// Menu button ids (`LootFabMenu`).
pub const BTN_CLEAR_QUEUE: i32 = -3;
pub const BTN_CYCLE_MODE: i32 = -2;
pub const BTN_CLEAR_FIXED: i32 = -1;
/// Ids from here are drop indices (selected in fixed mode, appended in queue mode).
pub const DROP_BASE: i32 = 0;
/// From here, a queue position to remove.
pub const QUEUE_REMOVE_BASE: i32 = 1000;
/// From here, a redstone mode to set.
pub const REDSTONE_BASE: i32 = 2000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Production {
    #[default]
    Fixed,
    Queue,
}

/// `FabSelection`: how one model's drops are chosen, the indices into its fabricator drops, and the queue's cursor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub mode: Production,
    pub entries: Vec<i32>,
    pub cursor: i32,
}

impl Selection {
    pub fn fixed(index: i32) -> Self {
        Self { mode: Production::Fixed, entries: vec![index], cursor: 0 }
    }

    /// The drop index chosen now, or -1.
    pub fn current(&self) -> i32 {
        if self.entries.is_empty() { -1 } else { self.entries[self.cursor.rem_euclid(self.entries.len() as i32) as usize] }
    }

    fn advanced(&self) -> Self {
        if self.mode == Production::Fixed || self.entries.is_empty() {
            return self.clone();
        }
        Self { cursor: (self.cursor + 1) % self.entries.len() as i32, ..self.clone() }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fab {
    pub slots: [Option<Stack>; SLOTS],
    pub energy: i32,
    pub runtime: i32,
    pub current_sel: i32,
    pub redstone: Redstone,
    /// By model id.
    pub selections: BTreeMap<String, Selection>,
}

/// `InternalItemHandler.insertItemInternal` on one slot: what is left of `stack`.
fn insert(slot: &mut Option<Stack>, stack: Stack, max: u32, simulate: bool) -> Option<Stack> {
    match slot {
        None => {
            let (put, rest) = (stack.count.min(max), stack.count.saturating_sub(max));
            if !simulate {
                *slot = Some(Stack { count: put, ..stack.clone() });
            }
            (rest > 0).then(|| Stack { count: rest, ..stack })
        }
        Some(held) if held.same(&stack) => {
            let room = max.saturating_sub(held.count);
            let put = stack.count.min(room);
            if !simulate {
                held.count += put;
            }
            (stack.count > put).then(|| Stack { count: stack.count - put, ..stack })
        }
        Some(_) => Some(stack),
    }
}

impl Fab {
    /// The model of the prediction in the input slot.
    fn model<'a>(&self, models: &'a Models) -> Option<&'a Model> {
        let s = self.slots[PREDICTION_IN].as_ref().filter(|s| s.count > 0 && s.item == PREDICTION)?;
        models.get(s.model.as_deref()?)
    }

    pub fn selection(&self, model: &str) -> Selection {
        self.selections.get(model).cloned().unwrap_or_default()
    }

    /// `getSelectedDrop`: the chosen drop's index, or -1 when none is set or it is past the model's drops.
    pub fn selected_drop(&self, model: &Model) -> i32 {
        let i = self.selections.get(&model.id).map_or(-1, Selection::current);
        if i < 0 || i as usize >= model.fab_drops.len() { -1 } else { i }
    }

    fn insert_in_output(&mut self, c: &Ctx<'_>, drop: Stack, simulate: bool) -> bool {
        let max = (c.max_stack)(&drop.item);
        let mut left = Some(drop);
        for i in 1..SLOTS {
            let Some(stack) = left.take() else { break };
            left = insert(&mut self.slots[i], stack, max, simulate);
        }
        left.is_none()
    }

    /// `serverTick`. The cost is `fabPowerCost` FE a tick for the 60 ticks of a drop.
    pub fn tick(&mut self, c: &Ctx<'_>, powered: bool) {
        let Some(model) = self.model(c.models) else {
            self.runtime = 0;
            return;
        };
        let selection = self.selected_drop(model);
        if self.current_sel != selection {
            self.current_sel = selection;
            self.runtime = 0;
            return;
        }
        if selection == -1 {
            self.runtime = 0;
        } else if !self.redstone.matches(powered) {
        } else if self.runtime >= RUNTIME {
            let d = &model.fab_drops[selection as usize];
            let out = Stack::new(&d.item, d.count);
            if self.insert_in_output(c, out.clone(), true) {
                self.runtime = 0;
                self.insert_in_output(c, out, false);
                if let Some(p) = self.slots[PREDICTION_IN].as_mut() {
                    p.count -= 1;
                }
                if self.slots[PREDICTION_IN].as_ref().is_some_and(|p| p.count == 0) {
                    self.slots[PREDICTION_IN] = None;
                }
                if let Some(s) = self.selections.get(&model.id) {
                    if s.mode == Production::Queue {
                        let next = s.advanced();
                        self.selections.insert(model.id.clone(), next);
                    }
                }
            }
        } else if self.energy >= c.cfg.fab_power_cost {
            self.energy -= c.cfg.fab_power_cost;
            self.runtime += 1;
        }
    }

    /// `LootFabMenu.clickMenuButton`: true when the id was one of its buttons.
    pub fn button(&mut self, models: &Models, cfg: &Config, id: i32) -> bool {
        let _ = cfg;
        if (REDSTONE_BASE..REDSTONE_BASE + 3).contains(&id) {
            self.redstone = Redstone::from_index(id - REDSTONE_BASE);
            return true;
        }
        let Some(model) = self.model(models) else { return false };
        let key = model.id.clone();
        let drops = model.fab_drops.len() as i32;
        let sel = self.selection(&key);
        match id {
            BTN_CLEAR_QUEUE => {
                if sel.mode == Production::Queue {
                    self.selections.insert(key, Selection { mode: Production::Queue, entries: vec![], cursor: 0 });
                }
            }
            BTN_CYCLE_MODE => {
                // `cycleMode`: the fixed pick becomes the queue's first entry, and the queue's current one the fixed pick.
                let updated = if sel.mode == Production::Fixed {
                    Selection { mode: Production::Queue, entries: sel.entries.clone(), cursor: 0 }
                } else if sel.current() == -1 {
                    Selection::default()
                } else {
                    Selection::fixed(sel.current())
                };
                self.selections.insert(key, updated);
                self.runtime = 0;
            }
            BTN_CLEAR_FIXED => {
                self.selections.remove(&key);
            }
            QUEUE_REMOVE_BASE..REDSTONE_BASE => {
                let pos = (id - QUEUE_REMOVE_BASE) as usize;
                if pos >= sel.entries.len() {
                    return true;
                }
                let mut entries = sel.entries.clone();
                entries.remove(pos);
                let mut cursor = sel.cursor;
                if (pos as i32) < cursor {
                    cursor -= 1;
                }
                cursor = if entries.is_empty() { 0 } else { cursor.rem_euclid(entries.len() as i32) };
                self.selections.insert(key, Selection { mode: Production::Queue, entries, cursor });
            }
            DROP_BASE..QUEUE_REMOVE_BASE if id < drops => {
                if sel.mode == Production::Queue {
                    let mut entries = sel.entries.clone();
                    entries.push(id);
                    self.selections.insert(key, Selection { mode: Production::Queue, entries, cursor: sel.cursor });
                } else {
                    self.selections.insert(key, Selection::fixed(id));
                }
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::tests::zombie, tier::Tiers};

    fn fab(energy: i32) -> Fab {
        Fab { slots: std::array::from_fn(|i| (i == 0).then(|| Stack::prediction("hostilenetworks:zombie", 2))), energy, current_sel: -1, ..Fab::default() }
    }

    struct World {
        cfg: Config,
        tiers: Tiers,
        models: Models,
    }

    fn world() -> World {
        World { cfg: Config::default(), tiers: Tiers::defaults(), models: Models::from([("hostilenetworks:zombie".into(), zombie())]) }
    }

    fn step(f: &mut Fab, w: &World, n: i32) {
        let c = Ctx { cfg: &w.cfg, tiers: &w.tiers, models: &w.models, max_stack: &|_| 64 };
        for _ in 0..n {
            f.tick(&c, false);
        }
    }

    #[test]
    fn nothing_is_made_until_a_drop_is_chosen() {
        let (w, mut f) = (world(), fab(1_000_000));
        step(&mut f, &w, 100);
        assert_eq!((f.runtime, f.energy, f.slots[1].clone()), (0, 1_000_000, None));
        assert!(f.button(&w.models, &w.cfg, 0));
        step(&mut f, &w, 1);
        assert_eq!(f.current_sel, 0, "the first tick only notes the choice");
        step(&mut f, &w, 60);
        assert_eq!((f.runtime, f.energy), (60, 1_000_000 - 60 * 256));
        step(&mut f, &w, 1);
        assert_eq!(f.slots[1], Some(Stack::new("minecraft:rotten_flesh", 64)));
        assert_eq!((f.runtime, f.slots[0].as_ref().unwrap().count), (0, 1));
    }

    #[test]
    fn a_full_stack_goes_on_to_the_next_slot() {
        let (w, mut f) = (world(), fab(1_000_000));
        f.slots[1] = Some(Stack::new("minecraft:rotten_flesh", 40));
        f.button(&w.models, &w.cfg, 0);
        step(&mut f, &w, 62);
        assert_eq!((f.slots[1].as_ref().map(|s| s.count), f.slots[2].as_ref().map(|s| s.count)), (Some(64), Some(40)));
    }

    #[test]
    fn it_waits_without_energy_and_without_room() {
        let (w, mut f) = (world(), fab(255));
        f.button(&w.models, &w.cfg, 0);
        step(&mut f, &w, 10);
        assert_eq!(f.runtime, 0);
        let mut f = fab(1_000_000);
        for i in 1..SLOTS {
            f.slots[i] = Some(Stack::new("minecraft:stone", 64));
        }
        f.button(&w.models, &w.cfg, 0);
        step(&mut f, &w, 80);
        assert_eq!((f.runtime, f.slots[0].as_ref().unwrap().count), (60, 2), "a finished drop waits for room and keeps its prediction");
    }

    #[test]
    fn a_queue_steps_through_its_entries() {
        let (w, mut f) = (world(), fab(10_000_000));
        let b = |f: &mut Fab, id| assert!(f.button(&w.models, &w.cfg, id));
        b(&mut f, BTN_CYCLE_MODE);
        b(&mut f, 1);
        b(&mut f, 0);
        assert_eq!(f.selection("hostilenetworks:zombie").entries, [1, 0]);
        f.slots[0] = Some(Stack::prediction("hostilenetworks:zombie", 2));
        step(&mut f, &w, 62);
        assert_eq!(f.slots[1], Some(Stack::new("reliquary:zombie_heart", 2)));
        assert_eq!(f.selection("hostilenetworks:zombie").cursor, 1);
        b(&mut f, QUEUE_REMOVE_BASE);
        assert_eq!(f.selection("hostilenetworks:zombie").entries, [0]);
        b(&mut f, BTN_CLEAR_QUEUE);
        assert!(f.selection("hostilenetworks:zombie").entries.is_empty());
        b(&mut f, BTN_CYCLE_MODE);
        assert_eq!(f.selection("hostilenetworks:zombie"), Selection::default());
    }

    #[test]
    fn redstone_buttons_work_without_a_model() {
        let (w, mut f) = (world(), Fab::default());
        assert!(f.button(&w.models, &w.cfg, REDSTONE_BASE + 2));
        assert_eq!(f.redstone, Redstone::OnWhenPowered);
        assert!(!f.button(&w.models, &w.cfg, BTN_CYCLE_MODE), "a queue needs a prediction");
    }
}
