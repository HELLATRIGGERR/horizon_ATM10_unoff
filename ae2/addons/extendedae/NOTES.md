# ExtendedAE port: working notes

Slice 2: branch `feat/extendedae-3` (on `wip/fullpack-4` 2b056e630), worktree `C:\PET\atm10-wt\eae2`, target
`C:\PET\atm10-work\target-eae2` (seeded from the full-pack target of the same commit). Slice 1 was
`feat/extendedae-2` in `C:\PET\atm10-wt\eae`. Generated content: `C:\PET\atm10-work\eae-content\extendedae`, from
`ATM10_DIR=... AE2_ADDON_CONTENT_ROOT=C:\PET\atm10-work\eae-content bun run ae2/scripts/prepare-addon.ts extendedae`;
the same root holds copies of `megacells` and `merequester` for their suites. World suites:
`C:\PET\atm10-work\logs\eae2\run-suites.sh <suite>...`. The mod's source at the pinned commit, Glodium's
`neoforge-1.21` branch and AE2's at the port's commit are cloned (read-only) under `C:\PET\atm10-work\src-eae`.

## State

Slice 1 is complete: rows in `horizon-client/src/ae2x/extendedae.rs` (`Defs::add_variant`, installed with ME
Requester's `ExtMachine`s by `defs::get`), windows in `game/ae2_machine_ui.rs` (`MenuShape`), layouts in
`game/ae2x/extendedae/screens.rs`, kits in `upgrade.rs`, reached through `game/ae2x/mod.rs` (`ae2x_use_on`,
`part_model`).

Slice 2 so far: the Crystal Assembler, Circuit Slicer and Crystal Fixer (`ExtMachine`s, `game/ae2x/extendedae/
machines.rs`) with their recipe types, the Extended IO Port (a variant), and the materials. The addon hooks it
added to `game/ae2x/mod.rs`: `orientation` (placement and the wrench), `machine_menu`, `machine_may_place`,
`slot_insert`/`slot_extract` (automation), `private_power_face` (Forge Energy), `machine_progress`,
`machine_menu_items`, `io_port_budget`, `ae2x_block_use`, `ae2x_machine_click`; in the library
`ae2x::extendedae::reset_unsaved` (from `save::ae2_reset_unsaved`), and `ExtMachine::power_capacity`.

The Extended Import and Export Buses are part variants of kind `CableBus` (`base_id` for the bus code through
`ae2_buses::part_kind`, `ae2_world::bus_card_slots`, `BusState::multiplier`); their window's three more cards are
entries 45..48. Next: the Extended Inscriber, Charger and Molecular Assembler; a Memory Card on the extended buses.

Facts checked for slice 2, at the source and in Glodium 2.2's classes (`javap`):

- `RecipeSearchContext.load` reads `current` and `last` with `tag.getString` where it means `nbt`: a reloaded
  machine has no recipe. `TileCrystalAssembler` and `TileCircuitCutter` do not save `progress`; the fixer does.
- `RecipeExecutor` with `mainNode != null` always takes the grid branch (`ifPresent`): the machine's own buffer
  first, then the grid's energy service; the offline branch is never reached for these machines.
- `AEFluidKey.addDrops` drops nothing: a broken assembler loses its tank.
- AE2's export bus moves its whole budget into an item handler, stack by stack (`ItemHandlerFacade.insert`); the
  port's took one grid stack a key, so any bus past 64 operations (an Extended one at 8 and more) stopped at 64.
  Fixed in `ae2_buses` (the export host's `transfer_atomic`). The import side already went slot by slot.

Facts checked at the source and in the jar:

- The Extended Pattern Provider has no pages: `ContainerExPatternProvider` is plain `PatternProviderMenu`,
  `ex_pattern_provider.json` lays out 4 rows of 9 at once. Only the Extended Interface has pages (2 of 18 + 18).
- `ItemUpgrade.useOn` shrinks the stack, but `ServerPlayerGameMode.useItemOn` restores the count in creative.
- 2.2.42 (ATM10 8.4) has no tag. The commit that sets its version is `1f86f927` on `1.21.1-neoforge`; the jar's
  `CraftingMatrixThread` and `ContainerRenamer` classes date from it, its `VoidCellInventory` from 2.2.41 (the void
  cell fixes after it are 2.2.43).

## Verification log

- 2026-11-02, old laptop: the earlier tree did not compile (a raw string ended at the `"#` of a colour); fixed.
  Drive and pattern milestones passed. Real bug found by the test: the event replay kept a block's state only for
  `ae2:` names, so an addon machine never became ready. ME Requester found the same; both now go through
  `event_block_is_ae2` (`ae2_world::is_machine_block`).
- 2026-11-02, this PC, on `land/atm10` with ME Requester: the laptop's `ECONNRESET` did not come back; it was the
  loaded machine. The suite then failed twice on itself, not on the port:
  - "the 36 patterns on the ground: 0": a plain `/setblock ... air` drops nothing in Java either (AE2's block
    entities are `Clearable`, and `SetBlockCommand` clears the old one first). The test breaks them with
    `setblock ... destroy` (`Level.destroyBlock`), which drops everything.
  - "the kit replaced the part", once: the server hears of a sneak with the client's next tick
    (`ServerboundPlayerInputPacket`), so a use sent right after `key shift down` can reach it unsneaking and open
    the part's menu. The test waits two client ticks first (`sneakInteract`). Other suites that sneak-use the same
    way share the race.
  - The craft request in the terminal is a middle click now (AE2's gesture on the head).
