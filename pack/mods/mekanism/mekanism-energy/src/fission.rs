//! Mekanism Generators' Fission Reactor (`FissionReactorValidator`, `FissionReactorMultiblockData`,
//! `FissionReactorCache`): a cuboid of fission reactor casings, 3x3x3 to 18x18x18, with reactor ports, logic adapters
//! (`CasingType.OTHER`) and reactor glass in its walls; inside, columns of fission fuel assemblies, each under one
//! control rod assembly. Its fuel tank holds `maxFuelPerAssembly` an assembly, as does its waste tank; its coolant
//! tank (water or a cooled chemical coolant) `cooledCoolantPerTank` a block of its volume, its heated coolant tank
//! `heatedCoolantPerTank`. Active, it burns up to its rate limit of fissile fuel a tick (at most `burnPerAssembly` an
//! assembly) for `energyPerFissionFuel` heat a mB, and every whole mB burnt becomes a mB of nuclear waste. Above
//! 373.15 K its heat boils its coolant by its boil efficiency (the assemblies' mean surface area over
//! `surfaceAreaTarget`): water at half that heat (steam), a chemical coolant at its conductivity (its heated variant).
//! The air takes `(T - ambient) / 20010` degrees a tick (its heat capacity: `casingHeatCapacity` a wall block). Above
//! 1200 K it takes `min(T, 1800) / 12000` damage a tick, below it repairs `(1200 - T) / 120000`; past 100 % damage and
//! 1200 K it melts down (`meltdowns.chance` scaled by the damage, a tick) or, with meltdowns off, turns itself off
//! until it is safe again.
use crate::multiblock::{self, Failure, Formation, Inner, Node, Pos};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// `MIN_DAMAGE_TEMPERATURE`: above this it takes damage, below it repairs.
pub const MIN_DAMAGE_TEMPERATURE: f64 = 1200.0;
/// `MAX_DAMAGE_TEMPERATURE`: the damage a tick stops growing here.
pub const MAX_DAMAGE_TEMPERATURE: f64 = 1800.0;
/// `MAX_DAMAGE`: 100 %.
pub const MAX_DAMAGE: f64 = 100.0;
/// `EXPLOSION_CHANCE`: a meltdown's blast chance a tick for each unit of heat it held.
pub const EXPLOSION_CHANCE: f64 = 1.953125E-6;
/// `HeatUtils.BASE_BOIL_TEMP`.
pub const BASE_BOIL_TEMP: f64 = crate::boiler::BASE_BOIL_TEMP;
/// `INVERSE_CONDUCTION_COEFFICIENT`: what a heat handler beside a port sees.
pub const INVERSE_CONDUCTION: f64 = 10.0;
/// `simulateEnvironment`'s `invConduction` (written out in Mekanism: the air's 10 000, its insulation 10 000 and its
/// conduction 10).
const ENVIRONMENT_INVERSE_CONDUCTION: f64 = 20_010.0;
/// `waterConductivity`: the share of its heat water takes.
const WATER_CONDUCTIVITY: f64 = 0.5;
/// The heat a capacitor ignores (`BasicHeatCapacitor.update`: `1.0E-6F`).
const EPSILON: f64 = 1.0E-6_f32 as f64;

/// `generators.toml` `[fission_reactor]` and `[fission_reactor.meltdowns]`, and `general.toml`
/// `[energy_conversion] maxEnergyPerSteam` (`HeatUtils.getWaterThermalEnthalpy`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FissionConfig {
    pub energy_per_fuel: i64,
    pub casing_heat_capacity: f64,
    pub surface_area_target: f64,
    pub default_burn_rate: f64,
    pub burn_per_assembly: i64,
    pub max_fuel_per_assembly: i64,
    pub cooled_coolant_per_tank: i32,
    pub heated_coolant_per_tank: i64,
    pub excess_waste: f64,
    pub meltdowns_enabled: bool,
    pub meltdown_radius: f32,
    pub meltdown_chance: f64,
    pub meltdown_radiation_multiplier: f64,
    pub post_meltdown_damage: f64,
    pub water_thermal_enthalpy: f64,
}

impl Default for FissionConfig {
    /// Mekanism's defaults (ATM10 gives a mB of fuel 250 000 heat).
    fn default() -> Self {
        Self {
            energy_per_fuel: 1_000_000,
            casing_heat_capacity: 1000.0,
            surface_area_target: 4.0,
            default_burn_rate: 0.1,
            burn_per_assembly: 1,
            max_fuel_per_assembly: 8000,
            cooled_coolant_per_tank: 100_000,
            heated_coolant_per_tank: 1_000_000,
            excess_waste: 0.9,
            meltdowns_enabled: true,
            meltdown_radius: 8.0,
            meltdown_chance: 0.001,
            meltdown_radiation_multiplier: 50.0,
            post_meltdown_damage: 75.0,
            water_thermal_enthalpy: 10.0,
        }
    }
}

impl FissionConfig {
    pub fn from_tables(generators: Option<&toml::Table>, general: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        let float = |t: &toml::Table, key: &str, lo: f64, hi: f64| {
            t.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).filter(|v| (lo..=hi).contains(v))
        };
        let long = |t: &toml::Table, key: &str, lo: i64, hi: i64| t.get(key).and_then(toml::Value::as_integer).filter(|v| (lo..=hi).contains(v));
        if let Some(t) = generators.and_then(|g| g.get("fission_reactor")).and_then(toml::Value::as_table) {
            if let Some(v) = long(t, "energyPerFissionFuel", 0, i64::MAX) {
                c.energy_per_fuel = v;
            }
            if let Some(v) = float(t, "casingHeatCapacity", 1.0, 1_000_000.0) {
                c.casing_heat_capacity = v;
            }
            if let Some(v) = float(t, "surfaceAreaTarget", 1.0, f64::MAX) {
                c.surface_area_target = v;
            }
            if let Some(v) = float(t, "defaultBurnRate", 0.001, 1.0) {
                c.default_burn_rate = v;
            }
            if let Some(v) = long(t, "burnPerAssembly", 1, 1_000_000) {
                c.burn_per_assembly = v;
            }
            if let Some(v) = long(t, "maxFuelPerAssembly", 1, 2_251_799_813_685_247) {
                c.max_fuel_per_assembly = v;
            }
            if let Some(v) = long(t, "cooledCoolantPerTank", 1, 368_224) {
                c.cooled_coolant_per_tank = v as i32;
            }
            if let Some(v) = long(t, "heatedCoolantPerTank", 1000, 1_581_510_980_256_305) {
                c.heated_coolant_per_tank = v;
            }
            if let Some(v) = float(t, "excessWaste", 0.001, 1.0) {
                c.excess_waste = v;
            }
            if let Some(m) = t.get("meltdowns").and_then(toml::Value::as_table) {
                if let Some(v) = m.get("enabled").and_then(toml::Value::as_bool) {
                    c.meltdowns_enabled = v;
                }
                if let Some(v) = float(m, "radius", 1.0, 500.0) {
                    c.meltdown_radius = v as f32;
                }
                if let Some(v) = float(m, "chance", 0.0, 1.0) {
                    c.meltdown_chance = v;
                }
                if let Some(v) = float(m, "radiationMultiplier", 0.1, 1_000_000.0) {
                    c.meltdown_radiation_multiplier = v;
                }
                if let Some(v) = float(m, "postMeltdownDamage", 0.0, 100.0) {
                    c.post_meltdown_damage = v;
                }
            }
        }
        if let Some(v) = general
            .and_then(|g| g.get("energy_conversion"))
            .and_then(toml::Value::as_table)
            .and_then(|s| s.get("maxEnergyPerSteam"))
            .and_then(toml::Value::as_integer)
            .filter(|v| *v >= 0)
        {
            c.water_thermal_enthalpy = v as f64;
        }
        c
    }

    /// `<config>/Mekanism/generators.toml` and `general.toml`.
    pub fn load(config: &Path) -> Self {
        let read = |name: &str| std::fs::read_to_string(config.join("Mekanism").join(name)).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_tables(read("generators.toml").as_ref(), read("general.toml").as_ref())
    }

    /// The tanks' capacities for a reactor of `shape` (`setAssemblies`, `setVolume`).
    pub fn capacities(&self, shape: &Shape) -> Capacities {
        let volume = i64::from(shape.volume.max(0));
        let fuel = i64::from(shape.assemblies.max(0)).saturating_mul(self.max_fuel_per_assembly);
        Capacities {
            fuel,
            waste: fuel,
            cooled: (volume * i64::from(self.cooled_coolant_per_tank)).min(i64::from(i32::MAX)) as i32,
            heated: volume.saturating_mul(self.heated_coolant_per_tank),
        }
    }

    /// `getMaxBurnRate`.
    pub fn max_burn_rate(&self, assemblies: i32) -> i64 {
        i64::from(assemblies).saturating_mul(self.burn_per_assembly)
    }

    /// `getBoilEfficiency`: the assemblies' mean surface area over the target, at most 1.
    pub fn boil_efficiency(&self, shape: &Shape) -> f64 {
        if shape.assemblies == 0 {
            return 0.0;
        }
        (f64::from(shape.surface_area) / f64::from(shape.assemblies) / self.surface_area_target).min(1.0)
    }

    /// The heat capacity of a reactor of `locations` wall blocks (`onCreated`).
    pub fn heat_capacity(&self, locations: usize) -> f64 {
        self.casing_heat_capacity * locations as f64
    }
}

/// Its tanks' capacities, in mB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capacities {
    pub fuel: i64,
    pub waste: i64,
    /// The coolant tank, as water or as a chemical.
    pub cooled: i32,
    pub heated: i64,
}

// ---- Formation ----------------------------------------------------------------------------------------------

/// What the validator finds inside a reactor at a position (`validateInner`: air, a fuel assembly or a control
/// rod assembly).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inside {
    Air,
    Fuel,
    Rod,
    Invalid,
    Unloaded,
}

/// A column of fuel assemblies (`FormedAssembly`): its lowest assembly and how many there are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assembly {
    pub pos: Pos,
    pub height: i32,
}

/// What `postcheck` works out of a formed reactor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    /// `fuelAssemblies`.
    pub assemblies: i32,
    /// `surfaceArea`: six faces an assembly, less two for each pair that touch.
    pub surface_area: i32,
    /// `getVolume`: the whole box.
    pub volume: i32,
    pub columns: Vec<Assembly>,
}

/// `FormationProtocol.doUpdate` with the `FissionReactorValidator`: the cuboid (no message when there is none), every
/// block of it (logic adapters are its `OTHER` blocks; inside, fuel and control rod assemblies besides air), then
/// `postcheck`.
pub fn form(nodes: &BTreeMap<Pos, Node>, mut inside: impl FnMut(Pos) -> Inside) -> Result<(Formation, Shape), Failure> {
    if !nodes.values().any(|n| n.is_multiblock()) {
        return Err(Failure::Fail);
    }
    let cuboid = multiblock::fetch_cuboid(nodes, multiblock::MIN_BOUNDS, multiblock::MAX_BOUNDS).ok_or(Failure::Fail)?;
    let mut kinds: BTreeMap<Pos, Inside> = BTreeMap::new();
    let formation = multiblock::validate(cuboid, nodes, |p| {
        let kind = inside(p);
        kinds.insert(p, kind);
        match kind {
            Inside::Air => Inner::Air,
            Inside::Invalid => Inner::Invalid,
            Inside::Unloaded => Inner::Unloaded,
            Inside::Fuel | Inside::Rod => Inner::Internal,
        }
    })?;
    let shape = postcheck(&formation, &kinds)?;
    Ok((formation, shape))
}

/// One column as `postcheck` gathers it (`FuelAssembly`).
#[derive(Default)]
struct Column {
    fuel: BTreeSet<i32>,
    rod: Option<Pos>,
    first: Option<Pos>,
}

/// `FissionReactorValidator.postcheck` over the inside `kinds` of a validated structure.
fn postcheck(formation: &Formation, kinds: &BTreeMap<Pos, Inside>) -> Result<Shape, Failure> {
    // Columns by (x, z), in the order their first block was met (Java's `HashMap` order only picks which failure
    // speaks first when several columns are bad).
    let mut columns: Vec<([i32; 2], Column)> = Vec::new();
    let mut fuel_at: BTreeSet<Pos> = BTreeSet::new();
    let mut assemblies = 0;
    let mut surface = 0;
    for &p in &formation.internal {
        let key = [p[0], p[2]];
        let index = match columns.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                columns.push((key, Column { first: Some(p), ..Column::default() }));
                columns.len() - 1
            }
        };
        let column = &mut columns[index].1;
        match kinds.get(&p) {
            Some(Inside::Fuel) => {
                column.fuel.insert(p[1]);
                assemblies += 1;
                surface += 6;
                for o in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                    if fuel_at.contains(&[p[0] + o[0], p[1] + o[1], p[2] + o[2]]) {
                        surface -= 2;
                    }
                }
                fuel_at.insert(p);
            }
            Some(Inside::Rod) => {
                if column.rod.is_some() {
                    return Err(Failure::ExtraControlRod(p));
                }
                column.rod = Some(p);
            }
            _ => {}
        }
    }
    if columns.is_empty() {
        return Err(Failure::MissingFuelAssembly);
    }
    let mut out = Vec::new();
    for ([x, z], column) in &columns {
        let Some(rod) = column.rod else { return Err(Failure::MissingControlRod([*x, *z])) };
        let Some(&lowest) = column.fuel.first() else { return Err(Failure::BadFuelAssembly(rod)) };
        let mut prev = -1;
        let mut first = true;
        for &y in &column.fuel {
            if !first && y != prev + 1 {
                return Err(Failure::MalformedFuelAssembly([*x, y, *z]));
            }
            first = false;
            prev = y;
        }
        if rod[1] != prev + 1 {
            return Err(Failure::BadControlRod(rod));
        }
        let _ = column.first;
        out.push(Assembly { pos: [*x, lowest, *z], height: column.fuel.len() as i32 });
    }
    Ok(Shape { assemblies, surface_area: surface, volume: formation.cuboid.volume(), columns: out })
}

// ---- Ports and the logic adapter -------------------------------------------------------------------------------

/// A port's mode (`FissionPortMode`, the block's `mode`): fuel and coolant in, waste out, or heated coolant out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortMode {
    #[default]
    Input,
    OutputWaste,
    OutputCoolant,
}

impl PortMode {
    pub const ALL: [Self; 3] = [Self::Input, Self::OutputWaste, Self::OutputCoolant];

    /// Its `mode` property.
    pub fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::OutputWaste => "output_waste",
            Self::OutputCoolant => "output_coolant",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    /// `getNext`.
    pub fn next(self) -> Self {
        match self {
            Self::Input => Self::OutputWaste,
            Self::OutputWaste => Self::OutputCoolant,
            Self::OutputCoolant => Self::Input,
        }
    }

    /// Its text (`FISSION_PORT_MODE_*`).
    pub fn text(self) -> &'static str {
        match self {
            Self::Input => "input only",
            Self::OutputWaste => "output waste",
            Self::OutputCoolant => "output coolant",
        }
    }
}

/// A logic adapter's mode (`FissionReactorLogic`, saved as `logic_type` by its ordinal).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Logic {
    #[default]
    Disabled,
    Activation,
    Temperature,
    CriticalWasteLevel,
    Damaged,
    Depleted,
}

impl Logic {
    pub const ALL: [Self; 6] = [Self::Disabled, Self::Activation, Self::Temperature, Self::CriticalWasteLevel, Self::Damaged, Self::Depleted];

    /// `BY_ID` (wrapping).
    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(6) as usize]
    }

    pub fn id(self) -> i32 {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0) as i32
    }

    /// Its name (`REACTOR_LOGIC_*`).
    pub fn text(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Activation => "Activation",
            Self::Temperature => "High Temperature",
            Self::CriticalWasteLevel => "Critical Waste Level",
            Self::Damaged => "Damage Critical",
            Self::Depleted => "Insufficient Fuel",
        }
    }

    /// Its description (`DESCRIPTION_REACTOR_*`).
    pub fn description(self, excess_waste: f64) -> String {
        match self {
            Self::Disabled => "Will not handle redstone".into(),
            Self::Activation => "Activate the reactor when powered, and deactivate when unpowered".into(),
            Self::Temperature => "Output when the reactor reaches dangerous temperatures".into(),
            Self::CriticalWasteLevel => format!("Output when the reactor has reached a critical level of waste ({}%)", (excess_waste * 100.0).round() as i64),
            Self::Damaged => "Output when the reactor reaches critical damage levels (100%+).".into(),
            Self::Depleted => "Output when the reactor has insufficient fuel to sustain a reaction".into(),
        }
    }
}

/// `RedstoneStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    Outputting,
    Powered,
}

impl Status {
    pub fn text(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Outputting => "Outputting",
            Self::Powered => "Powered",
        }
    }
}

/// `IRedstoneControl.RedstoneControl`, saved as `control_type` by its ordinal; the adapter supports all but
/// `DISABLED`, which a new adapter still holds until it loads (`supportedOrNextType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RedstoneControl {
    #[default]
    Disabled,
    High,
    Low,
    Pulse,
}

impl RedstoneControl {
    pub fn by_id(id: i32) -> Self {
        [Self::Disabled, Self::High, Self::Low, Self::Pulse][id.rem_euclid(4) as usize]
    }

    pub fn id(self) -> i32 {
        match self {
            Self::Disabled => 0,
            Self::High => 1,
            Self::Low => 2,
            Self::Pulse => 3,
        }
    }

    /// `supportedOrNextType` for the adapter: `DISABLED` becomes `HIGH`.
    pub fn supported(self) -> Self {
        if self == Self::Disabled { Self::High } else { self }
    }

    /// The next mode the adapter's button gives (`getNext` while unsupported ones are skipped).
    pub fn next(self) -> Self {
        match self {
            Self::Disabled | Self::Pulse => Self::High,
            Self::High => Self::Low,
            Self::Low => Self::Pulse,
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::High => "High",
            Self::Low => "Low",
            Self::Pulse => "Pulse",
        }
    }

    /// `isRedstoneActivated` for a block powered now (`powered`) and last tick (`was`).
    pub fn activated(self, powered: bool, was: bool) -> bool {
        match self {
            Self::Disabled => true,
            Self::High => powered,
            Self::Low => !powered,
            Self::Pulse => powered && !was,
        }
    }
}

/// What a logic adapter reads of its formed reactor (`getStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Watch {
    pub temperature: f64,
    pub waste: i64,
    pub waste_capacity: i64,
    pub damage: f64,
    pub fuel_empty: bool,
}

/// `TileEntityFissionReactorLogicAdapter.getStatus` of a formed reactor (`can_function`: the adapter's redstone
/// control lets it work).
pub fn status(logic: Logic, watch: &Watch, can_function: bool, config: &FissionConfig) -> Status {
    let outputting = match logic {
        Logic::Disabled => false,
        Logic::Activation => return if can_function { Status::Powered } else { Status::Idle },
        Logic::Temperature => watch.temperature >= MIN_DAMAGE_TEMPERATURE,
        Logic::CriticalWasteLevel => watch.waste >= crate::clamp_to_long(watch.waste_capacity as f64 * config.excess_waste),
        Logic::Damaged => watch.damage >= MAX_DAMAGE,
        Logic::Depleted => watch.fuel_empty,
    };
    if outputting { Status::Outputting } else { Status::Idle }
}

// ---- The tick --------------------------------------------------------------------------------------------------

/// What it saves beside its tanks (`FissionReactorCache`: `reactor_damage`, `injection_rate`, `burn_time`,
/// `partial_waste`, `disabled`, `active`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Core {
    pub damage: f64,
    /// `rateLimit`; -1 in a cache that never had one (`defaultBurnRate` then).
    pub rate_limit: f64,
    pub burn_remaining: f64,
    pub partial_waste: f64,
    pub active: bool,
    pub force_disabled: bool,
}

impl Core {
    /// A new cache's (`rateLimit` -1).
    pub fn new() -> Self {
        Self { rate_limit: -1.0, ..Self::default() }
    }

    /// `FissionReactorCache.getRateLimit`.
    pub fn rate(&self, config: &FissionConfig) -> f64 {
        if self.rate_limit == -1.0 { config.default_burn_rate } else { self.rate_limit }
    }

    /// `FissionReactorCache.merge`.
    pub fn merge(&mut self, other: &Core) {
        self.damage = self.damage.max(other.damage);
        self.rate_limit = self.rate_limit.max(other.rate_limit);
        self.burn_remaining += other.burn_remaining;
        self.partial_waste += other.partial_waste;
        self.active |= other.active;
        self.force_disabled |= other.force_disabled;
    }

    /// `FissionReactorCache.apply` on a formed reactor: the rate limit within its max burn rate, `setForceDisable`
    /// then `setActive` (which a force disabled reactor refuses).
    pub fn applied(&self, max_burn: i64, config: &FissionConfig) -> Core {
        let mut out = *self;
        out.rate_limit = self.rate(config).clamp(0.0, max_burn as f64);
        out.active = self.active && !self.force_disabled;
        out
    }

    /// `setActive`: turning on is refused while force disabled.
    pub fn set_active(&mut self, active: bool) {
        if self.active != active && (!active || !self.force_disabled) {
            self.active = active;
        }
    }

    /// `setForceDisable`: also turns it off.
    pub fn set_force_disabled(&mut self, disabled: bool) {
        if self.force_disabled != disabled {
            self.force_disabled = disabled;
            if disabled {
                self.set_active(false);
            }
        }
    }

    /// `setRateLimit`: within 0 and its max burn rate.
    pub fn set_rate_limit(&mut self, rate: f64, max_burn: i64) {
        self.rate_limit = rate.clamp(0.0, max_burn as f64);
    }
}

/// Its tanks, in mB. The coolant is water or a chemical, never both (`MergedTank`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tanks {
    pub fuel: i64,
    /// The heated coolant (steam or a heated chemical coolant).
    pub heated: i64,
    pub waste: i64,
    /// The cooled chemical coolant.
    pub coolant: i64,
    pub water: i32,
}

/// The cooled chemical coolant in its coolant tank (`CooledCoolant`: sodium is 5 heat a mB at a conductivity of 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CooledCoolant {
    pub thermal_enthalpy: f64,
    pub conductivity: f64,
}

/// What its coolant tank holds and where the heated coolant can go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coolant {
    /// A chemical coolant in the tank (`None`: water, or a chemical that isn't a coolant, or nothing).
    pub chemical: Option<CooledCoolant>,
    /// The heated coolant tank takes what boils (empty, or holding the same heated chemical).
    pub heated_takes: bool,
}

/// The heat capacitor: what it holds (`storedHeat`), its capacity, and what was handed to it this tick
/// (`heatToHandle`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heat {
    pub stored: f64,
    pub capacity: f64,
}

impl Heat {
    pub fn temperature(&self) -> f64 {
        self.stored / self.capacity
    }
}

/// What a tick did, for its screen, its logic adapters and the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    /// `lastBurnRate`.
    pub burn_rate: f64,
    /// `lastBoilRate`.
    pub boil_rate: i64,
    /// `lastEnvironmentLoss`.
    pub environment_loss: f64,
    /// Nuclear waste that had no room in the waste tank (Mekanism radiates it: `leftoverWaste * radioactivity` at the
    /// reactor's centre).
    pub waste_overflow: i64,
    /// The damage roll came up (or a force disabled reactor reached it with meltdowns on): it melts down.
    pub meltdown: bool,
}

/// `burnFuel`: up to the rate limit, the fuel there is and its assemblies' max a tick; the heat is handed to the
/// capacitor (`pending`), each whole mB burnt becomes waste.
fn burn(tanks: &mut Tanks, core: &mut Core, caps: &Capacities, pending: &mut f64, assemblies: i32, config: &FissionConfig) -> (f64, i64) {
    let mut stored = tanks.fuel as f64 + core.burn_remaining;
    let to_burn = core.rate_limit.min(stored).min(config.max_burn_rate(assemblies) as f64);
    stored -= to_burn;
    // `setStackSize((long) storedFuel)`: an empty tank stays empty.
    if tanks.fuel > 0 {
        tanks.fuel = crate::clamp_to_long(stored).max(0);
    }
    core.burn_remaining = stored % 1.0;
    *pending += to_burn * config.energy_per_fuel as f64;
    core.partial_waste += to_burn;
    let new_waste = core.partial_waste.floor() as i64;
    let mut overflow = 0;
    if new_waste > 0 {
        core.partial_waste %= 1.0;
        let room = (caps.waste - tanks.waste).max(0);
        overflow = (new_waste - room).max(0);
        tanks.waste += new_waste.min(room);
    }
    (to_burn, overflow)
}

/// `handleCoolant`: the heat above 373.15 K times the boil efficiency boils water at half that heat into steam (50
/// heat a mB at 10 J a mB of steam), or a chemical coolant at its conductivity into its heated variant; the heat it
/// took is handed back to the capacitor. What boiled is used up even when the heated tank has no room for it.
fn boil(tanks: &mut Tanks, caps: &Capacities, heat: &Heat, pending: &mut f64, efficiency: f64, coolant: Coolant, config: &FissionConfig) -> i64 {
    let available = efficiency * (heat.stored - BASE_BOIL_TEMP * heat.capacity);
    let clamp = |heated: f64, stored: i64| crate::clamp_to_long(heated).clamp(0, stored);
    let insert = |tanks: &mut Tanks, n: i64| {
        if coolant.heated_takes {
            tanks.heated += n.min((caps.heated - tanks.heated).max(0));
        }
    };
    if tanks.water > 0 {
        let case = available * WATER_CONDUCTIVITY;
        let rate = clamp(crate::boiler::STEAM_ENERGY_EFFICIENCY * case / config.water_thermal_enthalpy, i64::from(tanks.water));
        if rate > 0 {
            tanks.water -= rate as i32;
            insert(tanks, rate);
            *pending -= rate as f64 * config.water_thermal_enthalpy / crate::boiler::STEAM_ENERGY_EFFICIENCY;
        }
        return rate.max(0);
    }
    if tanks.coolant > 0 {
        let Some(c) = coolant.chemical else { return 0 };
        let rate = clamp(available * c.conductivity / c.thermal_enthalpy, tanks.coolant);
        if rate > 0 {
            tanks.coolant -= rate;
            insert(tanks, rate);
            *pending -= rate as f64 * c.thermal_enthalpy;
        }
        return rate;
    }
    0
}

/// `BasicHeatCapacitor.update`: what was handed to it lands, unless it is too small to count.
fn update(heat: &mut Heat, pending: &mut f64) {
    if *pending != 0.0 && pending.abs() > EPSILON {
        heat.stored += *pending;
    }
    *pending = 0.0;
}

/// `FissionReactorMultiblockData.tick` (with `updateHeatCapacitors` and `handleDamage`), less the ports' output:
/// the burn while active, the coolant, the air, the heat landing, then the damage. `roll` draws in [0, 1)
/// (`world.random.nextDouble`) for the meltdown chance once it is past 100 % and 1200 K. `handed` is the heat other
/// blocks gave it since its last tick.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    tanks: &mut Tanks,
    core: &mut Core,
    heat: &mut Heat,
    caps: &Capacities,
    shape: &Shape,
    coolant: Coolant,
    ambient: f64,
    handed: f64,
    roll: impl FnOnce() -> f64,
    config: &FissionConfig,
) -> Tick {
    let mut out = Tick::default();
    let mut pending = handed;
    if core.active {
        let (burnt, overflow) = burn(tanks, core, caps, &mut pending, shape.assemblies, config);
        out.burn_rate = burnt;
        out.waste_overflow = overflow;
    }
    out.boil_rate = boil(tanks, caps, heat, &mut pending, config.boil_efficiency(shape), coolant, config);
    // `simulateEnvironment`.
    let transfer = (heat.temperature() - ambient) / ENVIRONMENT_INVERSE_CONDUCTION;
    pending -= transfer * heat.capacity;
    out.environment_loss = transfer.max(0.0);
    update(heat, &mut pending);
    // `handleDamage`.
    let temperature = heat.temperature();
    if temperature > MIN_DAMAGE_TEMPERATURE {
        core.damage += temperature.min(MAX_DAMAGE_TEMPERATURE) / (MIN_DAMAGE_TEMPERATURE * 10.0);
    } else {
        core.damage = (core.damage - (MIN_DAMAGE_TEMPERATURE - temperature) / (MIN_DAMAGE_TEMPERATURE * 100.0)).max(0.0);
    }
    if core.damage >= MAX_DAMAGE && temperature >= MIN_DAMAGE_TEMPERATURE {
        if core.force_disabled && config.meltdowns_enabled {
            core.set_force_disabled(false);
            out.meltdown = true;
        } else if roll() < core.damage / MAX_DAMAGE * config.meltdown_chance {
            if config.meltdowns_enabled {
                out.meltdown = true;
            } else {
                core.set_force_disabled(true);
            }
        }
    } else if core.damage < MAX_DAMAGE && temperature < MIN_DAMAGE_TEMPERATURE {
        core.set_force_disabled(false);
    }
    out
}

/// `meltdownHappened` on a formed reactor: its heated coolant goes (and, with radiation on, the radioactive tanks:
/// the fuel, the waste), it is off, its damage `postMeltdownDamage`, its burn and waste remainders gone and its heat
/// back at the air's. The radioactivity it would spread: the dumped tanks' times `radiationMultiplier` (out of scope
/// here: the caller only reports it).
pub fn meltdown_happened(tanks: &mut Tanks, core: &mut Core, heat: &mut Heat, ambient: f64, radiation: bool, config: &FissionConfig) {
    if radiation {
        tanks.fuel = 0;
        tanks.waste = 0;
    }
    tanks.heated = 0;
    core.active = false;
    core.damage = config.post_meltdown_damage;
    core.burn_remaining = 0.0;
    core.partial_waste = 0.0;
    heat.stored = heat.capacity * ambient;
}

/// A meltdown's blast roll a tick (`Meltdown.update`): `next_int() % 10 == 0` and then a draw under the heat it held
/// times `EXPLOSION_CHANCE`.
pub fn meltdown_blasts(next_int: i32, draw: f64, magnitude: f64) -> bool {
    next_int % 10 == 0 && draw < magnitude * EXPLOSION_CHANCE
}

/// `Meltdown`'s lifetime in ticks.
pub const MELTDOWN_DURATION: i32 = 100;

#[cfg(test)]
mod tests {
    use super::*;

    const ATM10: FissionConfig = FissionConfig {
        energy_per_fuel: 250_000,
        casing_heat_capacity: 1000.0,
        surface_area_target: 4.0,
        default_burn_rate: 0.1,
        burn_per_assembly: 1,
        max_fuel_per_assembly: 8000,
        cooled_coolant_per_tank: 100_000,
        heated_coolant_per_tank: 1_000_000,
        excess_waste: 0.9,
        meltdowns_enabled: true,
        meltdown_radius: 8.0,
        meltdown_chance: 0.001,
        meltdown_radiation_multiplier: 50.0,
        post_meltdown_damage: 75.0,
        water_thermal_enthalpy: 10.0,
    };
    /// Plains (0.8f) as Mekanism reads it.
    const PLAINS: f64 = 300.0 + 25.0 * (0.8_f32 as f64 - 0.8);
    const SODIUM: CooledCoolant = CooledCoolant { thermal_enthalpy: 5.0, conductivity: 1.0 };

    /// A reactor box of casings `size` (length, height, width) at the origin; `inside` names the fuel and rods.
    fn reactor(size: [i32; 3], inside: &[(Pos, Inside)]) -> Result<(Formation, Shape), Failure> {
        let [l, h, w] = size;
        let mut nodes = BTreeMap::new();
        for x in 0..l {
            for y in 0..h {
                for z in 0..w {
                    let wall = x == 0 || y == 0 || z == 0 || x == l - 1 || y == h - 1 || z == w - 1;
                    if wall {
                        nodes.insert([x, y, z], Node::Frame);
                    }
                }
            }
        }
        let inside: BTreeMap<Pos, Inside> = inside.iter().copied().collect();
        form(&nodes, |p| inside.get(&p).copied().unwrap_or(Inside::Air))
    }

    fn column(x: i32, z: i32, fuel: i32, rod: bool) -> Vec<(Pos, Inside)> {
        let mut out: Vec<(Pos, Inside)> = (0..fuel).map(|i| ([x, 1 + i, z], Inside::Fuel)).collect();
        if rod {
            out.push(([x, 1 + fuel, z], Inside::Rod));
        }
        out
    }

    #[test]
    fn the_oracle_s_formations() {
        let min = column(1, 1, 1, true);
        assert!(reactor([3, 4, 3], &min).is_ok());
        assert_eq!(reactor([3, 3, 3], &column(1, 1, 1, false)).err(), Some(Failure::MissingControlRod([1, 1])));
        assert_eq!(reactor([3, 3, 3], &[([1, 1, 1], Inside::Rod)]).err(), Some(Failure::BadFuelAssembly([1, 1, 1])));
        assert_eq!(reactor([3, 3, 3], &[]).err(), Some(Failure::MissingFuelAssembly));
        assert_eq!(reactor([3, 5, 3], &[([1, 1, 1], Inside::Fuel), ([1, 3, 1], Inside::Rod)]).err(), Some(Failure::BadControlRod([1, 3, 1])));
        let gap = [([1, 1, 1], Inside::Fuel), ([1, 3, 1], Inside::Fuel), ([1, 4, 1], Inside::Rod)];
        assert_eq!(reactor([3, 6, 3], &gap).err(), Some(Failure::MalformedFuelAssembly([1, 3, 1])));
        let two = [([1, 1, 1], Inside::Fuel), ([1, 2, 1], Inside::Rod), ([1, 3, 1], Inside::Rod)];
        assert_eq!(reactor([3, 5, 3], &two).err(), Some(Failure::ExtraControlRod([1, 3, 1])));
        assert_eq!(reactor([3, 5, 3], &[([1, 1, 1], Inside::Rod), ([1, 2, 1], Inside::Fuel)]).err(), Some(Failure::BadControlRod([1, 1, 1])));
        assert!(reactor([3, 5, 3], &min).is_ok(), "air above the rod");
        assert!(reactor([3, 5, 3], &[([1, 2, 1], Inside::Fuel), ([1, 3, 1], Inside::Rod)]).is_ok(), "air below the fuel");
        let mut dirt = min.clone();
        dirt.push(([1, 3, 1], Inside::Invalid));
        assert_eq!(reactor([3, 5, 3], &dirt).err(), Some(Failure::InvalidInner([1, 3, 1])));
        let (_, shape) = reactor([5, 5, 5], &[column(1, 1, 2, true), column(2, 1, 2, true)].concat()).unwrap();
        assert_eq!((shape.assemblies, shape.surface_area, shape.volume, shape.columns.len()), (4, 16, 125, 2));
        let layer: Vec<(Pos, Inside)> = (1..4).flat_map(|x| (1..4).flat_map(move |z| column(x, z, 1, true))).collect();
        let (_, shape) = reactor([5, 5, 5], &layer).unwrap();
        assert_eq!((shape.assemblies, shape.surface_area), (9, 30));
        assert_eq!(ATM10.boil_efficiency(&shape), 30.0 / 9.0 / 4.0);
    }

    #[test]
    fn capacities_follow_assemblies_and_volume() {
        let shape = Shape { assemblies: 1, surface_area: 6, volume: 36, columns: Vec::new() };
        assert_eq!(ATM10.capacities(&shape), Capacities { fuel: 8000, waste: 8000, cooled: 3_600_000, heated: 36_000_000 });
        assert_eq!(ATM10.heat_capacity(34), 34_000.0);
    }

    fn one() -> Shape {
        Shape { assemblies: 1, surface_area: 6, volume: 36, columns: Vec::new() }
    }

    /// The oracle's W and S (3x4x3, one assembly, 0.1 mB/t, fuel kept full): its heat after each tick.
    #[test]
    fn it_heats_as_on_the_oracle() {
        let shape = one();
        let caps = ATM10.capacities(&shape);
        let mut heat = Heat { stored: 34_000.0 * PLAINS, capacity: 34_000.0 };
        assert_eq!(heat.stored, 10_200_000.01013279);
        let mut core = Core { active: true, ..Core::new().applied(1, &ATM10) };
        let mut tanks = Tanks { fuel: 8000, ..Tanks::default() };
        let coolant = Coolant { chemical: None, heated_takes: true };
        let mut rows = Vec::new();
        for _ in 0..11 {
            tick(&mut tanks, &mut core, &mut heat, &caps, &shape, coolant, PLAINS, 0.0, || 1.0, &ATM10);
            tanks.fuel = 8000;
            rows.push((heat.stored, core.burn_remaining, core.partial_waste, tanks.waste));
        }
        assert_eq!(rows[0], (10_225_000.01013279, 0.8999999999996362, 0.1, 0));
        assert_eq!(rows[1].0, 10_249_998.760757478);
        assert_eq!(rows[2].0, 10_274_996.26206929);
        assert_eq!((rows[9].0, rows[9].2, rows[9].3), (10_449_943.795735585, 0.9999999999999999, 0), "0.1 ten times is under 1");
        assert_eq!((rows[10].0, rows[10].1, rows[10].2, rows[10].3), (10_474_931.304791776, 0.8999999999959982, 0.09999999999999987, 1));
    }

    /// The oracle's S: sodium boiling from 373.15 K, the reactor holding at 373.8817 K.
    #[test]
    fn sodium_holds_it_at_the_oracle_s_temperature() {
        let shape = one();
        let caps = ATM10.capacities(&shape);
        let mut heat = Heat { stored: 12_693_825.6863, capacity: 34_000.0 };
        let mut core = Core { active: true, ..Core::new().applied(1, &ATM10) };
        let mut tanks = Tanks { fuel: 8000, coolant: 3_600_000, ..Tanks::default() };
        let coolant = Coolant { chemical: Some(SODIUM), heated_takes: true };
        let mut boiled = 0;
        for _ in 0..1000 {
            let t = tick(&mut tanks, &mut core, &mut heat, &caps, &shape, coolant, PLAINS, 0.0, || 1.0, &ATM10);
            boiled = t.boil_rate;
            tanks.fuel = 8000;
            tanks.coolant = 3_600_000;
        }
        assert!((heat.temperature() - 373.8817).abs() < 1e-3, "{}", heat.temperature());
        assert!((4974..=4976).contains(&boiled), "{boiled}");
    }

    /// Runs a 3x4x3 of one assembly from the air's heat at `rate` with its fuel kept full, `water` or dry, for
    /// `ticks`: the heat, damage and boil after each.
    fn run_one(rate: f64, water: bool, ticks: usize) -> Vec<(f64, f64, i64)> {
        let shape = one();
        let caps = ATM10.capacities(&shape);
        let mut heat = Heat { stored: 34_000.0 * PLAINS, capacity: 34_000.0 };
        let mut core = Core { active: true, ..Core::new().applied(1, &ATM10) };
        core.set_rate_limit(rate, 1);
        let mut tanks = Tanks { fuel: 8000, water: if water { 3_600_000 } else { 0 }, ..Tanks::default() };
        let coolant = Coolant { chemical: None, heated_takes: true };
        (0..ticks)
            .map(|_| {
                let t = tick(&mut tanks, &mut core, &mut heat, &caps, &shape, coolant, PLAINS, 0.0, || 1.0, &ATM10);
                tanks.fuel = 8000;
                tanks.heated = 0;
                if water {
                    tanks.water = 3_600_000;
                }
                (heat.stored, core.damage, t.boil_rate)
            })
            .collect()
    }

    /// The oracle's W2: water boils from its 101st tick, 67 mB then 24 012 by the 150th.
    #[test]
    fn water_boils_as_on_the_oracle() {
        let rows = run_one(0.1, true, 150);
        assert_eq!(rows[99].0, 12_693_825.686264288);
        assert_eq!((rows[100].0, rows[100].2), (12_715_351.057294967, 67));
        assert_eq!(rows[149].0, 12_736_890.916988336);
        assert_eq!(rows[100..150].iter().map(|r| r.2).sum::<i64>(), 24_012);
    }

    /// The oracle's M: dry at 1 mB/t, its damage from 1200 K on.
    #[test]
    fn dry_it_takes_damage_as_on_the_oracle() {
        let rows = run_one(1.0, false, 1000);
        assert_eq!(rows[0].0, 10_450_000.01013279);
        assert_eq!(rows[99].0, 35_138_256.771447696);
        assert_eq!(rows[149], (47_560_725.90591071, 3.0339724558561616, 0));
        assert_eq!(rows[199].1, 9.637275531423418);
        assert_eq!(rows[399].1, 39.631082930784146);
        assert_eq!(rows[799].1, 99.63108293078527);
        assert_eq!((rows[999].0, rows[999].1), (254_061_841.33723912, 129.6310829307864));
    }

    #[test]
    fn damage_and_the_force_disable() {
        let shape = one();
        let caps = ATM10.capacities(&shape);
        let off = FissionConfig { meltdowns_enabled: false, meltdown_chance: 1.0, ..ATM10 };
        let mut heat = Heat { stored: 1300.0 * 34_000.0, capacity: 34_000.0 };
        let mut core = Core { active: true, damage: 99.99, ..Core::new().applied(1, &off) };
        let mut tanks = Tanks::default();
        let coolant = Coolant { chemical: None, heated_takes: true };
        let t = tick(&mut tanks, &mut core, &mut heat, &caps, &shape, coolant, PLAINS, 0.0, || 0.0, &off);
        assert!(!t.meltdown);
        assert!(core.force_disabled && !core.active);
        core.set_active(true);
        assert!(!core.active, "refused while force disabled");
        let mut on = core;
        let t = tick(&mut tanks, &mut on, &mut heat, &caps, &shape, coolant, PLAINS, 0.0, || 1.0, &ATM10);
        assert!(t.meltdown && !on.force_disabled, "a force disabled reactor melts down once meltdowns are on");
        heat.stored = 1000.0 * 34_000.0;
        core.damage = 50.0;
        tick(&mut tanks, &mut core, &mut heat, &caps, &shape, coolant, PLAINS, 0.0, || 1.0, &off);
        assert!(!core.force_disabled);
        assert!((core.damage - (50.0 - 200.0 / 120_000.0)).abs() < 1e-6, "{}", core.damage);
    }

    #[test]
    fn logic_statuses() {
        let w = Watch { temperature: 1200.0, waste: 7200, waste_capacity: 8000, damage: 100.0, fuel_empty: true };
        for (logic, want) in [
            (Logic::Disabled, Status::Idle),
            (Logic::Temperature, Status::Outputting),
            (Logic::CriticalWasteLevel, Status::Outputting),
            (Logic::Damaged, Status::Outputting),
            (Logic::Depleted, Status::Outputting),
        ] {
            assert_eq!(status(logic, &w, false, &ATM10), want, "{logic:?}");
        }
        assert_eq!(status(Logic::Activation, &w, true, &ATM10), Status::Powered);
        let cool = Watch { temperature: 1199.9, waste: 7199, waste_capacity: 8000, damage: 99.9, fuel_empty: false };
        for logic in [Logic::Temperature, Logic::CriticalWasteLevel, Logic::Damaged, Logic::Depleted] {
            assert_eq!(status(logic, &cool, true, &ATM10), Status::Idle, "{logic:?}");
        }
        assert_eq!(Logic::by_id(7), Logic::Activation);
        assert_eq!(RedstoneControl::Disabled.supported(), RedstoneControl::High);
        assert_eq!(PortMode::Input.next().next().next(), PortMode::Input);
    }

    #[test]
    fn its_settings_come_from_generators_toml() {
        let generators: toml::Table =
            "[fission_reactor]\nenergyPerFissionFuel = 250000\n[fission_reactor.meltdowns]\nenabled = false\nchance = 0.5\n".parse().unwrap();
        let c = FissionConfig::from_tables(Some(&generators), None);
        assert_eq!((c.energy_per_fuel, c.meltdowns_enabled, c.meltdown_chance, c.default_burn_rate), (250_000, false, 0.5, 0.1));
    }
}
