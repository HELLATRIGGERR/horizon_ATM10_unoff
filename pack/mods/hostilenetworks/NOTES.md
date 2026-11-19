# Hostile Neural Networks port: state

Worktree `port5`, branch `port/hostilenetworks`, on f52a62f1dc. Source and licence: see README.md.

## How to run

```
source /d/VSCODE_PROJECTS/DEV_1/atm10-work/env.sh; bash /d/VSCODE_PROJECTS/DEV_1/atm10-work/wait-slot.sh
HOSTILENETWORKS_CARRY=<carry with ATM10's 17 models laid over it> bun run pack/scripts/hostilenetworks.ts units content stage build world
```

`units` runs the rules crate's tests (25). `content` makes the content mod from the carry: the Simulation Chamber and
the Loot Fabricator go from the carved pumpkin template (no right-click) onto `minecraft:furnace` (block entity, menu,
`facing`), as Iron Furnaces' do. `stage`, `build` and `world` stage the game with every bundle of `pack/mods`, build it
and run `tests/chamber.ts`.

## What the bundle does

- Hooks, all on main (no new fork hook): `world_lifecycle`, `component_types` (API 65), `block_entity`,
  `block_entity_ticks`, `block_entity_container`, `block_entity_slot_accepts`, `block_entity_tick`, `menu_button`,
  `fe_storage`, `fe_receive`, `screen`, `screen_draw`, `screen_click`, `screen_tooltip`.
- Tiers and data models are read from the game's data (`native_data_files("model_tiers")` and `("data_models")`, every
  namespace, sub-folders too; id `<namespace>:<path>`), once per thread and dropped when a world opens or closes.
  `data_models/blocks/` is skipped (`block_data_models_enabled` is off in the mod's config); drops of items the game
  doesn't have are dropped (Java drops `optional` ones only).
- The block entity keeps the Java keys (`energy`, `model`, `runtime`, `predSuccess`, `failState`, `redstoneState`,
  `simMode`; the fabricator's `selection`, `redstoneState` and `saved_selections`, whose entries are an int array).
- The screens read only the menu's data (16 bits a slot, two 15-bit pieces an amount): the server's tick packs
  `ChamberView` and `FabView` (`view.rs`) into `entity.data`. A model is named by its place among the sorted ids.
- Chamber: size 232x230, model slot at (-13, 1), matrix (176, 7), outputs (196, 7) and (186, 27) on a hopper menu, its
  fifth slot hidden; redstone and mode buttons at (228, 0) and (228, 20) (button ids 0 to 2 and 3, 4). Fabricator:
  176x178, prediction slot (79, 62), sixteen outputs on a nine-by-two menu (the eighteenth hidden), the drop palette,
  the page buttons, the queue grid and the mode and clear buttons off the right edge, redstone off the left edge.

## Differences from Java

- The model slot takes any item (the slot hook doesn't say which); the rules read nothing from an item that is not a
  data model. Java's slot takes data models only, and the matrix slot only that model's input.
- Texts are the mod's English, not translated keys. Names (the sim target, drops, tiers) are the last part of the id in
  words (`self_aware` is "Self Aware"), not the language file's names; tier colours are the mod's tier files' by name.
- The button icons are vanilla items drawn as items (redstone, lever, redstone torch; ender eye, experience bottle; item
  frame, knowledge book; barrier), where Java blits their textures.
- The chamber's body text shows the run's lines as the run goes on; Java's scrolling `TickableTextList` types them.
- The palette's hover frame is not drawn (the draw hook has no pointer), and the palette's hit boxes follow the drawn
  grid (Java's are two pixels off it); the palette's page is kept by the client, as Java's screen does.
- The prediction roll takes one draw of the level's random when a run could start (matrix and model in, energy there).

## Not done

- The Deep Learner's data gain from kills (and from mining, for block models) is a stub: `Instance::data_gained` in the
  core is what a kill hook would call. It needs a hook that names the mob and the killer; `native_mob_killed` hears only
  a player's melee blow. No fork hook is added while an upstream merge is in progress; the kill hook comes after it.
- The Deep Learner item and screen, the Data Center (multiblock), its I/O port, the Fabrication Directive, the
  attunement of a blank data model to a mob.
