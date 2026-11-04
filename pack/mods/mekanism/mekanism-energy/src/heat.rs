//! Mekanism Generators' Heat Generator (`TileEntityHeatGenerator`): it burns
//! lava from its tank and takes heat from lava beside it (and from an
//! ultra-warm dimension) into a heat capacitor (`BasicHeatCapacitor`), and
//! turns the heat it loses to the air into Joules at the Carnot efficiency
//! against the biome's temperature (`HeatAPI.getAmbientTemp`). Its fuel slot
//! takes lava buckets and anything that burns in a furnace
//! (`FluidFuelInventorySlot.fillOrBurn`). Its store gives out of its front;
//! its lava tank is a fluid handler on every other side, which takes lava
//! from anyone and gives it to no other block (`VariableCapacityFluidTank.input`).
//!
//! Heat a capacitor is handed waits in `heatToHandle` until the end of the
//! tick (`ITileHeatHandler.updateHeatCapacitors`), so everything in one tick
//! reads the temperature the tick started with.
use crate::config::GeneratorsConfig;
use crate::container::{Automation, EnergyContainer};
use crate::sides::{Direction, RelativeSide};

/// `HEAT_CAPACITY`.
pub const HEAT_CAPACITY: f64 = 10.0;
/// `INVERSE_CONDUCTION_COEFFICIENT`.
pub const INVERSE_CONDUCTION: f64 = 5.0;
/// `INVERSE_INSULATION_COEFFICIENT`, on every side but the bottom, which has
/// `HeatAPI.DEFAULT_INVERSE_INSULATION` (0).
pub const INVERSE_INSULATION: f64 = 100.0;
/// `THERMAL_EFFICIENCY`: the share of the difference to the air lost a tick.
const THERMAL_EFFICIENCY: f64 = 0.5;
/// `HeatAPI.AMBIENT_TEMP`, Kelvin.
pub const AMBIENT_TEMP: f64 = 300.0;
/// `HeatAPI.AIR_INVERSE_COEFFICIENT`.
const AIR_INVERSE_COEFFICIENT: f64 = 10_000.0;
/// `HeatAPI.EPSILON` (`com.mojang.math.Constants.EPSILON`, a float): changes
/// smaller than this are dropped.
const EPSILON: f64 = 1.0e-6_f32 as f64;
/// Millibuckets in a bucket.
pub const BUCKET: u32 = 1000;

/// `HeatAPI.getAmbientTemp(biomeTemperature)`: 300 K in a biome of 0.8,
/// 25 K a unit of biome temperature, clamped to -5..=5.
pub fn ambient_temperature(biome_temperature: f32) -> f64 {
    AMBIENT_TEMP + 25.0 * (f64::from(biome_temperature).clamp(-5.0, 5.0) - 0.8)
}

/// `MAX_PRODUCTION`: lava on all six faces and inside, the Nether, and a burn:
/// `30 * 7 + 100 + 800 = 1110` J/t on ATM10.
pub fn max_production(config: &GeneratorsConfig) -> u64 {
    config
        .heat_generation_lava
        .saturating_mul(7)
        .saturating_add(config.heat_generation_nether)
        .saturating_add(config.heat_generation)
        .min(i64::MAX as u64)
}

/// `getMaxOutput`: twice the most it can make (`TileEntityGenerator`).
pub fn max_output(config: &GeneratorsConfig) -> u64 {
    max_production(config).saturating_mul(2).min(i64::MAX as u64)
}

/// `getBoost`: `heatGenerationLava` for each face with lava beside it (and
/// one more were it lava-logged), and `heatGenerationNether` in an
/// ultra-warm dimension.
pub fn boost(config: &GeneratorsConfig, lava_sides: u32, ultra_warm: bool) -> f64 {
    let mut boost = if config.heat_generation_lava == 0 {
        0
    } else {
        config.heat_generation_lava.saturating_mul(u64::from(lava_sides))
    };
    if ultra_warm {
        boost = boost.saturating_add(config.heat_generation_nether);
    }
    boost as f64
}

/// The biome's temperature as the generator reads it (`CachedAmbientTemperature`),
/// once, after it is placed or loaded: at its own place, and beside each face
/// (`Direction` order) for what it loses to the air there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    pub own: f64,
    pub sides: [f64; 6],
}

impl Ambient {
    pub fn uniform(kelvin: f64) -> Self {
        Self { own: kelvin, sides: [kelvin; 6] }
    }
}

/// The fluid a container in the fuel slot holds (its `IFluidHandlerItem`,
/// one tank), as `IFluidHandlerSlot.fillTank` reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemFluid {
    /// mB it holds.
    pub amount: u32,
    /// `isFluidValid`: the tank takes its fluid (`FluidTags.LAVA`).
    pub valid: bool,
    /// The tank is empty or holds that fluid (`isFluidEqual`), so an insert
    /// takes it.
    pub same: bool,
    /// It gives all or nothing: a bucket (`FluidBucketWrapper.drain` of less
    /// than its bucket gives none).
    pub whole: bool,
}

/// What a fuel slot holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuel {
    pub count: u32,
    /// The fluid it holds, if it is a fluid container.
    pub fluid: Option<ItemFluid>,
    /// `ItemStack.getBurnTime`, ticks in a furnace.
    pub burn_time: i32,
    /// It leaves something when used (`hasCraftingRemainingItem`, a bucket).
    pub has_remainder: bool,
    /// The tank is empty or holds `minecraft:lava`, so the lava a burn makes
    /// (`new FluidStack(Fluids.LAVA, fuel)`) goes in. Otherwise the burn still
    /// uses the item up, and its lava is lost, as in Java.
    pub takes_lava: bool,
}

/// What `fill_or_burn` did to the slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotUse {
    Kept,
    /// The item became what it leaves (`getCraftingRemainingItem`).
    Remainder,
    /// One item went.
    ShrunkOne,
    /// The container gave this many mB into the tank and becomes what it is
    /// then (`setStack(handler.getContainer())`: a lava bucket an empty one).
    Drained(u32),
}

/// `FluidFuelInventorySlot.fillOrBurn`: a fluid container alone in the slot
/// whose fluid the tank takes fills it (`fillTank`, `fillHandlerFromOther`):
/// what fits, a bucket only whole, and a bucket that doesn't fit still counts
/// as handled, nothing moving; anything else that burns gives `burnTime / 20`
/// mB of lava when that fits, leaving its remainder (a stack of several with
/// a remainder waits).
pub fn fill_or_burn(lava: &mut u32, capacity: u32, fuel: Option<Fuel>) -> SlotUse {
    let Some(fuel) = fuel.filter(|f| f.count > 0) else {
        return SlotUse::Kept;
    };
    let needed = capacity.saturating_sub(*lava);
    if needed == 0 {
        return SlotUse::Kept;
    }
    if fuel.count == 1
        && let Some(fluid) = fuel.fluid.filter(|f| f.valid && f.same && f.amount > 0)
    {
        let take = fluid.amount.min(needed);
        if fluid.whole && take < fluid.amount {
            return SlotUse::Kept;
        }
        *lava += take;
        return SlotUse::Drained(take);
    }
    let value = fuel.burn_time / 20;
    if value <= 0 || value as u32 > needed {
        return SlotUse::Kept;
    }
    if fuel.has_remainder && fuel.count > 1 {
        return SlotUse::Kept;
    }
    if fuel.takes_lava {
        *lava += value as u32;
    }
    if fuel.has_remainder { SlotUse::Remainder } else { SlotUse::ShrunkOne }
}

/// The sides the lava tank is on (`FluidTankHelper.forSide(facing).addTank(lavaTank,
/// LEFT, RIGHT, BACK, TOP, BOTTOM)`): all but the front.
pub const TANK_SIDES: [RelativeSide; 5] =
    [RelativeSide::Left, RelativeSide::Right, RelativeSide::Back, RelativeSide::Top, RelativeSide::Bottom];

/// Whether the lava tank is on the absolute face `side` of a generator
/// facing `facing` (`FluidTankHolder.getTanks(side)`).
pub fn tank_on(facing: Direction, side: Direction) -> bool {
    TANK_SIDES.iter().any(|s| s.direction(facing) == side)
}

/// `BasicFluidTank.insert` into the lava tank holding `stored` of `capacity`
/// (`VariableCapacityFluidTank.input`: anyone may insert, at no rate limit):
/// what of `offered` mB it takes. `valid`: the fluid is in `FluidTags.LAVA`
/// (`isFluidValid`); `same`: the tank is empty or holds that fluid with
/// those components (`isFluidEqual`).
pub fn tank_insert(stored: u32, capacity: u32, valid: bool, same: bool, offered: u32) -> u32 {
    if offered == 0 || !valid {
        return 0;
    }
    let needed = capacity.saturating_sub(stored);
    if needed == 0 || (stored > 0 && !same) {
        return 0;
    }
    offered.min(needed)
}

/// `BasicFluidTank.extract` of up to `amount` from the lava tank holding
/// `stored`: `ConstantPredicates.notExternal`, so another block (a pipe, a
/// bus: `AutomationType.EXTERNAL`) takes nothing; the generator itself and a
/// player do.
pub fn tank_extract(stored: u32, automation: Automation, amount: u32) -> u32 {
    if stored == 0 || amount == 0 || automation == Automation::External {
        return 0;
    }
    amount.min(stored)
}

/// A heat generator as its block entity keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatGenerator {
    /// Joules held.
    pub energy: u64,
    /// Lava in the tank, mB.
    pub lava: u32,
    /// The capacitor's heat; none until it is first read (Mekanism's -1),
    /// when it starts at the air's: `HEAT_CAPACITY * ambient`.
    pub heat: Option<f64>,
}

/// What a tick of the heat did, for its screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatMade {
    /// `setActive`: it burnt lava.
    pub active: bool,
    /// `producingEnergy`: Joules gained this tick.
    pub produced: u64,
    /// `lastTransferLoss`: to a heat handler below (none is ported: 0).
    pub transfer_loss: f64,
    /// `lastEnvironmentLoss`: degrees a tick lost to the air.
    pub environment_loss: f64,
}

impl HeatGenerator {
    /// `getTotalTemperature`.
    pub fn temperature(&self, ambient: &Ambient) -> f64 {
        self.heat.unwrap_or(HEAT_CAPACITY * ambient.own) / HEAT_CAPACITY
    }

    /// Its store: `BasicEnergyContainer.output(heatGenerator)`.
    pub fn container(&self, config: &GeneratorsConfig) -> EnergyContainer {
        let mut c = EnergyContainer::output(config.heat_storage);
        c.stored = self.energy.min(c.capacity);
        c
    }

    /// `onUpdateServer` after the ejector and the fuel slot: the boost, a burn
    /// of `heatGenerationFluidRate` mB when there is room for energy, the
    /// Carnot conversion and the loss to the air (`simulate`), then the
    /// capacitor takes what it was handed (`update`).
    pub fn heat_tick(&mut self, config: &GeneratorsConfig, ambient: &Ambient, lava_sides: u32, ultra_warm: bool) -> HeatMade {
        let mut container = self.container(config);
        let before = container.stored;
        let mut to_handle = 0.0_f64;
        to_handle += boost(config, lava_sides, ultra_warm);
        let mut active = false;
        if container.needed() > 0 && self.lava >= config.heat_fluid_rate {
            active = true;
            self.lava -= config.heat_fluid_rate;
            to_handle += config.heat_generation as f64;
        }
        // simulate(): 1 - Qc / Qh of what leaves for the air.
        let heat = *self.heat.get_or_insert(HEAT_CAPACITY * ambient.own);
        let temp = heat / HEAT_CAPACITY;
        let air = ambient.own;
        let carnot = 1.0 - air.min(temp) / air.max(temp);
        let heat_lost = THERMAL_EFFICIENCY * (temp - air);
        to_handle += -heat_lost;
        let joules = crate::clamp_to_long((heat_lost.abs() * carnot).max(0.0)).max(0) as u64;
        container.insert(joules.min(max_production(config)), false, Automation::Internal);
        // ITileHeatHandler.simulateEnvironment: each face to the air there.
        let mut environment = 0.0;
        for (side, side_air) in ambient.sides.iter().enumerate() {
            let insulation = if side == 0 { 0.0 } else { INVERSE_INSULATION };
            let inverse_conduction = AIR_INVERSE_COEFFICIENT + insulation + INVERSE_CONDUCTION;
            let transfer = (temp - side_air) / inverse_conduction;
            to_handle += -transfer * HEAT_CAPACITY;
            if transfer > 0.0 {
                environment += transfer;
            }
        }
        // BasicHeatCapacitor.update.
        if to_handle != 0.0 && to_handle.abs() > EPSILON {
            self.heat = Some(heat + to_handle);
        }
        self.energy = container.stored;
        HeatMade { active, produced: container.stored - before, transfer_loss: 0.0, environment_loss: environment }
    }
}

/// Mekanism's `setActive` with `blockDeactivationDelay`: a block turns on at
/// once, and off at once only if it hasn't changed for the delay; after
/// going off it waits the delay before showing another change off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// `currentActive` (saved as `active_state`).
    pub current: bool,
    /// `updateDelay` (saved as `update_delay`).
    pub delay: u32,
}

impl Activity {
    /// The start of `tickServer`: the delay runs down; at its end the block
    /// shows `current` if it doesn't. `shown` is the block's `active`. The
    /// value to set the block's `active` to, if any.
    pub fn tick(&mut self, shown: bool) -> Option<bool> {
        if self.delay > 0 {
            self.delay -= 1;
            if self.delay == 0 && shown != self.current {
                return Some(self.current);
            }
        }
        None
    }

    /// `setActive(active)`: the value to set the block's `active` to, if any.
    pub fn set(&mut self, active: bool, shown: bool, deactivation_delay: u32) -> Option<bool> {
        if active == self.current {
            return None;
        }
        self.current = active;
        if shown == active {
            return None;
        }
        if active {
            return Some(true);
        }
        let now = (self.delay == 0).then_some(false);
        self.delay = deactivation_delay;
        now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atm10() -> GeneratorsConfig {
        GeneratorsConfig {
            solar_generation: 150,
            heat_generation: 800,
            heat_generation_lava: 30,
            heat_generation_nether: 100,
            heat_tank_capacity: 24_000,
            heat_fluid_rate: 10,
            solar_storage: 96_000,
            heat_storage: 160_000,
        }
    }

    #[test]
    fn atm10s_limits() {
        let c = atm10();
        assert_eq!(max_production(&c), 1110, "30 * 7 + 100 + 800");
        assert_eq!(max_output(&c), 2220);
        assert_eq!(max_production(&GeneratorsConfig::default()), 30 * 7 + 100 + 200, "Mekanism's own 510");
        assert_eq!(boost(&c, 6, false), 180.0);
        assert_eq!(boost(&c, 2, true), 160.0);
        assert_eq!(boost(&GeneratorsConfig { heat_generation_lava: 0, ..c }, 6, false), 0.0);
    }

    #[test]
    fn the_air_is_300_kelvin_in_plains() {
        // Plains' 0.8 is a float: a hair over 300 K.
        assert!((ambient_temperature(0.8) - 300.0).abs() < 1e-5);
        assert_eq!(ambient_temperature(2.0), 330.0);
        assert_eq!(ambient_temperature(-0.5), 267.5);
        assert_eq!(ambient_temperature(9.0), ambient_temperature(5.0), "clamped");
    }

    const LAVA_BUCKET: ItemFluid = ItemFluid { amount: 1000, valid: true, same: true, whole: true };
    fn fuel(count: u32, fluid: Option<ItemFluid>, burn_time: i32, has_remainder: bool) -> Option<Fuel> {
        Some(Fuel { count, fluid, burn_time, has_remainder, takes_lava: true })
    }

    #[test]
    fn a_bucket_gives_a_thousand_millibuckets_and_coal_eighty() {
        let bucket = fuel(1, Some(LAVA_BUCKET), 20_000, true);
        let mut lava = 0;
        assert_eq!(fill_or_burn(&mut lava, 24_000, bucket), SlotUse::Drained(1000));
        assert_eq!(lava, 1000);
        // Less than a bucket of room: the bucket waits.
        let mut lava = 23_500;
        assert_eq!(fill_or_burn(&mut lava, 24_000, bucket), SlotUse::Kept);
        assert_eq!(lava, 23_500);
        let coal = fuel(3, None, 1600, false);
        assert_eq!(fill_or_burn(&mut lava, 24_000, coal), SlotUse::ShrunkOne);
        assert_eq!(lava, 23_580);
        let mut full = 24_000;
        assert_eq!(fill_or_burn(&mut full, 24_000, coal), SlotUse::Kept);
        let stone = fuel(1, None, 0, false);
        assert_eq!(fill_or_burn(&mut lava, 24_000, stone), SlotUse::Kept);
        // Two of something with a remainder (stacked buckets of another fuel) wait.
        let two = fuel(2, None, 200, true);
        assert_eq!(fill_or_burn(&mut lava, 24_000, two), SlotUse::Kept);
        assert_eq!(fill_or_burn(&mut lava, 24_000, None), SlotUse::Kept);
    }

    #[test]
    fn a_container_fills_what_fits_and_other_fluids_burn_or_wait() {
        // A container that gives part of what it holds fills the room.
        let tank_item = ItemFluid { amount: 3000, whole: false, ..LAVA_BUCKET };
        let mut lava = 22_500;
        assert_eq!(fill_or_burn(&mut lava, 24_000, fuel(1, Some(tank_item), 0, false)), SlotUse::Drained(1500));
        assert_eq!(lava, 24_000);
        // A fluid the tank refuses (heavy water) is no fuel and stays.
        let water = ItemFluid { valid: false, ..LAVA_BUCKET };
        let mut lava = 0;
        assert_eq!(fill_or_burn(&mut lava, 24_000, fuel(1, Some(water), 0, true)), SlotUse::Kept);
        // Two lava buckets stacked (a mod's stacking bucket) don't fill: one burns
        // only alone, as anything with a remainder.
        assert_eq!(fill_or_burn(&mut lava, 24_000, fuel(2, Some(LAVA_BUCKET), 20_000, true)), SlotUse::Kept);
        // A tank holding another lava (flowing lava, by a bus): the bucket can't
        // fill it, so it burns, and the burn's lava doesn't go in either.
        let other = Fuel { takes_lava: false, ..fuel(1, Some(ItemFluid { same: false, ..LAVA_BUCKET }), 20_000, true).unwrap() };
        let mut lava = 500;
        assert_eq!(fill_or_burn(&mut lava, 24_000, Some(other)), SlotUse::Remainder);
        assert_eq!(lava, 500);
    }

    #[test]
    fn the_lava_tank_takes_lava_on_every_side_but_the_front_and_gives_none_to_blocks() {
        for facing in [Direction::North, Direction::East, Direction::South, Direction::West] {
            for side in [Direction::Down, Direction::Up, Direction::North, Direction::South, Direction::West, Direction::East] {
                assert_eq!(tank_on(facing, side), side != facing, "{facing:?} {side:?}");
            }
        }
        assert_eq!(tank_insert(0, 24_000, true, true, 1000), 1000);
        assert_eq!(tank_insert(23_500, 24_000, true, true, 1000), 500);
        assert_eq!(tank_insert(24_000, 24_000, true, true, 1000), 0);
        assert_eq!(tank_insert(0, 24_000, false, true, 1000), 0, "only lava");
        assert_eq!(tank_insert(500, 24_000, true, false, 1000), 0, "only the lava already there");
        assert_eq!(tank_insert(0, 24_000, true, false, 1000), 1000, "an empty tank takes either lava");
        assert_eq!(tank_extract(1000, Automation::External, 1000), 0);
        assert_eq!(tank_extract(1000, Automation::Internal, 400), 400);
        assert_eq!(tank_extract(0, Automation::Internal, 400), 0);
    }

    /// The first ticks of a new generator with lava in its tank, in plains:
    /// the capacitor starts at the air's 300 K, so the first tick makes no
    /// energy; the 800 heat of the burn raises it 80 K for the next.
    #[test]
    fn a_burn_heats_it_and_the_heat_becomes_energy() {
        let c = atm10();
        let air = Ambient::uniform(300.0);
        let mut g = HeatGenerator { energy: 0, lava: 1000, heat: None };
        let first = g.heat_tick(&c, &air, 0, false);
        assert_eq!(first, HeatMade { active: true, produced: 0, transfer_loss: 0.0, environment_loss: 0.0 });
        assert_eq!(g.lava, 990);
        assert_eq!(g.heat, Some(3800.0));
        assert_eq!(g.temperature(&air), 380.0);
        let second = g.heat_tick(&c, &air, 0, false);
        // 0.5 * 80 = 40 heat to the air, at 1 - 300/380.
        assert_eq!(second.produced, (40.0 * (1.0 - 300.0 / 380.0)) as u64);
        assert_eq!(second.produced, 8);
        assert!(second.environment_loss > 0.0);
        assert_eq!(g.lava, 980);
    }

    /// Burning on: the temperature settles where the 800 heat a tick equals
    /// what leaves, about 1880 K, and it makes about 664 J/t.
    #[test]
    fn a_steady_burn_settles_near_664_joules_a_tick() {
        let c = atm10();
        let air = Ambient::uniform(300.0);
        let mut g = HeatGenerator { energy: 0, lava: 24_000, heat: None };
        let mut made = g.heat_tick(&c, &air, 0, false);
        for _ in 0..200 {
            g.energy = 0;
            made = g.heat_tick(&c, &air, 0, false);
        }
        let t = g.temperature(&air);
        assert!((t - 1881.0).abs() < 2.0, "{t}");
        assert!((663..=665).contains(&made.produced), "{made:?}");
        // Six faces of lava add 180 heat: hotter, more energy.
        let mut lava = HeatGenerator { energy: 0, lava: 24_000, heat: None };
        let mut hot = lava.heat_tick(&c, &air, 6, false);
        for _ in 0..200 {
            lava.energy = 0;
            hot = lava.heat_tick(&c, &air, 6, false);
        }
        assert!((837..=840).contains(&hot.produced), "{hot:?}");
    }

    /// No lava in the tank: only the lava around it heats it.
    #[test]
    fn lava_around_it_alone_makes_about_96_joules_a_tick() {
        let c = atm10();
        let air = Ambient::uniform(300.0);
        let mut g = HeatGenerator { energy: 0, lava: 0, heat: None };
        let mut made = g.heat_tick(&c, &air, 6, false);
        assert!(!made.active);
        for _ in 0..200 {
            g.energy = 0;
            made = g.heat_tick(&c, &air, 6, false);
        }
        assert!(!made.active);
        assert!((95..=97).contains(&made.produced), "{made:?}");
        // Nothing around it and nothing to burn: it stays at the air's temperature.
        let mut cold = HeatGenerator { energy: 0, lava: 0, heat: None };
        for _ in 0..10 {
            assert_eq!(cold.heat_tick(&c, &air, 0, false).produced, 0);
        }
        assert_eq!(cold.temperature(&air), 300.0);
    }

    #[test]
    fn a_full_store_stops_the_burn_but_not_the_heat() {
        let c = atm10();
        let air = Ambient::uniform(300.0);
        let mut g = HeatGenerator { energy: 160_000, lava: 1000, heat: Some(10_000.0) };
        let made = g.heat_tick(&c, &air, 0, false);
        assert!(!made.active, "no room: no burn");
        assert_eq!(g.lava, 1000);
        assert_eq!(made.produced, 0);
        assert!(g.heat.unwrap() < 10_000.0, "it still cools");
    }

    #[test]
    fn the_carnot_share_is_capped_at_the_max_production() {
        let c = atm10();
        let air = Ambient::uniform(300.0);
        let mut g = HeatGenerator { energy: 0, lava: 0, heat: Some(1_000_000.0) };
        assert_eq!(g.heat_tick(&c, &air, 0, false).produced, 1110);
    }

    #[test]
    fn it_turns_on_at_once_and_off_after_the_delay() {
        let mut a = Activity::default();
        assert_eq!(a.set(true, false, 60), Some(true), "on at once");
        // Off: at once the first time, then the delay holds further changes off.
        assert_eq!(a.set(false, true, 60), Some(false));
        assert_eq!(a.delay, 60);
        assert_eq!(a.set(true, false, 60), Some(true));
        assert_eq!(a.set(false, true, 60), None, "within the delay: still shown on");
        for _ in 0..59 {
            assert_eq!(a.tick(true), None);
        }
        assert_eq!(a.tick(true), Some(false), "the delay ran out");
        assert_eq!(a.set(false, false, 60), None);
    }
}
