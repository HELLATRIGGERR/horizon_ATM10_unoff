//! NeoForge's `FluidStack` (`net.neoforged.neoforge.fluids.FluidStack`): a
//! fluid of the registry ([`FluidKind`]: vanilla's and the content mods'), an
//! amount in millibuckets and its data components, kept as the NBT of their
//! patch (`DataComponentPatch`), which a stack only carries and compares. What
//! tanks hold and what fluid handlers move (docs/mod-fluids.md, slice 5).
use crate::fluid::FluidKind;
use crate::nbt::{Compound, JavaString, Tag};
use serde_json::Value;

/// `FluidType.BUCKET_VOLUME`: millibuckets in a bucket.
pub const BUCKET_VOLUME: i32 = 1000;

/// A fluid, how much of it, and its components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FluidStack {
    fluid: FluidKind,
    amount: i32,
    components: Compound,
}

impl Default for FluidStack {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl FluidStack {
    /// `FluidStack.EMPTY`.
    pub const EMPTY: Self = Self {
        fluid: FluidKind::Empty,
        amount: 0,
        components: Compound::new(),
    };

    /// `new FluidStack(fluid, amount)`.
    pub fn new(fluid: FluidKind, amount: i32) -> Self {
        Self {
            fluid,
            amount,
            components: Compound::new(),
        }
    }

    /// `new FluidStack(fluid, amount, patch)`: the patch as its NBT.
    pub fn with_components(fluid: FluidKind, amount: i32, components: Compound) -> Self {
        Self {
            fluid,
            amount,
            components,
        }
    }

    /// The registry's fluid named `name` (`minecraft:lava`), `amount` of it;
    /// none for a name the registry lacks.
    pub fn of(name: &str, amount: i32) -> Option<Self> {
        Some(Self::new(FluidKind::from_name(name)?, amount))
    }

    /// `isEmpty`: the empty fluid, or no amount.
    pub fn is_empty(&self) -> bool {
        self.fluid == FluidKind::Empty || self.amount <= 0
    }

    /// `getFluid`: the empty fluid when the stack is empty.
    pub fn fluid(&self) -> FluidKind {
        if self.is_empty() {
            FluidKind::Empty
        } else {
            self.fluid
        }
    }

    /// The fluid's registry name (`minecraft:empty` for an empty stack).
    pub fn name(&self) -> &'static str {
        self.fluid().name()
    }

    /// `getAmount`: 0 when empty.
    pub fn amount(&self) -> i32 {
        if self.is_empty() { 0 } else { self.amount }
    }

    /// `setAmount`.
    pub fn set_amount(&mut self, amount: i32) {
        self.amount = amount;
    }

    /// `grow`.
    pub fn grow(&mut self, added: i32) {
        self.amount = self.amount().saturating_add(added);
    }

    /// `shrink`.
    pub fn shrink(&mut self, removed: i32) {
        self.grow(removed.saturating_neg());
    }

    /// `limitSize`.
    pub fn limit_size(&mut self, amount: i32) {
        if !self.is_empty() && self.amount > amount {
            self.amount = amount;
        }
    }

    /// `split`: takes at most `amount` off this stack and answers it.
    pub fn split(&mut self, amount: i32) -> Self {
        let taken = amount.min(self.amount());
        let out = self.copy_with_amount(taken);
        self.shrink(taken);
        out
    }

    /// `copyWithAmount`: the empty stack stays empty.
    pub fn copy_with_amount(&self, amount: i32) -> Self {
        if self.is_empty() {
            return Self::EMPTY;
        }
        Self {
            amount,
            ..self.clone()
        }
    }

    /// The NBT of the stack's component patch; empty when it has none.
    pub fn components(&self) -> &Compound {
        &self.components
    }

    /// `is(Fluid)`.
    pub fn is(&self, fluid: FluidKind) -> bool {
        self.fluid() == fluid
    }

    /// `is(TagKey<Fluid>)`: a content mod's fluid by its registry entry's
    /// tags; vanilla's fluids by the root's tags they are in
    /// ([`crate::mod_fluids::vanilla_in_tag`]: at least `minecraft:water`
    /// and `minecraft:lava` and NeoForge's `c:water` and `c:lava`). The empty
    /// fluid is in no tag.
    pub fn is_tag(&self, tag: &str) -> bool {
        fluid_in_tag(self.fluid(), tag)
    }

    /// `FluidStack.isSameFluid`.
    pub fn is_same_fluid(a: &Self, b: &Self) -> bool {
        a.is(b.fluid())
    }

    /// `FluidStack.isSameFluidSameComponents`.
    pub fn is_same_fluid_same_components(a: &Self, b: &Self) -> bool {
        if !a.is(b.fluid()) {
            return false;
        }
        (a.is_empty() && b.is_empty()) || a.components == b.components
    }

    /// `FluidStack.matches`: also the same amount.
    pub fn matches(a: &Self, b: &Self) -> bool {
        a.amount() == b.amount() && Self::is_same_fluid_same_components(a, b)
    }

    /// `save`, through `FluidStack.CODEC`: `{id: "minecraft:lava", amount:
    /// 1000}`, with `components` when the patch has any. None for an empty
    /// stack, which the codec cannot encode.
    pub fn save(&self) -> Option<Compound> {
        if self.is_empty() {
            return None;
        }
        let mut out = Compound::new();
        out.insert(
            JavaString::from("id"),
            Tag::String(JavaString::from(self.name())),
        );
        out.insert(JavaString::from("amount"), Tag::Int(self.amount));
        if !self.components.is_empty() {
            out.insert(
                JavaString::from("components"),
                Tag::Compound(self.components.clone()),
            );
        }
        Some(out)
    }

    /// `saveOptional`: an empty compound for an empty stack.
    pub fn save_optional(&self) -> Compound {
        self.save().unwrap_or_default()
    }

    /// `FluidStack.parse`: `id` a registered fluid other than
    /// `minecraft:empty` (a bare path is `minecraft:`'s, as a
    /// `ResourceLocation`), `amount` a positive int, `components` optional.
    /// None otherwise.
    pub fn parse(tag: &Tag) -> Option<Self> {
        let Tag::Compound(map) = tag else {
            return None;
        };
        let Some(Tag::String(id)) = map.get(&JavaString::from("id")) else {
            return None;
        };
        let id = id.to_string_lossy();
        let name = if id.contains(':') {
            id
        } else {
            format!("minecraft:{id}")
        };
        let fluid = FluidKind::from_name(&name).filter(|f| *f != FluidKind::Empty)?;
        let amount = match map.get(&JavaString::from("amount"))? {
            Tag::Byte(n) => i32::from(*n),
            Tag::Short(n) => i32::from(*n),
            Tag::Int(n) => *n,
            Tag::Long(n) => i32::try_from(*n).ok()?,
            _ => return None,
        };
        if amount <= 0 {
            return None;
        }
        let components = match map.get(&JavaString::from("components")) {
            None => Compound::new(),
            Some(Tag::Compound(patch)) => patch.clone(),
            Some(_) => return None,
        };
        Some(Self {
            fluid,
            amount,
            components,
        })
    }

    /// `parseOptional`: the empty stack for an empty compound and for one that
    /// does not parse.
    pub fn parse_optional(tag: &Compound) -> Self {
        if tag.is_empty() {
            return Self::EMPTY;
        }
        Self::parse(&Tag::Compound(tag.clone())).unwrap_or(Self::EMPTY)
    }

    /// `FluidStack.CODEC` reading a recipe's JSON (a result: Mekanism's
    /// `{"id": "mekanism:brine", "amount": 1}`): `id` a registered fluid
    /// other than `minecraft:empty` (`FLUID_NON_EMPTY_CODEC`), `amount` a
    /// positive int, `components` an optional `DataComponentPatch`, kept as
    /// its NBT (`JsonOps.convertTo(NbtOps)`). The codec's error otherwise.
    pub fn from_json(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or_else(|| format!("Not a map: {v}"))?;
        let fluid = fluid_from_json(o.get("id").ok_or("No key id in MapLike")?)?;
        let amount = positive_int(o.get("amount").ok_or("No key amount in MapLike")?)?;
        Ok(Self {
            fluid,
            amount,
            components: component_patch(o.get("components"))?,
        })
    }

    /// `FluidStack.OPTIONAL_CODEC` (`ExtraCodecs.optionalEmptyMap`): `{}` is
    /// the empty stack, anything else as [`FluidStack::from_json`].
    pub fn from_json_optional(v: &Value) -> Result<Self, String> {
        match v.as_object() {
            Some(o) if o.is_empty() => Ok(Self::EMPTY),
            _ => Self::from_json(v),
        }
    }

    /// `FluidStack.fixedAmountCodec(amount)`: `id` and `components` only.
    pub fn from_json_fixed_amount(v: &Value, amount: i32) -> Result<Self, String> {
        let o = v.as_object().ok_or_else(|| format!("Not a map: {v}"))?;
        Ok(Self {
            fluid: fluid_from_json(o.get("id").ok_or("No key id in MapLike")?)?,
            amount,
            components: component_patch(o.get("components"))?,
        })
    }
}

/// `FluidStack.FLUID_NON_EMPTY_CODEC` (`BuiltInRegistries.FLUID.holderByNameCodec()`,
/// not `minecraft:empty`): a fluid of the registry by its id, a bare path
/// being `minecraft:`'s.
pub fn fluid_from_json(v: &Value) -> Result<FluidKind, String> {
    let id = v.as_str().ok_or_else(|| format!("Not a string: {v}"))?;
    let valid = |s: &str, extra: &str| {
        !s.is_empty()
            && s.chars().all(|c| {
                c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || "_-.".contains(c)
                    || extra.contains(c)
            })
    };
    let name = match id.split_once(':') {
        Some((ns, path)) if valid(ns, "") && valid(path, "/") => id.to_owned(),
        None if valid(id, "/") => format!("minecraft:{id}"),
        _ => return Err(format!("Not a valid resource location: {id}")),
    };
    match FluidKind::from_name(&name) {
        Some(FluidKind::Empty) => Err("Fluid must not be minecraft:empty".into()),
        Some(kind) => Ok(kind),
        None => Err(format!(
            "Unknown registry key in ResourceKey[minecraft:root / minecraft:fluid]: {name}"
        )),
    }
}

/// `ExtraCodecs.POSITIVE_INT` on JSON: `Codec.INT` (a number's `intValue`,
/// a boolean as 1 or 0), above zero.
pub fn positive_int(v: &Value) -> Result<i32, String> {
    let n = match v {
        Value::Number(n) => match n.as_i64() {
            Some(i) => i as i32,
            None => n.as_f64().map_or(0, |d| d as i32),
        },
        Value::Bool(b) => i32::from(*b),
        _ => return Err(format!("Not a number: {v}")),
    };
    if n > 0 {
        Ok(n)
    } else {
        Err(format!("Value must be positive: {n}"))
    }
}

/// A recipe's JSON as NBT, `JsonOps.convertTo(NbtOps)`: how a component
/// value from a recipe meets the NBT a stack keeps.
pub fn json_to_nbt(v: &Value) -> Option<Tag> {
    let text: Vec<u16> = v.to_string().encode_utf16().collect();
    crate::datafix::json::to_nbt(&crate::datafix::json::parse_strict(&text)?)
}

/// `DataComponentPatch.CODEC`'s map, optional (absent: no patch), as NBT.
fn component_patch(v: Option<&Value>) -> Result<Compound, String> {
    match v {
        None => Ok(Compound::new()),
        Some(v @ Value::Object(_)) => match json_to_nbt(v) {
            Some(Tag::Compound(patch)) => Ok(patch),
            _ => Err(format!("Not a component patch: {v}")),
        },
        Some(v) => Err(format!("Not a map: {v}")),
    }
}

/// [`FluidStack::is_tag`] for a fluid.
pub fn fluid_in_tag(fluid: FluidKind, tag: &str) -> bool {
    if fluid == FluidKind::Empty {
        return false;
    }
    if let Some(m) = fluid.mod_fluid() {
        return m.in_tag(fluid, tag);
    }
    crate::mod_fluids::vanilla_in_tag(fluid, tag)
}

/// `FluidType.getBucket` (`FluidUtil.getFilledBucket`): the bucket a fluid's
/// family fills, its flowing fluid's too (`WaterFluid.getBucket`,
/// `BaseFlowingFluid.getBucket`); none for a fluid without one.
pub fn filled_bucket(fluid: FluidKind) -> Option<&'static str> {
    if fluid.is_water() {
        return Some("minecraft:water_bucket");
    }
    if fluid.is_lava() {
        return Some("minecraft:lava_bucket");
    }
    fluid.mod_fluid()?.bucket.as_deref()
}

/// `BucketItem.content` of a filled bucket item: water's, lava's, or the
/// content mod's fluid whose bucket it is.
pub fn bucket_fluid(item: &str) -> Option<FluidKind> {
    match item {
        "minecraft:water_bucket" => Some(FluidKind::Water),
        "minecraft:lava_bucket" => Some(FluidKind::Lava),
        _ => crate::mod_fluids::kind_by_bucket(item),
    }
}

/// `FluidType.getSound(SoundActions.BUCKET_FILL)`: water's and lava's, a
/// mod's from its type; none when its type has none.
pub fn fill_sound(fluid: FluidKind) -> Option<&'static str> {
    if fluid.is_water() {
        return Some("minecraft:item.bucket.fill");
    }
    if fluid.is_lava() {
        return Some("minecraft:item.bucket.fill_lava");
    }
    fluid.mod_fluid()?.fill_sound.as_deref()
}

/// `FluidType.getSound(SoundActions.BUCKET_EMPTY)`, as [`fill_sound`].
pub fn empty_sound(fluid: FluidKind) -> Option<&'static str> {
    if fluid.is_water() {
        return Some("minecraft:item.bucket.empty");
    }
    if fluid.is_lava() {
        return Some("minecraft:item.bucket.empty_lava");
    }
    fluid.mod_fluid()?.empty_sound.as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(map: &[(&str, Tag)]) -> Tag {
        Tag::Compound(
            map.iter()
                .map(|(k, v)| (JavaString::from(*k), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn a_stack_saves_and_parses_as_the_codec_does() {
        crate::mod_fluids::register_test_fluids();
        // Mekanism's heat generator keeps `stored: {amount: 1000, id: "minecraft:lava"}`.
        let lava = FluidStack::parse(&tag(&[
            ("amount", Tag::Int(1000)),
            ("id", Tag::String("minecraft:lava".into())),
        ]))
        .unwrap();
        assert_eq!((lava.fluid(), lava.amount()), (FluidKind::Lava, 1000));
        assert_eq!(
            FluidStack::parse(&Tag::Compound(lava.save().unwrap())),
            Some(lava.clone())
        );
        // A bare path is minecraft's; a mod fluid by its id; components kept.
        let mut patch = Compound::new();
        patch.insert("minecraft:custom_data".into(), tag(&[("v", Tag::Int(1))]));
        let brine = FluidStack::parse(&tag(&[
            ("id", Tag::String("test:brine".into())),
            ("amount", Tag::Long(250)),
            ("components", Tag::Compound(patch.clone())),
        ]))
        .unwrap();
        assert_eq!(
            (brine.name(), brine.amount(), brine.components()),
            ("test:brine", 250, &patch)
        );
        let saved = brine.save().unwrap();
        assert_eq!(
            saved.get(&JavaString::from("components")),
            Some(&Tag::Compound(patch))
        );
        assert_eq!(
            FluidStack::parse(&tag(&[
                ("id", Tag::String("water".into())),
                ("amount", Tag::Int(5))
            ]))
            .map(|s| s.fluid()),
            Some(FluidKind::Water)
        );
        // The codec refuses the empty fluid, unknown fluids and amounts under 1.
        for bad in [
            tag(&[
                ("id", Tag::String("minecraft:empty".into())),
                ("amount", Tag::Int(5)),
            ]),
            tag(&[
                ("id", Tag::String("nope:fluid".into())),
                ("amount", Tag::Int(5)),
            ]),
            tag(&[
                ("id", Tag::String("minecraft:water".into())),
                ("amount", Tag::Int(0)),
            ]),
            tag(&[("id", Tag::String("minecraft:water".into()))]),
        ] {
            assert_eq!(FluidStack::parse(&bad), None, "{bad:?}");
        }
        assert_eq!(
            FluidStack::parse_optional(&Compound::new()),
            FluidStack::EMPTY
        );
        assert_eq!(FluidStack::EMPTY.save(), None);
        assert!(FluidStack::EMPTY.save_optional().is_empty());
    }

    #[test]
    fn empty_stacks_and_comparisons_follow_neoforge() {
        crate::mod_fluids::register_test_fluids();
        let water = FluidStack::new(FluidKind::Water, 1000);
        // An emptied stack reads as the empty fluid.
        let mut drained = water.clone();
        drained.shrink(1000);
        assert!(drained.is_empty());
        assert_eq!((drained.fluid(), drained.amount()), (FluidKind::Empty, 0));
        assert_eq!(FluidStack::EMPTY.copy_with_amount(5), FluidStack::EMPTY);
        let mut stack = FluidStack::new(FluidKind::Lava, 700);
        let part = stack.split(500);
        assert_eq!((part.amount(), stack.amount()), (500, 200));
        assert!(FluidStack::is_same_fluid_same_components(&part, &stack));
        assert!(!FluidStack::matches(&part, &stack));
        let mut patch = Compound::new();
        patch.insert("x".into(), Tag::Int(1));
        let patched = FluidStack::with_components(FluidKind::Lava, 500, patch);
        assert!(FluidStack::is_same_fluid(&patched, &part));
        assert!(!FluidStack::is_same_fluid_same_components(&patched, &part));
        assert!(FluidStack::is_same_fluid_same_components(
            &FluidStack::EMPTY,
            &drained
        ));
    }

    #[test]
    fn tags_buckets_and_sounds_come_from_the_registry() {
        crate::mod_fluids::register_test_fluids();
        let brine = FluidKind::from_name("test:brine").unwrap();
        assert!(FluidStack::new(brine, 1).is_tag("c:brine"));
        assert!(!FluidStack::new(brine, 1).is_tag("minecraft:water"));
        assert!(FluidStack::new(FluidKind::FlowingLava, 1).is_tag("minecraft:lava"));
        assert!(!FluidStack::new(FluidKind::Water, 1).is_tag("minecraft:lava"));
        assert!(!FluidStack::EMPTY.is_tag("minecraft:water"));
        assert_eq!(
            filled_bucket(FluidKind::FlowingWater),
            Some("minecraft:water_bucket")
        );
        assert_eq!(filled_bucket(brine), Some("test:brine_bucket"));
        assert_eq!(filled_bucket(FluidKind::Empty), None);
        assert_eq!(bucket_fluid("test:brine_bucket"), Some(brine));
        assert_eq!(bucket_fluid("minecraft:lava_bucket"), Some(FluidKind::Lava));
        assert_eq!(bucket_fluid("minecraft:milk_bucket"), None);
        assert_eq!(
            fill_sound(FluidKind::Lava),
            Some("minecraft:item.bucket.fill_lava")
        );
        assert_eq!(empty_sound(brine), Some("minecraft:item.bucket.empty"));
        let sludge = FluidKind::from_name("test:sludge").unwrap();
        assert_eq!(fill_sound(sludge), None);
    }
}
