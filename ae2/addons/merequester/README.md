# ME Requester 1.21.1-1.5.0

[ME Requester](https://github.com/AlmostReliable/merequester) by AlmostReliable, as All the Mods 10
ships it. Code LGPL-3.0 (tag `v1.21.1-neoforge-1.5.0`, `LICENSE`), so the port's derived files carry
the same terms; the port is written from the mod's behaviour, data and docs, not copied from its
source. Its assets (licence not stated) are generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

Feature `ae2-merequester`; content `content/merequester` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts merequester`; settings from ATM10's
`config/merequester-common.toml` (`[mods.merequester]` in `pack/pack.toml`).

## What works

| ME Requester | Port |
|---|---|
| `merequester:requester` block, `AEBaseBlock.metalProps`, facing and spin (`FacingWithVerticalSpin`), `active` | an addon machine (`MachineKind::Ext`, `ae2x/merequester.rs`); placed as AE2's facing machines; `active` while a row shows anything but idling |
| `Config`: `requests` 5 (1-64), `idle_energy` 5 AE/t, `require_channel` true | read from `merequester-common.toml`, wrong values back to their defaults |
| grid node on every side but the front, `TickingRequest(1, 20)` and its tick-rate modulation | the grid node, AE2's device scheduler |
| `Request`: on/off, key (item or fluid), amount, batch (a bucket for fluids), the update rules | `ae2-merequester-core` |
| `StorageManager`: known, pending, buffer and total per row, the stack watcher | the same, the watcher read before and after each tick |
| statuses Idle, Request, Plan, Missing, Cpu, Link, Export and `handleRequests`' re-handling | the same state machine (`ae2-merequester-core`), tested on its own |
| `beginCraftingCalculation(..., REPORT_MISSING_ITEMS)`, `submitJob`, CPU errors as No CPU | the port's crafting calculation (`Strategy::Full`) and job submission, a third requester kind beside the Interface and the Export Bus (`ae2_autocrafting.rs`) |
| `insertCraftedItems` into the row's buffer, then `poweredInsert` into the network | the CPU delivers to the buffer; Export inserts with power |
| saved `requests`, `request_status` (the jobs' links), `storage_manager` | the same tags in the block entity |
| `addAdditionalDrops`: buffered and pending items | dropped when broken |
| the screen: header, 3 to 10 rows of 19, footer, toggle, ghost slot, Amount to Stock and Batch Size fields (B for fluids), submit, status bar in its colours, scrollbar, "No ME Requesters found!" | `game/ae2x/merequester/screen.rs`, from the mod's textures and sizes |
| `doAction`: set with the cursor's stack or one of it, a bucket's fluid on a right click, shift-click clears, creative middle click; shift-click from the inventory fills the first free row; locked rows refuse | `menu.rs` |
| recipes `requester` and `requester_terminal`, loot table, pickaxe tag | from the jar |
| Memory Card: `exported_requests` copied with sneak, pasted without | `ae2_try_requester_memory` |
| Requester Terminal part: `getActiveMachines`, grouped by name, sorted by position, search on names and keys | `menu.rs`, its part model from the jar |

`/ae2 requester status|set|toggle|clear x y z ...` shows and edits a requester (rows, buffers, known
amounts, the server status, jobs submitted in this run).

## Differences

- The Requester Terminal has no terminal style button: its rows follow ATM10's `terminalStyle = "FULL"`
  and `terminalMargin = 25`. The setting comes with the AE2 terminal screens' branch.
- The help button opens AE2's guide at its home page: addon guide pages aren't compiled yet, so
  `merequester.md` isn't there.
- No fluid-container tooltip on a ghost slot (`getEmptyingTooltip`), no JEI drag and drop, no wireless
  terminal (needs AE2WTLib).
- The status bar's and fields' tooltips are the mod's; the number fields' validation messages
  (`whole_number`, `NumberLessThanMinValue`) aren't shown, the text only turns red.
- The scrollbar jumps to a press and moves with the wheel; its handle can't be dragged.
- A menu's lines go to the client as one JSON payload when they change, not as the mod's partial
  updates. The stack watcher is a read of the network's amounts at each requester tick.

## Tests

`ae2-merequester-core` (rows, buffers, every status path), `horizon-client` (config, saved tags,
drops) and the world test `merequester-stock` (a real pattern provider and molecular assembler; Crafting,
Missing, No CPU, a job through a restart, a broken CPU, the screen, a Memory Card, the terminal):

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts merequester-stock
```
