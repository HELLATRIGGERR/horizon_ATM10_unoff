use super::*;

impl Game<'_> {
    /// A mod asked the recipe viewer to show `stack` (`native_show_recipes`): its recipes, its
    /// uses, or what a key over it does in the viewer (`NativeRecipeLookup`). A recipe viewer acts
    /// on it and sets `handled`, so no other viewer does. Heard at the start of the frame after the
    /// ask, once for each, in the order asked. Client-side.
    pub(in crate::game) fn native_recipe_lookup(
        &mut self,
        stack: &mod_api::NativeStack,
        lookup: &mod_api::NativeRecipeLookup,
        handled: &mut bool,
    ) {
        self.fire_recipe_lookup(stack, lookup, handled);
    }
}
