//! Mekanism 10.7's chemicals (`mekanism.api.chemical`): one `Chemical` type
//! for what were gases, infuse types, pigments and slurries, a registry of
//! them (`mekanism:chemical`), `ChemicalStack`s of a chemical and a `long`
//! amount in mB, the tanks that hold them (`BasicChemicalTank`), the
//! `IChemicalHandler` contract a block shows by side (`ISidedChemicalHandler`,
//! `ProxyChemicalHandler`, `ChemicalUtils.insert` and `extract`) and the
//! ingredients recipes take them by (`ChemicalStackIngredient`).
//!
//! The registry is the pack's: each chemical as the NeoForge oracle saw it
//! once the data packs were loaded (`ChemicalFacts`), declared in its mod's
//! content at `data/<ns>/mekanism/chemical/<path>.json` (docs/mekanism-chemicals.md):
//! its icon, tint, attributes (fuel, radioactivity, coolants: Mekanism's data
//! maps, final), tags and the kind its builder gave it. Nothing here reads
//! files or knows the game.
use crate::container::Automation;
use serde_json::Value;
use std::collections::HashMap;

/// `MekanismAPI.EMPTY_CHEMICAL_KEY`: the registry's default, no chemical.
pub const EMPTY: &str = "mekanism:empty";

/// What its builder made a chemical, by its icon (`ChemicalBuilder.builder`,
/// `infuseType`, `pigment`, `cleanSlurry`, `dirtySlurry`): Mekanism 10.7 has
/// one type, and these are the four it merged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChemicalKind {
    /// `mekanism:liquid/...` and any other icon: what were gases.
    Gas,
    InfuseType,
    Pigment,
    Slurry,
}

impl ChemicalKind {
    pub fn of_texture(texture: &str) -> Self {
        let path = texture.split_once(':').map_or(texture, |(_, p)| p);
        if path.starts_with("infuse_type/") {
            Self::InfuseType
        } else if path.starts_with("pigment/") {
            Self::Pigment
        } else if path.starts_with("slurry/") {
            Self::Slurry
        } else {
            Self::Gas
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Gas => "gas",
            Self::InfuseType => "infuse_type",
            Self::Pigment => "pigment",
            Self::Slurry => "slurry",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Gas, Self::InfuseType, Self::Pigment, Self::Slurry].into_iter().find(|k| k.name() == name)
    }
}

/// `ChemicalFuel` (`mekanism:chemical_attribute_fuel`): burns `burn_ticks`
/// ticks a mB, `energy_per_tick` Joules each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuel {
    pub burn_ticks: u32,
    pub energy_per_tick: u64,
}

impl Fuel {
    /// `ChemicalFuel.energyDensity`: Joules a mB.
    pub fn energy_density(&self) -> u64 {
        self.energy_per_tick.saturating_mul(u64::from(self.burn_ticks))
    }
}

/// `CooledCoolant` and `HeatedCoolant`: the other variant, the thermal
/// enthalpy, the conductivity and (heated) the temperature.
#[derive(Clone, Debug, PartialEq)]
pub struct Coolant {
    pub other: String,
    pub thermal_enthalpy: f64,
    pub conductivity: f64,
    pub temperature: Option<f64>,
}

/// One registered chemical.
#[derive(Clone, Debug, PartialEq)]
pub struct Chemical {
    pub id: String,
    /// `getIcon`: a block atlas sprite (`mekanism:liquid/hydrogen`).
    pub texture: String,
    /// `getTint`: Java's int, `0xAARRGGBB` as the builder set it (Mekanism's
    /// own infuse types leave the alpha 0).
    pub tint: u32,
    /// `getColorRepresentation`: a bar's colour, the tint unless the chemical
    /// overrides it (`ChemicalUtil.chemical`: bio and fungi).
    pub color: u32,
    pub kind: ChemicalKind,
    /// `isGaseous`: the builder's flag or `#mekanism:gaseous`.
    pub gaseous: bool,
    /// `getRadioactivity`: Sv/h a mB; 0 for none.
    pub radioactivity: f64,
    /// `hasAttributesWithValidation` as the pack runs (radiation on): the
    /// default attribute validator keeps it out of ordinary tanks.
    pub validated: bool,
    /// `getOreTag`: the item tag a slurry is made of.
    pub solid_tag: Option<String>,
    pub fuel: Option<Fuel>,
    pub cooled: Option<Coolant>,
    pub heated: Option<Coolant>,
    /// Its chemical tags (`#mekanism:gaseous`, `#mekanism:carbon`), sorted.
    pub tags: Vec<String>,
}

fn number(v: &Value) -> Option<f64> {
    v.as_f64()
}

/// `0xAARRGGBB`, `#RRGGBB` or Java's int.
pub fn parse_tint(v: &Value) -> Option<u32> {
    if let Some(n) = v.as_i64() {
        return Some(n as u32 & 0xFFFF_FFFF);
    }
    let s = v.as_str()?;
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).or_else(|| s.strip_prefix('#'))?;
    u32::from_str_radix(hex, 16).ok()
}

impl Chemical {
    /// A declaration (`data/<ns>/mekanism/chemical/<path>.json`) of `id`.
    pub fn from_declaration(id: &str, v: &Value) -> Result<Self, String> {
        if !valid_id(id) || id == EMPTY {
            return Err(format!("{id}: not a chemical id"));
        }
        let texture = v.get("texture").and_then(Value::as_str).ok_or_else(|| format!("{id}: no texture"))?.to_owned();
        let colour = |key: &str, default: u32| -> Result<u32, String> {
            match v.get(key) {
                None => Ok(default),
                Some(t) => parse_tint(t).ok_or_else(|| format!("{id}: {key} is 0xAARRGGBB or a number")),
            }
        };
        let tint = colour("tint", 0xFFFFFF)?;
        let color = colour("color", tint)?;
        let kind = match v.get("kind").and_then(Value::as_str) {
            None => ChemicalKind::of_texture(&texture),
            Some(k) => ChemicalKind::from_name(k).ok_or_else(|| format!("{id}: kind {k} is gas, infuse_type, pigment or slurry"))?,
        };
        let attributes = v.get("attributes").cloned().unwrap_or(Value::Null);
        let fuel = match attributes.get("fuel") {
            None => None,
            Some(f) => {
                let burn_ticks = f.get("burn_time").and_then(Value::as_u64).filter(|t| *t >= 1).ok_or_else(|| format!("{id}: a fuel burns at least a tick"))?;
                let energy_per_tick = f.get("energy").and_then(Value::as_u64).filter(|e| *e >= 1).ok_or_else(|| format!("{id}: a fuel gives energy"))?;
                Some(Fuel { burn_ticks: burn_ticks.min(u64::from(u32::MAX)) as u32, energy_per_tick })
            }
        };
        let coolant = |key: &str, other: &str| -> Result<Option<Coolant>, String> {
            let Some(c) = attributes.get(key) else {
                return Ok(None);
            };
            Ok(Some(Coolant {
                other: c.get(other).and_then(Value::as_str).ok_or_else(|| format!("{id}: {key} names its {other}"))?.to_owned(),
                thermal_enthalpy: c.get("thermal_enthalpy").and_then(number).ok_or_else(|| format!("{id}: {key} has a thermal_enthalpy"))?,
                conductivity: c.get("conductivity").and_then(number).unwrap_or(1.0),
                temperature: c.get("temperature").and_then(number),
            }))
        };
        let radioactivity = attributes.get("radioactivity").and_then(number).or_else(|| v.get("radioactivity").and_then(number)).unwrap_or(0.0);
        let mut tags: Vec<String> = v
            .get("tags")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).map(|t| t.trim_start_matches('#').to_owned()).collect())
            .unwrap_or_default();
        tags.sort();
        tags.dedup();
        let gaseous = v.get("gaseous").and_then(Value::as_bool).unwrap_or(false) || tags.iter().any(|t| t == "mekanism:gaseous");
        Ok(Self {
            id: id.to_owned(),
            texture,
            tint,
            color,
            kind,
            gaseous,
            radioactivity,
            validated: v.get("validated").and_then(Value::as_bool).unwrap_or(radioactivity > 0.0),
            solid_tag: v.get("solid_tag").and_then(Value::as_str).map(str::to_owned),
            fuel,
            cooled: coolant("cooled_coolant", "heated")?,
            heated: coolant("heated_coolant", "cooled")?,
            tags,
        })
    }

    pub fn is_radioactive(&self) -> bool {
        self.radioactivity > 0.0
    }

    /// `chemical.<ns>.<path>`, the translation key.
    pub fn translation_key(&self) -> String {
        let (ns, path) = self.id.split_once(':').unwrap_or(("minecraft", &self.id));
        format!("chemical.{ns}.{}", path.replace('/', "."))
    }
}

/// A resource location: `ns:path` of `[a-z0-9_.-]` and `[a-z0-9_.-/]`.
pub fn valid_id(id: &str) -> bool {
    let Some((ns, path)) = id.split_once(':') else {
        return false;
    };
    let ok = |s: &str, slash: bool| !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b) || slash && b == b'/');
    ok(ns, false) && ok(path, true)
}

/// The `mekanism:chemical` registry: every chemical by id, numbered from 1 in
/// id order for the menus' data slots (0 is none), and its tags.
#[derive(Clone, Debug, Default)]
pub struct ChemicalRegistry {
    chemicals: Vec<Chemical>,
    index: HashMap<String, usize>,
    tags: HashMap<String, Vec<usize>>,
}

impl ChemicalRegistry {
    /// The registry of `chemicals`; a second declaration of an id is left out.
    pub fn new(mut chemicals: Vec<Chemical>) -> Self {
        chemicals.sort_by(|a, b| a.id.cmp(&b.id));
        chemicals.dedup_by(|a, b| a.id == b.id);
        let index = chemicals.iter().enumerate().map(|(i, c)| (c.id.clone(), i)).collect();
        let mut tags: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, c) in chemicals.iter().enumerate() {
            for t in &c.tags {
                tags.entry(t.clone()).or_default().push(i);
            }
        }
        Self { chemicals, index, tags }
    }

    pub fn len(&self) -> usize {
        self.chemicals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chemicals.is_empty()
    }

    pub fn all(&self) -> &[Chemical] {
        &self.chemicals
    }

    pub fn get(&self, id: &str) -> Option<&Chemical> {
        self.index.get(id).map(|&i| &self.chemicals[i])
    }

    /// Its number on a menu's data slots: 1 and up in id order, 0 for none.
    pub fn number(&self, id: Option<&str>) -> i32 {
        id.and_then(|id| self.index.get(id)).map_or(0, |&i| i as i32 + 1)
    }

    pub fn by_number(&self, n: i32) -> Option<&Chemical> {
        usize::try_from(n - 1).ok().and_then(|i| self.chemicals.get(i))
    }

    /// The chemicals of `tag` (`mekanism:carbon`), in id order.
    pub fn tagged(&self, tag: &str) -> impl Iterator<Item = &Chemical> {
        self.tags.get(tag.trim_start_matches('#')).into_iter().flatten().map(|&i| &self.chemicals[i])
    }

    /// `Holder.is(tag)`.
    pub fn is(&self, id: &str, tag: &str) -> bool {
        self.get(id).is_some_and(|c| c.tags.iter().any(|t| t == tag.trim_start_matches('#')))
    }

    /// `ChemicalAttributeValidator.DEFAULT.process`: a chemical whose
    /// attributes need validating (radioactive ones, radiation on) stays out
    /// of a tank that keeps the default validator.
    pub fn passes_default(&self, stack: &ChemicalStack) -> bool {
        stack.id().is_none_or(|id| self.get(id).is_none_or(|c| !c.validated))
    }
}

/// `ChemicalStack`: a chemical and an amount in mB (a `long`). Empty when it
/// holds no chemical, `mekanism:empty`, or 0 or less.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChemicalStack {
    chemical: String,
    amount: i64,
}

impl ChemicalStack {
    pub const EMPTY: Self = Self { chemical: String::new(), amount: 0 };

    pub fn new(chemical: &str, amount: i64) -> Self {
        Self { chemical: chemical.to_owned(), amount }
    }

    pub fn is_empty(&self) -> bool {
        self.chemical.is_empty() || self.chemical == EMPTY || self.amount <= 0
    }

    /// `getAmount`: 0 when empty.
    pub fn amount(&self) -> i64 {
        if self.is_empty() { 0 } else { self.amount }
    }

    /// The chemical, none when empty (`getChemicalHolder` is the empty one).
    pub fn id(&self) -> Option<&str> {
        (!self.is_empty()).then_some(self.chemical.as_str())
    }

    /// `copyWithAmount`: empty for an empty stack or 0.
    pub fn copy_with_amount(&self, amount: i64) -> Self {
        if self.is_empty() || amount == 0 { Self::EMPTY } else { Self { chemical: self.chemical.clone(), amount } }
    }

    pub fn set_amount(&mut self, amount: i64) {
        self.amount = amount;
    }

    pub fn grow(&mut self, by: i64) {
        self.amount = self.amount.saturating_add(by);
    }

    pub fn shrink(&mut self, by: i64) {
        self.amount = self.amount.saturating_sub(by);
    }

    /// `split`: up to `amount` taken out into a new stack.
    pub fn split(&mut self, amount: i64) -> Self {
        let taken = amount.min(self.amount());
        let out = self.copy_with_amount(taken);
        self.shrink(taken);
        out
    }

    /// `isSameChemical`: the same chemical, two empty stacks alike.
    pub fn is_same_chemical(a: &Self, b: &Self) -> bool {
        a.id() == b.id()
    }

    /// The stack as a tank keeps it: empty ones normalised.
    pub fn normalized(self) -> Self {
        if self.is_empty() { Self::EMPTY } else { self }
    }

    /// `ChemicalStack.CODEC`'s saved form `{id, amount}` as it reads back
    /// (`parseOptional`, `LENIENT_OPTIONAL_CODEC`): an unregistered chemical,
    /// `mekanism:empty` or an amount below 1 make an empty stack (Mekanism logs
    /// "Tried to load invalid chemical").
    pub fn from_saved(id: Option<&str>, amount: Option<i64>, registry: &ChemicalRegistry) -> Self {
        match (id, amount) {
            (Some(id), Some(amount)) if amount > 0 && id != EMPTY && registry.get(&full_id(id)).is_some() => Self::new(&full_id(id), amount),
            _ => Self::EMPTY,
        }
    }

    /// `ChemicalStack.CODEC` on a recipe's JSON: `{"id", "amount"}`, a
    /// registered chemical and a positive amount, or an error.
    pub fn from_json(v: &Value, registry: &ChemicalRegistry) -> Result<Self, String> {
        let id = v.get("id").and_then(Value::as_str).ok_or("a chemical stack has an id")?;
        let id = full_id(id);
        if id == EMPTY {
            return Err("Chemical must not be mekanism:empty".into());
        }
        if registry.get(&id).is_none() {
            return Err(format!("Unknown registry key in mekanism:chemical: {id}"));
        }
        let amount = v.get("amount").and_then(Value::as_i64).ok_or("a chemical stack has an amount")?;
        if amount < 1 {
            return Err(format!("Value must be positive: {amount}"));
        }
        Ok(Self::new(&id, amount))
    }
}

/// `ResourceLocation.parse`: a bare path is `minecraft:`'s.
pub fn full_id(id: &str) -> String {
    if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") }
}

// ---- Tanks ----------------------------------------------------------------------

/// A tank's checks (`BasicChemicalTank`'s `canExtract`, `canInsert` by
/// automation type, and `isValid`: its validator and attribute validator).
pub struct TankRules<'a> {
    pub can_extract: &'a dyn Fn(&ChemicalStack, Automation) -> bool,
    pub can_insert: &'a dyn Fn(&ChemicalStack, Automation) -> bool,
    pub valid: &'a dyn Fn(&ChemicalStack) -> bool,
}

/// `ConstantPredicates.alwaysTrueBi`.
pub fn always(_: &ChemicalStack, _: Automation) -> bool {
    true
}

/// `ConstantPredicates.internalOnly`.
pub fn internal_only(_: &ChemicalStack, a: Automation) -> bool {
    a == Automation::Internal
}

/// `ConstantPredicates.notExternal`.
pub fn not_external(_: &ChemicalStack, a: Automation) -> bool {
    a != Automation::External
}

/// `ConstantPredicates.alwaysTrue` as a validator.
pub fn any(_: &ChemicalStack) -> bool {
    true
}

/// `AutomationType.handler(side)`: another block on a side, the block itself
/// on the null side.
pub fn handler_automation(side: bool) -> Automation {
    if side { Automation::External } else { Automation::Internal }
}

/// `BasicChemicalTank`: what it holds and its capacity; its rules are passed
/// in, as they read the machine around it. Its insert and extract rates are
/// unlimited (`Long.MAX_VALUE`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChemicalTank {
    pub stored: ChemicalStack,
    pub capacity: i64,
}

impl ChemicalTank {
    pub fn new(capacity: i64) -> Self {
        Self { stored: ChemicalStack::EMPTY, capacity }
    }

    pub fn with(capacity: i64, stored: ChemicalStack) -> Self {
        Self { stored: stored.normalized(), capacity }
    }

    pub fn is_empty(&self) -> bool {
        self.stored.is_empty()
    }

    pub fn stored(&self) -> i64 {
        self.stored.amount()
    }

    pub fn needed(&self) -> i64 {
        (self.capacity - self.stored()).max(0)
    }

    /// `BasicChemicalTank.insert`: into an empty tank or one of the same
    /// chemical, what fits, when the stack is valid and may come in this way;
    /// what is left.
    pub fn insert(&mut self, stack: &ChemicalStack, simulate: bool, automation: Automation, rules: &TankRules<'_>) -> ChemicalStack {
        self.insert_rated(stack, simulate, automation, rules, i64::MAX)
    }

    /// `insert` of a tank whose `getInsertRate` is `rate` for this automation
    /// type (a chemical tank's own: its tier's output, `INTERNAL`).
    pub fn insert_rated(&mut self, stack: &ChemicalStack, simulate: bool, automation: Automation, rules: &TankRules<'_>, rate: i64) -> ChemicalStack {
        if stack.is_empty() {
            return stack.clone();
        }
        let same = !self.is_empty() && ChemicalStack::is_same_chemical(&self.stored, stack);
        if !self.is_empty() && !same {
            return stack.clone();
        }
        let needed = self.needed().min(rate);
        if needed <= 0 {
            return stack.clone();
        }
        if !((rules.valid)(stack) && (rules.can_insert)(stack, automation)) {
            return stack.clone();
        }
        let to_add = stack.amount().min(needed);
        if !simulate {
            if same {
                self.stored.grow(to_add);
            } else {
                self.stored = stack.copy_with_amount(to_add);
            }
        }
        stack.copy_with_amount(stack.amount() - to_add)
    }

    /// `BasicChemicalTank.extract`: up to `amount`, when it may go out this way.
    pub fn extract(&mut self, amount: i64, simulate: bool, automation: Automation, rules: &TankRules<'_>) -> ChemicalStack {
        self.extract_rated(amount, simulate, automation, rules, i64::MAX)
    }

    /// `extract` of a tank whose `getExtractRate` is `rate` for this automation type.
    pub fn extract_rated(&mut self, amount: i64, simulate: bool, automation: Automation, rules: &TankRules<'_>, rate: i64) -> ChemicalStack {
        if self.is_empty() || amount < 1 || !(rules.can_extract)(&self.stored, automation) {
            return ChemicalStack::EMPTY;
        }
        let size = self.stored().min(amount).min(rate);
        if size == 0 {
            return ChemicalStack::EMPTY;
        }
        let out = self.stored.copy_with_amount(size);
        if !simulate {
            self.stored.shrink(size);
            self.stored = std::mem::take(&mut self.stored).normalized();
        }
        out
    }

    /// `setStackSize`: an empty tank stays empty; 0 or less empties it; the
    /// rest is capped at the capacity. The size it has (or would have).
    pub fn set_stack_size(&mut self, amount: i64, simulate: bool) -> i64 {
        if self.is_empty() {
            return 0;
        }
        if amount <= 0 {
            if !simulate {
                self.stored = ChemicalStack::EMPTY;
            }
            return 0;
        }
        let amount = amount.min(self.capacity);
        if self.stored() != amount && !simulate {
            self.stored.set_amount(amount);
        }
        amount
    }

    /// `growStack`: by up to what fits (negative: shrinks); how much it changed.
    pub fn grow_stack(&mut self, amount: i64, simulate: bool) -> i64 {
        let current = self.stored();
        if current == 0 {
            return 0;
        }
        let amount = if amount > 0 { amount.min(self.needed()) } else { amount };
        self.set_stack_size(current.saturating_add(amount), simulate) - current
    }

    /// `shrinkStack`: how much it went down.
    pub fn shrink_stack(&mut self, amount: i64, simulate: bool) -> i64 {
        -self.grow_stack(-amount, simulate)
    }
}

// ---- The handler -------------------------------------------------------------------

/// `IChemicalHandler` as one side of a block shows it (`ProxyChemicalHandler`
/// over `ISidedChemicalHandler`): its tanks there, and inserting or extracting
/// one. The whole-handler forms are `ChemicalUtils.insert` and `extract`
/// over the tanks (a stack goes into the tanks holding its chemical first,
/// then the empty ones; an extraction drains every tank of the first
/// chemical it finds).
pub trait ChemicalHandler {
    fn tanks(&self) -> usize;
    fn in_tank(&self, tank: usize) -> ChemicalStack;
    fn capacity(&self, tank: usize) -> i64;
    fn is_valid(&self, tank: usize, stack: &ChemicalStack) -> bool;
    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack;
    fn extract_tank(&mut self, tank: usize, amount: i64, simulate: bool) -> ChemicalStack;

    /// `insertChemical(stack, action)`: what is left.
    fn insert(&mut self, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if stack.is_empty() {
            return ChemicalStack::EMPTY;
        }
        let tanks = self.tanks();
        if tanks == 0 {
            return stack.clone();
        }
        if tanks == 1 {
            return self.insert_tank(0, stack, simulate);
        }
        let mut rest = stack.clone();
        let mut empty = Vec::new();
        for tank in 0..tanks {
            let held = self.in_tank(tank);
            if held.is_empty() {
                empty.push(tank);
            } else if ChemicalStack::is_same_chemical(&held, stack) {
                rest = self.insert_tank(tank, &rest, simulate);
                if rest.is_empty() {
                    return ChemicalStack::EMPTY;
                }
            }
        }
        for tank in empty {
            rest = self.insert_tank(tank, &rest, simulate);
            if rest.is_empty() {
                return ChemicalStack::EMPTY;
            }
        }
        rest
    }

    /// `extractChemical(amount, action)`.
    fn extract(&mut self, amount: i64, simulate: bool) -> ChemicalStack {
        if amount == 0 {
            return ChemicalStack::EMPTY;
        }
        let tanks = self.tanks();
        if tanks == 0 {
            return ChemicalStack::EMPTY;
        }
        if tanks == 1 {
            return self.extract_tank(0, amount, simulate);
        }
        let mut out = ChemicalStack::EMPTY;
        let mut to_drain = amount;
        for tank in 0..tanks {
            if out.is_empty() || ChemicalStack::is_same_chemical(&out, &self.in_tank(tank)) {
                let drained = self.extract_tank(tank, to_drain, simulate);
                if !drained.is_empty() {
                    if out.is_empty() {
                        out = drained.clone();
                    } else {
                        out.grow(drained.amount());
                    }
                    to_drain -= drained.amount();
                    if to_drain == 0 {
                        break;
                    }
                }
            }
        }
        out
    }

    /// `extractChemical(stack, action)`: only that chemical.
    fn extract_stack(&mut self, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if stack.is_empty() {
            return ChemicalStack::EMPTY;
        }
        let tanks = self.tanks();
        if tanks == 0 {
            return ChemicalStack::EMPTY;
        }
        if tanks == 1 {
            let held = self.in_tank(0);
            return if !held.is_empty() && ChemicalStack::is_same_chemical(&held, stack) { self.extract_tank(0, stack.amount(), simulate) } else { ChemicalStack::EMPTY };
        }
        let mut out = ChemicalStack::EMPTY;
        let mut to_drain = stack.amount();
        for tank in 0..tanks {
            if ChemicalStack::is_same_chemical(stack, &self.in_tank(tank)) {
                let drained = self.extract_tank(tank, to_drain, simulate);
                if !drained.is_empty() {
                    if out.is_empty() {
                        out = drained.clone();
                    } else {
                        out.grow(drained.amount());
                    }
                    to_drain -= drained.amount();
                    if to_drain == 0 {
                        break;
                    }
                }
            }
        }
        out
    }
}

/// A Mekanism block's tanks as one side sees them: the tanks its chemical side
/// configuration puts there (`ConfigChemicalTankHolder.getTanks`), whether
/// that side takes in and gives out (`ConfigHolder.canInsert`,
/// `canExtract`), and the null side (`side` false), which reads them all and
/// changes nothing (`ProxyHandler.readOnly`). Its tank operations are
/// `IMekanismChemicalHandler`'s, with `AutomationType.handler(side)`.
pub struct SidedTanks<'t, 'r> {
    pub tanks: Vec<(&'t mut ChemicalTank, &'r TankRules<'r>)>,
    pub side: bool,
    pub can_insert: bool,
    pub can_extract: bool,
}

impl ChemicalHandler for SidedTanks<'_, '_> {
    fn tanks(&self) -> usize {
        self.tanks.len()
    }

    fn in_tank(&self, tank: usize) -> ChemicalStack {
        self.tanks.get(tank).map_or(ChemicalStack::EMPTY, |(t, _)| t.stored.clone())
    }

    fn capacity(&self, tank: usize) -> i64 {
        self.tanks.get(tank).map_or(0, |(t, _)| t.capacity)
    }

    fn is_valid(&self, tank: usize, stack: &ChemicalStack) -> bool {
        self.tanks.get(tank).is_some_and(|(_, r)| (r.valid)(stack))
    }

    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if !self.side || !self.can_insert {
            return stack.clone();
        }
        let automation = handler_automation(self.side);
        match self.tanks.get_mut(tank) {
            Some((t, r)) => t.insert(stack, simulate, automation, r),
            None => stack.clone(),
        }
    }

    fn extract_tank(&mut self, tank: usize, amount: i64, simulate: bool) -> ChemicalStack {
        if !self.side || !self.can_extract {
            return ChemicalStack::EMPTY;
        }
        let automation = handler_automation(self.side);
        match self.tanks.get_mut(tank) {
            Some((t, r)) => t.extract(amount, simulate, automation, r),
            None => ChemicalStack::EMPTY,
        }
    }
}

// ---- Ingredients -----------------------------------------------------------------

/// `ChemicalIngredient`: its six types (`mekanism:single`, `tag`, `compound`,
/// `difference`, `intersection`, `empty`).
#[derive(Clone, Debug, PartialEq)]
pub enum ChemicalIngredient {
    Single(String),
    Tag(String),
    Compound(Vec<ChemicalIngredient>),
    Difference(Box<ChemicalIngredient>, Box<ChemicalIngredient>),
    Intersection(Vec<ChemicalIngredient>),
    Empty,
}

impl ChemicalIngredient {
    /// `mapCodecNonEmpty`: a `type`'s own codec, else `{"chemical"}` or
    /// `{"tag"}` but not both. A chemical the registry lacks fails, as
    /// `holderByNameCodec` does; a tag may hold none.
    pub fn from_map(v: &Value, registry: &ChemicalRegistry) -> Result<Self, String> {
        let o = v.as_object().ok_or("a chemical ingredient is an object")?;
        if let Some(kind) = o.get("type").and_then(Value::as_str) {
            return match full_id(kind).as_str() {
                "mekanism:single" => Self::single(o.get("chemical"), registry),
                "mekanism:tag" => Self::tag(o.get("tag")),
                "mekanism:compound" => {
                    let list = o.get("children").or_else(|| o.get("ingredients")).ok_or("a compound has children")?;
                    Ok(Self::Compound(Self::list(list, registry)?))
                }
                "mekanism:difference" => Ok(Self::Difference(
                    Box::new(Self::from_codec(o.get("base").ok_or("a difference has a base")?, registry)?),
                    Box::new(Self::from_codec(o.get("subtracted").ok_or("a difference has subtracted")?, registry)?),
                )),
                "mekanism:intersection" => {
                    let list = Self::list(o.get("children").ok_or("an intersection has children")?, registry)?;
                    if list.len() < 2 {
                        return Err("an intersection has two children or more".into());
                    }
                    Ok(Self::Intersection(list))
                }
                "mekanism:empty" => Err("a chemical ingredient is not empty here".into()),
                other => Err(format!("Unknown registry key in mekanism:chemical_ingredient_type: {other}")),
            };
        }
        match (o.get("chemical"), o.get("tag")) {
            (Some(c), None) => Self::single(Some(c), registry),
            (None, Some(t)) => Self::tag(Some(t)),
            (Some(_), Some(_)) => Err("A chemical ingredient has a chemical or a tag, not both".into()),
            (None, None) => Err("A chemical ingredient has a chemical or a tag".into()),
        }
    }

    /// `codecNonEmpty`: a list (a compound) or a map.
    fn from_codec(v: &Value, registry: &ChemicalRegistry) -> Result<Self, String> {
        match v {
            Value::Array(_) => Ok(Self::Compound(Self::list(v, registry)?)),
            _ => Self::from_map(v, registry),
        }
    }

    fn list(v: &Value, registry: &ChemicalRegistry) -> Result<Vec<Self>, String> {
        let list: Vec<Self> = v.as_array().ok_or("a list of chemical ingredients")?.iter().map(|c| Self::from_map(c, registry)).collect::<Result<_, _>>()?;
        if list.is_empty() {
            return Err("an empty list of chemical ingredients".into());
        }
        Ok(list)
    }

    fn single(v: Option<&Value>, registry: &ChemicalRegistry) -> Result<Self, String> {
        let id = full_id(v.and_then(Value::as_str).ok_or("a chemical is an id")?);
        if id == EMPTY {
            return Err("Chemical must not be mekanism:empty".into());
        }
        if registry.get(&id).is_none() {
            return Err(format!("Unknown registry key in mekanism:chemical: {id}"));
        }
        Ok(Self::Single(id))
    }

    fn tag(v: Option<&Value>) -> Result<Self, String> {
        let tag = v.and_then(Value::as_str).ok_or("a tag is an id")?.trim_start_matches('#');
        if !valid_id(&full_id(tag)) {
            return Err(format!("not a tag: {tag}"));
        }
        Ok(Self::Tag(full_id(tag)))
    }

    /// `test`: whether the chemical `id` (none: an empty stack) is one of it.
    pub fn test(&self, id: Option<&str>, registry: &ChemicalRegistry) -> bool {
        match self {
            Self::Empty => id.is_none(),
            _ if id.is_none() => false,
            Self::Single(c) => id == Some(c.as_str()),
            Self::Tag(t) => id.is_some_and(|id| registry.is(id, t)),
            Self::Compound(list) => list.iter().any(|c| c.test(id, registry)),
            Self::Difference(base, sub) => base.test(id, registry) && !sub.test(id, registry),
            Self::Intersection(list) => list.iter().all(|c| c.test(id, registry)),
        }
    }

    /// `getChemicals`: the registry's chemicals it takes, in id order.
    pub fn chemicals<'r>(&self, registry: &'r ChemicalRegistry) -> Vec<&'r Chemical> {
        registry.all().iter().filter(|c| self.test(Some(&c.id), registry)).collect()
    }
}

/// `ChemicalStackIngredient`: an ingredient and an amount (`{..., "amount"}`).
#[derive(Clone, Debug, PartialEq)]
pub struct ChemicalStackIngredient {
    pub ingredient: ChemicalIngredient,
    pub amount: i64,
}

impl ChemicalStackIngredient {
    pub fn from_json(v: &Value, registry: &ChemicalRegistry) -> Result<Self, String> {
        let amount = v.get("amount").and_then(Value::as_i64).ok_or("a chemical ingredient has an amount")?;
        if amount < 1 {
            return Err(format!("Value must be positive: {amount}"));
        }
        Ok(Self { ingredient: ChemicalIngredient::from_map(v, registry)?, amount })
    }

    /// `testType`: the chemical alone.
    pub fn test_type(&self, id: Option<&str>, registry: &ChemicalRegistry) -> bool {
        self.ingredient.test(id, registry)
    }

    /// `test`: the chemical and at least the amount.
    pub fn test(&self, stack: &ChemicalStack, registry: &ChemicalRegistry) -> bool {
        self.test_type(stack.id(), registry) && stack.amount() >= self.amount
    }
}

/// `ChemicalUtil.emit` through `EmitUtils.sendToAcceptors`: up to `max` of
/// `tank` (taken as the block itself, `INTERNAL`) split evenly among
/// `targets` handlers that take it; what went out.
pub fn emit(tank: &mut ChemicalTank, rules: &TankRules<'_>, max: i64, targets: &mut [&mut dyn ChemicalHandler]) -> i64 {
    if tank.is_empty() || max <= 0 || targets.is_empty() {
        return 0;
    }
    let offered = tank.extract(max, true, Automation::Internal, rules);
    if offered.is_empty() {
        return 0;
    }
    // `ChemicalHandlerTarget`: the handlers that take any of it.
    let mut takers: Vec<usize> = (0..targets.len()).filter(|&i| targets[i].insert(&offered, true).amount() < offered.amount()).collect();
    if takers.is_empty() {
        return 0;
    }
    let sent = crate::split::send_to_acceptors(takers.len(), offered.amount() as u64, |i, amount, simulate| {
        let stack = offered.copy_with_amount(amount.min(i64::MAX as u64) as i64);
        targets[takers[i]].insert(&stack, simulate).amount().max(0) as u64
    });
    takers.clear();
    let sent = sent.min(i64::MAX as u64) as i64;
    if sent > 0 {
        tank.extract(sent, false, Automation::Internal, rules);
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn registry() -> ChemicalRegistry {
        let decl = |id: &str, v: Value| Chemical::from_declaration(id, &v).unwrap();
        ChemicalRegistry::new(vec![
            decl("mekanism:oxygen", json!({"texture": "mekanism:liquid/liquid", "tint": "0x6CE2FF", "tags": ["mekanism:gaseous"]})),
            decl("mekanism:hydrogen", json!({"texture": "mekanism:liquid/liquid", "tint": "0xFFFFFF", "gaseous": true,
                "attributes": {"fuel": {"burn_time": 1, "energy": 200}}, "tags": ["mekanism:gaseous"]})),
            decl("mekanism:carbon", json!({"texture": "mekanism:infuse_type/base", "tint": 2894892, "tags": ["mekanism:carbon"]})),
            decl("mekanism:plutonium", json!({"texture": "mekanism:liquid/liquid", "attributes": {"radioactivity": 0.02}, "validated": true})),
            decl("mekanism:sodium", json!({"texture": "mekanism:liquid/liquid",
                "attributes": {"cooled_coolant": {"heated": "mekanism:superheated_sodium", "thermal_enthalpy": 5.0, "conductivity": 1.0}}})),
        ])
    }

    #[test]
    fn a_declaration_reads_its_facts() {
        let r = registry();
        assert_eq!(r.len(), 5);
        let h = r.get("mekanism:hydrogen").unwrap();
        assert_eq!((h.tint, h.kind, h.gaseous), (0xFFFFFF, ChemicalKind::Gas, true));
        assert_eq!(h.color, h.tint, "the bar colour is the tint unless declared");
        assert_eq!(h.fuel.unwrap().energy_density(), 200);
        assert_eq!(r.get("mekanism:carbon").unwrap().kind, ChemicalKind::InfuseType);
        assert_eq!(r.get("mekanism:carbon").unwrap().tint, 0x2C2C2C);
        assert!(r.get("mekanism:oxygen").unwrap().gaseous, "the gaseous tag makes it gaseous");
        assert!(r.get("mekanism:plutonium").unwrap().is_radioactive());
        assert_eq!(r.get("mekanism:sodium").unwrap().cooled.as_ref().unwrap().other, "mekanism:superheated_sodium");
        assert_eq!(r.get("mekanism:hydrogen").unwrap().translation_key(), "chemical.mekanism.hydrogen");
        // Numbered in id order from 1.
        assert_eq!(r.number(Some("mekanism:carbon")), 1);
        assert_eq!(r.by_number(2).unwrap().id, "mekanism:hydrogen");
        assert_eq!(r.number(None), 0);
        assert_eq!(r.tagged("#mekanism:gaseous").map(|c| c.id.as_str()).collect::<Vec<_>>(), ["mekanism:hydrogen", "mekanism:oxygen"]);
        assert!(Chemical::from_declaration("mekanism:empty", &json!({"texture": "x:y"})).is_err());
        assert!(Chemical::from_declaration("mekanism:x", &json!({})).is_err(), "a chemical has an icon");
    }

    #[test]
    fn stacks_read_and_compare_as_mekanisms() {
        let r = registry();
        assert!(ChemicalStack::EMPTY.is_empty());
        assert!(ChemicalStack::new("mekanism:hydrogen", 0).is_empty());
        assert!(ChemicalStack::new(EMPTY, 5).is_empty());
        assert_eq!(ChemicalStack::new("mekanism:hydrogen", 7).copy_with_amount(0), ChemicalStack::EMPTY);
        let mut s = ChemicalStack::new("mekanism:hydrogen", 7);
        assert_eq!(s.split(5), ChemicalStack::new("mekanism:hydrogen", 5));
        assert_eq!(s.amount(), 2);
        assert!(ChemicalStack::is_same_chemical(&ChemicalStack::EMPTY, &ChemicalStack::new("mekanism:oxygen", 0)));
        // The saved form: an unknown chemical or a bad amount read as empty.
        assert_eq!(ChemicalStack::from_saved(Some("mekanism:oxygen"), Some(3), &r), ChemicalStack::new("mekanism:oxygen", 3));
        assert!(ChemicalStack::from_saved(Some("mekanism:steam"), Some(3), &r).is_empty());
        assert!(ChemicalStack::from_saved(Some("mekanism:oxygen"), Some(0), &r).is_empty());
        assert!(ChemicalStack::from_saved(Some("mekanism:empty"), Some(3), &r).is_empty());
        // The recipe codec fails where the saved form reads empty.
        assert_eq!(ChemicalStack::from_json(&json!({"id": "mekanism:hydrogen", "amount": 2}), &r).unwrap().amount(), 2);
        assert!(ChemicalStack::from_json(&json!({"id": "mekanism:hydrogen", "amount": 0}), &r).is_err());
        assert!(ChemicalStack::from_json(&json!({"id": "mekanism:steam", "amount": 1}), &r).is_err());
    }

    #[test]
    fn a_tank_takes_one_chemical_by_its_rules() {
        let r = registry();
        let valid = |s: &ChemicalStack| r.passes_default(s);
        let rules = TankRules { can_extract: &always, can_insert: &always, valid: &valid };
        let mut tank = ChemicalTank::new(1000);
        let h = ChemicalStack::new("mekanism:hydrogen", 600);
        assert_eq!(tank.insert(&h, true, Automation::External, &rules), ChemicalStack::EMPTY);
        assert!(tank.is_empty(), "simulated");
        assert_eq!(tank.insert(&h, false, Automation::External, &rules), ChemicalStack::EMPTY);
        assert_eq!(tank.insert(&h, false, Automation::External, &rules), ChemicalStack::new("mekanism:hydrogen", 200));
        assert_eq!(tank.stored(), 1000);
        assert_eq!(tank.insert(&ChemicalStack::new("mekanism:oxygen", 1), false, Automation::External, &rules).amount(), 1, "another chemical");
        assert_eq!(tank.extract(300, false, Automation::External, &rules), ChemicalStack::new("mekanism:hydrogen", 300));
        assert_eq!(tank.extract(5000, false, Automation::External, &rules).amount(), 700);
        assert!(tank.is_empty() && tank.stored == ChemicalStack::EMPTY);
        // The default attribute validator keeps a radioactive chemical out.
        assert_eq!(tank.insert(&ChemicalStack::new("mekanism:plutonium", 5), false, Automation::Internal, &rules).amount(), 5);
        // An output tank: anything takes out, only the block puts in.
        let out = TankRules { can_extract: &always, can_insert: &internal_only, valid: &any };
        assert_eq!(tank.insert(&h, false, Automation::External, &out).amount(), 600);
        assert_eq!(tank.insert(&h, false, Automation::Internal, &out), ChemicalStack::EMPTY);
        // grow, shrink and setStackSize as BasicChemicalTank's.
        assert_eq!(tank.grow_stack(1000, false), 400);
        assert_eq!(tank.shrink_stack(8, false), 8);
        assert_eq!(tank.stored(), 992);
        assert_eq!(tank.shrink_stack(2000, false), 992);
        assert!(tank.is_empty());
        assert_eq!(tank.grow_stack(5, false), 0, "an empty tank doesn't grow");
    }

    #[test]
    fn the_handler_fills_matching_tanks_first_and_drains_one_chemical() {
        let rules = TankRules { can_extract: &always, can_insert: &always, valid: &any };
        let (mut a, mut b, mut c) = (ChemicalTank::new(100), ChemicalTank::with(100, ChemicalStack::new("mekanism:oxygen", 50)), ChemicalTank::with(100, ChemicalStack::new("mekanism:hydrogen", 90)));
        let mut side = SidedTanks { tanks: vec![(&mut a, &rules), (&mut b, &rules), (&mut c, &rules)], side: true, can_insert: true, can_extract: true };
        // Hydrogen tops up the third tank, then fills the empty first.
        assert_eq!(side.insert(&ChemicalStack::new("mekanism:hydrogen", 30), false), ChemicalStack::EMPTY);
        assert_eq!((side.in_tank(0).amount(), side.in_tank(2).amount()), (20, 100));
        // Extracting by amount drains the first chemical found across tanks.
        assert_eq!(side.extract(110, false), ChemicalStack::new("mekanism:hydrogen", 110));
        assert_eq!(side.extract_stack(&ChemicalStack::new("mekanism:oxygen", 10), true), ChemicalStack::new("mekanism:oxygen", 10));
        // The null side reads but changes nothing; a side the config closes neither.
        side.side = false;
        assert_eq!(side.insert(&ChemicalStack::new("mekanism:oxygen", 5), false).amount(), 5);
        assert!(side.extract(5, false).is_empty());
        side.side = true;
        side.can_extract = false;
        assert!(side.extract(5, false).is_empty());
    }

    #[test]
    fn ingredients_read_and_test_by_type_and_tag() {
        let r = registry();
        let carbon = ChemicalStackIngredient::from_json(&json!({"tag": "mekanism:carbon", "amount": 10}), &r).unwrap();
        assert!(carbon.test(&ChemicalStack::new("mekanism:carbon", 10), &r));
        assert!(!carbon.test(&ChemicalStack::new("mekanism:carbon", 9), &r));
        assert!(carbon.test_type(Some("mekanism:carbon"), &r));
        let single = ChemicalStackIngredient::from_json(&json!({"chemical": "mekanism:oxygen", "amount": 1}), &r).unwrap();
        assert!(single.test_type(Some("mekanism:oxygen"), &r) && !single.test_type(Some("mekanism:hydrogen"), &r));
        assert!(ChemicalStackIngredient::from_json(&json!({"chemical": "mekanism:steam", "amount": 1}), &r).is_err());
        assert!(ChemicalStackIngredient::from_json(&json!({"chemical": "mekanism:oxygen", "tag": "mekanism:carbon", "amount": 1}), &r).is_err());
        let diff = ChemicalIngredient::from_map(&json!({"type": "mekanism:difference", "base": {"tag": "mekanism:gaseous"}, "subtracted": {"chemical": "mekanism:oxygen"}}), &r).unwrap();
        assert_eq!(diff.chemicals(&r).iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["mekanism:hydrogen"]);
        assert!(!diff.test(None, &r));
    }

    struct Sink(ChemicalTank);
    impl ChemicalHandler for Sink {
        fn tanks(&self) -> usize { 1 }
        fn in_tank(&self, _: usize) -> ChemicalStack { self.0.stored.clone() }
        fn capacity(&self, _: usize) -> i64 { self.0.capacity }
        fn is_valid(&self, _: usize, _: &ChemicalStack) -> bool { true }
        fn insert_tank(&mut self, _: usize, s: &ChemicalStack, sim: bool) -> ChemicalStack {
            self.0.insert(s, sim, Automation::External, &TankRules { can_extract: &always, can_insert: &always, valid: &any })
        }
        fn extract_tank(&mut self, _: usize, a: i64, sim: bool) -> ChemicalStack {
            self.0.extract(a, sim, Automation::External, &TankRules { can_extract: &always, can_insert: &always, valid: &any })
        }
    }

    #[test]
    fn emit_splits_evenly_up_to_the_rate() {
        let rules = TankRules { can_extract: &always, can_insert: &internal_only, valid: &any };
        let mut tank = ChemicalTank::with(2400, ChemicalStack::new("mekanism:hydrogen", 2000));
        let (mut a, mut b) = (Sink(ChemicalTank::new(100)), Sink(ChemicalTank::new(10_000)));
        let sent = emit(&mut tank, &rules, 1024, &mut [&mut a, &mut b]);
        assert_eq!(sent, 1024);
        assert_eq!((a.0.stored(), b.0.stored(), tank.stored()), (100, 924, 976));
    }
}
