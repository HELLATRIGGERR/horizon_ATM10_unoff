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
    // native_hooks/shared/loot.rs `native_block_loot`, `native_use_drops`; native_hooks/living_death.rs
    // `capture_hit`; mobs/creatures/death_weapon.rs: Mystical Agriculture's crops have no loot table, their drops are
    // the bundle's.
    (HookId::LootDrops, "loot_drops"),
    // network/lan_actions/tick.rs `guest_player_tick`; network/remote/mod.rs.
    (HookId::PlayerTickPre, "player_tick_pre"),
    (HookId::PlayerTickPost, "player_tick_post"),
    // items/use_hand.rs: the block's native_use_item_on (a part hook + block_listens, `block_heard`): Just Dire
    // Things' Paradox Machine takes a bucket.
    (HookId::UseItemOn, "use_item_on"),
    // items/use_hand.rs: the block's native_use_without_item (`block_heard`): Just Dire Things' machines pass a
    // click with the Ferricore Wrench on to the item's `useOn` instead of opening their menu.
    (HookId::UseWithoutItem, "use_without_item"),
    // native_hooks/shared.rs `use_item_on_block`: the broadcast `UseItemOnBlockEvent` of each phase.
    (HookId::UseItemOnBlock, "use_item_on_block"),
    // modding/api_values.rs `api_menu_quick_move`.
    (HookId::MenuQuickMove, "menu_quick_move"),
    // modding/api_values.rs `api_block_entity_update_tag`.
    (HookId::BlockEntityUpdateTag, "block_entity_update_tag"),
    // modding/api_values.rs `api_block_entity_submit`.
    (HookId::BlockEntitySubmit, "block_entity_submit"),
    // native_hooks/shared.rs `screen_char_canceled`.
    (HookId::ScreenCharTypedPre, "screen_char_typed_pre"),
    // native_hooks/shared.rs `item_use_first` (a part hook).
    (HookId::OnItemUseFirst, "on_item_use_first"),
];

/// Whether `injected` (the hooks of applied injections) holds the gated hook `hook`.
fn injected_in(injected: &[&str], hook: HookId) -> bool {
    GATES
        .iter()
        .any(|&(id, name)| id as usize == hook as usize && injected.contains(&name))
}

/// Whether a bundle applied to this tree injects into the gated hook `hook`.
pub(in crate::game) fn injected(hook: HookId) -> bool {
    #[cfg(test)]
    if TEST_INJECTED.with_borrow(|names| injected_in(names, hook)) {
        return true;
    }
    injected_in(staged::INJECTED, hook)
}

/// A block-scoped part hook as upstream gates it (`has_part_hook` and a part on `state`'s block,
/// `block_listens`), or a bundle injects into `hook`: an injection is not scoped to a block, so
/// every block's use asks it and the bundle's helper picks its own blocks.
pub(in crate::game) fn block_heard(hook: HookId, state: horizon_world::registry::BlockStateId) -> bool {
    (mod_api::__private::has_part_hook(hook)
        && crate::game::api_values::block_state::block_listens(hook, state))
        || injected(hook)
}

#[cfg(test)]
thread_local! {
    /// The hooks a unit test says a bundle injects into ([`with_injected`]), beside the stage's.
    static TEST_INJECTED: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs `f` as if a staged bundle injected into `hooks` (catalog names), as a unit test can't
/// apply one.
#[cfg(test)]
pub(in crate::game) fn with_injected<R>(hooks: &[&'static str], f: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_INJECTED.with_borrow_mut(Vec::clear);
        }
    }
    TEST_INJECTED.with_borrow_mut(|names| *names = hooks.to_vec());
    let _reset = Reset;
    f()
}

/// A bundle as a unit test injects it: the hook points it stands for call it first (under
/// `cfg(test)`), as horizon-mixins puts a HEAD helper first. It answers by block name and
/// records each call, so a test sees which hooks a click reached and in what order.
#[cfg(test)]
pub(in crate::game) mod test_bundle {
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    pub(in crate::game) struct Bundle {
        /// `use_item_on`'s answer, by block.
        pub use_item_on: HashMap<String, mod_api::InteractionResult>,
        /// `use_without_item`'s answer, by block.
        pub use_without_item: HashMap<String, mod_api::InteractionResult>,
        /// The blocks `fluid_interact_on_use` takes containers on.
        pub fluid_interact_on_use: Vec<String>,
        /// Each hook asked, with the block.
        pub calls: Vec<(&'static str, String)>,
    }

    thread_local! {
        static BUNDLE: RefCell<Option<Bundle>> = const { RefCell::new(None) };
    }

    /// Runs `f` with `bundle` injected (its hooks and `with_injected`'s gates), and hands the
    /// bundle back with its calls.
    pub(in crate::game) fn with_bundle<R>(bundle: Bundle, f: impl FnOnce() -> R) -> (R, Bundle) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                BUNDLE.with_borrow_mut(|b| *b = None);
            }
        }
        BUNDLE.with_borrow_mut(|b| *b = Some(bundle));
        let _reset = Reset;
        let out = super::with_injected(
            &["use_item_on", "use_without_item", "use_item_on_block"],
            f,
        );
        let bundle = BUNDLE.with_borrow_mut(Option::take).expect("the bundle");
        (out, bundle)
    }

    fn answer(
        hook: &'static str,
        block: &str,
        pick: impl FnOnce(&Bundle) -> Option<mod_api::InteractionResult>,
        answer: &mut Option<mod_api::InteractionResult>,
    ) {
        BUNDLE.with_borrow_mut(|b| {
            if let Some(b) = b {
                b.calls.push((hook, block.to_owned()));
                if answer.is_none() {
                    *answer = pick(b);
                }
            }
        });
    }

    pub(in crate::game) fn use_item_on(block: &str, out: &mut Option<mod_api::InteractionResult>) {
        answer("use_item_on", block, |b| b.use_item_on.get(block).copied(), out);
    }

    pub(in crate::game) fn use_without_item(block: &str, out: &mut Option<mod_api::InteractionResult>) {
        answer("use_without_item", block, |b| b.use_without_item.get(block).copied(), out);
    }

    pub(in crate::game) fn fluid_interact_on_use(block: &str, takes: &mut bool) {
        BUNDLE.with_borrow_mut(|b| {
            if let Some(b) = b {
                b.calls.push(("fluid_interact_on_use", block.to_owned()));
                *takes |= b.fluid_interact_on_use.iter().any(|n| n == block);
            }
        });
    }
}

/// `listened`, or a bundle injects into `hook`.
pub(in crate::game) fn heard(hook: HookId) -> bool {
    mod_api::__private::listened(hook) || injected(hook)
}

/// `has_part_hook`, or a bundle injects into `hook`.
pub(in crate::game) fn part_heard(hook: HookId) -> bool {
    mod_api::__private::has_part_hook(hook) || injected(hook)
}

/// Whether horizon-mixins applied any bundle to this tree. The HUD atlas packs the mod image
/// slot for it as for a trait mod (`runtime/resource_tables.rs` `mod_image_slot`): fork main asked
/// `built_with_native_mods`, which the merge dropped, and a bundle's `mod_image` (FTB Chunks' maps)
/// needs the slot.
pub(in crate::game) fn bundles_staged() -> bool {
    !staged::INJECTED.is_empty()
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
    fn a_bundles_loot_drops_opens_the_loot_gate() {
        assert!(injected_in(&["loot_drops"], HookId::LootDrops));
        assert!(!injected_in(&["right_click_block"], HookId::LootDrops));
    }

    #[test]
    fn the_block_use_gates_open_for_a_bundles_injection() {
        assert!(injected_in(&["use_item_on"], HookId::UseItemOn));
        assert!(injected_in(&["use_without_item"], HookId::UseWithoutItem));
        assert!(injected_in(&["use_item_on_block"], HookId::UseItemOnBlock));
        assert!(!injected_in(&["use_item_on"], HookId::UseWithoutItem));
        assert!(!injected_in(&["use_on"], HookId::UseItemOn));
        // Every block asks an injection, with no part on it: the bundle picks its blocks itself.
        let any = horizon_world::registry::BlockStateId(1);
        with_injected(&["use_item_on", "use_without_item", "use_item_on_block"], || {
            assert!(block_heard(HookId::UseItemOn, any));
            assert!(block_heard(HookId::UseWithoutItem, any));
            assert!(heard(HookId::UseItemOnBlock));
        });
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