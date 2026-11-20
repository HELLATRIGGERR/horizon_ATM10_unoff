//! N5: the hooks a staged mixin bundle injects into count as heard at the engine's listener gates.
//!
//! Upstream skips some hook targets unless a trait mod listens (`listened`, `has_part_hook`,
//! `item_listens`): the call is not built, so the target never runs. A mixin bundle injects its
//! code into the target itself, and no trait mod listens for it, so its hook would never run. The
//! gates in [`GATES`] ask [`heard`], [`part_heard`] or [`item_heard`] instead: the trait-mod
//! answer, or true when horizon-mixins applied an injection on that hook to this tree.
//!
//! What was applied comes from horizon-mixins' own record of a stage, `.horizon-mixins-stage.json`
//! at the workspace root: the build script lists its injections' hooks in
//! `$OUT_DIR/bundle_hooks.rs`. A checkout has no record, so its list is empty and every gate is
//! upstream's. A gated target a bundle starts injecting into needs a row in [`GATES`] and its gate
//! changed to one of the three predicates.
use mod_api::HookId;

mod staged {
    include!(concat!(env!("OUT_DIR"), "/bundle_hooks.rs"));
}

/// The listener-gated hook targets bundles inject into, with their catalog names (the `hook`
/// of a bundle's `[[injection]]`), and where each gate is.
const GATES: &[(HookId, &str)] = &[
    // native_hooks/shared.rs `right_click_block`.
    (HookId::RightClickBlock, "right_click_block"),
    // native_hooks/shared.rs `place_canceled`, `player_place_canceled`; blocks/dispenser.rs.
    (HookId::EntityPlace, "entity_place"),
    // native_hooks/shared.rs `incoming_damage`; mobs/creatures/death_weapon.rs.
    (HookId::LivingIncomingDamage, "living_incoming_damage"),
    // native_hooks/use_item_hand.rs `entity_interaction`.
    (HookId::EntityInteract, "entity_interact"),
    // native_hooks/use_item_hand.rs `use_item_in_hand` (a part hook).
    (HookId::UseItem, "use_item"),
    // entities/explosion_detonate.rs.
    (HookId::ExplosionDetonate, "explosion_detonate"),
    // native_hooks/on_block_placed_by.rs `block_item_placed` (an item part hook).
    (HookId::OnBlockPlacedBy, "on_block_placed_by"),
    // native_hooks/shared.rs `screen_render_listened`: no screen view is built without it.
    (HookId::ScreenRenderPre, "screen_render_pre"),
    (HookId::ScreenRenderBackground, "screen_render_background"),
    (HookId::ScreenRenderForeground, "screen_render_foreground"),
    (HookId::ScreenRenderPost, "screen_render_post"),
    // network/remote/client_level_tick.rs: the client's level tick.
    (HookId::LevelTickPre, "level_tick_pre"),
    (HookId::LevelTickPost, "level_tick_post"),
    // native_hooks/use_item_hand.rs `item_on_interaction` (a part hook): Iron Furnaces' upgrades and augments.
    (HookId::UseOn, "use_on"),
    // native_hooks/shared.rs `BLOCK_CONTACT_HOOKS` (`any_inside`, a scoped hook; `inside_listens`, per block):
    // Occultism's spirit fire. An injection is not scoped to a block, so every block an entity is inside asks it.
    (HookId::EntityInside, "entity_inside"),
    // modding/api_values.rs `api_menu_button` (a part hook): HNN's fabricator palette and every machine screen's
    // buttons of a bundle (Mekanism's too).
    (HookId::MenuButton, "menu_button"),
];

/// Whether `injected` (the hooks of applied injections) holds the gated hook `hook`.
fn injected_in(injected: &[&str], hook: HookId) -> bool {
    GATES
        .iter()
        .any(|&(id, name)| id as usize == hook as usize && injected.contains(&name))
}

/// Whether a bundle applied to this tree injects into the gated hook `hook`.
pub(in crate::game) fn injected(hook: HookId) -> bool {
    injected_in(staged::INJECTED, hook)
}

/// `listened`, or a bundle injects into `hook`.
pub(in crate::game) fn heard(hook: HookId) -> bool {
    mod_api::__private::listened(hook) || injected(hook)
}

/// `has_part_hook`, or a bundle injects into `hook`.
pub(in crate::game) fn part_heard(hook: HookId) -> bool {
    mod_api::__private::has_part_hook(hook) || injected(hook)
}

/// `item_listens` for `item`, or a bundle injects into `hook` (for every item: an injection is
/// not scoped to one).
pub(in crate::game) fn item_heard(hook: HookId, item: &str) -> bool {
    mod_api::__private::item_listens(hook, item) || injected(hook)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gate_opens_for_an_injected_hook_only() {
        let injected = ["right_click_block", "server_tick", "mods"];
        assert!(injected_in(&injected, HookId::RightClickBlock));
        assert!(!injected_in(&injected, HookId::EntityPlace));
        // A hook outside the gated set is never asked about, even when injected.
        assert!(!injected_in(&injected, HookId::ServerTick));
        assert!(!injected_in(&[], HookId::RightClickBlock));
    }

    #[test]
    fn every_gate_names_a_vocabulary_hook() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../horizon-mod-vocab/src/hooks");
        let mut declared = String::new();
        for entry in std::fs::read_dir(&dir).expect("the vocabulary's hooks") {
            let path = entry.expect("a hooks entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                declared.push_str(&std::fs::read_to_string(path).expect("a hook file"));
            }
        }
        for (_, name) in GATES {
            assert!(
                declared.contains(&format!("fn {name}(")),
                "{name} is no vocabulary hook"
            );
        }
    }
}