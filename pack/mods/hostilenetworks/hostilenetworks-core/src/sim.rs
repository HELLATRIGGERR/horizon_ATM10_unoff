//! `SimChamberTileEntity`: one tick of the Simulation Chamber. A data model, a prediction matrix and FE give a base
//! drop and, by the model's accuracy, predictions (inference), or one more point of data on the model (training).
use crate::{
    config::Config,
    model::{Instance, Model, Models, PREDICTION},
    tier::Tiers,
};

pub const MODEL: usize = 0;
pub const INPUT: usize = 1;
pub const BASE_OUT: usize = 2;
pub const PREDICTION_OUT: usize = 3;
pub const SLOTS: usize = 4;

/// An item stack as the machines see it: the item, its count and the three components that matter here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
    /// `hostilenetworks:data_model`: the model a data model or a prediction is of.
    pub model: Option<String>,
    /// `hostilenetworks:data`.
    pub data: i32,
    /// `hostilenetworks:iterations`.
    pub iters: i32,
}

impl Stack {
    pub fn new(item: &str, count: u32) -> Self {
        Self { item: item.into(), count, ..Self::default() }
    }

    /// `DataModel.getPredictionDrop`.
    pub fn prediction(model: &str, count: u32) -> Self {
        Self { item: PREDICTION.into(), count, model: Some(model.into()), ..Self::default() }
    }

    /// `ItemStack.isSameItemSameComponents`.
    pub fn same(&self, other: &Self) -> bool {
        self.item == other.item && self.model == other.model && self.data == other.data && self.iters == other.iters
    }
}

/// What the rules read of the world: the config, the tiers, the models and an item's stack size.
pub struct Ctx<'a> {
    pub cfg: &'a Config,
    pub tiers: &'a Tiers,
    pub models: &'a Models,
    pub max_stack: &'a dyn Fn(&str) -> u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Redstone {
    #[default]
    Ignored,
    OffWhenPowered,
    OnWhenPowered,
}

impl Redstone {
    pub const ALL: [Self; 3] = [Self::Ignored, Self::OffWhenPowered, Self::OnWhenPowered];

    pub fn matches(self, power: bool) -> bool {
        match self {
            Self::Ignored => true,
            Self::OffWhenPowered => !power,
            Self::OnWhenPowered => power,
        }
    }

    pub fn from_index(i: i32) -> Self {
        Self::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()
    }

    pub fn index(self) -> i32 {
        self as i32
    }

    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % 3)
    }
}

/// `SimChamberTileEntity.FailureState`, in the order the menu's data carries it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fail {
    #[default]
    None,
    Output,
    Energy,
    Input,
    Model,
    Faulty,
    EnergyMidCycle,
    Redstone,
    CannotTrain,
}

impl Fail {
    pub const ALL: [Self; 9] =
        [Self::None, Self::Output, Self::Energy, Self::Input, Self::Model, Self::Faulty, Self::EnergyMidCycle, Self::Redstone, Self::CannotTrain];

    pub fn from_index(i: i32) -> Self {
        Self::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()
    }

    pub fn index(self) -> i32 {
        self as i32
    }

    /// The language key of the line the screen shows.
    pub fn key(self) -> &'static str {
        match self {
            Self::None => "hostilenetworks.fail.none",
            Self::Output => "hostilenetworks.fail.output",
            Self::Energy => "hostilenetworks.fail.energy",
            Self::Input => "hostilenetworks.fail.input",
            Self::Model => "hostilenetworks.fail.model",
            Self::Faulty => "hostilenetworks.fail.faulty",
            Self::EnergyMidCycle => "hostilenetworks.fail.energy_mid_cycle",
            Self::Redstone => "hostilenetworks.fail.redstone",
            Self::CannotTrain => "hostilenetworks.fail.cannot_train",
        }
    }
}

/// `SimMode`: inference makes loot and predictions 25% faster, training makes the model's data and costs 20% more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Inference,
    Training,
}

impl Mode {
    /// `SimMode.BASE_RUNTIME`.
    pub const BASE_RUNTIME: i32 = 300;

    pub fn from_index(i: i32) -> Self {
        if i == 1 { Self::Training } else { Self::Inference }
    }

    pub fn index(self) -> i32 {
        self as i32
    }

    pub fn speed(self) -> f32 {
        match self {
            Self::Inference => 1.25,
            Self::Training => 1.0,
        }
    }

    pub fn cost_multiplier(self) -> f32 {
        match self {
            Self::Inference => 1.0,
            Self::Training => 1.2,
        }
    }

    /// The length of a run in ticks: 240 and 300.
    pub fn runtime(self) -> i32 {
        (Self::BASE_RUNTIME as f32 / self.speed()).round() as i32
    }

    /// `adjustCost`: a model's FE a tick in this mode.
    pub fn adjust_cost(self, base: i32) -> i32 {
        (base as f32 * self.cost_multiplier()).ceil() as i32
    }

    pub fn next(self) -> Self {
        match self {
            Self::Inference => Self::Training,
            Self::Training => Self::Inference,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chamber {
    pub slots: [Option<Stack>; SLOTS],
    pub energy: i32,
    pub runtime: i32,
    /// The predictions this run will make; 0 is a failed one.
    pub prediction_success: i32,
    pub fail: Fail,
    pub redstone: Redstone,
    pub mode: Mode,
    /// The model of the item in slot 0 the runtime belongs to.
    pub current: Option<String>,
}

fn can_stack(held: &Option<Stack>, want: &Stack, max: u32) -> bool {
    held.as_ref().is_none_or(|h| h.same(want) && h.count < max)
}

impl Chamber {
    /// The model of slot 0, when the item holds one the game has.
    fn model<'a>(&self, c: &'a Ctx<'_>) -> Option<(&'a Model, i32)> {
        let s = self.slots[MODEL].as_ref().filter(|s| s.count > 0)?;
        Some((c.models.get(s.model.as_deref()?)?, s.data))
    }

    /// `hasPowerFor`.
    pub fn has_power_for(&self, model: &Model) -> bool {
        self.energy >= self.mode.adjust_cost(model.sim_cost)
    }

    /// `canUpgrade`: training needs a tier above and, with config 2, no step past the next tier's data.
    pub fn can_upgrade(c: &Ctx<'_>, inst: &Instance<'_>) -> bool {
        if inst.is_max() || c.cfg.sim_model_upgrade == 0 {
            return false;
        }
        !(c.cfg.sim_model_upgrade == 2 && inst.data + 1 > inst.next_tier_data())
    }

    /// `canStartSimulation`: the input, the redstone, the training, room in both outputs and the power.
    fn can_start(&mut self, c: &Ctx<'_>, model: &Model, data: i32, powered: bool) -> bool {
        let matrix = self.slots[INPUT].as_ref().is_some_and(|s| s.count > 0 && model.input_matches(&s.item));
        self.fail = if !matrix {
            Fail::Input
        } else if !self.redstone.matches(powered) {
            Fail::Redstone
        } else if self.mode == Mode::Training && !Self::can_upgrade(c, &Instance::new(model, c.tiers, c.cfg.continuous_accuracy, data)) {
            Fail::CannotTrain
        } else {
            let base = Stack::new(&model.base_drop.item, model.base_drop.count);
            let pred = Stack::prediction(&model.id, 1);
            if !(can_stack(&self.slots[BASE_OUT], &base, (c.max_stack)(&base.item)) && can_stack(&self.slots[PREDICTION_OUT], &pred, (c.max_stack)(&pred.item))) {
                Fail::Output
            } else if !self.has_power_for(model) {
                Fail::Energy
            } else {
                Fail::None
            }
        };
        self.fail == Fail::None
    }

    /// `serverTick`. `powered` is `hasNeighborSignal`; `roll` is `random.nextFloat()`, used once, when a run starts.
    pub fn tick(&mut self, c: &Ctx<'_>, powered: bool, roll: f32) {
        if self.mode == Mode::Training && c.cfg.sim_model_upgrade == 0 {
            self.mode = Mode::Inference;
        }
        let Some((model, data)) = self.model(c) else {
            self.fail = Fail::Model;
            self.runtime = 0;
            return;
        };
        if self.current.as_deref() != Some(&model.id) {
            self.runtime = 0;
            self.current = Some(model.id.clone());
        }
        let inst = Instance::new(model, c.tiers, c.cfg.continuous_accuracy, data);
        if !inst.tier().can_sim {
            self.fail = Fail::Faulty;
            self.runtime = 0;
            return;
        }
        if self.runtime == 0 {
            if self.can_start(c, model, data, powered) {
                self.runtime = self.mode.runtime();
                self.prediction_success = inst.roll_predictions(roll);
                if let Some(m) = self.slots[INPUT].as_mut() {
                    m.count -= 1;
                }
                if self.slots[INPUT].as_ref().is_some_and(|m| m.count == 0) {
                    self.slots[INPUT] = None;
                }
            }
        } else if !self.has_power_for(model) {
            self.fail = Fail::EnergyMidCycle;
        } else if !self.redstone.matches(powered) {
            self.fail = Fail::Redstone;
        } else {
            self.fail = Fail::None;
            self.runtime -= 1;
            if self.runtime == 0 {
                self.finish(c, model, &inst);
            } else {
                self.energy -= self.mode.adjust_cost(model.sim_cost);
            }
        }
    }

    /// The end of a run: inference puts the base drop and the predictions out, training adds a point of data.
    fn finish(&mut self, c: &Ctx<'_>, model: &Model, inst: &Instance<'_>) {
        if self.mode == Mode::Inference {
            match self.slots[BASE_OUT].as_mut() {
                Some(out) => out.count += 1,
                None => self.slots[BASE_OUT] = Some(Stack::new(&model.base_drop.item, model.base_drop.count)),
            }
            if self.prediction_success > 0 {
                let n = self.prediction_success as u32;
                match self.slots[PREDICTION_OUT].as_mut() {
                    Some(out) => out.count += n,
                    None => self.slots[PREDICTION_OUT] = Some(Stack::prediction(&model.id, n)),
                }
            }
        }
        let upgrade = self.mode == Mode::Training && Self::can_upgrade(c, inst);
        if let Some(m) = self.slots[MODEL].as_mut() {
            if upgrade {
                m.data += 1;
            }
            m.iters += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::zombie;

    struct World {
        cfg: Config,
        tiers: Tiers,
        models: Models,
    }

    impl World {
        fn new() -> Self {
            Self { cfg: Config::default(), tiers: Tiers::defaults(), models: Models::from([("hostilenetworks:zombie".into(), zombie())]) }
        }

        fn ctx(&self) -> Ctx<'_> {
            Ctx { cfg: &self.cfg, tiers: &self.tiers, models: &self.models, max_stack: &|_| 64 }
        }
    }

    fn loaded(data: i32, energy: i32) -> Chamber {
        let mut model = Stack::new("hostilenetworks:data_model", 1);
        (model.model, model.data) = (Some("hostilenetworks:zombie".into()), data);
        Chamber {
            slots: [Some(model), Some(Stack::new("hostilenetworks:prediction_matrix", 3)), None, None],
            energy,
            ..Chamber::default()
        }
    }

    fn run(c: &mut Chamber, w: &World, ticks: i32, roll: f32) {
        for _ in 0..ticks {
            c.tick(&w.ctx(), false, roll);
        }
    }

    #[test]
    fn the_modes_have_the_sources_numbers() {
        assert_eq!((Mode::Inference.runtime(), Mode::Training.runtime()), (240, 300));
        assert_eq!((Mode::Inference.adjust_cost(128), Mode::Training.adjust_cost(128), Mode::Training.adjust_cost(100)), (128, 154, 121));
        // 100 * 1.2f is 120.0000076 in a float, as in Java: the ceiling is 121.
    }

    #[test]
    fn a_faulty_model_does_not_simulate() {
        let (w, mut c) = (World::new(), loaded(5, 100_000));
        run(&mut c, &w, 3, 0.0);
        assert_eq!((c.fail, c.runtime, c.slots[INPUT].as_ref().unwrap().count), (Fail::Faulty, 0, 3));
    }

    #[test]
    fn an_inference_run_makes_the_base_drop_and_a_prediction() {
        let (w, mut c) = (World::new(), loaded(6, 100_000));
        // Basic is 5% accurate: a roll of 0.01 succeeds.
        run(&mut c, &w, 1, 0.01);
        assert_eq!((c.runtime, c.prediction_success, c.slots[INPUT].as_ref().unwrap().count), (240, 1, 2));
        run(&mut c, &w, 239, 0.9);
        assert_eq!((c.runtime, c.energy), (1, 100_000 - 239 * 128));
        run(&mut c, &w, 1, 0.9);
        // The last tick of a run costs nothing.
        assert_eq!((c.runtime, c.energy), (0, 100_000 - 239 * 128));
        assert_eq!(c.slots[BASE_OUT], Some(Stack::new("hostilenetworks:overworld_prediction", 1)));
        assert_eq!(c.slots[PREDICTION_OUT], Some(Stack::prediction("hostilenetworks:zombie", 1)));
        assert_eq!(c.slots[MODEL].as_ref().unwrap().iters, 1);
        assert_eq!(c.slots[MODEL].as_ref().unwrap().data, 6, "inference adds no data");
    }

    #[test]
    fn a_failed_prediction_makes_only_the_base_drop() {
        let (w, mut c) = (World::new(), loaded(6, 100_000));
        run(&mut c, &w, 1, 0.5);
        run(&mut c, &w, 240, 0.5);
        assert_eq!(c.slots[BASE_OUT].as_ref().map(|s| s.count), Some(1));
        assert_eq!(c.slots[PREDICTION_OUT], None);
    }

    #[test]
    fn a_self_aware_model_always_predicts_and_stacks_the_outputs() {
        let (w, mut c) = (World::new(), loaded(1254, 1_000_000));
        run(&mut c, &w, 1 + 240 + 1 + 240, 0.99);
        assert_eq!(c.slots[BASE_OUT].as_ref().map(|s| s.count), Some(2));
        assert_eq!(c.slots[PREDICTION_OUT].as_ref().map(|s| s.count), Some(2));
        assert_eq!(c.slots[INPUT].as_ref().map(|s| s.count), Some(1));
    }

    #[test]
    fn training_adds_one_point_of_data_and_no_loot() {
        let (w, mut c) = (World::new(), loaded(6, 100_000));
        c.mode = Mode::Training;
        run(&mut c, &w, 1 + 300, 0.0);
        let m = c.slots[MODEL].as_ref().unwrap();
        assert_eq!((m.data, m.iters), (7, 1));
        assert_eq!((c.slots[BASE_OUT].clone(), c.slots[PREDICTION_OUT].clone()), (None, None));
        assert_eq!(c.energy, 100_000 - 299 * 154, "training draws 20% more");
    }

    #[test]
    fn training_stops_at_the_top_tier_and_at_a_boundary_in_mode_two() {
        let mut w = World::new();
        let mut c = loaded(1254, 1_000_000);
        c.mode = Mode::Training;
        run(&mut c, &w, 2, 0.0);
        assert_eq!(c.fail, Fail::CannotTrain);
        w.cfg.sim_model_upgrade = 2;
        let mut c = loaded(54, 1_000_000);
        c.mode = Mode::Training;
        run(&mut c, &w, 2, 0.0);
        assert_eq!(c.fail, Fail::None, "advanced at 54: a point of data stays under the next tier's 354");
        let mut c = loaded(354, 1_000_000);
        c.mode = Mode::Training;
        // Superior at 354 is below self-aware's 1254, so it still trains; the boundary case is the top.
        run(&mut c, &w, 2, 0.0);
        assert_eq!(c.fail, Fail::None);
        w.cfg.sim_model_upgrade = 0;
        let mut c = loaded(6, 1_000_000);
        c.mode = Mode::Training;
        run(&mut c, &w, 1, 0.0);
        assert_eq!(c.mode, Mode::Inference, "training is off with the config at 0");
    }

    #[test]
    fn the_failures_are_named() {
        let w = World::new();
        let mut c = loaded(6, 100);
        run(&mut c, &w, 1, 0.0);
        assert_eq!(c.fail, Fail::Energy);
        c.energy = 100_000;
        c.slots[INPUT] = Some(Stack::new("minecraft:stick", 1));
        run(&mut c, &w, 1, 0.0);
        assert_eq!(c.fail, Fail::Input);
        c.slots[INPUT] = Some(Stack::new("hostilenetworks:prediction_matrix", 1));
        c.slots[BASE_OUT] = Some(Stack::new("hostilenetworks:overworld_prediction", 64));
        run(&mut c, &w, 1, 0.0);
        assert_eq!(c.fail, Fail::Output);
        c.slots[BASE_OUT] = None;
        c.redstone = Redstone::OffWhenPowered;
        c.tick(&w.ctx(), true, 0.0);
        assert_eq!(c.fail, Fail::Redstone);
        c.slots[MODEL] = None;
        run(&mut c, &w, 1, 0.0);
        assert_eq!(c.fail, Fail::Model);
    }

    #[test]
    fn a_run_waits_when_the_power_runs_out() {
        let (w, mut c) = (World::new(), loaded(6, 128 * 3));
        run(&mut c, &w, 1 + 3, 0.9);
        assert_eq!((c.runtime, c.energy), (237, 0));
        run(&mut c, &w, 1, 0.9);
        assert_eq!((c.runtime, c.fail), (237, Fail::EnergyMidCycle));
        c.energy = 1000;
        run(&mut c, &w, 1, 0.9);
        assert_eq!((c.runtime, c.fail), (236, Fail::None));
    }
}
