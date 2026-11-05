// Mekanism Generators' Gas-Burning Generator in the game (support module
// `crate::game::mek_gas` of the `mekanism` native bundle). Its rules are the
// bundle's crate (`mekanism_energy::gas`, MIT, from Mekanism Generators
// 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module is the glue:
// its tick (`TileEntityGasGenerator.onUpdateServer`: the ejector out of its
// front, the fuel slot filling the tank from a chemical item, the burn), its
// fuel tank in Mekanism's saved form and as a chemical handler on every side
// but the front (`ChemicalTankHelper.forSide`: any block puts fuel in, none
// takes it out), what it burns kept here by position (Mekanism doesn't save
// it), its screen (`GuiGasGenerator`) and `/mek probe`. The rest of a
// generator (its store, Forge Energy and cables on its front) is `mek_world`'s.

use super::{
    Game, mek_chemicals,
    mek_ui::{self, Pen},
    mek_world::{self, key},
};
use horizon_client::containers::ModBlockEntity;
use horizon_world::nbt::Tag;
use mekanism_energy::{
    chemical::{self, ChemicalHandler, ChemicalRegistry, ChemicalStack, ChemicalTank, Fuel, SidedTanks, TankRules},
    chemical_tank::{self, OwnTank},
    config::EnergyConfig,
    container::EnergyContainer,
    display,
    gas::{self, Burner},
    generator::GeneratorKind,
    menu::{GasView, GeneratorView},
    sides::{Direction, RelativeSide},
};
use std::{cell::RefCell, collections::HashMap};

const HYDROGEN: &str = "mekanism:hydrogen";

thread_local! {
    /// Each generator's burner and whether its tank was empty after its last
    /// tick, by dimension and position (not saved).
    static LIVE: RefCell<HashMap<(String, [i32; 3]), (Burner, bool)>> = RefCell::new(HashMap::new());
}

pub(super) fn is_gas(id: &str) -> bool {
    GeneratorKind::of_block(id) == Some(GeneratorKind::GasBurning)
}

/// `ChemicalUtil.hydrogenEnergyPerTick` and `hydrogenEnergyDensity`:
/// hydrogen's fuel attribute (0 without one).
fn hydrogen(registry: &ChemicalRegistry) -> Fuel {
    registry.get(HYDROGEN).and_then(|c| c.fuel).unwrap_or(Fuel { burn_ticks: 1, energy_per_tick: 0 })
}

/// The Joules it holds: 1000 times hydrogen's energy density, from the
/// registry once read (200 000 J on ATM10 before).
pub(super) fn storage() -> u64 {
    match mek_chemicals::loaded() {
        Some(r) => gas::storage(hydrogen(&r).energy_density()),
        None => GeneratorKind::GasBurning.capacity(&Default::default()),
    }
}

/// `HAS_FUEL`: a chemical with a fuel attribute.
fn fuel_of(stack: &ChemicalStack, registry: &ChemicalRegistry) -> Option<Fuel> {
    registry.get(stack.id()?).and_then(|c| c.fuel)
}

pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    LIVE.with(|l| l.borrow_mut().remove(&(dimension.to_owned(), pos)));
}

pub(super) fn clear() {
    LIVE.with(|l| l.borrow_mut().clear());
}

fn read_tank(entity: &ModBlockEntity, config: &EnergyConfig, registry: &ChemicalRegistry) -> ChemicalTank {
    let stored = mek_chemicals::read_tanks(&entity.nbt, 1, registry).pop().unwrap_or(ChemicalStack::EMPTY);
    ChemicalTank::with(config.generators.gas_tank_capacity, stored)
}

/// `FuelTank`'s rules: anything puts in (`alwaysTrueBi`), only the generator
/// takes out (`notExternal`), a fuel that passes the default attribute
/// validator.
fn with_rules<R>(registry: &ChemicalRegistry, use_it: impl FnOnce(&TankRules<'_>) -> R) -> R {
    let valid = |s: &ChemicalStack| fuel_of(s, registry).is_some() && registry.passes_default(s);
    use_it(&TankRules { can_extract: &chemical::not_external, can_insert: &chemical::always, valid: &valid })
}

/// `native_block_entity_tick` of a gas-burning generator: the ejector at
/// `maxOutput` out of its front, the fuel slot (`ChemicalInventorySlot.fill`:
/// a chemical item emptying into the tank), the burn, then the screen's data
/// slots. The energy slot charges nothing; redstone control is IGNORED.
pub(super) fn tick(game: &mut Game<'_>, config: &EnergyConfig, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dimension = game.native_dimension().to_owned();
    let registry = mek_chemicals::registry(game);
    let facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    if mek_world::generator_facing(&entity.nbt) != facing || !entity.nbt.contains_key(&key("facing")) {
        entity.nbt.insert(key("facing"), Tag::String(key(facing.name())));
    }
    let h = hydrogen(&registry);
    let mut tank = read_tank(entity, config, &registry);
    let before = tank.clone();
    let id = (dimension, pos);
    let (mut burner, was_empty) = LIVE
        .with(|l| l.borrow_mut().remove(&id))
        .unwrap_or_else(|| (Burner::new(h.energy_per_tick, fuel_of(&tank.stored, &registry)), tank.is_empty()));
    // What another block put into an empty tank since the last tick.
    if was_empty && !tank.is_empty() {
        burner.filled(fuel_of(&tank.stored, &registry));
    }
    let mut store = EnergyContainer::output(storage());
    store.stored = mek_world::saved_energy(&entity.nbt).unwrap_or(0).min(store.capacity);
    let energy_before = store.stored;
    mek_world::eject(game, config, pos, &[GeneratorKind::GasBurning.energy_face(facing)], &mut store, burner.max_output);

    // The fuel slot fills the tank from a chemical item.
    let empty = tank.is_empty();
    with_rules(&registry, |rules| {
        super::mek_tanks::with_slot_item(entity, 0, &registry, config, |item| chemical_tank::fill_from_item(item, &mut OwnTank { tank: &mut tank, rules, block: None }))
    });
    if empty && !tank.is_empty() {
        burner.filled(fuel_of(&tank.stored, &registry));
    }

    let mut stored = tank.stored();
    let fuel = fuel_of(&tank.stored, &registry);
    let capacity = tank.capacity;
    burner.tick(&mut stored, capacity, fuel, &mut store, true, h.energy_per_tick);
    tank.stored = tank.stored.copy_with_amount(stored).normalized();

    if store.stored != energy_before {
        mek_world::write_energy(&mut entity.nbt, store.stored);
    }
    if tank != before {
        mek_chemicals::write_tanks(&mut entity.nbt, std::slice::from_ref(&tank.stored));
    }
    let mut data = GeneratorView {
        stored: store.stored,
        capacity: store.capacity,
        produced: burner.production(),
        max_output: burner.max_output,
        ..GeneratorView::default()
    }
    .pack();
    data.extend(
        GasView {
            chemical: registry.number(tank.stored.id()),
            amount: tank.stored().max(0) as u64,
            capacity: tank.capacity.max(0) as u64,
            burn_rate: burner.used(),
        }
        .pack(),
    );
    entity.data = data;
    let empty = tank.is_empty();
    LIVE.with(|l| l.borrow_mut().insert(id, (burner, empty)));
}

/// The fuel tank through `face` (`ChemicalTankHelper.forSide(facing)` with
/// the left, right, back, top and bottom): another block puts fuel in and
/// takes none out; the front has no handler; the null side reads it.
pub(super) fn chemical_handler<R>(
    entity: &mut ModBlockEntity,
    face: Option<Direction>,
    registry: &ChemicalRegistry,
    use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R,
) -> Option<R> {
    let facing = mek_world::generator_facing(&entity.nbt);
    if face.is_some_and(|f| RelativeSide::from_directions(facing, f) == RelativeSide::Front) {
        return None;
    }
    let (config, _) = mek_world::energy_config();
    let mut tank = read_tank(entity, &config, registry);
    let before = tank.clone();
    let answer = with_rules(registry, |rules| {
        let mut handler = SidedTanks { tanks: vec![(&mut tank, rules)], side: face.is_some(), can_insert: true, can_extract: true };
        use_it(&mut handler)
    });
    if tank != before {
        mek_chemicals::write_tanks(&mut entity.nbt, std::slice::from_ref(&tank.stored));
    }
    Some(answer)
}

/// What `/mek probe` adds for a gas-burning generator.
pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], nbt: &horizon_world::nbt::Compound, data: &[i32], out: &mut serde_json::Value) {
    let registry = mek_chemicals::registry(game);
    let (config, _) = mek_world::energy_config();
    let stored = mek_chemicals::read_tanks(nbt, 1, &registry).pop().unwrap_or(ChemicalStack::EMPTY);
    let live = LIVE.with(|l| l.borrow().get(&(game.native_dimension().to_owned(), pos)).copied());
    let view = GasView::unpack(data).unwrap_or_default();
    out["tank"] = serde_json::json!({ "chemical": stored.id(), "amount": stored.amount(), "capacity": config.generators.gas_tank_capacity });
    out["burn_rate"] = view.burn_rate.into();
    out["facing"] = mek_world::generator_facing(nbt).name().into();
    if let Some((b, _)) = live {
        out["burner"] = serde_json::json!({ "burn_ticks": b.burn_ticks, "max_burn_ticks": b.max_burn_ticks, "generation_rate": b.generation_rate });
    }
}

// ---- The screen ------------------------------------------------------------------------

/// `GuiChemicalGauge` WIDE at (55, 18).
const GAUGE: mek_ui::Rect = [55.0, 18.0, 66.0, 50.0];

/// `GuiGasGenerator`'s own elements over a generator's screen: the fuel
/// gauge, the fuel slot and the burn rate beside the inventory's label.
pub(super) fn draw(pen: &mut Pen<'_, '_>, data: &[i32]) {
    let view = GasView::unpack(data).unwrap_or_default();
    pen.slot("extra", Some("overlay_minus"), 16.0, 34.0);
    let level = if view.capacity == 0 { 0.0 } else { view.amount as f64 / view.capacity as f64 };
    let rgb = mek_chemicals::registry(pen.game).by_number(view.chemical).map_or(0x808080, |c| c.color & 0xFF_FFFF);
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], GAUGE);
    let inner = GAUGE[3] - 2.0;
    let h = (level.clamp(0.0, 1.0) * f64::from(inner)) as i32 as f32;
    if h > 0.0 {
        pen.fill([GAUGE[0] + 1.0, GAUGE[1] + 1.0 + inner - h, GAUGE[2] - 2.0, h], 0xFF00_0000 | rgb);
    }
    pen.whole("gauge/wide", [64.0, 48.0], [GAUGE[0] + 1.0, GAUGE[1] + 1.0, 64.0, 48.0]);
    let rate = format!("Burn Rate: {} mB/t", view.burn_rate);
    let w = pen.game.native_text_width(&rate);
    pen.text(&rate, [168.0 - w, 72.0], mek_ui::TITLE);
}

/// The fuel gauge's tooltip.
pub(super) fn tip(data: &[i32], at: [f32; 2]) -> Option<Vec<String>> {
    let view = GasView::unpack(data)?;
    if !mek_ui::inside(GAUGE, at) {
        return None;
    }
    if view.amount == 0 {
        return Some(vec!["Empty".into()]);
    }
    let name = mek_chemicals::loaded().as_deref().and_then(|r| r.by_number(view.chemical).map(mek_chemicals::name));
    Some(vec![match name {
        Some(n) => format!("{n}: {} mB", display::thousands(view.amount)),
        None => format!("{} mB", display::thousands(view.amount)),
    }])
}
