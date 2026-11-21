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
(`crates/horizon-client-render/src/jei_recipes`, MIT; byte for byte JEI's own). The recipe window's sprites come
from the player's jar through the carry: `bun run pack/scripts/jei.ts carry` runs `horizon-port-pack carry <work> jei`,
which writes JEI's `assets/jei` into `<work>/mods/jei/carry`, a content mod whose GUI textures the game names
`jei:gui/sprites/...`; `tools/atm10/play-atm10.ts` loads it as every carry, and the world tests take it from
`JEI_CARRY`. Without it the window draws fills in the same sizes. Nothing of it is committed.

| File | What it is |
|---|---|
| `jei-core/` | the list's order and JEI's search (tokenizer, prefixes, modes), with unit tests |
| `jei-core/src/recipes.rs` | the recipe screen: lookups, categories, JEI's crafting grid, history, the page drawn |
| `runtime/jei_world.rs` | the glue: the game's items in, the panel out, Ctrl+O, cheat mode, settings, `/jeiclient` |
| `runtime/jei_recipes.rs` | the recipe screen's glue: the tables and the mods' categories in, the page and catalysts out |
| `hooks/` | the named hook points it uses: `render_frame_pre`, `panel_event`, `screen_key_pressed_pre`, `mods`, `settings`, `settings_click`, `register_client_commands`, `command`, `recipe_lookup`, `server_started`, `world_lifecycle` |
| `tests/item-list.ts` | the world test: the list in the inventory, searches, pages, Ctrl+O, a screenshot |
| `tests/recipes.ts` | the world test: R, U, tabs, catalysts, clicks on ingredients, Backspace, screenshots |

Build and test: `bun run pack/scripts/jei.ts units stage build world`.

## On upstream's hooks (merge of nightly-source/2026-11-18)

- `frame` -> `render_frame_pre`, `commands` -> `register_client_commands`; the bookmarks learn the open world's
  folder at `server_started` and forget it at `world_lifecycle`'s close phases.
- `panel_event` carries upstream's `PanelEvent`, whose stacks are `ItemKey`s: `jei_world::NativePanelEvent` is the
  fork's shape again (`NativePanelEvent::of`), the stack's id read from its key (a fluid or a mod's ingredient on the
  panel arrives as the item drawn for it; upstream has no `Catalyst` event). A `Key` over the list names its entry
  by `index`, as `Clicked` does: a fluid or a chemical there is read from JEI's own list by it.
- The mod list row (`mods`), the settings page (`settings`, `settings_click`) are the fork's compat hooks, which
  nothing in the merged engine fires: upstream declares mod settings as data (`content/mod_settings.rs`,
  `setting_changed`). Until JEI's settings are declared that way, its page is missing from Options > Mods.
- `native_text_width` takes the batch now: the window measures with `HudBatch::text_width` on the GUI atlas
  (`jei_window::text_width`); `native_sprite` takes a tint (`None`).
