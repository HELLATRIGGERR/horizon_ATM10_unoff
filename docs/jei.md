# JEI (Just Enough Items) port plan

> **Handoff (2026-11-17): slice 6 ("the list's size, mod names and blacklist from ATM10's config") done and tested.**
> **Slice 6 as built:**
> - Engine helper `native_panel_grid(mod_id, NativePanelGrid)`, **API 434** (`native_panel.rs`; types `NativePanelGrid
>   {max_columns, max_rows, horizontal, vertical, keep_search}` and `NativePanelAlign {Start, Center, End}` with
>   `parse` (JEI's LEFT/TOP, CENTER, RIGHT/BOTTOM) and `offset` in `mod_api.rs`; `native-hooks.json` helper
>   `panel_grid`; `NATIVE_HOOKS_API = 434`; `docs/atm10-port.md` and `docs/native-mods.md` list it). Kept per mod for
>   the session. `browser_layout` takes what fits up to the limits (`fit_columns`, `fit_rows`) and aligns the grid in
>   the free width and the page bar + grid in the height over the search box; the viewer style's area is now `[grid x,
>   0, grid width, h]` (the same as before for the right alignment). Without a call: Horizon's Recipes panel keeps 9 x
>   16 at the right, the recipe book style 12 columns centred. `keep_search` keeps the panel (page bar, search box)
>   with no items: `show_items`/`show_recipes` no longer drop it. Unit tests `a_mods_grid_takes_what_fits_up_to_its_limits`,
>   `a_mod_that_keeps_its_search_box_shows_an_empty_panel`.
> - Bundle: `jei_world::grid` sends `[ingredientList] maxColumns/maxRows/horizontalAlignment/verticalAlignment` at
>   world load and on each config change (ATM10: 18 x 99, RIGHT, TOP), with `keep_search` = the overlay is on (not hidden by Ctrl+O), so the box stays with no items as JEI's does (a search
>   or a config change that leaves nothing; before, a refilter from the config screen to nothing dropped the panel);
>   `show` sends no items while `hideIngredientsWhenNotSearching` is on and the query is empty. Air is left out of the
>   list (it was its first, empty slot). The bookmark panel
>   aligns with `[bookmarkList] horizontalAlignment/verticalAlignment` (LEFT, TOP: it now sits at the left edge, it was
>   centred).
> - `jei-core/src/mod_id_format.rs`: `config/jei/jei-mod-id-format.ini` (`[modname] modNameFormat`, default `blue
>   italic`, the codes 9 and o; empty = no line); written in MezzConfig's layout when missing (ATM10's instance has none; its
>   `config/jei` holds only `jei-client.ini` and `blacklist.json`, `defaultconfigs` and `kubejs` have no JEI config).
>   New `tooltip` injection (`hooks/tooltip.rs`): `jei_world::tooltip` puts the mod name (`jei_core::mod_name`) last in
>   every item tooltip, not twice. Format kept in its own lock (`MOD_NAME_FORMAT`), read at world load.
> - `jei-core/src/blacklist.rs`: `config/jei/blacklist.json` (version 2, read only, at world load): `(hide_mode, type,
>   ingredient)`; item types (`item_stack`, `net.minecraft.world.item.ItemStack`) hide their id from the list and the
>   search (`jei_world::filter`), WILDCARD all variants, another mode only the plain id; other types are kept (ATM10's
>   file = 14 Mekanism chemicals, which are not ingredients of the list yet). `[cheating] showHiddenIngredients` shows
>   them again.
> - Probe `/jeiclient`: `displayed`, `area`, `blacklist {entries, items}`, `modNameFormat`, `hideUnsearched`; `set
>   <section> <key> <value>` sets a config value (dev only).
> - World test `tests/list-size.ts` (`JEI_LIST=jei/list-size bash jei.sh world`, 4 checks, shots in
>   `jei-work/logs/shots-list-size`): a 1920x1080 window at GUI scale 2 (the test sets `AE2_TEST_DISPLAY_SCALE` to half
>   the display's and puts `guiScale:2` in `options.txt`; 960x540 GUI): the list takes 18 columns (area `630 0 324
>   540`, 26 rows, 5 pages); a new `jei-mod-id-format.ini` with `blue italic`, the line under Stone's tooltip; a
>   `blacklist.json` (4 of ATM10's chemicals and bedrock) hides bedrock from the search until `[cheating]
>   showHiddenIngredients`; `hideIngredientsWhenNotSearching` leaves the search box (and wrench) with no items, a
>   search shows them, a clear empties it again. Shots checked by eye: 18 columns from the creative inventory's right
>   edge to the screen's, the page bar and search box as wide; "Stone" over a blue italic "Minecraft"; the empty panel
>   = the search box alone.
> - Green after the slice: `jei/list-size` (4 checks), `jei/item-list` (11), `jei/recipes` (9), `jei/bookmarks` (6),
>   `jei/transfer` (4), `jei/transfer-ae2` (4), `jei/config` (4), `jei.sh units` (22); engine `cargo test --profile iter
>   -p horizon-client --lib panel` 5 pass (the two new ones among them); `horizon-mod-api-gen` 16 pass and the one that
>   fails on main (`new_hook_placeholder_is_reported_then_filled`). The bookmark shots show the list at the left edge.
> **Gaps left:** `ingredientList.buttonNavigationVisibility` and `drawBackground`, the `lookups`/`tooltips` options and
>   the rest of `OPTS` still change nothing; a JEI-window recipe page doesn't narrow the list (JEI lays its list out
>   right of the recipe GUI); creative tabs' contents, fluids/chemicals as ingredients (the blacklist's chemicals wait
>   for them) and item colours still wait for the game (see API gaps below).

> **Handoff (2026-11-17): slice 5 ("JEI's config screen") done and tested; the ini-driven list layout, mod-name format,
> blacklist and hide-until-search are next.**
> **Slice 5 as built:**
> - `jei-core/src/config.rs` (20 unit tests in `jei-core`): `Ini::parse/write/get/set/value/complete/fresh` for
>   `config/jei/jei-client.ini` in JEI 19.70's format (MezzConfig: `[section]`, tab-indented `# Name:`, `# Description:`,
>   `# Valid Values:`, `# Default Value:` over `key = value`, a blank line between values, two between sections).
>   Comments, order and unknown keys are kept; missing values are added with JEI's comments; legacy values (an older
>   JEI's, e.g. ATM10's `[appearance] centerSearch`) are taken over by `LEGACY`. `OPTS` = 63 options of JEI 19.70
>   (`ClientConfig`, `IngredientFilterConfig`, `IngredientGridConfig`, the shared grid config; names and descriptions
>   from JEI's en_us `jei.config.client.<section>.<key>`), `CATEGORIES` (10, the screen's order), `Opt::cycle/label/valid`.
>   `ATM10` = the pack's own overrides, applied to a new file (`[ingredientList] maxRows = 99, maxColumns = 18`).
>   `bookmarks.rs`: `toggle_at(item, front)` and `grid_within(area, cols, rows)`.
> - `runtime/jei_config.rs` (`[[module]] config` in `horizon-mixin.toml`): the ini is read once and again when a world
>   loads (`reload()`), written `.tmp` + rename on each change; `CHANGED` -> `jei_world::frame` refilters and re-reads
>   Ctrl+O. Cheat mode and the bookmark overlay are session-only, as JEI 19.70's `ClientToggleState` (the old
>   `.horizon-mod-data/jei/*` setting files are no longer read). The screen is drawn over the container screen from
>   `screen_render_post`: JEI's `gui_background_v2`, `button_*_v2` category buttons (the open one pressed), the title on
>   the navigation band, the game's `widget/button` for each value, tooltip = name + description + "Default Value".
>   Click = next value, right click = previous, Shift = ten for numbers, wheel scrolls, "Done" and Escape close. The
>   wrench (the engine's settings button right of the search box, rect `[a.x + a.w - 20, a.h - 26, 20, 20]` from
>   `native_panel_area`) is taken in `screen_mouse_button_pressed_pre`; the next frame opens the screen, or with Ctrl
>   toggles cheat mode (`toggleCheatModeConfigButton`); JEI's `icons/config_button(_cheat)` is drawn over it. Widgets
>   recorded: `JEI Config: <category>`, each option's name, `Done`. Probe `/jeiclient` -> `config {file, cheat,
>   bookmarks, wrench, screen {category, frame, rows [{name, value}]}}`.
> - Wired: the six search modes (`[search]`), cheat mode, Ctrl+O (`ingredientList.overlaysEnabled`), bookmarks
>   `addBookmarksToFrontEnabled` (FRONT/END), `bookmarkList.maxColumns/maxRows` (panel grid), the bookmark overlay
>   toggle; Options > Mods goes through the same values. Keys `key.jei.toggleCheatMode` and
>   `key.jei.toggleBookmarkOverlay` (unbound, as JEI 19.70) are in `KEYS`. The other options are kept and shown but
>   change nothing yet. No engine change in this slice (no new hook; fork API still **433**).
> - JEI's real wrench in ATM10 shows "Install the MezzConfig GUI mod" in chat (the standalone MezzConfig has no
>   screen); the port draws the screen as the brief asks.
> - World test `tests/config.ts` (`JEI_LIST=jei/config bash jei.sh world`, 4 checks, shots in `jei-work/logs/shots-config`):
>   a new ini in JEI's format with ATM10's `maxRows`/`maxColumns`; the wrench opens the screen on Search; a right click on
>   "Search Ingredient Tags" (Require Prefix -> Enabled) is in the file at once with its comments and "ingots" finds more;
>   Cheats -> "Enable Cheat Mode" ON; Done and Escape close; after `fixture.restart()` the file and the search mode are
>   kept and cheat mode is off. Shots checked by eye: JEI's grey background and band, the nine category buttons on the
>   left, the values as vanilla buttons with the label ("Require Prefix", "ON"), the tooltip over the Cheats page, the
>   wrench drawn as a red wrench in JEI's button with cheat mode on (grey off). Cosmetic: the page note "1-9 / 14" on
>   Search sits against the Done button.
> - Green after the slice: `jei/item-list` (11 checks), `jei/recipes` (9), `jei/bookmarks` (6), `jei/transfer` (4),
>   `jei/transfer-ae2` (4), `jei.sh units` (20).
> **Next was (a)-(d), the ini-driven list layout, mod-name format, blacklist and hide-until-search: done in slice 6.**

> **Handoff (2026-11-17): slice 4 ("+" recipe transfer: vanilla grids 4a, AE2's terminals 4b) done and tested.**
> **Slice 4a as built:**
> - Engine helper `native_recipe_transfer(&NativeRecipeTransfer, run, max, craft_missing) -> NativeTransferAnswer`,
>   **API 433** (`crates/horizon-client/src/game/native_recipe_transfer.rs`, new `mod` in `game.rs`; types
>   `NativeRecipeTransfer`, `NativeTransferHandler {CraftingGrid, Furnace, Ae2Encoding, Ae2Crafting}`,
>   `NativeTransferAnswer {handler, too_large, missing, craftable, done}` in `mod_api.rs`; `native-hooks.json` helper
>   `recipe_transfer`; `NATIVE_HOOKS_API = 433`; `docs/atm10-port.md` lists it). `recipe_book.rs`'s
>   `mod_crafting_open` became `pub(super)`. Vanilla handlers: crafting grid (inventory 2x2 / table / mod grid; JEI
>   counts inventory + grid; too large = recipe's used cells > grid; run = `transfer_viewer_recipe(book_id, max)`),
>   furnace/blast furnace/smoker (only its own recipe type; slot 300 input; run = quick-move the input out, pick the
>   ingredient up, right click (one) or left click (Shift, all) on 300, put the rest back). `craft_missing` (Ctrl) is
>   AE2's (4b).
> - Bundle: `jei_window.rs` draws JEI's "+" (13 px, 2 px right of the recipe, bottom aligned:
>   `RecipeLayout.RECIPE_BUTTON_SIZE/SPACING`), `button_enabled/highlight/disabled_v2` + `icons/recipe_transfer`;
>   hidden with no handler; grey when missing/too large (`button_state`); on hover red `0x66FF0000` over missing slots,
>   blue `0x400000FF` over AE2-craftable ones, tooltip "Move Items" + red "Missing Items" / "Recipe is too large to
>   craft in the 2x2 player crafting grid." (AE2: "Encode pattern", "Move items", "Missing items will be skipped",
>   "CTRL + click to craft unavailable items", "Found no compatible items"; COSMETIC orange `0x80FFA500` / blue
>   `0x804545FF` button tint). Widget label `Move Items <recipe index>`. `Action::Transfer(i)`;
>   `jei_recipes::frame` asks `native_recipe_transfer` (dry run) for the shown recipes each frame
>   (`jei_window::transfers/set_answers`) and runs it on click (Shift = max, Ctrl = craft missing), closing the window
>   when done. `jei_recipes` keeps each vanilla category's native recipe indices (`State.native`) to hand the window
>   `(category id, book_id)` per recipe. The recipe x moved 7.5 px left to make room for the "+".
>   Probe `/jeiclient` -> `recipes.window.transfers` `[{recipe, handler, too_large, missing, craftable}]`, `shown`.
> - World test `tests/transfer.ts` (4 checks, green; `JEI_LIST=jei/transfer bash jei.sh world`, shots in
>   `jei-work/logs/shots-transfer`): crafting table "+" fills the grid (slot 200 = crafting table) and closes the
>   window; the chest's "+" grey with 4 missing (red slots, tooltip "Missing Items"); the chest too large for the
>   inventory's 2x2 grid; furnace: a smelting "+" moves one iron ore (the page shows 2 recipes at a time and which
>   ore comes first is not fixed, so the test holds raw iron, iron ore and deepslate ore and accepts whichever).
>   Test gotchas: `interact` needs a clear line to the block (the test clears `x-1..x`, `y..y+2`, `z-1..z+2`);
>   closing a crafting table returns the grid to the inventory. `jei/recipes`, `jei/bookmarks`, `jei/item-list`,
>   `jei.sh units` (18) and `cargo test -p horizon-mod-api-gen` stay green (but for the main-only failure
>   `new_hook_placeholder_is_reported_then_filled`).
> **Slice 4b as built (AE2's terminals, `ae2jeiintegration`'s handlers):** `ae2_recipe_transfer` in
>   `game/ae2_terminal.rs` (feature `ae2-lab`), asked first by `native_recipe_transfer`; no new hook. A remote client
>   (JEI's tests, `server-thread`) sends `ae2:jei_transfer` (JSON `JeiTransfer {window, key, run, craft, crafting,
>   inputs: [[(item, count)]], outputs}`, `key` = the recipe's hash) and the host (`lan_jei_transfer`, dispatched in
>   `lan_actions.rs` beside `ae2:clear_encoding`) answers a dry run on the same channel (`{key, missing, craftable}`,
>   kept per recipe in a client thread-local and asked again at most every 400 ms; until the first answer the "+" is
>   active with nothing marked). An in-frame terminal does both itself (`ae2_local_recipe_transfer`). Shared:
>   `JeiTransfer::{craftable_slots, missing, encode}`, `jei_best_alternative`, `jei_take`, `jei_stow`.
>   Encoding terminal (`EncodePatternTransferHandler`): always pressable, no tint, "Encode pattern" + blue
>   "Highlighted elements are already craftable" over the inputs the network can craft; run: `minecraft:crafting` ->
>   crafting mode, each 3x3 cell one item; any other category -> processing mode, inputs merged by item into the first
>   slots, outputs likewise; each slot's alternative = the one the network stores most, else craftable, else held, else
>   the first (`EncodingHelper`); host: `lan_load_encoding_state` / `lan_store_encoding_state`, local: `EncodingState`
>   + `ae2_store_local_encoding`. Crafting terminal (`UseCraftingRecipeTransfer`): `minecraft:crafting` only (else
>   hidden); dry run = `CraftingTermMenu.findMissingIngredients` (inventory first, then the network's snapshot, each
>   counted once per slot; else craftable (blue) or missing (red)); all missing -> grey, "Move Items" + red "Found no
>   compatible items"; missing -> orange `0x80FFA500`, "Move items" + red "Missing items will be skipped"; craftable ->
>   blue `0x804545FF`, blue "CTRL + click to craft unavailable items" ("Will craft unavailable items" with Ctrl down);
>   run = `FillCraftingGridFromRecipePacket`: the grid's items set aside, each cell from them, else one from the
>   network (`lan_craft_grid_extract` / `ae2_grid_extract`, the most stored first), else the inventory; leftovers to
>   the network, the inventory, the ground; Ctrl opens the craft confirmation for the first missing craftable key
>   (`lan_open_terminal_craft_confirm` / `ae2_open_local_craft_confirm`; AE2 asks for all of them in one list).
> - World test `tests/transfer-ae2.ts` (`JEI_LIST=jei/transfer-ae2`, 4 checks; `JEI_MODE=in-frame` runs it in-frame; both
>   green, shots checked by eye in `jei-work/logs/shots-transfer-ae2*`):
>   creative energy cell, drive (1k cell, 8 oak planks), crafting terminal and Pattern Encoding Terminal on cable
>   buses. The chest's "+" pulls the 8 planks from the network into the grid (slots 327-335, result 336, network 0);
>   the torch's (stick held, no coal) stays pressable, orange, coal red; the encoding terminal's "+" encodes the chest
>   (slots 300-308, 304 empty, preview 313) and an iron smelting recipe (processing: 300 the ore, 318 the ingot).
> - Also green after 4b: `jei/transfer`, `jei/recipes`, `jei/bookmarks`, `jei/item-list`, `jei.sh units` (18), AE2's
>   `terminal-craft-gestures` and `inframe-pattern-encoding` (own binary).
> **Known gaps of 4b:** components ignored (items by id); fluids and other key types are not encoded (only items);
> the dry run's craftable keys are recomputed per query (`ae2_craftable_outputs`); Ctrl crafts the first missing key
> only.
> **Known gaps of the vanilla part:** items matched by id only (components ignored); a mod crafting grid's nearby
> sources aren't counted in the dry run; JEI's bookmark star above the "+" is not drawn.

> **Slice 3 handoff (kept): bookmarks done; tests green.**
> **Known gaps:**
> - The bookmark panel shows only where the item list's `native_panel_area` exists (the inventory and creative
>   screens), not on AE2's terminals; its page arrows and wheel work only there too.
> - The recipe window covers bookmarks: while it is open the panel is dimmed with the screen (JEI keeps its overlays
>   bright), and RECIPE bookmarks (JEI's "+" on a recipe) are not drawn or made; they are kept in the file as read.
> - Bookmarks are kept per world only for the world the window's own integrated server has open: the world folder
>   reaches the bundle from the server thread's `world_lifecycle` hook (`jei_bookmarks::lifecycle`; the window's `Game`
>   has no `world_root`). Joined to a LAN or remote server they live in memory only (JEI keys those by server address).
> - Other ingredient types (fluids, chemicals) and an item's components are kept in the file but the panel draws and
>   adds plain item stacks. A reaches the bookmarks three ways: the panel's `Key` event (the list's and the screen's
>   slots), `screen_key_pressed_pre` (over a bookmark or a stack of the recipe window: `jei_bookmarks::slot_at`,
>   `jei_window::stack_at`) and `recipe_lookup` (other mods' screens).
> **Slice 3 as built:** `jei-core/src/bookmarks.rs` (18 unit tests in all, `jei.sh units`): `Bookmarks::parse_json/
> write_json` in JEI 19.70's `bookmarks.json` (`BookmarkJsonConfig`: an array, `{"version":2}` then one element each,
> `{"bookmarkType":"INGREDIENT","type":"item_stack","ingredient":{"id":"minecraft:iron_ingot","count":1}}`; elements it
> doesn't draw are kept as read), `import_ini`/`merge` for the legacy `bookmarks.ini` (`T:` + the stack as SNBT or JSON),
> `toggle` (A adds at the end, `addBookmarksToFrontEnabled = false` in ATM10, or removes), `grid(area)` (18 px slots
> under a 20 px page row, at most 9 columns x 16 rows, ATM10's `bookmarkList`). `runtime/jei_bookmarks.rs` (a
> `[[module]]` in `horizon-mixin.toml`): per world `config/jei/world/local/<world folder>/bookmarks.json`, written
> beside as `.tmp` and renamed over it on each change (as `JsonArrayFileHelper.write`); a `bookmarks.ini` there is
> imported once, merged, saved at once and renamed `bookmarks.ini.bak`; a file that isn't JEI's array goes aside as
> `bookmarks.json.bak`. The panel is drawn from `screen_render_post` left of the container screen, `[8, list.y,
> screen.x - 16, list.h]` from `native_panel_area(jei)`, as JEI's `bookmark_list_background_v2` and
> `bookmark_list_slot_background_v2` (nine-slice from the carry; fills without it), page row with JEI's arrows and "n/m"
> (13 px arrows on the 0x30000000 band), the items with `native_item_stack`, the hovered one's tooltip. Left click
> opens its recipes, right click its uses (through `jei_window::ask_open`), the wheel turns the pages, the last page
> wraps to the first. A (`key.jei.bookmark`, "Add/Remove Bookmark", `key.keyboard.a`) over a list or screen slot (panel
> `Key` event), over a stack of the recipe window or over a bookmark toggles it. Item names for the labels come from
> `native_recipes()` each time its tables change (the recipe window used to take them only on its first open).
> Hooks added: `world_lifecycle` (`hooks/world_lifecycle.rs`); probe `bookmarks` in `/jeiclient` (items, page, pages,
> area, slots, file). **Tests:** `jei/bookmarks` 6 checks (A adds iron and gold ingot, both on the left; the file in
> JEI's format; click = recipes, right click = uses; A over a window stack adds, A over a bookmark takes it off; restart
> keeps them and imports and backs up a `bookmarks.ini`; A over a bookmark removes it and the file follows),
> `jei/recipes` 9 and `jei/item-list` 11 pass; `jei.sh units` 18 pass. Screenshots in `jei-work/logs/shots-bookmarks/`
> checked by eye: the panel with JEI's grey background and slot sprites, page arrows with "1/1", the ingots and a stick,
> the tooltip, the window over it. Test traps: with nothing typed the whole list sits under y=150, so a click there
> opens an item's recipes (the test clicks above the list, y=6, then); after a restart the labels need the item names.
> **Engine:** no change in this slice.

> **Textures as built:** `bun run pack/scripts/jei.ts carry` (`jei.sh carry`) runs `horizon-port-pack carry <work> jei`
> on the player's jar: JEI's `assets/jei` (105 files) become the content mod `<work>/mods/jei/carry`, loaded as every
> carry (`--dev-mod`; `tools/atm10/play-atm10.ts` takes each `<work>/mods/<id>/carry`, the world tests `JEI_CARRY`).
> Its GUI textures are the sprites `jei:gui/sprites/...`. `jei_window.rs` draws the window with them: `gui_background_v2`,
> `tab_selected`/`tab_unselected`, `catalyst_tab_v2` with one `recipe_catalyst_slot_background_v2` round the
> catalysts (`RecipeCatalysts.draw`: inset 5), `button_*_v2`, `icons/arrow_*_v2`, `slot`, `output_slot` (26 px round
> the crafting and furnace results), `recipe_arrow`, `icons/flame`, and JEI's `pageNavigationBackground` (0x30000000,
> `colors.json`) behind the title and page rows (`RecipesGui.render`). The engine reads `.mcmeta` only for vanilla's
> sprites, so the nine-slice borders (from the jar's `.mcmeta`) are constants there and the bundle tiles them itself
> (`nine`). Without the carry everything falls back to the fills and the game's sprites. Recipes sit as JEI's
> categories (javap): crafting 116 x 54 with the arrow at 61 and the result at 95; the furnace 82 x 54, arrow (26, 17),
> result (61, 19). The item list's nav bar and search box were JEI's already: the engine's copies in
> `crates/horizon-client-render/src/jei_recipes` are byte for byte the jar's. Not animated: the furnace's arrow and
> flame are drawn full. No engine change. Shots: `jei-work/logs/shots-recipes/` (`r-iron-ingot`, `smelting`,
> `u-inscriber`), `shots-item-list/`.
> **Slice 2b as built:** `jei-core/src/window.rs` places JEI's `RecipesGui` from JEI's own sizes (javap of
> `RecipesGui`, `RecipeGuiTab`, `RecipeCatalysts`, `RecipeGuiSizing`): 198 px wide at least (wider for a wider
> recipe), height `clamp(screen - 58, 175, 350)`, centred and moved left of the item list (`native_panel_area`); 24 px
> tabs along the top overlapping it by 3, a page of `(width - 4) / 24` of them holding the selected one; the catalysts'
> tab on the left overlapping by 6, in as many columns as needed; the title row and the "x/y" page row with 13 px
> arrows; recipes placed as JEI's vanilla categories (3x3 grid, arrow, result; the furnace's input over its flame,
> 82 px wide, XP top right and time bottom right) and the mods' machines (inputs, arrow, result, extra outputs four to
> a row), paged by height with the spare room spread between them; unit tests in `tests.rs` (`mod window`).
> `runtime/jei_window.rs` draws it from `native_screen_render_post` over the open screen (the screen darkened, the
> list left bright), takes clicks (`screen_mouse_button_pressed_pre`/`released_pre`: everything off the list stays the
> window's), the wheel (pages; categories over the tabs) and keys from `screen_key_pressed_pre` (Escape closes and
> clears the history as JEI's `onClose`, leaving the inventory open; Page Up/Down; R/U over its stacks; Backspace as
> before). The hooks only read the game, so they queue an `Action` that `jei_recipes::frame` does. The window closes
> when its screen goes (undrawn for 500 ms). `jei_recipes.rs` keeps the book, viewer and history and hands
> `Viewer::page()` to the window; the panel shows only the item list now (`native_panel_recipes`/`_catalysts` are no
> longer called). Notes are wrapped to the window's width in pixels; "Recipe: <id>" notes are left out (JEI shows ids
> only on advanced tooltips; F3+H isn't read: no helper gives it to a bundle).
> **Engine (minimal, generic):** `native_panel_area(mod_id)` (`native_panel.rs`), **API 431** (stamped; 430 is taken on
> another branch), listed in `docs/atm10-port.md` and `docs/native-mods.md`. Item icons use the existing
> `native_item_stack` (API 313), buttons the engine's copy of JEI's `button_*_v2`/`arrow_*_v2`.
> **Tests:** `jei/recipes` 9 checks (window centred, 198 px, clear of the list; 44 pages of iron-ingot uses and Next
> Page; R tabs; the Furnace tab; right click on an input; Backspace history; Escape closes and the inventory stays; U
> on the Inscriber and the catalyst click; U on AE2's Charger) and `jei/item-list` pass; `jei.sh units` and
> `jei-core`'s 13 unit tests pass (`jei.sh units`). Screenshots in `jei-work/logs/shots-recipes/` were checked by eye: crafting
> (`r-iron-ingot`), smelting (`smelting`, XP and time in the corners), the Inscriber (`u-inscriber`,
> `inscriber-recipes`, notes no longer clip), multi-page (`u-iron-ingot`, `-page-2`), the Charger (`u-charger`).
> **Left:** JEI's own textures are done (below, 2026-11-17). No Mekanism screenshot: the
> JEI world test loads AE2's content only (no Mekanism items or categories). No tab-page arrows (the title's arrows
> walk the categories, and the tab page follows); no shapeless icon, no "+"/bookmark buttons (slices 3, 4); no mod
> name line in the tooltips. The screen under the window still draws: it shows faintly through the dim, and its own
> tooltip (a creative tab's "Search Items") can show where it reaches past the window onto the list; hiding the screen
> under a mod's window needs an engine hook.
> **Slice 2 as built (commits on `feat/jei`):** `jei-core/src/recipes.rs` (Book/Viewer: R = categories whose outputs
> hold the item, U = categories whose inputs hold it, catalyst categories first with all recipes; JEI's vanilla order
> crafting, stonecutting, smelting, smoking, blasting, campfire, smithing, brewing, then the mods' categories; JEI's 3x3
> placement; history; focus-locked slots; chance/count lines wrapped at 26 characters into `note`). `runtime/
> jei_recipes.rs` (the tables, `native_collect_recipe_categories()` into the book, rebuilt when `native_recipes()`'s Arc
> or `native_recipe_categories_version()` moves; events Key R/U, SlotClicked, Catalyst, Tab, Back; `recipe_lookup`;
> probe `recipes` in `/jeiclient`). `jei_world.rs`: KEYS toggleOverlay/showRecipe/showUses/recipeBack; a click on the
> list without cheat mode = recipes (left) / uses (right); Backspace through `screen_key_pressed_pre` (not while
> `mod_browser_typing()`). Hook `recipe_lookup` is **API 429** (pending fragment stamped; 428 is taken by FTB Chunks'
> `native_map_tile` on another branch), helper `native_show_recipes`.
> **Engine changes (minimal, generic):** (1) `native_panel_catalysts(mod_id, stacks)` (`native_panel.rs`, fragment
> `tools/native-hook-fragments/panel_catalysts.json`, docs/native-mods.md): a column of slots left of the recipes
> (recipes shift right by 22 px), hover gives `Key`, click gives `NativePanelEvent::Catalyst {index, stack, right}`
> (`mod_api.rs`); (2) `native_panel_recipes.rs`: a recipe's slots beyond its layout draw as more outputs after the
> result, a `note` with newlines takes a row per line; (3) the dev harness's `click-button LABEL` (`game/agent.rs`) falls
> back to the labelled widgets, so a tab (`<icon name> tab`) or a slider clicks by name (`screen`'s `buttons` still lists
> buttons only).
> **Test results:** `jei/recipes` 6 checks and `jei/item-list` 11 checks pass; `jei.sh units` 9 pass; engine
> `cargo test -p horizon-client --lib` (filters `panel`, `recipe`) 23 pass, 4 ignored; `horizon-mod-api-gen` 16 pass and the one that fails on main
> (`new_hook_placeholder_is_reported_then_filled`). Screenshots land in `jei-work/logs/shots-recipes/` on a pass (`r-iron-ingot`, `smelting`, `uses-of-input`,
> `u-iron-ingot` and `-page-2` with "1/44" and both arrows, `u-inscriber`, `inscriber-recipes`, `back-to-list`).
> **Seen in the screenshots (960x540 window, GUI scale 2, the panel is about 130 units wide):** tabs with icons (crafting
> table, furnace, ...) with the selected one framed; the catalyst column left of the recipes (Crafting Table, Crafter;
> Furnace; the Inscriber); a 3x3 crafting recipe in its grid; smelting rows with the flame and "0.7 XP, 10s"; the
> Inscriber's own category first on U with the machine as the catalyst and its press recipes; the page count and arrows
> above the title once a category holds more than two recipes.
> **Gaps:** the panel shows only on the inventory/creative screens, so R/U over other containers' slots need the screen
> to ask through `recipe_lookup`; no fuel/compost/anvil/grindstone categories; no chance overlay on the slot (odds are
> note lines); at most 256 recipes and 16 tabs; Escape steps back like Backspace instead of closing; a note's lines are
> wrapped at 26 characters whatever the panel's width, so the Inscriber's notes clip at the screen edge in a small
> window (2b's own window fixes it); the tutorial toast covers the panel's top right in later shots of a run; JEI's own
> textures/lang still not carried.
> **Build/test:** `bash D:/VSCODE_PROJECTS/DEV_1/atm10-work/jei-work/jei.sh [units stage build world]` (own stage and
> target under `jei-work`, `AE2_PROFILE=iter`, through `wait-slot.sh 3`); `JEI_LIST="jei/recipes jei/item-list"` picks
> the world tests. Never edit sources while `stage` runs (`E_STALE_SOURCE`). Test traps: `key` does not reach the panel's
> box (`type TEXT`, `type ` do); empty the box with `click-at X Y right`; after `type` the box keeps the keys, so click
> the list's empty area (`click-at X 150`) before R/U/Backspace (the engine's `native_panel_key` ignores keys while the
> box is focused); `scroll-at X Y 0` is the hover; list items' widget label = item name, tab label `<icon name> tab`,
> catalyst label `<name> catalyst`, recipe slot label `<name> <recipe>:<slot>`; a passing run's folder is deleted,
> `jei.sh` copies the PNGs to `jei-work/logs/shots-<name>/`.

ATM10 ships JEI 19.70.0.464 (`jei-1.21.1-neoforge-19.70.0.464.jar`, MIT). This port is JEI itself, its look and
behaviour, as a native bundle `pack/mods/jei` (bundle id `jei`); it is not Horizon's own Recipes viewer. Behaviour is
read from the player's jar (targeted `javap`, file listings), written anew in the crate `jei-core`; no JEI file is in
the repository. JEI's artwork that the engine already carries under MIT (`crates/horizon-client-render/src/jei_recipes`,
drawn by `NativePanelStyle::Viewer`) covers slice 1; the recipe window's come from the player's jar at run time through
the carry (`pack/scripts/jei.ts carry`: `horizon-port-pack carry <work> jei`, a content mod `jei` with JEI's
`assets/jei`), never committed.

## What ATM10 players use

- The ingredient list right of every container screen and the inventory (creative too): pages, the page count in the
  nav bar, the mouse wheel and PgUp/PgDn turn pages; the search box at its foot with JEI's syntax; Ctrl+O hides and
  shows it; Ctrl+F focuses the search; the config button (click: config, Ctrl+click: cheat mode).
- Search syntax (`ElementPrefixParser`, `SearchTokenizer`): words AND, `|` OR, `-` excludes, `"..."` a phrase, `\`
  escapes; prefixes `@` mod name, `#` tag, `$` tooltip, `%` creative tab, `^` colour, `&` resource id. Modes per
  prefix (`IngredientFilterConfig` defaults): mod name and tag *require the prefix*, tooltip *enabled* (plain words
  search tooltips too), colour, resource id and creative tab *disabled* (their prefix does nothing until the player
  enables them in the config). Mod ids and aliases count for `@` (`searchModIds`, `searchModAliases`).
- Cheat mode: a click on the list gives one, Shift/right click a stack (creative or op); off by default.
- R recipes, U uses, a tab per category with its catalysts, page arrows, Backspace history; A bookmarks on the left.
- "+" moves a recipe into the crafting grid; red slots for what is missing (the dry-run); AE2's terminals.

## Slices

1. **The ingredient list and its search** (done). Items in JEI's order (mod name, Minecraft first, then the
   registry's order within a mod); JEI's whole search syntax and modes; paging; Ctrl+O; cheat mode (give through
   `native_give`, creative only) as a setting; the Options > Mods page with JEI's keys and the cheat/search-mode
   switches; `/jeiclient` probe (dev only) for tests. Hooks: `frame`, `panel_event`, `screen_key_pressed_pre`, `mods`,
   `settings`, `settings_click`, `commands`, `command`; helpers `native_recipes`, `native_panel_items`,
   `native_panel_style`, `native_give`, `native_setting`, `native_mod_key`.
2. **The recipe screen** (done, in the engine's right-edge panel): R/U over any slot or list item (`panel_event` Key, `recipe_lookup` for other mods' screens),
   one tab per category with catalysts, vanilla's types from `native_recipes` and the mods' from
   `native_collect_recipe_categories()` (Mekanism 21 + boiler/SPS/fission, AE2 7, ExtendedAE 3, Advanced AE 1; again
   when `native_recipe_categories_version()` moves), page arrows, Backspace history. Helpers `native_panel_recipes`.
   **2b. The centred window** (done): the same screen as JEI's own window, drawn by the bundle over the screen (see
   the handoff); the engine's panel keeps the item list.
3. **Bookmarks** (done): A adds/removes, a left panel, per-world `bookmarks.json` in JEI's format (see the handoff).
   **Config** (done, slice 5): JEI's config screen and `config/jei/jei-client.ini` (see the handoff); key rebinding is through `mods`
   keys already.
4. **Recipe transfer**: "+" with the dry-run state (red missing slots, grey "not possible"); vanilla grids through
   `native_transfer_recipe`/`native_place_recipe`; AE2's Pattern Encoding Terminal (encode) and crafting terminal
   (pull from the network) by calling our AE2 code directly (`crates/horizon-client/src/ae2*`, `ae2/addons`,
   `docs/ae2-crates.md`).
5. **The ATM10 addons**: `jeistuff`, `ftb-jei-extras`, `ae2jeiintegration`, `refinedstorage-jei-integration`.

## API gaps

- The engine laid its panel out JEI-style (right edge, full height, 9 columns, nav bar, config button), showed it on
  the creative screen and kept Backspace only for the id `recipes`. Slice 1 makes these follow
  `NativePanelStyle::Viewer`, so any viewer that asks for the style gets them; the config button opens the owner's own
  Options > Mods page. The search box is recorded as a widget (`click-button` reaches it).
- No mouse wheel over the panel, no way for a mod to set the panel's page (PgUp/PgDn, JEI's scroll) or focus its
  search (Ctrl+F); no wider grid than 9 columns (JEI's `maxColumns`; the handoff's (a)).
- No tooltip lines per item: `$` (and plain words, tooltip search being on) has nothing past the name and mod name,
  which JEI leaves out of it anyway; no creative tabs' contents (`%`, and JEI's subtypes: potions, enchanted books,
  filled tanks) and no item colours (`^` needs the texture's colours). The crate takes them once the game gives them.
- No mod display names: `@` and the sort use a table of ATM10's mods in the crate, with the namespace for the rest.
- Fluids and chemicals are not ingredients the panel can list (JEI lists fluids after items).
- The recipe window's textures arrive with the carry of the player's jar (`jei.ts carry`); a game without it draws
  the window with fills. The nine-slice borders are read from JEI's `.mcmeta` once (javap and the jar's files) and
  kept in `jei_window.rs`: the engine reads `.mcmeta` only for vanilla's GUI sprites.
