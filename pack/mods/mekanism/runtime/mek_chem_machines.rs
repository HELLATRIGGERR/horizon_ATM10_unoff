// Mekanism's Chemical Oxidizer and Chemical Infuser in the game (support module
// `crate::game::mek_chem_machines` of the `mekanism` native bundle). Their rules
// are the bundle's crate (`mekanism_energy::chemical_machine` and `machine`,
// MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this
// module is their part of the glue: their tick after the shared one
// (`mek_machines`: upgrades, the ejector, the energy slot), the chemical item
// slots (`ChemicalInventorySlot.fill` and `drain`), the recipe
// (`mekanism:oxidizing`, `mekanism:chemical_infusing`), the ejector of their
// output tank and output slot, the infuser's tank rules, and their screens
// (`GuiChemicalOxidizer`, `GuiChemicalInfuser`). Their block entities, energy,
// side configuration, chemical handler by side and `/mek probe` are
// `mek_machines`' as for the other machines.

use super::{
    Game, mek_chemicals,
    mek_machines::{self, Saved},
    mek_ui::{self, Pen, Rect},
};
use horizon_client::containers::ModBlockEntity;
use mekanism_energy::{
    chemical::{self, ChemicalRegistry, ChemicalStack, ChemicalTank, TankRules},
    chemical_machine::{self, Infusing},
    chemical_tank::{self, OwnTank},
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    display,
    machine::{self, Inputs, MachineKind, Upgrade},
    menu::{self, InfuserView, MachineView},
    separator,
    sides::{DataType, Direction, RelativeSide},
};

/// What a tick did, for the shared end of it (`mek_machines::tick`).
pub(super) struct Made {
    pub(super) per_tick: u64,
    pub(super) required: u32,
    pub(super) warnings: i32,
    /// The recipe's activity; none without a recipe.
    pub(super) active: Option<bool>,
    /// Data slots after the machine's (`InfuserView`).
    pub(super) extra: Vec<i32>,
}

/// The infuser's tank rules (`getInitialChemicalTanks`): the left and right
/// `inputModern` (only the infuser takes out; in goes a chemical a recipe
/// takes with what the other side holds; valid, one a recipe takes at all and
/// the default attribute validator), the centre `output`.
pub(super) fn with_infuser_rules<R>(
    left: &ChemicalStack,
    right: &ChemicalStack,
    registry: &ChemicalRegistry,
    use_it: impl FnOnce(&TankRules<'_>, &TankRules<'_>, &TankRules<'_>) -> R,
) -> R {
    let into_left = |s: &ChemicalStack, _: Automation| mek_machines::chemical_infusing_takes(s, right, registry);
    let into_right = |s: &ChemicalStack, _: Automation| mek_machines::chemical_infusing_takes(s, left, registry);
    let valid = |s: &ChemicalStack| mek_machines::chemical_infusing_takes(s, &ChemicalStack::EMPTY, registry) && registry.passes_default(s);
    let l = TankRules { can_extract: &chemical::not_external, can_insert: &into_left, valid: &valid };
    let r = TankRules { can_extract: &chemical::not_external, can_insert: &into_right, valid: &valid };
    use_it(&l, &r, &separator::output_rules())
}

/// The faces of a config type that gives out (`OUTPUT` or `INPUT_OUTPUT`).
fn output_faces(sides: &[DataType; 6], facing: Direction) -> Vec<Direction> {
    Direction::ALL
        .into_iter()
        .filter(|&d| matches!(sides[RelativeSide::from_directions(facing, d).index()], DataType::Output | DataType::InputOutput))
        .collect()
}

/// The chemical ejector (`TileComponentEjector` with `ChemicalUtil.emit`): the
/// output tank (the oxidizer's only one, the infuser's centre: `setCanTankEject`)
/// gives up to `rate` a tick, split among the chemical handlers on its output
/// faces, as the machine itself.
pub(super) fn eject(game: &mut Game<'_>, pos: [i32; 3], facing: Direction, sides: &[DataType; 6], tanks: &mut [ChemicalTank], rate: i64) {
    let Some(tank) = tanks.last_mut() else {
        return;
    };
    let faces = output_faces(sides, facing);
    if tank.is_empty() || faces.is_empty() {
        return;
    }
    let rules = separator::output_rules();
    let offered = tank.extract(rate, true, Automation::Internal, &rules);
    let sent = mek_chemicals::emit(game, pos, &faces, &offered);
    if sent > 0 {
        tank.extract(sent, false, Automation::Internal, &rules);
    }
}

/// `TileComponentEjector.outputItems`: the output slot's chemical item out of
/// the faces whose item type gives out, once it takes no more of the output
/// tank (its extract predicate).
#[allow(clippy::too_many_arguments)]
pub(super) fn eject_items(
    game: &mut Game<'_>,
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
    kind: MachineKind,
    items: &[DataType; 6],
    facing: Direction,
    tanks: &[ChemicalTank],
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) {
    let slot = kind.slots().output;
    let Some(mut tank) = tanks.last().cloned() else {
        return;
    };
    let rules = separator::output_rules();
    let full = super::mek_tanks::with_slot_item(entity, slot, registry, config, |item| {
        !chemical_tank::drain_accepts(item, &OwnTank { tank: &mut tank, rules: &rules, block: None })
    });
    if full == Some(true) {
        mek_machines::eject_items(game, pos, entity, slot, &output_faces(items, facing));
    }
}

/// `onUpdateServer` from the chemical slots on.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    entity: &mut ModBlockEntity,
    kind: MachineKind,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    recipe: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    if kind.oxidizes() { oxidize(entity, saved, tanks, store, recipe, registry, config) } else { infuse(entity, saved, tanks, store, recipe, registry, config) }
}

/// `TileEntityChemicalOxidizer`: the output slot fills a chemical item from
/// the tank, then the recipe (`OneInputCachedRecipe.itemToChemical`): 100
/// ticks, the input's chemical into the tank when the last is done.
#[allow(clippy::too_many_arguments)]
fn oxidize(
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    let kind = MachineKind::ChemicalOxidizer;
    let slots = kind.slots();
    let rules = separator::output_rules();
    if let Some(tank) = tanks.first_mut() {
        super::mek_tanks::with_slot_item(entity, slots.output, registry, config, |item| {
            chemical_tank::drain_into_item(item, &mut OwnTank { tank, rules: &rules, block: None })
        });
    }
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let required = saved.upgrades.ticks(chemical_machine::OXIDIZER_TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let mut made = Made { per_tick, required, warnings: 0, active: None, extra: Vec::new() };
    let input = mek_machines::stack_in(entity, slots.input).cloned();
    let found = input.as_ref().and_then(mek_machines::find_oxidizing);
    let Some(recipe) = found else {
        // The cached recipe no longer takes the input: from the start.
        if input.is_some() {
            made.warnings |= menu::WARN_NO_RECIPE;
        }
        if memo.is_some() {
            saved.progress = 0;
            made.active = Some(false);
        }
        return made;
    };
    // A first cache takes the saved progress; another recipe starts at 0.
    if memo.as_deref() != Some(recipe.id.as_str()) {
        saved.progress = if memo.is_none() { machine::resumed(saved.progress, required) } else { 0 };
        *memo = Some(recipe.id.clone());
    }
    let Some(tank) = tanks.first_mut() else {
        return made;
    };
    let count = input.as_ref().map_or(0, |i| i.count);
    let room = chemical_machine::room(tank, &recipe.output);
    if room == 0 {
        made.warnings |= menu::WARN_OUTPUT;
    }
    if per_tick > store.stored {
        made.warnings |= menu::WARN_ENERGY;
    }
    let ran = machine::process(&mut saved.progress, required, store, per_tick, 1, Inputs::of(count, recipe.input.count), room, true);
    if ran.finished {
        mek_machines::shrink(entity, slots.input, recipe.input.count * ran.operations);
        let out = recipe.output.copy_with_amount(recipe.output.amount().saturating_mul(i64::from(ran.operations)));
        tank.insert(&out, false, Automation::Internal, &rules);
    }
    made.active = Some(ran.active());
    made
}

/// `TileEntityChemicalInfuser`: the left and right slots fill their tanks
/// from chemical items, the output slot drains the centre into one, then
/// `2^speed` operations of the recipe the two tanks hold, either way round.
fn infuse(
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    let kind = MachineKind::ChemicalInfuser;
    let slots = kind.slots();
    let [left, right, center] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    // `leftInputSlot.fillTank`, `rightInputSlot.fillTank`, `outputSlot.drainTank`.
    for (slot, is_left) in [(slots.input, true), (slots.infusion.unwrap_or(1), false)] {
        let (l, r) = (left.stored.clone(), right.stored.clone());
        with_infuser_rules(&l, &r, registry, |lr, rr, _| {
            let (tank, rules) = if is_left { (&mut *left, lr) } else { (&mut *right, rr) };
            super::mek_tanks::with_slot_item(entity, slot, registry, config, |item| chemical_tank::fill_from_item(item, &mut OwnTank { tank, rules, block: None }))
        });
    }
    let rules = separator::output_rules();
    super::mek_tanks::with_slot_item(entity, slots.output, registry, config, |item| {
        chemical_tank::drain_into_item(item, &mut OwnTank { tank: center, rules: &rules, block: None })
    });

    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = chemical_machine::infuser_operations(saved.upgrades.get(Upgrade::Speed));
    let mut made = Made { per_tick, required: 0, warnings: 0, active: Some(false), extra: Vec::new() };
    let mut used = 0;
    match mek_machines::find_chemical_infusing(&left.stored, &right.stored, registry) {
        Some(recipe) => {
            *memo = Some(recipe.id.clone());
            // `ChemicalChemicalToChemicalCachedRecipe`: the recipe's sides swap
            // when the left tank holds its right ingredient.
            let straight = recipe.left.test_type(left.stored.id(), registry) && recipe.right.test_type(right.stored.id(), registry);
            let (l, r) = if straight { (recipe.left.amount, recipe.right.amount) } else { (recipe.right.amount, recipe.left.amount) };
            let infusing = Infusing { left: l, right: r, output: recipe.output.clone() };
            let ran = chemical_machine::infuse(&infusing, left, right, center, store, per_tick, baseline);
            if ran.short_energy {
                made.warnings |= menu::WARN_ENERGY;
            }
            if ran.full {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if ran.short_input {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
            used = ran.energy;
            made.active = Some(ran.operations > 0);
        }
        None => {
            if !left.is_empty() || !right.is_empty() {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
        }
    }
    let tank = |t: &ChemicalTank| (registry.number(t.stored.id()), t.stored().max(0) as u64);
    made.extra = InfuserView { tanks: [tank(left), tank(right), tank(center)], energy_used: used }.pack();
    made
}

// ---- The screens ------------------------------------------------------------------------

/// `GuiChemicalGauge` STANDARD: the oxidizer's at (131, 13); the infuser's left,
/// centre and right at (25, 13), (79, 4) and (133, 13).
const OXIDIZER_GAUGE: Rect = [131.0, 13.0, 18.0, 60.0];
const INFUSER_GAUGES: [Rect; 3] = [[25.0, 13.0, 18.0, 60.0], [133.0, 13.0, 18.0, 60.0], [79.0, 4.0, 18.0, 60.0]];
/// `GuiHorizontalPowerBar` at (115, 75): its frame (the bar inside is 52x4).
pub(super) const POWER_BAR: Rect = [115.0, 75.0, 56.0, 6.0];
/// `GuiProgress` LARGE_RIGHT at (64, 40); the infuser's SMALL_RIGHT and SMALL_LEFT.
const OXIDIZER_PROGRESS: Rect = [64.0, 40.0, 48.0, 8.0];
const INFUSER_PROGRESS: [Rect; 2] = [[47.0, 39.0, 28.0, 8.0], [101.0, 39.0, 28.0, 8.0]];

/// Where Mekanism puts their slots (`getInitialInventory`).
pub(super) fn slots(kind: MachineKind) -> Vec<Option<[f32; 2]>> {
    if kind.oxidizes() {
        vec![Some([26.0, 36.0]), Some([152.0, 55.0]), Some([152.0, 14.0])]
    } else {
        vec![Some([6.0, 56.0]), Some([154.0, 56.0]), Some([80.0, 65.0]), Some([154.0, 14.0])]
    }
}

pub(super) fn gauge(pen: &mut Pen<'_, '_>, r: Rect, number: i32, amount: u64, capacity: i64) {
    let rgb = mek_chemicals::registry(pen.game).by_number(number).map_or(0x808080, |c| c.color & 0xFF_FFFF);
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], r);
    let inner = r[3] - 2.0;
    let h = ((amount as f64 / capacity.max(1) as f64).clamp(0.0, 1.0) * f64::from(inner)) as i32 as f32;
    if h > 0.0 {
        pen.fill([r[0] + 1.0, r[1] + 1.0 + inner - h, r[2] - 2.0, h], 0xFF00_0000 | rgb);
    }
    pen.whole("gauge/standard", [16.0, r[3] - 2.0], [r[0] + 1.0, r[1] + 1.0, 16.0, r[3] - 2.0]);
}

/// Their own elements over the machine panel: the horizontal power bar, the
/// gauges and the progress.
pub(super) fn draw(pen: &mut Pen<'_, '_>, kind: MachineKind, view: &MachineView, data: &[i32]) {
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let w = (level * 52.0) as i32 as f32;
    if w > 0.0 {
        pen.sprite("bar/horizontal_power", [0.0, 0.0, w, 4.0], [POWER_BAR[0] + 1.0, POWER_BAR[1] + 1.0, w, 4.0]);
    }
    if kind.oxidizes() {
        gauge(pen, OXIDIZER_GAUGE, view.infuse, view.infused, chemical_machine::OXIDIZER_TANK);
        let [px, py, pw, ph] = OXIDIZER_PROGRESS;
        pen.sprite("progress/large_right", [0.0, 0.0, pw, ph], OXIDIZER_PROGRESS);
        let share = if view.required == 0 { 0.0 } else { (f64::from(view.progress) / f64::from(view.required)).min(1.0) };
        let done = (share * f64::from(pw)) as i32 as f32;
        if done > 0.0 {
            pen.sprite("progress/large_right", [0.0, ph, done, ph], [px, py, done, ph]);
        }
        return;
    }
    let extra = InfuserView::unpack(data).unwrap_or_default();
    for (r, (number, amount)) in INFUSER_GAUGES.into_iter().zip(extra.tanks) {
        gauge(pen, r, number, amount, chemical_machine::INFUSER_TANK);
    }
    for (r, name) in INFUSER_PROGRESS.into_iter().zip(["progress/small_right", "progress/small_left"]) {
        let row = if view.active { 8.0 } else { 0.0 };
        pen.sprite(name, [0.0, row, r[2], r[3]], r);
    }
}

pub(super) fn tank_line(number: i32, amount: u64) -> String {
    if amount == 0 {
        return "Empty".to_owned();
    }
    match mek_chemicals::loaded().as_deref().and_then(|r| r.by_number(number).map(mek_chemicals::name)) {
        Some(name) => format!("{name}: {} mB", display::thousands(amount)),
        None => format!("{} mB", display::thousands(amount)),
    }
}

/// The gauges' and progress's tooltips.
pub(super) fn tip(kind: MachineKind, view: &MachineView, data: &[i32], at: [f32; 2]) -> Option<Vec<String>> {
    if kind.oxidizes() {
        if mek_ui::inside(OXIDIZER_GAUGE, at) {
            return Some(vec![tank_line(view.infuse, view.infused)]);
        }
        if mek_ui::inside(OXIDIZER_PROGRESS, at) {
            let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
            return Some(vec![format!("Progress: {}%", (share * 100.0).floor())]);
        }
        return None;
    }
    let extra = InfuserView::unpack(data)?;
    INFUSER_GAUGES.into_iter().zip(extra.tanks).find(|(r, _)| mek_ui::inside(*r, at)).map(|(_, (n, a))| vec![tank_line(n, a)])
}
