# AE2 Import Export Card 1.21.1-1.9.1

[AE2 Import Export Card](https://github.com/starforcraft/AE2-Import-Export-Card) by Ultramega, as All the Mods 10 8.4
ships it. Code MIT (`LICENSE.txt`, branch `1.21.1`, which has no tags: commit `9187cf6`, whose sources are the jar's
classes one for one and whose 45 asset and data files are the jar's byte for byte; the later commits change French
text only). The port's files that also carry AE2's rules stay LGPL-3.0 with them. Its assets state no licence of their
own; they are generated locally from the player's jar and never committed. The jar bundles GrandPower 3.0.2 (MIT), read
only for Applied Flux's energy. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

Feature `ae2-importexportcard`; content `content/ae2importexportcard` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts ae2importexportcard` (the screens' own textures also go under
`assets/ae2importexportcard/ae2/textures/ae2importexportcard/`, `own_gui_textures`, for the GUI atlas); settings from
`config/ae2importexportcard-server.toml` (`[mods.ae2importexportcard]` in `pack/pack.toml`). It is a server config, so
All the Mods 10 ships none: energy export stays on and nothing is blocked there.

## Slice 1: the Import Card

| AE2 Import Export Card | Port |
|---|---|
| `ModItems` (`Upgrades.createUpgradeCardItem`, one a stack), the recipes | the items (`content.json`), the jar's three recipes |
| `commonSetup`: `Upgrades.add` of each card on AE2's wireless terminal and wireless crafting terminal, one each; on the cards three Capacity Cards, a Fuzzy Card, an Inverter Card on the Import Card | upgrade rows (`ae2x/importexportcard.rs`); the cards are no `IUpgradeableItem`, so AE2's `add_item_upgrade` leaves them and they have no slots in the registry: their own cards go in by their screen |
| `ModDataComponents`: `filter_config` (`ConfigInventory` under ""), `selected_inventory_slots` (40 ints: 0 off, else the filter slot from 1; 36..39 the armour, feet first; `normalizeSelectedInventorySlots`), `selected_curio_slots`, `block_picker_amount`; AE2's `upgrades` and `exported_settings` (`fuzzy_mode`) on the card | the same, in the source form; registered for commands |
| `ServerConfig`: `allowEnergyExport` (true), `blockedItems`, `blockedFluids` (empty), identifiers only; `allowsExport` | `Config` |
| `WirelessTerminalItemMixin.inventoryTick` (not on the client): each terminal stack `Inventory.tick` ticks (the main inventory, the armour, the off hand); `getActionableNode`: an active access point of its linked grid in range, no battery needed; each Import or Export Card in its slots | `lan_iecard_tick` from the guest's tick (`lan_ae2x_guest_tick`, before AE2WTLib's) |
| `tickUpgradeCard`: the filter of 18 + 9 a Capacity Card (up to 3), the card's fuzzy mode, its Fuzzy and Inverter Cards; for each checked slot but the terminal's own | the same |
| `importFluidFromItem`: a container's fluid the filter passes (`passesFilter`) into the network, `poweredInsert` on the grid's energy (`ActionHostEnergySource`); the emptied container stays | buckets (the port's only fluid containers): `FluidBucketWrapper`, a lone bucket drained whole |
| `importItem`: the stack, when the filter passes it (`findFuzzy`, exact keys or with the Fuzzy Card `fuzzyEquals`, the Inverter Card turning it; an empty filter passes nothing), as much as the network takes | `ae2_grid_insert_encoded` (the grid's power) |
| `MEStorageScreenMixin`: the Import Card's toolbar button (`UpgradeItemButton`, `textures/gui/import_card.png`), shown with the card; `MEStorageMenuMixin`'s `importMenu` action | a toolbar button after the terminal's own on AE2's ME, crafting and pattern encoding terminal screens, from the item's view (`iecard` in the terminal's addon view) |
| `UpgradeContainerMenu` and `UpgradeScreen` (`screens/import_card.json`, `upgrade_<n>.png` by the Capacity Cards): 18 to 45 `FakeSlot`s, the card's 4 slots (`UpgradesPanel`), the inventory and armour slots, a press checking (or with the right button unchecking) a slot, the two mass select buttons, shift-click (`quickMoveStack`: a card into the card's slots, else an empty filter slot), the fuzzy mode button with the Fuzzy Card, back; `UpgradeUpdateData` with the whole selection | a sub screen of the terminal (`Sub::IeCard`), its menu open underneath, presses sent as `ae2importexportcard:card_menu`; filter slots take the cursor's item (right press: clear, or a bucket's fluid, `EMPTY_ITEM`) |

## Slice 2: the Export Card

| AE2 Import Export Card | Port |
|---|---|
| `Upgrades.add`: three Capacity Cards, a Fuzzy Card, a Crafting Card and an Acceleration Card on the Export Card, 5 slots | upgrade rows |
| `exportToPlayerSlot`: the slot's filter (counted from 1, within the active slots), allowed by the config; with the Fuzzy Card the network's first key it matches (`resolveExportKey`, allowed); not into the terminal itself (energy only) | the same; the port's listing order picks among fuzzy matches |
| `exportItemToPlayerSlot`: up to the item's full stack, one an operation (64 with the Acceleration Card), `poweredExtraction` on the grid's energy, into the slot (`PlayerInvWrapper`: the armour slots take what they equip); none in the network asks the Crafting Card | the same |
| `exportFluidToPlayerSlot`: a container in the slot filled from the network (a bucket, or 64 with the Acceleration Card, asked); no crafting | a lone empty bucket, filled with a bucket of a fluid that has one |
| `requestCraftingIfPossible`: allowed, the Crafting Card, `isCraftable`, no CPU crafting it (`getRequestedAmount`); the terminal item's last calculation, when done, is submitted (`submitJob` with no requester, a `MachineSource`), then a new `CRAFT_LESS` one begins for the asked amount; one future for each terminal item (a field of the `Item`) | `iecard_request_craft`, the calculation by terminal item id, sliced each grid tick; `ae2_begin_unowned_craft` (a machine's job, its results to the network) |
| `UpgradeHost.onUpgradesChanged`: a selection past the active filter slots is turned off; `UpgradeScreen`'s Export Card numbers its filter slots and counts a pressed slot through them | the same |

## Slice 3: the Block Picker Card

| AE2 Import Export Card | Port |
|---|---|
| `MinecraftMixin.pickBlock` (TAIL, survival, a block) and `BlockPickerData`; `BlockPickerHandler.handle`: survival, the block's clone stack held nowhere (all 41 slots), a free main slot, allowed by the config; the first carried terminal (41 slots) with the card and in range brings `min(amount, room)`, its battery paying an AE an item (`poweredExtraction` with the menu host); the free slot first, then the main inventory; `pickSlot`: the suitable hotbar slot, selected | 26.3 picks on the server, so the pick is the server's `tryPickItem` (`ae2x_pick_from_network`, now told whether it is an entity's), after AE2WTLib's Pick Block, whose packet comes first in Java; no packet |
| `BlockPickerCardConfig` (1 to 64, 1 unset; `setAmountOnTerminal` on every card of the terminal) | `block_picker_amount`, `set_block_picker_amount` |
| `BlockPickerAmountScreen` (`screens/block_picker_amount.json`): +1, +16, +32 and -1, -16, -32, the amount, back; `SET_BLOCK_PICKER_AMOUNT`; the toolbar button's message says the amount | a sub screen of the terminal; the amount is set by its buttons |

## Slice 4: AE2WTLib's universal terminal

| AE2 Import Export Card | Port |
|---|---|
| with AE2WTLib, `Upgrades.add` of each card on the universal terminal and on the wireless pattern encoding terminal, one each (not `addUpgradeToAllTerminals`: the pattern access terminal and the other addons' terminals take none) | upgrade rows |
| `Ae2WtlibUtil.getGridFromStack` (`WTMenuHost.getActionableNode`); `ItemWUT.inventoryTick` calls each held terminal's, so a universal terminal's cards tick once for each terminal it holds | the same range; a pass for each held terminal |
| the buttons on AE2WTLib's terminal screens (`MEStorageScreen` subclasses) | the terminal screen of a universal terminal turned to its ME, crafting or pattern encoding terminal |

Differences, known:

- The card screens cover the terminal's menu, which stays open, as AE2's sub screens do; Java opens
  `UpgradeContainerMenu` in its place. Dragging from an inventory slot does not pick its stack up (in Java a drag
  does); a press with an item on the cursor places it, as in Java. The armour can't be swapped from the screen.
- The Block Picker's amount has no text field: its six buttons set it.
- A bucket whose fluid the Import Card takes is left emptied for that tick; Java goes on with the full bucket's stack
  in the same tick (`importItem` on the original stack), which with an Inverter Card and an empty filter puts the
  filled bucket in too and loses the emptied one.
- Not ported: Curios (Horizon has no accessory slots: `CuriosTerminalTicker`, `selected_curio_slots`' panel), export
  into a storage cell or matter cannon in the slot (`ICellWorkbenchItem`), Applied Flux's energy (`FluxKey` with
  GrandPower's `ILongEnergyStorage`, `allowEnergyExport`, charging the terminal) and Applied Mekanistics' chemicals:
  the port has neither key type yet. The addon's guide pages (`assets/ae2/ae2guide`) are not carried.
- Fuzzy export picks the first matching key in the port's listing order; Java's `findFuzzy` order may differ.

Tests: `horizon-client-lib` (`importexportcard`: the rows, the config, the cards' data in the source form, the
filter, the selection and the Block Picker's amount), `horizon-client` (the filter slot's presses, the screens' view)
and the world test `ae2importexportcard-cards`: the Import Card puts the checked slots its filter passes into the
network, a bucket's water too, and nothing out of range; the Export Card fills a slot to a full stack and an empty
bucket with water, leaves the blocked diamonds, and its Crafting Card starts a job whose sticks it exports; a universal
terminal's Import Card and Block Picker Card work (16 cobblestone picked from the network into the selected slot); the
Import Card's screen sets a filter from the cursor and checks a slot.
