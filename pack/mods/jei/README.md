# JEI (native bundle)

Just Enough Items 19.70.0.464, as ATM10 ships it, for the pack's port. Plan and slices: `docs/jei.md`.

Slice 1, the ingredient list: every item beside every container screen, the inventory and the creative inventory,
in JEI's order (Minecraft first, then the mods by name), in pages, with JEI's search syntax and search modes
(`jei-core`), Ctrl+O to hide and show it, and cheat mode (a click gives one, a right click a stack; creative). The
settings are on the mod's page in Options > Mods (the panel's config button opens it), kept in
`horizon-mods/.horizon-mod-data/jei/` under JEI's config names (`cheatItemsEnabled`, `overlayEnabled`,
`modNameSearchMode`...).

Slice 2, the recipe screen: R over an item of the list, of the inventory or of a recipe shows its recipes, U its uses
(a click on the list or on a recipe's ingredient does the same: left recipes, right uses), one tab per category in
JEI's order (vanilla's crafting, stonecutting, smelting, smoking, blasting, campfire, smithing, brewing, then the mods'
categories: Mekanism, AE2, ExtendedAE, Advanced AE), the category's catalysts in a column beside its recipes, the
page arrows with the page count, chance outputs after the result with their odds and the recipe's notes under it.
Backspace (and Escape, and the Back button) goes back through the history; U on a machine shows its own category
first. Other mods' screens reach it through `recipe_lookup` (Mekanism's progress arrows). The recipe screen is JEI's own
window (slice 2b): centred over the screen beside the item list, the tabs along its top, the catalysts on its left,
the category title and the "x/y" page row with their arrows, as many recipes as fit by height; Escape closes it
(the inventory stays open), the wheel and Page Up/Down turn its pages.

JEI is MIT (mezz, https://github.com/mezz/JustEnoughItems). Its behaviour is reimplemented here from what the mod
does; no file of it is in the repository. The panel's buttons and search box use JEI's artwork the engine carries
(`crates/horizon-client-render/src/jei_recipes`, MIT); further textures and the lang come from the player's jar at
run time through the carry.

| File | What it is |
|---|---|
| `jei-core/` | the list's order and JEI's search (tokenizer, prefixes, modes), with unit tests |
| `jei-core/src/recipes.rs` | the recipe screen: lookups, categories, JEI's crafting grid, history, the page drawn |
| `runtime/jei_world.rs` | the glue: the game's items in, the panel out, Ctrl+O, cheat mode, settings, `/jeiclient` |
| `runtime/jei_recipes.rs` | the recipe screen's glue: the tables and the mods' categories in, the page and catalysts out |
| `hooks/` | the named hook points it uses: `frame`, `panel_event`, `screen_key_pressed_pre`, `mods`, `settings`, `settings_click`, `commands`, `command`, `recipe_lookup` |
| `tests/item-list.ts` | the world test: the list in the inventory, searches, pages, Ctrl+O, a screenshot |
| `tests/recipes.ts` | the world test: R, U, tabs, catalysts, clicks on ingredients, Backspace, screenshots |

Build and test: `bun run pack/scripts/jei.ts units stage build world`.
