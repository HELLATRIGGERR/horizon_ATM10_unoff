//! JEI's `config/jei/blacklist.json` (JEI 19.70's hidden ingredients, version 2): `[{"version":2}, {"hide_mode":
//! "WILDCARD", "ingredient": {"ingredient": "mekanism:dirty_iron", "type": "mekanism.api.chemical.ChemicalStack"}},
//! ...]`. ATM10's own file hides 14 Mekanism chemicals (dirty and clean slurries). The runtime reads the JSON into
//! `(hide_mode, type, ingredient)` triples; this keeps what they hide. A `WILDCARD` entry hides every variant of an
//! id, another mode the plain ingredient of that id; items are hidden from the list and its search, other types (the
//! chemicals, fluids) are kept for when they are ingredients of the list. "Show Hidden Ingredients in JEI" (the
//! config's Cheats page) brings them back.

use std::collections::BTreeSet;

/// What the file hides.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Blacklist {
    items: BTreeSet<String>,
    /// Other types' ids, `(type, id)`.
    others: BTreeSet<(String, String)>,
}

/// Whether an ingredient type of the file is the item stack's (`VanillaTypes.ITEM_STACK`).
pub fn is_item_type(kind: &str) -> bool {
    matches!(kind, "item_stack" | "minecraft:item_stack" | "jei:item_stack" | "net.minecraft.world.item.ItemStack")
}

/// An ingredient's id as the file writes it: `ns:path`, or that with a component part after it (`ns:path{...}`,
/// `ns:path;...`), whose base id is kept.
fn base_id(ingredient: &str) -> &str {
    ingredient.split(['{', ';', ' ', '[']).next().unwrap_or("").trim()
}

impl Blacklist {
    /// From the file's entries, `(hide_mode, type, ingredient)`.
    pub fn from_entries<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>) -> Blacklist {
        let mut list = Blacklist::default();
        for (mode, kind, ingredient) in entries {
            let id = base_id(ingredient);
            // A mode other than WILDCARD names one variant: only the plain one is the id itself.
            if id.is_empty() || !id.contains(':') || (mode != "WILDCARD" && id != ingredient.trim()) {
                continue;
            }
            if is_item_type(kind) {
                list.items.insert(id.to_owned());
            } else {
                list.others.insert((kind.to_owned(), id.to_owned()));
            }
        }
        list
    }
    pub fn hides_item(&self, id: &str) -> bool {
        self.items.contains(id)
    }
    /// Whether it hides ingredient `id` of type `kind` (`Stack::kind`: "" an item, `fluid`, `mekanism:chemical`):
    /// the file names a type by JEI's uid (`fluid_stack`) or its class name (`mekanism.api.chemical.ChemicalStack`,
    /// ATM10's chemicals).
    pub fn hides(&self, kind: &str, id: &str) -> bool {
        if kind.is_empty() {
            return self.hides_item(id);
        }
        let uid = if kind == "fluid" { "fluid_stack" } else { kind };
        [uid, crate::type_name(kind)].into_iter().any(|t| self.others.contains(&(t.to_owned(), id.to_owned())))
    }
    /// Entries the file holds that it reads (items and others).
    pub fn len(&self) -> usize {
        self.items.len() + self.others.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn items(&self) -> usize {
        self.items.len()
    }
}
