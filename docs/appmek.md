# Applied Mekanistics on the AE2 port

> **Handoff (2026-11-17, branch `feat/appmek-2`, worktree `atm10-wt/fluids`).** Slices 1 to 3 are built and their tests
> pass; slice 1 is below, slices 2 and 3 now:
> - **Slice 2: the ME Interface and Pattern Provider as chemical handlers.** Helpers **API 437** (the other way from
>   435; 436 belongs to another branch): `native_chemical_tanks(dimension, pos, face)`, `native_chemical_insert`,
>   `native_chemical_extract` (`game/machines/native_chemical.rs`): AE2's generic inventories as
>   `GenericStackChemicalStorage` answers them, an interface's storage slots and a provider's return slots (blocks and
>   parts), one tank a slot, the slot capacity of the type. The engine finds the chemical key type by
>   `ae2_key_types::register_chemical_type`, which the crate calls. Mekanism's `mek_chemicals::with_handler` asks them
>   first (an `EngineHandler` over the helpers), so tanks, machines and tubes push into AE2 blocks; API 435's insert
>   and extract hooks keep to the bundle's own blocks (`with_own_handler`). The Pattern Provider pushing a processing
>   pattern's chemical into a machine needed nothing new: `ae2_key_target_insert` goes through the crate's external
>   storage strategy (API 435). `xtask mod-api` has nothing to fill for the helpers (they are `Game` methods the
>   Mekanism bundle's runtime calls, not hook points).
> - **Slice 3: the terminal draws a chemical.** `ae2_key_types::register_sprite(kind, fn)`: the GUI atlas sprite and
>   tint of a key, here from the chemical's declaration (`texture`, `tint`; the `<ns>:gui/chemical/<icon ns>/<icon
>   path>` copy `mekanism-content` makes, which Mekanism's gauges draw); `item_draw` draws a wrapped key of such a type
>   with `ItemRenderer::ae2_wrapped_sprite` (the fluid path's square, shared). Amounts: the type formats as fluids do
>   (`Slot`, `SlotLargeFont`, `Full` with ` B`). Seen in `appmek-terminal.png`: hydrogen in its white-grey gas texture
>   with `1`, chlorine in yellow-green with `2`, each filling the slot as a fluid does, not the tank icon.
> - **Tests (run, pass).** Crate units `appmek-work/units.sh` (5); world `pack/mods/mekanism/tests/appmek-handlers.ts`
>   (tank ejecting into an interface reaches the cell; a second tank's chlorine too; a provider's processing pattern,
>   installed through the provider's menu, fills a tank; terminal screenshot `appmek-terminal.png`, copied to
>   `appmek-work/logs/shots/appmek-handlers/` after a pass; `item replace block <provider> container.0` does not
>   install a pattern); `appmek-chemicals` (slice 1) still passes; Mekanism `chemical-tank`, `chemical-factories`,
>   `chemical-oxidizer-infuser`; `ae2.sh build` with `appflux-cells appflux-induction-card keytypes-flux-interface
>   interface-stock-editor provider-condenser-fluid`; `cargo test -p horizon-mod-api-gen` (19, all pass here).
>   The full `build.ts` was not run.
> - **Not done:** the container item strategy (chemical tanks' items in the terminal), P2P, JEI, the chemical's name
>   in the terminal's tooltip, portable cell and cell workbench world tests.
>
> **Slice 1 (branch `feat/appmek`).**
> - **Done (slice 1):** the plan; crate `crates/horizon-ae2x-appmek` (`defs.rs`: key type `appmek:chemical` 125/op,
>   8000/byte, 1000/unit `B`, codec `{id}`, slot capacity 4000, icon `mekanism:basic_chemical_tank`; the 1k-256k
>   chemical cells, 5 types, and portable cells, 18 types, with drive models and cards; `add_key_strategies(kind,
>   "appmek", import + export + external storage)` in the rows; `game.rs`: the `Ae2Addon` whose `Ae2KeyStrategies`
>   are Applied Mekanistics' import, export and external storage strategies over the host's chemical handler;
>   `install` adds it with `horizon_ae2::add_addon`); feature `ae2-appmek`; `game/ae2_key_strategies::addon(kind)`
>   falls back to an addon crate's strategies through forwarders monomorphized per raw id (2..15) that find the
>   owner with `ae2_ext::key_strategies` and call it with the `Game` as host; `Ae2Host::grid_key_move`,
>   `chemical_tanks`, `chemical_insert`, `chemical_extract`; hook points **API 435** (`native_chemical_handler_tanks`
>   `&self` read only, `_insert`, `_extract`; `NativeChemicalTankInfo { chemical, amount, capacity, extractable }`;
>   stamped from a fragment with `stamp_native_hooks.py --only chemical_handler`, dispatch filled by `xtask mod-api`);
>   Mekanism answers them (`horizon-mixin.toml` three `[[injection]]`s binding `game`, `hooks/chemical_handler_*.rs`,
>   `mek_chemicals::{tanks, insert, extract}`; `with_handler` split into `shared_handler` (tubes, valves, ports) and
>   `entity_handler`, with a read-only twin `with_handler_read` over a copy of the block entity);
>   `pack/scripts/mekanism.ts build` builds `ae2-lab,ae2-appmek`.
> - **Tests:** `appmek-work/units.sh`: the crate's 4 unit tests pass. World test `pack/mods/mekanism/tests/appmek-chemicals.ts`
>   (`MEK_LIST=appmek-chemicals bash appmek-work/mek.sh world`): an Import Bus takes hydrogen from a Basic Chemical
>   Tank and a Storage Bus puts it into another and lists it; an Export Bus filtered to hydrogen takes it back out
>   through the Storage Bus; an Import Bus fills a 1k chemical cell; the ME Terminal lists it; it stays through a
>   restart; an Export Bus gives it back. A tank's sides take in only by default (`FRONT_OUT_EJECT`): a bus extracts
>   only through a side set to output or input/output, as in Mekanism, so the test sets them. Beside it, on the same
>   stage: Mekanism `chemical-tank`, `chemical-factories`, `chemical-oxidizer-infuser` pass; built with `ae2-appflux`
>   too (`appmek-work/ae2.sh build <suites>`), AE2 `appflux-cells`, `appflux-accessor`, `appflux-induction-card` and
>   `storage-bus-condenser-fluid` pass. The full `build.ts` was not run.
> - **Not done after slice 1 (slices 2 and 3 above did the rest):** the container item strategy, P2P, JEI; no world
>   test of a chemical in a portable cell or the cell workbench yet.
> - Work dir `atm10-work/appmek-work`: `units.sh` (the crate's units), `xtask.sh` (`cargo run -p xtask -- mod-api`),
>   `mek.sh [content stage build units world]` (own content, stage, `target-mek`; `MEK_LIST` picks world tests;
>   `AE2_ADDON_CONTENT_ROOT` = its `content`, which holds `appmek` and `appflux` from `prepare-addon.ts`); the full
>   check's `intall-content` needs `prepare-addon.ts appmek` too.

Applied Mekanistics 1.6.3 (ATM10's `Applied-Mekanistics-1.6.3.jar`, mod id `appmek`, by ramidzkh): Mekanism's
chemicals as an AE2 key type, so ME networks store, move and show them. Behaviour is read from the jar with
`javap`. Licence: the jar carries no licence file and its `neoforge.mods.toml` says "See GitHub repository for
details"; the repository (AppliedEnergistics/Applied-Mekanistics) is LGPL-3.0, so the crate is
`LGPL-3.0-or-later`, as the other AE2 crates.

## What the jar does

| Java | What |
|---|---|
| `MekanismKeyType` | `AEKeyType` `appmek:chemical` (`text.appmek.chemical`, "Chemicals"): 125 per operation, 8000 per byte, 1000 per unit, unit `B` |
| `MekanismKey` | a `ChemicalStack` of amount 1; `MAP_CODEC` `{id}` (the chemical's registry id), no components; name `Chemical.getTextComponent` |
| `ChemicalStorageCell` | `BasicStorageCell(tier.idleDrain, bytes/1024 kB, bytes/128 per type, 5 types)`, 1k-256k; `isBlackListed` = fails `ChemicalAttributeValidator.DEFAULT` (radioactive) |
| `ChemicalPortableCellItem` | `PortableCellItem` of 18 types, 1k-256k, colour `0x80caff`; same blacklist |
| `AMItems` | `chemical_cell_housing`, `chemical_storage_cell_<size>`, `portable_chemical_cell_<size>`, `chemical_p2p_tunnel` |
| `initializeUpgrades` | cells: Inverter, Equal Distribution, Void (1 each); portable cells: Inverter 1, Energy 2 |
| `AppliedMekanistics` | import, export and external storage strategies; `ContainerItemStrategy`; `GenericSlotCapacities` = the fluids' (4000) |
| `stack/*` | the three strategies over `IChemicalHandler`, AE2's fluid ones with chemicals (`ChemicalHandlerFacade`: extractable-only filter) |
| `GenericStackChemicalStorage` | the ME Interface / Pattern Provider answers `IChemicalHandler` over its generic slots |
| not in this port yet | P2P tunnel, QIO adapter, Jade/EMI, the two mixins (Condenser, `MEInventoryHandler`) |

## Design: a chemical across the bundle/engine boundary

Mekanism lives in a pack bundle (`pack/mods/mekanism`), spliced into `horizon-client`'s `game` at source level; AE2
lives in `horizon-client` and the crates above it. Neither may name the other: the engine knows no chemical, and a
build without the bundle must still build the crate.

1. **The key type**, in a new crate `horizon-ae2x-appmek` (feature `ae2-appmek` of `horizon-atm10`), above
   `horizon-client` like Ender Drives. Its rows (`install_addon_rows("appmek", ...)`, run after Applied Flux's,
   so the raw ids follow ATM10's mod order) register `appmek:chemical` with the lib's `ae2_key_types::register`
   (amounts above, codec `{id}`, slot capacity 4000) and add the cells as plain `CellDef`s of that type, so drives,
   chests, the cell workbench and portable cells use AE2's own cell inventory. A key is
   `ItemKey { item_id: "<ns>:<chemical>", components: [] }`.
2. **The world strategies** use the v2 extension point: the rows call `add_key_strategies(kind, "appmek", import,
   export, external storage)` and the crate's `Ae2Addon::key_strategies()` answers through the host.
   `game/ae2_key_strategies::addon(kind)` falls back to the owner's `Ae2KeyStrategies` for a type registered that
   way (the wiring v2 left to its first user). Its fn pointers do not carry the type, so the fallback hands out
   forwarders monomorphized per raw id (`fn entries<const RAW: u16>`, a table for raw ids 2..16) that find the
   owner with `ae2_ext::key_strategies` and call it with the `Game` as the host, as `call_addon` does.
3. **The bridge, fork API 435**: three engine hook points that mirror the fluid handler's (API 422), with no
   chemical type in the engine (ids are strings, amounts `i64`):
   - `native_chemical_handler_tanks(dimension, pos, face, tanks: &mut Option<Vec<NativeChemicalTankInfo>>)`;
   - `native_chemical_handler_insert(dimension, pos, face, chemical, amount, simulate, inserted: &mut Option<i64>)`;
   - `native_chemical_handler_extract(dimension, pos, face, chemical, amount, simulate, extracted: &mut Option<i64>)`.

   Mekanism hooks them (HEAD, bound to the `Game`) with `mek_chemicals::with_handler`, so tanks, tubes, machines
   and multiblock valves answer as they do for tubes. The tanks hook takes `&self`, as the fluid one does: AE2
   lists a storage bus's keys and an import bus's offer from `&Game` (`ExternalStorageStrategy::entries`,
   `ImportStrategy::offered`), so Mekanism answers it with a read-only twin of `with_handler` (the tubes and
   valves read their statics already; a machine's tanks come from `read_tanks` of its block entity's NBT). `Ae2Host` gains `chemical_tanks`, `chemical_insert` and
   `chemical_extract`, one-line forwards to the hook points. Without Mekanism nobody answers: no mount, nothing
   moves. API 430 stays unused; 435 is this one.
4. **Names, colours, the blacklist**: from the content's chemical declarations (`data/<ns>/mekanism/chemical/*.json`,
   docs/mekanism-chemicals.md): `texture`, `tint`, `validated` (radioactive: kept out of cells). The name is the
   translation key `chemical.<ns>.<path>`.

## Slices

1. The key type, the cells and portable cells, the hook points and the three strategies (storage bus, import and
   export bus); unit tests; a Mekanism world test: a chemical tank on a storage bus, stored into a chemical cell,
   read in the terminal, exported back.
2. The ME Interface and Pattern Provider as chemical handlers for Mekanism's machines (a hook the other way: the
   engine asks AE2 from `with_handler`'s fall-through), the container item strategy (chemical tanks' items).
3. The terminal's icon and colour for a chemical (the declaration's sprite and tint, as Mekanism's own bars draw
   it), JEI's chemical ingredients (still notes there), the P2P tunnel.
