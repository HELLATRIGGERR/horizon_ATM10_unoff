# ExtendedAE port: working notes

Branch `feat/extendedae-2` (on `land/atm10`), worktree `C:\PET\atm10-wt\eae`, target
`C:\PET\atm10-work\target-eae`. Generated content: `C:\PET\atm10-work\eae-content\extendedae`, from
`ATM10_DIR=... AE2_ADDON_CONTENT_ROOT=C:\PET\atm10-work\eae-content bun run ae2/scripts/prepare-addon.ts extendedae`;
the same root holds copies of `megacells` and `merequester` for their suites.

## State

Slice 1 is complete: rows in `horizon-client/src/ae2x/extendedae.rs` (`Defs::add_variant`, installed with ME
Requester's `ExtMachine`s by `defs::get`), windows in `game/ae2_machine_ui.rs` (`MenuShape`), layouts in
`game/ae2x/extendedae/screens.rs`, kits in `upgrade.rs`, reached through `game/ae2x/mod.rs` (`ae2x_use_on`,
`part_model`).

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
