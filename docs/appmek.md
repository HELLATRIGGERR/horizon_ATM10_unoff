# Applied Mekanistics on the AE2 port

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
   way (the wiring v2 left to its first user).
3. **The bridge, fork API 435**: three engine hook points that mirror the fluid handler's (API 422), with no
   chemical type in the engine (ids are strings, amounts `i64`):
   - `native_chemical_handler_tanks(dimension, pos, face, tanks: &mut Option<Vec<NativeChemicalTankInfo>>)`;
   - `native_chemical_handler_insert(dimension, pos, face, chemical, amount, simulate, inserted: &mut Option<i64>)`;
   - `native_chemical_handler_extract(dimension, pos, face, chemical, amount, simulate, extracted: &mut Option<i64>)`.

   Mekanism hooks them (HEAD, bound to the `Game`) with `mek_chemicals::with_handler`, so tanks, tubes, machines
   and multiblock valves answer as they do for tubes. `Ae2Host` gains `chemical_tanks`, `chemical_insert` and
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
