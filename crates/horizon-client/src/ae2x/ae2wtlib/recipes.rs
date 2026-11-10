//! AE2WTLib's crafting-grid recipes `ae2wtlib:combine` and `ae2wtlib:upgrade`
//! (`Combine`, `Upgrade`, `Common`, `InputHelper`, their serializers),
//! AE2WTLib 19.5.1 by Mari023, https://github.com/Mari023/AE2WirelessTerminalLibrary
//! tag `19.5.1`, MIT; see `ae2/addons/ae2wtlib/`. The crafting grid runs them
//! as special recipes (`crafting::Special::Ae2wtlib`); the merge is the
//! registry's (`ae2x::wireless::merge_terminal`).
use super::UNIVERSAL_TERMINAL;
use crate::{
    ae2x::{defs, wireless},
    crafting::{BookDisplay, Ingredient, Placement},
    items::{ItemStack, Items},
    recipe_book::Display,
};
use horizon_world::nbt::{Compound, Tag};
use serde_json::Value;

/// `CombineSerializer.NAME`, `UpgradeSerializer.NAME`.
pub const COMBINE: &str = "ae2wtlib:combine";
pub const UPGRADE: &str = "ae2wtlib:upgrade";
/// `Common.category`: `CraftingBookCategory.EQUIPMENT`.
pub const CATEGORY: &str = "equipment";

/// One of the two recipes, with the terminals' names (`WTDefinition.CODEC`).
#[derive(Clone, Debug)]
pub enum Recipe {
    /// Two terminals into a new universal terminal.
    Combine { a: Ingredient, b: Ingredient, a_name: String, b_name: String },
    /// A terminal into a universal terminal that does not hold it yet.
    Upgrade { terminal: Ingredient, name: String },
}

/// A terminal a universal terminal can hold, by its name; the codec refuses
/// any other, and the recipe then does not load.
fn definition(name: &str) -> Option<&'static defs::WirelessTerminal> {
    defs::get().wireless_terminal_named(name).filter(|t| t.installed.is_some())
}

/// `InputHelper.WUT`.
fn universal_ingredient() -> Ingredient {
    Ingredient::of(vec![UNIVERSAL_TERMINAL.to_owned()])
}

/// `InputHelper.getInputStack`: the first stack of the grid's box, row by
/// row, the ingredient takes.
fn input<'a>(stacks: &[&'a ItemStack], ingredient: &Ingredient) -> Option<&'a ItemStack> {
    stacks.iter().copied().find(|s| s.count > 0 && ingredient.test(s))
}

/// The terminal's `Unit` component.
fn hold(stack: &mut ItemStack, terminal: &defs::WirelessTerminal) {
    if let Some(component) = terminal.installed.as_deref() {
        stack.data_mut().set_other(component, Some(Tag::Compound(Compound::new())));
    }
}

/// The JSON of either type (`MAP_CODEC`); None for any other type, or when
/// a field is missing or names no terminal.
pub fn parse(v: &Value, items: &Items) -> Option<Recipe> {
    let name = |key: &str| v[key].as_str().filter(|n| definition(n).is_some()).map(str::to_owned);
    match v["type"].as_str()? {
        COMBINE => Some(Recipe::Combine {
            a: Ingredient::parse(&v["terminalA"], items)?,
            b: Ingredient::parse(&v["terminalB"], items)?,
            a_name: name("terminalAName")?,
            b_name: name("terminalBName")?,
        }),
        UPGRADE => Some(Recipe::Upgrade { terminal: Ingredient::parse(&v["terminal"], items)?, name: name("terminalName")? }),
        _ => None,
    }
}

impl Recipe {
    /// `getResultItem`: `Common.outputStack`, a universal terminal holding
    /// the recipe's terminals, nothing else.
    pub fn output(&self) -> ItemStack {
        let mut out = ItemStack::new(UNIVERSAL_TERMINAL, 1);
        let names: Vec<&str> = match self {
            Self::Combine { a_name, b_name, .. } => vec![a_name, b_name],
            Self::Upgrade { name, .. } => vec![name],
        };
        for terminal in names.into_iter().filter_map(definition) {
            hold(&mut out, terminal);
        }
        out
    }

    /// `getIngredients`.
    fn ingredients(&self) -> Vec<Ingredient> {
        match self {
            Self::Combine { a, b, .. } => vec![a.clone(), b.clone()],
            Self::Upgrade { terminal, .. } => vec![terminal.clone(), universal_ingredient()],
        }
    }

    /// `matches`, over the stacks of the grid's box: exactly two, each
    /// ingredient taking one; an upgrade's universal terminal must not hold
    /// the terminal yet.
    pub fn matches(&self, stacks: &[&ItemStack]) -> bool {
        if stacks.len() != 2 {
            return false;
        }
        match self {
            Self::Combine { a, b, .. } => input(stacks, a).is_some() && input(stacks, b).is_some(),
            Self::Upgrade { terminal, name } => {
                let Some(definition) = definition(name) else { return false };
                input(stacks, terminal).is_some()
                    && input(stacks, &universal_ingredient()).is_some_and(|wut| !wireless::has_terminal(wut, definition))
            }
        }
    }

    /// `assemble`: `Common.mergeTerminal` of the terminals into the output
    /// (a combine) or into the universal terminal (an upgrade); None when a
    /// merge fails (`ItemStack.EMPTY`).
    pub fn assemble(&self, stacks: &[&ItemStack]) -> Option<ItemStack> {
        match self {
            Self::Combine { a, b, a_name, b_name } => {
                let wut = wireless::merge_terminal(&self.output(), input(stacks, a)?, definition(a_name)?).ok()?;
                wireless::merge_terminal(&wut, input(stacks, b)?, definition(b_name)?).ok()
            }
            Self::Upgrade { terminal, name } => {
                let wut = input(stacks, &universal_ingredient())?;
                wireless::merge_terminal(wut, input(stacks, terminal)?, definition(name)?).ok()
            }
        }
    }

    /// The recipe book's display: a shapeless recipe of `getIngredients`
    /// showing `getResultItem`, as a `CraftingRecipe` without a display of
    /// its own shows in the recipe viewers.
    pub fn book_display(&self) -> BookDisplay {
        BookDisplay { display: Display::Shapeless(self.ingredients()), result: self.output(), variants: Vec::new(), stacks: Vec::new() }
    }

    /// Placing it from the book: its ingredients, shapeless.
    pub fn placement(&self) -> Placement {
        Placement::of(self.ingredients().into_iter().map(Some).collect(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ae2_upgrade_inventory::Inventory,
        ae2x::ae2wtlib::{PATTERN_ACCESS_TERMINAL, PATTERN_ENCODING_TERMINAL},
    };

    const CRAFTING: &str = "ae2:wireless_crafting_terminal";

    fn combine(a: &str, b: &str, a_name: &str, b_name: &str) -> Recipe {
        Recipe::Combine {
            a: Ingredient::of(vec![a.to_owned()]),
            b: Ingredient::of(vec![b.to_owned()]),
            a_name: a_name.to_owned(),
            b_name: b_name.to_owned(),
        }
    }
    fn upgrade(item: &str, name: &str) -> Recipe {
        Recipe::Upgrade { terminal: Ingredient::of(vec![item.to_owned()]), name: name.to_owned() }
    }
    fn with_card(item: &str, power: f64) -> ItemStack {
        let mut stack = ItemStack::new(item, 1);
        let mut cards = Inventory::read(&stack).unwrap();
        assert!(cards.insert_one(&ItemStack::new("ae2:energy_card", 1)));
        cards.store(&mut stack).unwrap();
        wireless::transfer_energy(&mut stack, power, true, false).unwrap();
        stack
    }

    /// `ce.json`: the crafting and encoding terminals into one universal
    /// terminal, their cards and energy with them; its battery counts both
    /// terminals from the first merge, as `outputStack` holds both.
    #[test]
    fn combine_merges_two_terminals_their_cards_and_energy() {
        let base = crate::ae2_common_config::wireless().terminal_capacity;
        let recipe = combine(CRAFTING, PATTERN_ENCODING_TERMINAL, "crafting", "pattern_encoding");
        let crafting = with_card(CRAFTING, 2.0 * base);
        let mut encoding = ItemStack::new(PATTERN_ENCODING_TERMINAL, 1);
        wireless::transfer_energy(&mut encoding, 300_000.0, true, false).unwrap();
        assert!(recipe.matches(&[&encoding, &crafting]), "either order");
        assert!(!recipe.matches(&[&crafting, &crafting]));
        assert!(!recipe.matches(&[&crafting, &encoding, &ItemStack::new("minecraft:stick", 1)]), "two ingredients only");
        let wut = recipe.assemble(&[&encoding, &crafting]).unwrap();
        assert_eq!(&*wut.item, UNIVERSAL_TERMINAL);
        let held: Vec<_> = wireless::installed(&wut).iter().map(|t| t.name.as_str()).collect();
        assert_eq!(held, ["crafting", "pattern_encoding"]);
        let cards = Inventory::read(&wut).unwrap();
        assert_eq!(cards.slots.len(), 4);
        assert_eq!(cards.slots.iter().flatten().count(), 1, "the crafting terminal's card");
        // Two terminals and a card: three batteries; the crafting terminal's
        // full two fit, as the output already holds both terminals.
        assert_eq!(wireless::energy(&wut).unwrap(), (2.0 * base + 300_000.0, 3.0 * base));
        assert_eq!(recipe.output().count, 1);
        assert_eq!(wireless::installed(&recipe.output()).len(), 2, "the shown result holds both");
    }

    /// `upgrade_pattern_acces.json`: a terminal into a universal terminal not
    /// holding it yet; one holding it already matches no more.
    #[test]
    fn upgrade_adds_a_terminal_the_universal_terminal_lacks() {
        let recipe = upgrade(PATTERN_ACCESS_TERMINAL, "pattern_access");
        let mut wut = ItemStack::new(UNIVERSAL_TERMINAL, 1);
        hold(&mut wut, definition("crafting").unwrap());
        wireless::transfer_energy(&mut wut, 1000.0, true, false).unwrap();
        let mut access = ItemStack::new(PATTERN_ACCESS_TERMINAL, 1);
        access.data_mut().set_other("minecraft:custom_name", Some(Tag::String("Mine".into())));
        wireless::transfer_energy(&mut access, 500.0, true, false).unwrap();
        assert!(recipe.matches(&[&access, &wut]));
        assert!(!recipe.matches(&[&access, &access]));
        let upgraded = recipe.assemble(&[&access, &wut]).unwrap();
        let held: Vec<_> = wireless::installed(&upgraded).iter().map(|t| t.name.as_str()).collect();
        assert_eq!(held, ["crafting", "pattern_access"]);
        assert_eq!(wireless::energy(&upgraded).unwrap().0, 1500.0);
        assert!(upgraded.data().unwrap().other("minecraft:custom_name").is_some(), "the terminal's other components");
        assert!(!recipe.matches(&[&access, &upgraded]), "it holds the pattern access terminal now");
        assert!(upgrade(CRAFTING, "crafting").assemble(&[&ItemStack::new(CRAFTING, 1), &upgraded]).is_some(), "assemble does not check");
    }

    #[test]
    fn book_shows_the_ingredients_and_the_output() {
        let recipe = upgrade(CRAFTING, "crafting");
        let shown = recipe.book_display();
        let Display::Shapeless(ingredients) = &shown.display else { panic!("shapeless") };
        assert_eq!(ingredients.iter().map(|i| i.order.clone()).collect::<Vec<_>>(), [vec![CRAFTING.to_owned()], vec![UNIVERSAL_TERMINAL.to_owned()]]);
        assert_eq!(wireless::installed(&shown.result).len(), 1);
        assert_eq!(recipe.placement().ingredients.len(), 2);
        assert!(definition("ae2:wireless_terminal").is_none() && definition("restock").is_none(), "only the terminals it can hold");
    }
}
