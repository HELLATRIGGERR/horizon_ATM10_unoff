# AE2WTLib 19.5.1

[AE2WTLib](https://github.com/Mari023/AE2WirelessTerminalLibrary) (AE2 Wireless Terminals) by Mari023, as All the Mods 10
8.4 ships it. Code MIT (tag `19.5.1`, `LICENSE`; the bundled `ae2wtlib_api` too); the port's files that also carry AE2's
rules stay LGPL-3.0 with them. Assets CC BY-NC-SA 3.0 (README), generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in [content.json](content.json).

Feature `ae2-ae2wtlib`; content `content/ae2wtlib` from `ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts ae2wtlib`;
settings from `config/ae2wtlib.toml` (`[mods.ae2wtlib]` in `pack/pack.toml`; All the Mods 10 ships none: the magnet
range stays 16).

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
- The selector shows the terminals' items, not AE2WTLib's icons (`textures/wtlib/guis/icons.png`, from the bundled API
  jar, is in the content since slice 3).
- The Pattern Access screen has no selector yet: a universal terminal turned to it is left by the wheel or a hotkey.
- The terminals' own hotkeys (`wireless_pattern_encoding_terminal`, `wireless_pattern_access_terminal`) got their key
  mappings with slice 3; AE2's `wireless_terminal` key finds the universal terminal.
- The wireless encoding terminal shows no key-type button (AE2's `KeyTypeSelectionHost` would show it).
- AE2WTLib's own menus (`WCTMenu` with the armour and singularity slots and the scrolling upgrade panel) are not
  ported: the universal terminal opens AE2's terminal menus, and its cards go in by AE2's crafting-grid upgrade recipe;
  the magnet's and trash's buttons came with slices 3 and 4. The creative tab's charged universal terminal is not
  listed.

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

## Slice 3: the Magnet Card

| AE2WTLib | Port |
|---|---|
| `AE2wtlibItems.MAGNET_CARD`, `Upgrades.add`: one on AE2's wireless crafting terminal, one on the universal terminal | the item (`content.json`), its recipe from the jar, the upgrade rows |
| `MagnetMode` in `ae2wtlib:magnet_settings` (a byte: off, pickup to inventory, pickup to ME, pickup to ME without the magnet); `getMagnetMode`: invalid without an upgrade inventory, no card, else the byte | `ae2x/ae2wtlib/magnet.rs` |
| `MagnetHost`: `pickup_config`, `insert_config` (`ConfigInventory` of 27 item keys under the key ""), `pickup_mode`, `insert_mode` (`Codec.BOOL`, true allows; deny by default); `IPartitionList` with `FuzzyMode.IGNORE_ALL` | `Filter`: the keys' item ids, any components matching; an empty allow list takes nothing |
| `MagnetHandler.handle` from `ItemWCT.inventoryTick` (a universal terminal ticks its crafting terminal), once a server tick, for the first such stack ticked; the terminal of `CraftingTerminalHandler` (`WUTHandler.findTerminal`: a universal terminal holding crafting first) | `lan_ae2wtlib_magnet_tick`, in the guest's tick after its items' and before what it touches (`Player.aiStep`) |
| `handleMagnet`: not sneaking, the magnet bit on both, the pickup filter; item entities in the player's box grown by `magnet_card_range` (`ae2wtlib.toml`, 16.0) get `playerTouch` | the same box over the item entities; the touch is the guest's own pickup (`guest_touch_items`), pickup delay and target included |
| `ItemEntityPickupEvent.Pre` at the lowest priority, `insertStackInME`: `canInsert` (restock on and the item in the hotbar, else not sneaking, the pickup-to-ME bit and the insert filter), `inRange` (the access point the terminal reaches, powered), the grid's storage takes what it can without power (`getStorageService().getInventory()`), the stat and `onItemPickup`, the rest picked up as usual; an emptied entity leaves with no take | `lan_ae2wtlib_item_pickup` from the pickup routine (`lan_ae2x_item_pickup`), after native mods' refusal. Native hooks API 40 `native_item_picked_up` can only refuse and sees no components, so this is the engine's path, at the event's place |
| `MagnetHotkeyAction` (`ae2wtlib_magnet`): off, inventory, ME, ME without the magnet, off; the action bar says which; without a card nothing changes | `key.ae2.ae2wtlib_magnet` (unbound, as AE2's keys), the server's hotkey handler |
| `WirelessTerminalSettingsScreen` (`screens/wtlib/wireless_terminal_settings.json`) and `TerminalSettingsPacket`: pick block, craft if missing, restock, magnet, pickup to ME; the magnet's two boxes show the stored byte, inactive without a card; craft if missing only with pick block; each change sends all five, set on the item | the crafting terminal's sub screen, from a toolbar button (`ae2wtlib:terminal_settings`) |
| `MagnetMenu`/`MagnetScreen` (`screens/wtlib/magnet.json`, `wtlib/guis/magnet.png`, the API's `icons.png`) from `WCTScreen`'s Magnet Filter button, shown with the card: two `FakeSlot` filters of 27 over the `MagnetHost` of `CraftingTerminalHandler`'s terminal (the cursor's item, as a key of no amount; an empty cursor clears), allow/deny of each, copy up, copy down, switch | a sub screen of the crafting terminal with the player's inventory under the filters, its presses sent as `ae2wtlib:magnet_menu`; `prepare-addon.ts` reads the bundled API jar (`nested_jars`) and AE2-namespace textures outside `guis/` (`ae2_namespace.textures`), the atlas names them `ae2:<path>` |
| component ids `magnet_settings`, `pickup_config`, `insert_config`, `pickup_mode`, `insert_mode`, `restock`, `pick_block`, `craft_if_missing` | registered for commands; 26.3's SNBT refuses the empty key `ConfigInventory` writes under, so the filters can be set by their screen only |

Differences, known:

- `MagnetMenu` is its own menu in AE2WTLib (its back button reopens the terminal); here its screen covers the terminal's
  menu, which stays open, as AE2's sub screens do. Shift-clicking an item into the filters is not ported.
- The settings and Magnet Filter buttons sit on the terminal's left toolbar: the port opens AE2's crafting terminal
  layout, where AE2WTLib's spots beside the grid are taken.
- `PreventRemoteMovement` persistent data is not read (the port's item entities have none).
- The magnet works for guests (the server-thread player); the in-frame host player has no wireless terminals here.
- `CraftingTerminalHandler` keeps the terminal it found while that still holds crafting; the port looks again each tick.

Tests: `horizon-client-lib` (`magnet`: modes, the hotkey cycle, settings on the stored byte, filters and their edits,
the range config; the card rows), `horizon-client` (the settings screen's boxes and packet, the filter screen's view),
`horizon-command` and the component samples, and the world test `ae2wtlib-magnet`: the filter screen sets both filters
from the cursor and turns the pickup filter to allow; the hotkey turns the card through its four modes; the magnet pulls
what the pickup filter allows from 11 blocks away into the inventory; with pickup to ME cobblestone goes into the network
and the insert filter keeps dirt in the inventory; without the magnet a touched stack still goes in and a far one stays;
the settings screen sets the magnet's two bits.

## Slice 4: Restock, Pick Block, hotbar counts and the trash

| AE2WTLib | Port |
|---|---|
| `AE2wtlibEvents.restock(player, item, now, setStack)`: not creative, `item` not empty nor `ae2wtlib:no_restock` (in the jar: `#powah:reactors`, optional), `CraftingTerminalHandler.inRange` and its terminal's `restock`; the network fills the slot to `item.getMaxStackSize()`, `extract` from (or `insert` into, past a stack) `getStorageService().getInventory()`, unpowered; a slot holding another item is left; `UpdateRestockPacket` | `ae2x/ae2wtlib/restock.rs` (the move), `game/ae2x/ae2wtlib/restock.rs` (the player): run with the acting player's state in the game's (`as_guest`), its terminal found as the magnet finds it, the network the terminal's access point reaches (in range, and powered unless the range is free or the player creative); the guest's inventory goes to its client after the packet or tick that restocked it, as `UpdateRestockPacket` does (a slot back at the count the client last heard of is no change to the tick's sync) |
| `ServerPlayerGameModeMixin` (`useItemOn` returns: `original.copy()`, the hand), NeoForge's `RightClickBlock` (lowest priority, as `useItemOn` starts) and `EntityInteractSpecific` (the hand) | engine hooks around the guest's `use_item` in `UseItemOn` and before its `Interact` (`lan_actions.rs`), through `ae2x_restock_slot` / `ae2x_use_on_returned`; a hand left empty or holding something else is not restocked, as `original.copy()` of a used-up stack is empty |
| `LivingEntityUseItemEvent.Finish` (`getItem()` before, `getResultStack()`) | after the mods' Finish in `use_item_finished` (`item_use.rs`): a used-up stack is filled again, a bowl is left |
| `ArrowNockEvent` and `ArrowLooseEvent` with ammo: `player.getProjectile(bow)` restocked in place | in `BowItem.use`'s and `releaseUsing`'s places (`game.rs`), the ammo slot before the arrow is drawn |
| `ServerPlayerMixin.restockDrop`: `drop(boolean)` dropped something and the selected stack is not empty | after the guest's drop key (`PlayerAction` 4 and 5) |
| `MagnetHandler.sendRestockAble` (once a tick, before the magnet, from `ItemWCT.inventoryTick`): with restock on and in range, each item the player carries (inventory, armour, off hand; the first stack's key, not `no_restock`) and the network's amount (`getCachedInventory`), `RestockAmountPacket` | `lan_ae2wtlib_tick` before the magnet; `ae2wtlib:restock_amounts` (a JSON object of item to amount) |
| `GuiMixin.restockOverlay` (`Gui.renderSlot`, not creative, restock on in the client's terminal, `count != 1`, an item the server counted): `ReadableNumberConverter.format(count + network, 3)` in place of the count | the hotbar's slots and the off hand (`game.rs`); `horizon-ctl inventory` lists them as `hotbar_counts` |
| `MinecraftMixin.pickBlock` (none in the inventory, not creative nor spectator) and `PickBlockPacket`; `AE2wtlibEvents.pickBlock`: `pick_block` on, `getTargetGrid` (range only), `getSuitableHotbarSlot`, its stack must fit into the network (simulated), the network's `getMaxStackSize` of the item (simulated, then real), the slot selected; none there: with `craft_if_missing` and a pattern for it (`getCraftingFor`), `CraftAmountMenu.open(player, locator, what, 1)` | 26.3 picks on the server (`handlePickItemFromBlock`/`FromEntity`, `tryPickItem`), so the hook is there (`guest_pick_item`) and there is no packet; the amount screen is AE2's (`lan_open_terminal_craft_confirm`) on the crafting terminal, whose back button opens it |
| `RestockHotkeyAction` (`ae2wtlib_restock`: restock turned, `Restock: On/Off` in the action bar) and `StowHotkeyAction` (`ae2wtlib_stow`: in range, the main hand's stack into the network, not `isNotReplaceableByPickAction`, which NeoForge answers with `isEnchanted`) | the server's hotkey handler; key mappings `key.ae2.ae2wtlib_restock` and `key.ae2.ae2wtlib_stow` (unbound) |
| `TrashMenu`/`TrashScreen` (`screens/wtlib/trash.json`, `wtlib/guis/trash.png`) from `WCTScreen`'s trash button (`Icon.TRASH`): 27 slots of the menu host's `AppEngInternalInventory`, saved nowhere, the player's inventory, the back button (`returnToMainMenu`) | a `generic_9x3` window with a chest's rules (`game/ae2x/ae2wtlib/trash.rs`), drawn from the style; `ae2wtlib:trash` (0 open, 1 back); what it holds is gone when it closes or goes back |

Differences, known:

- AE2 machines whose menus the port opens before the item's use (`ae2-lab`'s own paths in `UseItemOn`) are not
  restocked around: in Java the `RightClickBlock` restock fills the held stack on any right click of a block.
- Restock and the hotbar's amounts read the network as it stands; AE2's `getCachedInventory` may lag a tick.
- Pick Block's craft amount turns a universal terminal to its crafting terminal (the amount screen's back button opens
  that); AE2WTLib leaves its current terminal and its back button opens that.
- The trash button sits on the terminal's left toolbar with the settings and magnet buttons.
- `CraftingTerminalHandler` keeps the client's counts until the crafting terminal is gone; here they stay shown only
  while the client holds one with restock on.

Tests: `horizon-client-lib` (`restock`: the move, the excess, the slot left, the hotbar count) and the world test
`ae2wtlib-restock`: the hotbar shows a stack and the network's stock; placing a block fills the stack from the network
(54 before the use, 1 after), the drop key replaces the dropped one, a golden apple eaten and an arrow shot are
replaced; the restock key turns it off and on; the stow key empties the hand into the network; a pick takes a stack of
cobblestone from the network into the selected empty slot, and planks, which only a pattern makes, open the craft
amount; the trash takes dirt and has none when it opens again.

## Later slices (`.build/ae2-addons-plan.md` §5.3)

- `WCTMenu`'s armour, off-hand and singularity slots, if the port takes AE2WTLib's own crafting terminal layout.
- **Quantum Bridge Card** (`UpgradeHelper.addUpgradeToAllTerminals(QUANTUM_BRIDGE_CARD, 1)`): `WTMenuHost.isQuantumLinked`
  (a singularity in `SINGULARITY`, the bridge by frequency, 22.5 AE/t, recharging from the network to half), sharing the
  cross-dimension work with AEInfinityBooster's Dimension Card: both need the wireless path (`ae2_wireless_reach`, today
  the guest's own dimension only) to reach a grid in another dimension; the magnet's `inRange` follows it.
- **Curios** slot, when Horizon has accessory slots (`findTerminal` looks there first).
