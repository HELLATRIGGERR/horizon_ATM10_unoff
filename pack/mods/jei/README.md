# JEI (native bundle)

Just Enough Items 19.70.0.464, as ATM10 ships it, for the pack's port. Plan and slices: `docs/jei.md`.

Slice 1, the ingredient list: every item beside every container screen, the inventory and the creative inventory,
in JEI's order (Minecraft first, then the mods by name), in pages, with JEI's search syntax and search modes
(`jei-core`), Ctrl+O to hide and show it, and cheat mode (a click gives one, a right click a stack; creative). The
settings are on the mod's page in Options > Mods (the panel's config button opens it), kept in
`horizon-mods/.horizon-mod-data/jei/` under JEI's config names (`cheatItemsEnabled`, `overlayEnabled`,
`modNameSearchMode`...).

JEI is MIT (mezz, https://github.com/mezz/JustEnoughItems). Its behaviour is reimplemented here from what the mod
does; no file of it is in the repository. The panel's buttons and search box use JEI's artwork the engine carries
(`crates/horizon-client-render/src/jei_recipes`, MIT); further textures and the lang come from the player's jar at
run time through the carry.

| File | What it is |
|---|---|
| `jei-core/` | the list's order and JEI's search (tokenizer, prefixes, modes), with unit tests |
| `runtime/jei_world.rs` | the glue: the game's items in, the panel out, Ctrl+O, cheat mode, settings, `/jeiclient` |
| `hooks/` | the named hook points it uses: `frame`, `panel_event`, `screen_key_pressed_pre`, `mods`, `settings`, `settings_click`, `commands`, `command` |
| `tests/item-list.ts` | the world test: the list in the inventory, searches, pages, Ctrl+O, a screenshot |

Build and test: `bun run pack/scripts/jei.ts units stage build world`.
