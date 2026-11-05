# AE2WTLib 19.5.1

[AE2WTLib](https://github.com/Mari023/AE2WirelessTerminalLibrary) (AE2 Wireless Terminals) by Mari023, as All the Mods 10
8.4 ships it. Code MIT (tag `19.5.1`, `LICENSE`; the bundled `ae2wtlib_api` too); the port's files that also carry AE2's
rules stay LGPL-3.0 with them. Assets CC BY-NC-SA 3.0 (README), generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in [content.json](content.json).

Feature `ae2-ae2wtlib`; content `content/ae2wtlib` from `ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts ae2wtlib`;
settings from `config/ae2wtlib.toml` (`[mods.ae2wtlib]` in `pack/pack.toml`; All the Mods 10 ships none: the magnet
range stays 16, read with the Magnet Card's slice).

## Slice 1: the terminal registry, the universal terminal, wireless encoding and access

| AE2WTLib | Port |
|---|---|
| `WTDefinition` (`AddTerminalEvent.builder`: name, menu, item, icon; `hotkeyName`, `upgradeCount`, `componentType`) and `wirelessTerminalList` | the definitions registry's wireless terminal table (`ae2x/defs.rs`): name, item, `TerminalKind` (the menu), hotkey, upgrade slots, the universal terminal's component; AE2's Wireless Terminal and Wireless Crafting Terminal are its first rows, AE2WTLib's three (`ae2x/ae2wtlib.rs`) follow in `onAe2Initialized`'s order, `crafting` taking the row of AE2's item (`AEItemsMixin`) |
| `WTDefinition.ofOrNull`, `WUTHandler` (`hasTerminal`, `nextTerminal`, `setCurrentTerminal`), `ItemWUT` (`getUpgrades` two slots for each terminal held, `onUpgradesChanged` a battery for each terminal held plus the energy cards, clamped to 1..100, `getChargeRate` 800 × (terminals + 1 + AE2 energy cards)), `Common.mergeTerminal` | `ae2x/wireless.rs`, over the table; what used to be `is_wireless`, `WirelessContext.item` and the two-id checks in `ae2_terminal.rs` now asks it, as do linking in an access point (`GridLinkables`), the Charger, the item's energy capability, upgrade cards (`capacity_of`) and item use |
| `UpgradeHelper.addUpgrades`: AE2's energy card up to each terminal's `upgradeCount`, up to their sum (6) on the universal terminal | upgrade rows |
| `ItemWET`, `WETMenuHost` (`PatternEncodingLogic` in `pattern_encoding_logic`, `markForSave` into the tag the item has) | the encoding terminal's menu on an item: `EncodingOwner` in `ae2_terminal.rs`; the logic in AE2's own NBT form (`mode`, `substitute`, `substituteFluids`, `stonecuttingRecipeId`, `blankPattern`, `encodedPattern`, `encodedInputs`, `encodedOutputs`) by `game/ae2x/ae2wtlib/` |
| `ItemWAT`, `WATMenuHost`, `WATMenu` (`PatternAccessTermMenu`), its show setting in `ae2:exported_settings` (`AE2wtlibConfigManager`) | the Pattern Access menu on a wireless item: the providers of the grid its access point is on, the battery and the link ticked as a terminal's |
| `ItemWUT.use` (`TERMINAL_EMPTY` when it holds none), `WUTHandler.open`: the current terminal's menu | `lan_open_ae2_wireless_source` by the item's `TerminalKind` |
| `TerminalSelectionButton` on the terminals' toolbars, `SelectTerminalPacket` | a toolbar button with the current terminal; its panel lists the terminals held and opens the one chosen (`ae2wtlib:select_terminal`) |
| `AE2wtlibClient.mouseScroll` and `CycleTerminalPacket`: shift and the wheel with no screen open turn the held universal terminal (down: back), and say which terminal it is now | the same (`ae2wtlib:cycle_terminal`); in a menu the packet turns it and opens the next |
| `Ae2wtlibLocatingService` on each terminal's hotkey: a universal terminal holding the terminal first, else the terminal's own item; it becomes the current one and opens | the server's hotkey handler, before AE2's own actions |
| component ids `current_terminal`, `has_<name>_terminal`, `pattern_encoding_logic` | kept, registered for commands |
| recipes: the two terminals from AE2's terminals, the encoding terminal from the wireless crafting terminal | carried from the jar |

Differences, known:

- `WTDefinition.ofOrNull` takes a universal terminal's first terminal in the order of a `HashMap` when no current one is
  set; the port takes the registration order, and sets it when the terminal opens.
- The selector shows the terminals' items, not AE2WTLib's icons (`textures/wtlib/guis/icons.png` is in the bundled API
  jar, which `prepare-addon.ts` does not read yet).
- The Pattern Access screen has no selector yet: a universal terminal turned to it is left by the wheel or a hotkey.
- The terminals' own hotkeys (`wireless_pattern_encoding_terminal`, `wireless_pattern_access_terminal`) are answered by
  the server, but the client has no key mapping for them yet; AE2's `wireless_terminal` key finds the universal terminal.
- The wireless encoding terminal shows no key-type button (AE2's `KeyTypeSelectionHost` would show it).
- AE2WTLib's own menus (`WCTMenu` with the armour, trash, magnet and singularity slots and the scrolling upgrade panel)
  are not ported: the universal terminal opens AE2's terminal menus, and its cards go in by AE2's crafting-grid upgrade
  recipe. The creative tab's charged universal terminal is not listed.

Tests: `horizon-client-lib` (the rows, the universal terminal's order, upgrade slots, battery, merging and charge rate),
`horizon-client` (the encoding logic's NBT form, the component ids) and the world test `ae2wtlib-terminals`: the
wireless pattern encoding terminal encodes a pattern from a physical blank and keeps its grid in the item through a
restart, the universal terminal opens its current terminal and the crafting and pattern access terminals chosen in its
selector, shift and the wheel turn it, and an Energy Card makes its battery (3 + 1) × 1,600,000.

## Slice 2: the combine and upgrade recipes

| AE2WTLib | Port |
|---|---|
| `CombineSerializer`, `UpgradeSerializer` (`terminalA`/`terminalB` and `terminalAName`/`terminalBName`; `terminal` and `terminalName`, names by `WTDefinition.CODEC`: with an unknown name the recipe does not load) | `ae2x/ae2wtlib/recipes.rs` reads them; the crafting loader keeps them as special crafting recipes (`crafting::Special::Ae2wtlib`); `prepare-addon.ts` carries the six recipes (`recipe_types` in `content.json`) |
| `Combine.matches`: exactly two stacks, each ingredient taking one (`InputHelper.getInputStack`, the first in the grid's box) | the same |
| `Upgrade.matches`: a universal terminal not holding the terminal yet, and the terminal | the same |
| `assemble`: `Common.mergeTerminal` of each terminal into `outputStack` (a universal terminal already holding both, so its battery counts both from the first merge), or into a copy of the universal terminal | `ae2x::wireless::merge_terminal` over the same output |
| `getResultItem` (`outputStack`), `getIngredients`, `category` equipment, not `isSpecial` | the recipe book's and the recipe panel's display: a shapeless recipe of the two ingredients showing the universal terminal holding the recipe's terminals, under equipment, one display id each; placeable from the book |

Differences, known:

- AE2WTLib's recipe advancements (`data/ae2wtlib/advancement/recipe/`) are not carried yet, so the book does not unlock the
  six recipes; the recipe panel lists them anyway. In the jar, `ce.json`'s advancement rewards
  `wireless_universal_terminal/cp`, a recipe that does not exist, so in Java the book never unlocks that one either.

Tests: `horizon-client-lib` (`recipes`: combining keeps cards and energy, the upgrade's check, the book display) and the
world test `ae2wtlib-recipes`: the crafting terminal with an Energy Card and the encoding terminal combine into a universal
terminal with both, the card, (2 + 1) x 1,600,000 of battery and both terminals' energy, the encoding terminal's name kept;
the pattern access terminal is added by the upgrade recipe; a universal terminal already holding it gives no result.

## Later slices (`.build/ae2-addons-plan.md` §5.3)

- **Magnet Card** (`magnet_card`, range 16 from `ae2wtlib.toml`), **Restock** and **Pick Block**: player ticks and
  pickups; `RESTOCK`, `PICK_BLOCK`, `PICKUP_CONFIG`, `INSERT_CONFIG` components; the settings and trash screens.
- **Quantum Bridge Card**: `WTMenuHost.isQuantumLinked` (a singularity in `SINGULARITY`, the bridge by frequency, 22.5 AE/t,
  recharging from the network to half), sharing the cross-dimension work with AEInfinityBooster's Dimension Card.
- **Curios** slot, when Horizon has accessory slots.
