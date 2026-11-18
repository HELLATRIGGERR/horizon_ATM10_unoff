use super::*;
#[path = "book_screen/open.rs"]
mod open;
#[path = "book_screen/placement.rs"]
mod placement;

#[cfg(test)]
#[path = "book_screen/lifecycle_tests.rs"]
mod lifecycle_tests;

impl Game<'_> {
    /// The mod screen showing now, if any.
    pub(in crate::game) fn shown_native_screen(&self) -> Option<&OpenNativeScreen> {
        // An extra screen can step aside for a menu of its own in the window.
        let editing = self.extra_screen_stepped_aside();
        self.mod_screen
            .as_ref()
            .filter(|s| self.screen == Screen::Inventory && self.container == Some(s.key))
            .filter(|_| !editing)
    }
    /// The extra screen showing now, if any (`screen_extras`).
    pub(in crate::game) fn mod_screen_extra(&self) -> Option<&super::super::screen_extras::Extra> {
        self.shown_native_screen().map(|s| &s.screen_extra)
    }
    /// The data slots of the mod screen showing now.
    pub(in crate::game) fn mod_screen_data(&self) -> Vec<i32> {
        self.shown_native_screen()
            .map(|s| s.data.clone())
            .unwrap_or_default()
    }
    /// The open mod screen's block and extra screen, also while the extra
    /// screen steps aside for a menu of its own.
    pub(in crate::game) fn open_native_screen_extra(
        &self,
    ) -> Option<(&str, &super::super::screen_extras::Extra)> {
        self.mod_screen
            .as_ref()
            .filter(|s| self.screen == Screen::Inventory && self.container == Some(s.key))
            .map(|s| (s.block.as_str(), &s.screen_extra))
    }
    /// Whether a mod's screen places the current container and player slots.
    pub(in crate::game) fn native_screen_shown(&self) -> bool {
        self.shown_native_screen().is_some()
    }
    /// The open mod screen's height.
    pub(in crate::game) fn native_screen_height(&self) -> Option<f32> {
        self.shown_native_screen().map(|s| s.screen.height)
    }
    pub(in crate::game) fn native_screen_menu(&self) -> Option<&str> {
        let screen = self.shown_native_screen()?;
        self.remote_menu_type()
            .or_else(|| self.mod_menu_at(screen.key))
    }
    pub(in crate::game) fn native_screen_toggle(&self) -> Option<[f32; 2]> {
        self.shown_native_screen()?.screen.recipe_book_toggle
    }
    pub(in crate::game) fn native_screen_block(&self) -> Option<&str> {
        Some(&self.shown_native_screen()?.block)
    }
    /// The explicitly assigned book; declarations bind it to the actual menu.
    pub(in crate::game) fn native_screen_book(&self) -> Option<&mod_api::RecipeBookTypeKey> {
        self.shown_native_screen()?.screen.recipe_book.as_ref()
    }
}
