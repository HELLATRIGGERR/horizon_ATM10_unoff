//! NeoForge's fluid ingredients (`net.neoforged.neoforge.fluids.crafting`):
//! what a recipe asks of a fluid. A [`FluidIngredient`] is one of NeoForge's
//! seven types (`NeoForgeRegistries.FLUID_INGREDIENT_TYPES`: `neoforge:single`,
//! `tag`, `empty`, `compound`, `components`, `difference`, `intersection`),
//! read from a recipe's JSON by their codecs and tested against a
//! [`FluidStack`]; a [`SizedFluidIngredient`] adds the amount, in its flat
//! (`{"tag": "minecraft:water", "amount": 10}`: Mekanism's, Create's,
//! Industrial Foregoing's) or nested form (`{"ingredient": {...}, "amount":
//! 10}`). Fluids are the registry's ([`FluidKind`]: vanilla's and the content
//! mods'), tags the root's ([`crate::mod_fluids::fluids_in_tag`]). A fluid the
//! registry lacks fails the read, as `holderByNameCodec` does, and the recipe
//! with it (docs/mod-fluids.md, slice 6).
use crate::fluid::FluidKind;
use crate::fluid_stack::{BUCKET_VOLUME, FluidStack, fluid_from_json, json_to_nbt, positive_int};
use crate::nbt::{Compound, Tag};
use serde_json::{Map, Value};

/// A fluid ingredient, as NeoForge's `FluidIngredient` subclasses.
#[derive(Clone, Debug, PartialEq)]
pub enum FluidIngredient {
    /// `EmptyFluidIngredient`: matches the empty stack only.
    Empty,
    /// `SingleFluidIngredient`: one fluid, by identity (`FluidStack.is(Holder)`:
    /// water, not flowing water).
    Single(FluidKind),
    /// `TagFluidIngredient`: a fluid tag, resolved when tested.
    Tag(String),
    /// `CompoundFluidIngredient`: any of its children (two or more).
    Compound(Vec<FluidIngredient>),
    /// `DataComponentFluidIngredient`: a fluid of `fluids` whose components
    /// hold `components` (a `DataComponentPredicate`, as NBT), or with
    /// `strict`, whose patch is exactly those.
    Components {
        fluids: FluidSet,
        components: Compound,
        strict: bool,
    },
    /// `DifferenceFluidIngredient`: the base's fluids but the subtracted's.
    Difference(Box<FluidIngredient>, Box<FluidIngredient>),
    /// `IntersectionFluidIngredient`: what all of its children match.
    Intersection(Vec<FluidIngredient>),
}

/// A `HolderSet<Fluid>` (`HolderSetCodec.create(Registries.FLUID, ..., false)`):
/// a `#tag`, or fluids by id.
#[derive(Clone, Debug, PartialEq)]
pub enum FluidSet {
    Tag(String),
    Direct(Vec<FluidKind>),
}

impl FluidSet {
    /// `HolderSetCodec`: `"#c:water"`, `"minecraft:water"` or a list of ids.
    fn parse(v: &Value) -> Result<Self, String> {
        match v {
            Value::String(s) => match s.strip_prefix('#') {
                Some(tag) => Ok(Self::Tag(tag_id(tag)?)),
                None => Ok(Self::Direct(vec![fluid_from_json(v)?])),
            },
            Value::Array(list) => Ok(Self::Direct(
                list.iter().map(fluid_from_json).collect::<Result<_, _>>()?,
            )),
            _ => Err(format!("Not a fluid set: {v}")),
        }
    }
    /// `HolderSet.stream`.
    fn fluids(&self) -> Vec<FluidKind> {
        match self {
            Self::Tag(tag) => crate::mod_fluids::fluids_in_tag(tag),
            Self::Direct(list) => list.clone(),
        }
    }
    /// `HolderSet.contains`.
    fn contains(&self, fluid: FluidKind) -> bool {
        match self {
            Self::Tag(tag) => crate::fluid_stack::fluid_in_tag(fluid, tag),
            Self::Direct(list) => list.contains(&fluid),
        }
    }
}

impl FluidIngredient {
    /// `FluidIngredient.CODEC`: a list (`CompoundFluidIngredient.of`: none is
    /// the empty ingredient, one is that one) or a map
    /// ([`FluidIngredient::parse_map`]).
    pub fn parse(v: &Value) -> Result<Self, String> {
        Self::codec(v, true)
    }

    /// `FluidIngredient.CODEC_NON_EMPTY`: as [`FluidIngredient::parse`], but
    /// an empty list fails.
    pub fn parse_non_empty(v: &Value) -> Result<Self, String> {
        Self::codec(v, false)
    }

    /// `FluidIngredient.codec(allowEmpty)`: `Codec.either(listCodec, MAP_CODEC_CODEC)`.
    fn codec(v: &Value, allow_empty: bool) -> Result<Self, String> {
        match v {
            Value::Array(_) => Ok(Self::compound(Self::list(v, allow_empty)?)),
            Value::Object(o) => Self::parse_map(o),
            _ => Err(format!("Not a fluid ingredient: {v}")),
        }
    }

    /// `LIST_CODEC` (`MAP_CODEC_CODEC.listOf()`), `LIST_CODEC_NON_EMPTY` without `allow_empty`.
    fn list(v: &Value, allow_empty: bool) -> Result<Vec<Self>, String> {
        let list = v.as_array().ok_or_else(|| format!("Not a list: {v}"))?;
        if list.is_empty() && !allow_empty {
            return Err(
                "Fluid ingredient cannot be empty, at least one item must be defined".into(),
            );
        }
        list.iter()
            .map(|e| match e {
                Value::Object(o) => Self::parse_map(o),
                _ => Err(format!("Not a map: {e}")),
            })
            .collect()
    }

    /// `CompoundFluidIngredient.of(List)`.
    fn compound(mut children: Vec<Self>) -> Self {
        match children.len() {
            0 => Self::Empty,
            1 => children.remove(0),
            _ => Self::Compound(children),
        }
    }

    /// `FluidIngredient.MAP_CODEC_NONEMPTY`
    /// (`NeoForgeExtraCodecs.dispatchMapOrElse`): with a `type`, that type's
    /// codec; without, `{"fluid": id}` or `{"tag": id}`, not both
    /// (`SINGLE_OR_TAG_CODEC`, `NeoForgeExtraCodecs.xor`). The empty
    /// ingredient fails here (`validate`).
    pub fn parse_map(o: &Map<String, Value>) -> Result<Self, String> {
        let ingredient = match o.get("type") {
            Some(t) => Self::dispatch(t, o)?,
            None => match (o.get("fluid"), o.get("tag")) {
                (Some(_), Some(_)) => {
                    return Err("Fluid ingredient has both a fluid and a tag".into());
                }
                (Some(f), None) => Self::Single(fluid_from_json(f)?),
                (None, Some(t)) => Self::Tag(tag_json(t)?),
                (None, None) => return Err("Fluid ingredient has neither a fluid nor a tag".into()),
            },
        };
        if ingredient == Self::Empty {
            return Err("Cannot serialize empty fluid ingredient using the map codec".into());
        }
        Ok(ingredient)
    }

    /// The `FluidIngredientType` named by `type` and its `codec`.
    fn dispatch(t: &Value, o: &Map<String, Value>) -> Result<Self, String> {
        let t = t.as_str().ok_or_else(|| format!("Not a string: {t}"))?;
        let field = |key: &str| o.get(key).ok_or_else(|| format!("No key {key} in MapLike"));
        Ok(match t {
            // SingleFluidIngredient.CODEC: `fluid`.
            "neoforge:single" => Self::Single(fluid_from_json(field("fluid")?)?),
            // TagFluidIngredient.CODEC: `tag`.
            "neoforge:tag" => Self::Tag(tag_json(field("tag")?)?),
            // EmptyFluidIngredient.CODEC: `MapCodec.unit`.
            "neoforge:empty" => Self::Empty,
            // CompoundFluidIngredient.CODEC: `children` (or `ingredients`), non-empty.
            "neoforge:compound" => {
                let children = o
                    .get("children")
                    .or_else(|| o.get("ingredients"))
                    .ok_or("No key children in MapLike")?;
                Self::Compound(Self::list(children, false)?)
            }
            // DataComponentFluidIngredient.CODEC: `fluids`, `components`, `strict` (false).
            "neoforge:components" => {
                let components = match field("components")? {
                    v @ Value::Object(_) => match json_to_nbt(v) {
                        Some(Tag::Compound(c)) => c,
                        _ => return Err(format!("Not a component map: {v}")),
                    },
                    v => return Err(format!("Not a map: {v}")),
                };
                let strict = match o.get("strict") {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    Some(v) => return Err(format!("Not a boolean: {v}")),
                };
                Self::Components {
                    fluids: FluidSet::parse(field("fluids")?)?,
                    components,
                    strict,
                }
            }
            // DifferenceFluidIngredient.CODEC: `base`, `subtracted`, each CODEC_NON_EMPTY.
            "neoforge:difference" => Self::Difference(
                Box::new(Self::parse_non_empty(field("base")?)?),
                Box::new(Self::parse_non_empty(field("subtracted")?)?),
            ),
            // IntersectionFluidIngredient.CODEC: `children`, non-empty.
            "neoforge:intersection" => Self::Intersection(Self::list(field("children")?, false)?),
            other => {
                return Err(format!(
                    "Unknown registry key in ResourceKey[minecraft:root / neoforge:fluid_ingredient_type]: {other}"
                ));
            }
        })
    }

    /// `test(FluidStack)`.
    pub fn test(&self, stack: &FluidStack) -> bool {
        match self {
            // EmptyFluidIngredient.test: `fluidStack.isEmpty()`.
            Self::Empty => stack.is_empty(),
            // SingleFluidIngredient.test: `fluidStack.is(this.fluid)`.
            Self::Single(fluid) => stack.is(*fluid),
            // TagFluidIngredient.test: `fluidStack.is(this.tag)`.
            Self::Tag(tag) => stack.is_tag(tag),
            Self::Compound(children) => children.iter().any(|c| c.test(stack)),
            Self::Components {
                fluids,
                components,
                strict,
            } => {
                if *strict {
                    // Against its stacks: `FluidStack.isSameFluidSameComponents`.
                    fluids.contains(stack.fluid())
                        && !stack.is_empty()
                        && same_patch(stack.components(), components)
                } else {
                    // `fluids.contains(stack.getFluidHolder()) && components.test(stack)`.
                    fluids.contains(stack.fluid())
                        && components.iter().all(|(k, v)| {
                            stack
                                .components()
                                .get(k)
                                .is_some_and(|have| same_value(have, v))
                        })
                }
            }
            Self::Difference(base, subtracted) => base.test(stack) && !subtracted.test(stack),
            Self::Intersection(children) => children.iter().all(|c| c.test(stack)),
        }
    }

    /// `generateStacks`: a bucket of each fluid it names.
    fn generate_stacks(&self) -> Vec<FluidStack> {
        match self {
            Self::Empty => Vec::new(),
            Self::Single(fluid) => vec![FluidStack::new(*fluid, BUCKET_VOLUME)],
            Self::Tag(tag) => crate::mod_fluids::fluids_in_tag(tag)
                .into_iter()
                .map(|f| FluidStack::new(f, BUCKET_VOLUME))
                .collect(),
            Self::Compound(children) => children.iter().flat_map(Self::generate_stacks).collect(),
            Self::Components {
                fluids, components, ..
            } => fluids
                .fluids()
                .into_iter()
                .map(|f| FluidStack::with_components(f, BUCKET_VOLUME, components.clone()))
                .collect(),
            Self::Difference(base, subtracted) => base
                .generate_stacks()
                .into_iter()
                .filter(|s| !subtracted.test(s))
                .collect(),
            Self::Intersection(children) => children
                .iter()
                .flat_map(Self::generate_stacks)
                .filter(|s| self.test(s))
                .collect(),
        }
    }

    /// `getStacks`: [`generate_stacks`](Self::generate_stacks) without a
    /// second stack of the same fluid and components
    /// (`FluidStackLinkedSet.createTypeAndComponentsSet`), in order.
    pub fn stacks(&self) -> Vec<FluidStack> {
        let mut out: Vec<FluidStack> = Vec::new();
        for s in self.generate_stacks() {
            if !out
                .iter()
                .any(|o| FluidStack::is_same_fluid_same_components(o, &s))
            {
                out.push(s);
            }
        }
        out
    }

    /// `isEmpty`: the empty ingredient itself.
    pub fn is_empty(&self) -> bool {
        *self == Self::Empty
    }

    /// `hasNoFluids`: it names no fluid (an empty or unknown tag).
    pub fn has_no_fluids(&self) -> bool {
        self.stacks().is_empty()
    }
}

/// `SizedFluidIngredient`: an ingredient and the millibuckets it takes.
#[derive(Clone, Debug, PartialEq)]
pub struct SizedFluidIngredient {
    pub ingredient: FluidIngredient,
    pub amount: i32,
}

impl SizedFluidIngredient {
    /// `SizedFluidIngredient.FLAT_CODEC`: the ingredient's map
    /// (`MAP_CODEC_NONEMPTY`) and `amount` beside it, a positive int (a
    /// bucket when absent: `optionalFieldAlwaysWrite(..., 1000)`).
    pub fn parse_flat(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or_else(|| format!("Not a map: {v}"))?;
        Ok(Self {
            ingredient: FluidIngredient::parse_map(o)?,
            amount: amount(o)?,
        })
    }

    /// `SizedFluidIngredient.NESTED_CODEC`: `ingredient`
    /// (`FluidIngredient.CODEC_NON_EMPTY`) and `amount`.
    pub fn parse_nested(v: &Value) -> Result<Self, String> {
        let o = v.as_object().ok_or_else(|| format!("Not a map: {v}"))?;
        let ingredient = o.get("ingredient").ok_or("No key ingredient in MapLike")?;
        Ok(Self {
            ingredient: FluidIngredient::parse_non_empty(ingredient)?,
            amount: amount(o)?,
        })
    }

    /// `test`: the ingredient matches and the stack holds at least `amount`.
    pub fn test(&self, stack: &FluidStack) -> bool {
        self.ingredient.test(stack) && stack.amount() >= self.amount
    }

    /// `getFluids`: the ingredient's stacks, each of `amount`.
    pub fn fluids(&self) -> Vec<FluidStack> {
        self.ingredient
            .stacks()
            .iter()
            .map(|s| s.copy_with_amount(self.amount))
            .collect()
    }
}

/// `optionalFieldAlwaysWrite(ExtraCodecs.POSITIVE_INT, "amount", 1000)`.
fn amount(o: &Map<String, Value>) -> Result<i32, String> {
    o.get("amount").map_or(Ok(BUCKET_VOLUME), positive_int)
}

/// `TagKey.codec(Registries.FLUID)`: a resource location, no `#`.
fn tag_json(v: &Value) -> Result<String, String> {
    tag_id(v.as_str().ok_or_else(|| format!("Not a string: {v}"))?)
}

/// A tag's id as a `ResourceLocation` reads it: a bare path is `minecraft:`'s.
fn tag_id(s: &str) -> Result<String, String> {
    let valid = |p: &str, extra: &str| {
        !p.is_empty()
            && p.chars().all(|c| {
                c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || "_-.".contains(c)
                    || extra.contains(c)
            })
    };
    match s.split_once(':') {
        Some((ns, path)) if valid(ns, "") && valid(path, "/") => Ok(s.to_owned()),
        None if valid(s, "/") => Ok(format!("minecraft:{s}")),
        _ => Err(format!("Not a valid resource location: {s}")),
    }
}

/// A stack's patch is exactly a predicate's map (`asPatch`), value by value.
fn same_patch(patch: &Compound, expected: &Compound) -> bool {
    patch.len() == expected.len()
        && expected
            .iter()
            .all(|(k, v)| patch.get(k).is_some_and(|have| same_value(have, v)))
}

/// `Objects.equals` of two component values, one kept as a stack's NBT and
/// one read from a recipe's JSON: the JSON's numbers take the smallest NBT
/// type that holds them, while the component's codec may write a wider one,
/// so numbers compare by value and number lists as arrays.
fn same_value(a: &Tag, b: &Tag) -> bool {
    fn number(t: &Tag) -> Option<f64> {
        Some(match t {
            Tag::Byte(n) => f64::from(*n),
            Tag::Short(n) => f64::from(*n),
            Tag::Int(n) => f64::from(*n),
            Tag::Long(n) => *n as f64,
            Tag::Float(bits) => f64::from(f32::from_bits(*bits)),
            Tag::Double(bits) => f64::from_bits(*bits),
            _ => return None,
        })
    }
    fn elements(t: &Tag) -> Option<Vec<Tag>> {
        Some(match t {
            Tag::List(l) => l.clone(),
            Tag::ByteArray(a) => a.iter().map(|b| Tag::Byte(*b as i8)).collect(),
            Tag::IntArray(a) => a.iter().map(|n| Tag::Int(*n)).collect(),
            Tag::LongArray(a) => a.iter().map(|n| Tag::Long(*n)).collect(),
            _ => return None,
        })
    }
    if let (Some(x), Some(y)) = (number(a), number(b)) {
        return x == y;
    }
    match (a, b) {
        (Tag::Compound(x), Tag::Compound(y)) => same_patch(x, y),
        (Tag::String(x), Tag::String(y)) => x == y,
        _ => match (elements(a), elements(b)) {
            (Some(x), Some(y)) => {
                x.len() == y.len() && x.iter().zip(&y).all(|(p, q)| same_value(p, q))
            }
            _ => a == b,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kind(name: &str) -> FluidKind {
        crate::mod_fluids::register_test_fluids();
        FluidKind::from_name(name).unwrap()
    }

    #[test]
    fn the_map_codec_reads_a_fluid_or_a_tag_or_a_type() {
        let brine = kind("test:brine");
        let read = |v: Value| FluidIngredient::parse(&v);
        assert_eq!(
            read(json!({"fluid": "test:brine"})),
            Ok(FluidIngredient::Single(brine))
        );
        assert_eq!(
            read(json!({"fluid": "water"})),
            Ok(FluidIngredient::Single(FluidKind::Water))
        );
        assert_eq!(
            read(json!({"tag": "c:brine"})),
            Ok(FluidIngredient::Tag("c:brine".into()))
        );
        assert_eq!(
            read(json!({"type": "neoforge:single", "fluid": "minecraft:lava"})),
            Ok(FluidIngredient::Single(FluidKind::Lava))
        );
        // Both, neither, the empty fluid, a fluid the registry lacks, an unknown type: the codec's errors.
        for bad in [
            json!({"fluid": "test:brine", "tag": "c:brine"}),
            json!({"amount": 10}),
            json!({"fluid": "minecraft:empty"}),
            json!({"fluid": "other:gone"}),
            json!({"fluid": "#c:brine"}),
            json!({"type": "neoforge:nothing", "fluid": "test:brine"}),
            json!({"type": "neoforge:empty"}),
            json!("test:brine"),
        ] {
            assert!(read(bad.clone()).is_err(), "{bad}");
        }
        // A list: none is the empty ingredient (not in CODEC_NON_EMPTY), one is itself, more a compound.
        assert_eq!(read(json!([])), Ok(FluidIngredient::Empty));
        assert!(FluidIngredient::parse_non_empty(&json!([])).is_err());
        assert_eq!(
            read(json!([{"tag": "c:brine"}])),
            Ok(FluidIngredient::Tag("c:brine".into()))
        );
        assert_eq!(
            read(json!([{"fluid": "water"}, {"fluid": "lava"}])),
            Ok(FluidIngredient::Compound(vec![
                FluidIngredient::Single(FluidKind::Water),
                FluidIngredient::Single(FluidKind::Lava)
            ]))
        );
    }

    #[test]
    fn each_type_tests_a_stack_as_neoforge_does() {
        let brine = kind("test:brine");
        let sludge = kind("test:sludge");
        let parse = |v: Value| FluidIngredient::parse(&v).unwrap();
        let stack = |f: FluidKind| FluidStack::new(f, 1);
        // A single fluid by identity; a tag by the registry's tags, both of a family.
        let single = parse(json!({"fluid": "minecraft:water"}));
        assert!(
            single.test(&stack(FluidKind::Water)) && !single.test(&stack(FluidKind::FlowingWater))
        );
        let tag = parse(json!({"tag": "c:brine"}));
        assert!(
            tag.test(&stack(brine))
                && tag.test(&stack(brine.flowing()))
                && !tag.test(&stack(sludge))
        );
        assert!(
            !tag.test(&FluidStack::EMPTY),
            "the empty fluid is in no tag"
        );
        let compound = parse(
            json!({"type": "neoforge:compound", "children": [{"tag": "c:brine"}, {"fluid": "lava"}]}),
        );
        assert!(
            compound.test(&stack(FluidKind::Lava))
                && compound.test(&stack(brine))
                && !compound.test(&stack(sludge))
        );
        let difference = parse(
            json!({"type": "neoforge:difference", "base": {"tag": "minecraft:water"}, "subtracted": {"fluid": "flowing_water"}}),
        );
        assert!(
            difference.test(&stack(FluidKind::Water))
                && !difference.test(&stack(FluidKind::FlowingWater))
        );
        assert_eq!(
            difference.stacks(),
            [FluidStack::new(FluidKind::Water, 1000)]
        );
        let intersection = parse(
            json!({"type": "neoforge:intersection", "children": [{"tag": "c:brine"}, {"fluid": "test:flowing_brine"}]}),
        );
        assert!(intersection.test(&stack(brine.flowing())) && !intersection.test(&stack(brine)));
        assert_eq!(
            intersection.stacks(),
            [FluidStack::new(brine.flowing(), 1000)]
        );
        assert!(
            FluidIngredient::parse(
                &json!({"type": "neoforge:compound", "children": [{"type": "neoforge:empty"}]})
            )
            .is_err(),
            "a list's entries are maps, which can't be empty"
        );
        // The empty ingredient takes only the empty stack and names no fluid.
        assert!(
            FluidIngredient::Empty.test(&FluidStack::EMPTY)
                && !FluidIngredient::Empty.test(&stack(brine))
        );
        assert!(FluidIngredient::Empty.has_no_fluids() && FluidIngredient::Empty.is_empty());
        // An unknown tag reads but names nothing.
        let none = parse(json!({"tag": "c:nothing"}));
        assert!(none.has_no_fluids() && !none.test(&stack(brine)) && !none.is_empty());
        // getStacks: a bucket of each, without repeats.
        let twice = parse(json!([{"tag": "c:brine"}, {"fluid": "test:brine"}]));
        assert_eq!(
            twice.stacks(),
            [
                FluidStack::new(brine.flowing(), 1000),
                FluidStack::new(brine, 1000)
            ],
            "the tag's fluids in registry order, then nothing new"
        );
    }

    #[test]
    fn a_components_ingredient_tests_the_stacks_data() {
        let brine = kind("test:brine");
        let parse = |v: Value| FluidIngredient::parse(&v).unwrap();
        let with = |pairs: &[(&str, Tag)]| {
            let mut c = Compound::new();
            for (k, v) in pairs {
                c.insert((*k).into(), v.clone());
            }
            FluidStack::with_components(brine, 100, c)
        };
        let loose = parse(
            json!({"type": "neoforge:components", "fluids": "#c:brine", "components": {"test:level": 3}}),
        );
        // The JSON's 3 reads as a byte; the stack's codec wrote an int: equal values.
        assert!(loose.test(&with(&[("test:level", Tag::Int(3))])));
        assert!(loose.test(&with(&[
            ("test:level", Tag::Int(3)),
            ("test:other", Tag::Byte(1))
        ])));
        assert!(!loose.test(&with(&[("test:level", Tag::Int(4))])));
        assert!(!loose.test(&with(&[])));
        assert!(!loose.test(&FluidStack::new(FluidKind::Water, 100)));
        let strict = parse(
            json!({"type": "neoforge:components", "fluids": ["test:brine"], "components": {"test:level": 3}, "strict": true}),
        );
        assert!(strict.test(&with(&[("test:level", Tag::Int(3))])));
        assert!(!strict.test(&with(&[
            ("test:level", Tag::Int(3)),
            ("test:other", Tag::Byte(1))
        ])));
        assert_eq!(strict.stacks()[0].components().len(), 1);
    }

    #[test]
    fn a_sized_ingredient_reads_flat_or_nested_and_wants_its_amount() {
        let brine = kind("test:brine");
        let flat =
            SizedFluidIngredient::parse_flat(&json!({"tag": "c:brine", "amount": 10})).unwrap();
        assert!(flat.test(&FluidStack::new(brine, 10)) && !flat.test(&FluidStack::new(brine, 9)));
        assert_eq!(flat.fluids()[1], FluidStack::new(brine, 10));
        let bucket = SizedFluidIngredient::parse_flat(&json!({"fluid": "test:brine"})).unwrap();
        assert_eq!(bucket.amount, 1000, "a bucket when absent");
        let nested = SizedFluidIngredient::parse_nested(
            &json!({"ingredient": [{"fluid": "test:brine"}], "amount": 5}),
        )
        .unwrap();
        assert_eq!(
            nested,
            SizedFluidIngredient {
                ingredient: FluidIngredient::Single(brine),
                amount: 5
            }
        );
        for bad in [
            json!({"tag": "c:brine", "amount": 0}),
            json!({"tag": "c:brine", "amount": "ten"}),
            json!({"type": "neoforge:empty", "amount": 5}),
        ] {
            assert!(SizedFluidIngredient::parse_flat(&bad).is_err(), "{bad}");
        }
        assert!(
            SizedFluidIngredient::parse_nested(&json!({"ingredient": [], "amount": 5})).is_err()
        );
    }
}
