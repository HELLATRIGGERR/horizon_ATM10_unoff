//! The addons' static rules (docs/ae2-crates.md, extension point v2): what an addon crate above
//! horizon-client answers about its own items, blocks, parts and machines from the item, the block
//! or the machine alone, with no game at hand. The lib's and the game's free functions that asked
//! an addon module (`extendedae::part_model(id)`, `advancedae::reset_unsaved(m)`...) ask
//! [`rules`]`("<id>")` instead, in the addon's place and order; the game's per-game behaviour stays
//! in `game::ae2_ext::Ae2Addon`.
//!
//! The rules are `&'static dyn`, installed before the game starts (like [`super::defs`]' rows);
//! every method has the answer of an addon that has nothing there, so an addon implements only its
//! own. A new question is one more default method here and one `rules(id)` call at the place that
//! asked the module.
use crate::{ae2_world::Machine, items::ItemStack};
use ae2_cell_core::{ItemKey, KeyType};
use horizon_world::nbt::Compound;
use std::sync::{Mutex, PoisonError};

/// One addon's static rules (see the module). The tables by item id the renderer and the screens
/// read (models, boxes, styles) are methods here too: a row lookup of the addon's own ids.
pub trait AddonRules: Sync {
    /// The addon's id (pack/pack.toml's).
    fn id(&self) -> &'static str;

    // The lib's questions (`ae2x::*`).

    /// Clears what a machine of its own does not save, as a save loads (`ae2x::reset_unsaved`).
    fn reset_unsaved(&self, _machine: &mut Machine) {}
    /// The fixed cell of item `id` (`ae2x::fixed_cell`): a creative cell of one key.
    fn fixed_cell(&self, _id: &str) -> Option<super::FixedCell> {
        None
    }
    /// The model a drive draws for its fixed cell `id` (`ae2x::fixed_cell_model`).
    fn fixed_cell_model(&self, _id: &str) -> Option<&'static str> {
        None
    }
    /// Whether a cell of its own handler fits inside another cell (`ae2x::handler_cell_fits_inside`).
    fn handler_cell_fits_inside(&self, _stack: &ItemStack) -> Option<bool> {
        None
    }
    /// A cell of its own handler's `getStatus`, as `CellState`'s LED number (`ae2x::handler_cell_state`).
    fn handler_cell_state(&self, _stack: &ItemStack) -> Option<u8> {
        None
    }
    /// The keys a cell of its own handler lists, in its order (`ae2x::handler_cell_keys`).
    fn handler_cell_keys(&self, _stack: &ItemStack, _known: &dyn Fn(&str, KeyType) -> bool) -> Option<Vec<ItemKey>> {
        None
    }
    /// The name of an item of its own that its data names (`item_tooltip`'s name line).
    fn item_name(&self, _id: &str, _translate: &dyn Fn(&str) -> Option<String>, _item_name: &dyn Fn(&str) -> String) -> Option<String> {
        None
    }
    /// The pack's content mods were loaded from `root` (`game::mods`): rows the pack adds.
    fn pack_loaded(&self, _root: &std::path::Path) {}

    // Tables by item id: models, boxes and styles of its parts and blocks.

    /// The model of its cable part `id` (`part_model`).
    fn part_model(&self, _id: &str) -> Option<&'static str> {
        None
    }
    /// The collision boxes of its cable part `id`, in 1/16 block (`part_boxes`).
    fn part_boxes(&self, _id: &str) -> Option<Vec<[f64; 6]>> {
        None
    }
    /// The style (sheet, slot style) of its part `id`'s screen (`part_style`).
    fn part_style(&self, _id: &str) -> Option<(&'static str, &'static str)> {
        None
    }
    /// The title of its part `id`'s menu (`part_menu_title`).
    fn part_menu_title(&self, _id: &str) -> Option<&'static str> {
        None
    }
    /// The operations per tick of its bus `id`: 1 for a plain one (`bus_operations_multiplier`).
    fn bus_operations_multiplier(&self, _id: &str) -> u64 {
        1
    }
    /// The screen of its display part `id` (`display_part_screen`).
    fn display_part_screen(&self, _id: &str) -> Option<&'static str> {
        None
    }
    /// Whether its item `id` opens a menu of its own when used (`item_opens_menu`).
    fn item_opens_menu(&self, _id: &str) -> bool {
        false
    }

    // Its machines' slots and menus, from the machine alone.

    /// Whether its machine opens a menu (`machine_menu`).
    fn machine_menu(&self, _machine: &Machine) -> bool {
        false
    }
    /// The menu window of its machine (`machine_window`).
    fn machine_window(&self, _machine: &Machine) -> Option<usize> {
        None
    }
    /// The capacity of slot `slot` of its machine (`machine_slot_capacity`).
    fn slot_capacity(&self, _machine: &Machine, _slot: usize) -> Option<u32> {
        None
    }
    /// Whether slot `index` of its machine kind takes items in (`slot_insert`).
    fn slot_insert(&self, _kind: crate::ae2_world::MachineKind, _index: usize) -> Option<bool> {
        None
    }
    /// Whether slot `index` of its machine kind gives items out (`slot_extract`).
    fn slot_extract(&self, _kind: crate::ae2_world::MachineKind, _index: usize) -> Option<bool> {
        None
    }
    /// Its machine's progress, done of total (`machine_progress`).
    fn progress(&self, _machine: &Machine) -> Option<(f64, f64)> {
        None
    }
    /// Its machine's screen data, appended (`machine_screen_data`).
    fn screen_data(&self, _machine: &Machine, _data: &mut Vec<i16>) {}
    /// Its machine's menu items, filled in (`machine_menu_items`).
    fn menu_items(&self, _machine: &Machine, _items: &mut [Option<ItemStack>]) {}
    /// Whether its block `id` takes random ticks (`random_ticks`).
    fn random_ticks(&self, _block: &str) -> bool {
        false
    }
    /// Whether its pattern-holding machine takes the item `item` (`pattern_inventory_accepts`).
    fn pattern_allowed(&self, _machine: &Machine, _item: &ItemStack) -> Option<bool> {
        None
    }
    /// A part record's own step before a bus config amount is set (`bus_config_amount`).
    fn bus_config_amount(&self, _part: &mut Compound, _slot: usize, _amount: Option<u64>) {}
}

static RULES: Mutex<Vec<&'static dyn AddonRules>> = Mutex::new(Vec::new());

/// Installs an addon's static rules. The program calls it before the game starts; a second
/// install of the same id is ignored.
pub fn install_rules(rules: &'static dyn AddonRules) {
    let mut installed = RULES.lock().unwrap_or_else(PoisonError::into_inner);
    if !installed.iter().any(|known| known.id() == rules.id()) {
        installed.push(rules);
    }
}

/// The static rules of the addon `id`, when it is built in.
pub fn rules(id: &str) -> Option<&'static dyn AddonRules> {
    RULES.lock().unwrap_or_else(PoisonError::into_inner).iter().copied().find(|rules| rules.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe;
    impl AddonRules for Probe {
        fn id(&self) -> &'static str {
            "rules-probe"
        }
        fn part_model(&self, id: &str) -> Option<&'static str> {
            (id == "rules-probe:part").then_some("rules-probe:part/model")
        }
    }

    #[test]
    fn an_addon_answers_its_own_ids_and_the_defaults_the_rest() {
        assert!(rules("rules-probe").is_none());
        install_rules(&Probe);
        install_rules(&Probe);
        let found = rules("rules-probe").unwrap();
        assert_eq!(found.part_model("rules-probe:part"), Some("rules-probe:part/model"));
        assert_eq!((found.part_model("ae2:cable_interface"), found.bus_operations_multiplier("x")), (None, 1));
        assert_eq!(RULES.lock().unwrap().iter().filter(|r| r.id() == "rules-probe").count(), 1);
    }
}
