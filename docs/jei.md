# JEI (Just Enough Items) port plan

> **Handoff (2026-11-13, slice 2 done, test green): the recipe screen shows R/U, category tabs, the catalyst column,
> page arrows and history in the engine's right-edge panel. Next: slice 2b, then slice 3.**
> **Slice 2b (next): JEI's recipe screen as a centred window, as real JEI draws it, not the right-edge panel.** The user
> wants JEI's own look: the window centred over the container, its background, the category title with the tab row on
> top, the page arrows and "1/N" under the title, the catalysts beside the recipe, JEI's recipe slots and arrows. FTB
> Chunks' claim map, a full-screen mod screen drawn by a bundle with `native_mod_image` (API 355), shows a bundle can
> draw its own screen; start from how it opens, draws and takes input. Keep `jei-core::recipes` (the Book/Viewer, the
> placements, the history) and the tables in `runtime/jei_recipes.rs`; replace the output (`native_panel_recipes`,
> `native_panel_catalysts`) with the bundle's own screen and keep the panel for the item list only. JEI's textures come
> from the player's jar through the carry (generated content, never committed).
> **Then slice 3:** bookmarks (A, the left panel, `bookmarks.ini`) and JEI's config screen (see Slices).
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
drawn by `NativePanelStyle::Viewer`) covers slice 1; the rest of JEI's textures and its lang come from the player's
jar at run time through the carry (generated content), never committed.

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
   JEI draws its recipe screen as a centred window over the container; the engine's panel draws pages beside it.
   **2b. The centred window** (next): the same screen as a window of the bundle's own, in JEI's look (see the handoff).
3. **Bookmarks and config**: A adds/removes a bookmark, a left panel (`bookmarks.ini` in JEI's format under
   `config/jei/`); JEI's config screen (`config/jei/jei-client.ini` keys); key rebinding through `mods` keys.
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
  search (Ctrl+F); no wider grid than 9 columns (JEI's `maxColumns`).
- No tooltip lines per item: `$` (and plain words, tooltip search being on) has nothing past the name and mod name,
  which JEI leaves out of it anyway; no creative tabs' contents (`%`, and JEI's subtypes: potions, enchanted books,
  filled tanks) and no item colours (`^` needs the texture's colours). The crate takes them once the game gives them.
- No mod display names: `@` and the sort use a table of ATM10's mods in the crate, with the namespace for the rest.
- Fluids and chemicals are not ingredients the panel can list (JEI lists fluids after items).
- JEI's own textures for slices 2-3 (recipe background, bookmarks, tabs) need the carry to bring `assets/jei/textures`
  from the player's jar into the generated content, as Mekanism's are.
