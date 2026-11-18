use super::*;

impl Game<'_> {
    /// Whether a mixin bundle listens to `native_entity_join_level` or `native_entity_leave_level`:
    /// one that hooks them hooks this one too and sets `listened` (HEAD, `*listened = true`). A
    /// trait mod needs not: any counts. Without either the game walks no entity list to tell them.
    /// Asked each tick and around each pass that removes entities: keep it short. Server-side.
    pub(in crate::game) fn native_entity_level_listened(&self, listened: &mut bool) {
        self.fire_entity_level_listened(listened);
    }
}
