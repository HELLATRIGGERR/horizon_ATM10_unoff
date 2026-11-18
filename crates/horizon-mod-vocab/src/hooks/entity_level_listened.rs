//! The `entity_level_listened` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// Whether a mixin bundle listens to `native_entity_join_level` or `native_entity_leave_level`:
    /// one that hooks them hooks this one too and sets `listened` (HEAD, `*listened = true`). A
    /// trait mod needs not: any counts. Without either the game walks no entity list to tell them.
    /// Asked each tick and around each pass that removes entities: keep it short. Server-side.
    #[hook(
        side = "server",
        contract = "collect",
        out = "listened",
        neoforge = "none (the fork's gate for entity_join_level and entity_leave_level)"
    )]
    fn entity_level_listened();
}
