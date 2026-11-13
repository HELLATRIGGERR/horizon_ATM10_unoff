# JEI (Just Enough Items) port plan

> **Handoff (2026-11-13): slice 1 is done, slice 2 (the recipe screen) is next.**
> **Slice 1 as it stands** (`JEI, slice 1: the ingredient list and its search`): `pack/mods/jei` (crate `jei-core`: JEI's
> order and search, 5 unit tests; `runtime/jei_world.rs`: the list follows the world in `frame`, search in `panel_event`,
> Ctrl+O through `screen_key_pressed_pre` + a flag the next frame flips, cheat mode through `native_give`, the Options >
> Mods page, the `/jeiclient` probe under `JEI_DEV=1`; 8 hooks in `horizon-mixin.toml`), `pack/scripts/jei.ts`, and the
> engine change in `crates/horizon-client/src/game/native_panel.rs`: the right-edge layout, the creative screen, Back, the
> config button and the right-click-empties-the-box rule follow `NativePanelStyle::Viewer`, not the id `recipes`; the box
> is a widget `Search...`. World test `jei/item-list` passes (11 checks, 25-35 s): 2021 items, Minecraft first, 27 pages
> of 7 x 11, 7 searches, Ctrl+O, a cheat-mode give; its screenshots show the list on the right edge like JEI's (nav
> bar `n/27` on top, search box and wrench at the foot), items drawn, the box emptying and the overlay vanishing on Ctrl+O.
> **Test traps** (they cost runs): the `key` control does not reach the panel's box (only `type TEXT` and `type \b`, one
> backspace, do); `click-button Search...` puts the caret at the click, mid-text, so empty the box with
> `click-at X Y right` (rect from `screen` buttons) and then `type`; a passing run's folder is deleted, so `jei.sh` copies
> the PNGs to `jei-work/logs/shots-<name>/` first.
> **Slice 2, the recipe screen: what to do.** R over a slot or a list item shows its recipes, U its uses (`panel_event`
> Key for the list; `screen_key_pressed_pre` + the slot under the pointer for container screens; `recipe_lookup` for what
> other mods' screens ask), as `native_panel_recipes(mod, title, tabs, tab, views)` (`native_panel_recipes.rs`:
> `RecipePage`, at most 256 recipes and 16 tabs, `NativeRecipeView` with layout, slots, results, note, action). One tab
> per category with its catalysts as the icon; vanilla's types from `native_recipes()` (tables of the open world) and the
> mods' from `native_collect_recipe_categories()` (Mekanism 21 + boiler/SPS/fission, AE2 7, ExtendedAE 3, Advanced AE
> 1), collected again when `native_recipe_categories_version()` moves; page arrows; Backspace history (the engine's
> Backspace and Back already follow the Viewer style: `close_transferred_recipe`, `show_recipes`'s `filtering`). Add the
> `recipe_lookup` hook (`hooks/recipe_lookup.rs`, a line in `horizon-mixin.toml`) and a test `tests/recipes.ts` (R on a
> list item, tabs, Backspace back to the list). Read Horizon's own viewer for the engine calls, not to copy:
> `D:/VSCODE_PROJECTS/DEV_1/atm10-work/ref/horizon-mods/native/recipes` (MIT, read-only). JEI draws a centred window over
> the container, the engine's panel draws pages beside it: keep the panel and say so in the commit.
> Build/test: `bash D:/VSCODE_PROJECTS/DEV_1/atm10-work/jei-work/jei.sh [units stage build world]` (`JEI_LIST=jei/item-list`
> picks tests; own stage and target under `jei-work`, `AE2_PROFILE=iter`, through `wait-slot.sh 3`); never edit sources
> while `stage` runs (`E_STALE_SOURCE`). Left after slice 2: slices 3-5 below.

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
2. **The recipe screen**: R/U over any slot or list item (`panel_event` Key, `recipe_lookup` for other mods' screens),
   one tab per category with catalysts, vanilla's types from `native_recipes` and the mods' from
   `native_collect_recipe_categories()` (Mekanism 21 + boiler/SPS/fission, AE2 7, ExtendedAE 3, Advanced AE 1; again
   when `native_recipe_categories_version()` moves), page arrows, Backspace history. Helpers `native_panel_recipes`.
   JEI draws its recipe screen as a centred window over the container; the engine's panel draws pages beside it.
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
