# Mekanism bundle: working notes

Design and order of work: `.build/mekanism-native-mod-design.md`, section 6 (outside the checkout).

## State

- S0-S3 committed (2eeaed18d).
- S5 (tiers, creative semantics, config reload) and S4 (cube on Forge Energy): committed; see the log for what ran.
- S6 (engine: block entity load, unload, save, changed): committed as hooks API 64 on the old base (85020d24a), moved
  onto `int/atm10` as API 83 (Forge Energy, once 63, is 81 there). Since the upstream merge at 6e1635c1e (upstream
  API 153) they are API 156 and Forge Energy is API 154.
- S7 (Universal Cables): committed. World test `universal-cable`.
- S8 (Solar Generator) and S9 (Heat Generator): on `feat/mekanism-generators`. The world reads S8 needed are hooks API 84 (157 since the upstream merge):
  the helpers `native_biome` and `native_dimension_type`; light, weather and the day time were there (`native_light`,
  `native_weather`, `native_day_time`), and lava beside a block is `native_block`. World tests `solar-generator`,
  `heat-generator`.
- S12 (the Enrichment Chamber, Crusher, Energized Smelter and Metallurgic Infuser): on `feat/mekanism-machines`, with
  hook points API 159 (`native_cooking_recipes`, the world's smelting recipes for the smelter). World tests
  `electric-machines` and `metallurgic-infuser`; their numbers were measured on a NeoForge 1.21.1 server with only
  Mekanism 10.7.19 and ATM10's configs, driven over RCON (`/tick freeze`, `/tick step`, `data get block`; the scripts
  and logs stay in the local work folder).
- Mod fluids, slice 1 (`docs/mod-fluids.md`, branch `feat/mod-fluids`): the overlay declares heavy water and brine
  (`content/data/mekanism/horizon/fluid/`, Mekanism's own textures, tints and `BaseFlowingFluid` defaults), and
  `mekanism-content` brings the pack's `c:` fluid tags; their buckets place and take sources that flow by those
  numbers, and AE2's fluid cells hold them. World test `mod-fluid-heavy-water` (no native bundle needed).
- Mod fluids, slice 2 (branch `feat/mod-fluids-2`): all 18 of Mekanism's fluids (and Generators' 4) are the carry's,
  declared from the NeoForge oracle's server and client facts; the overlay's two hand declarations went (the oracle's
  are the same values). `mekanism-content` counts the carry's fluids and warns when a carry from before the client's
  oracle run has none: carry again then (`tools/atm10/oracle-client.sh`, docs/mod-fluids.md).
- Mod fluids, slice 5 (branch `feat/mod-fluids-5`, hook points API 160): the heat generator's lava tank is a fluid
  handler (`fluid_handler_tanks`, `fluid_handler_fill`, `fluid_handler_drain`): every side but the front, lava from any
  block, none out; its fuel slot fills it through the item's fluid handler; `fluid_tanks` is read and written as a
  `FluidStack`. World test `heat-generator-tank` (AE2's export, storage and import buses against it).
- Chemicals, slice 1 (`docs/mekanism-chemicals.md`, branch `feat/mek-chemicals-1`, hook points API 162): the
  `mekanism:chemical` registry from the content's declarations (`data/<ns>/mekanism/chemical/`, written by
  `mekanism-content --chemicals` from the server oracle's chemicals-only run, `ChemicalFacts`: 112 chemicals in ATM10
  8.4, Mekanism's 64 and Generators' 3 declared), listed with the engine's new `native_data_files`;
  `mekanism_energy::chemical` (stacks and their saved and recipe codecs, `BasicChemicalTank`, the chemical handler by
  side, ingredients, the ejector's split) and `separator`; the Metallurgic Infuser's tank as a real chemical tank behind
  its chemical sides; the Electrolytic Separator (fluid tank and fluid handler, two output tanks, dump modes,
  ejection, upgrades, screen), its numbers measured on NeoForge over RCON (local `chem-work/separator`). World tests
  `electrolytic-separator`, `infuser-chemical-tank`; `/mek chemical <pos> <face|none> ...` and `/mek chemicals [id]`.
- Chemicals, slices 2 and 3 (branch `feat/mek-chemicals-2`, no new hook point): the Basic to Creative Chemical
  Tanks (`mekanism_energy::chemical_tank`, runtime `mek_tanks`: block entity, ejector, drain and fill slots, dump
  modes, side config, screen, the item keeping its contents in `custom_data.mekanism`, tooltip and bar), the item
  chemical handler, the separator's output slots and item ejector and the infuser's infusion slot taking chemical
  items; the Basic to Ultimate Pressurized Tubes (`network` with a chemical content, runtime `mek_tubes`, their OBJ
  model from `mekanism-content`). Measured on the NeoForge oracle over RCON (local `chem-work/tanks`, t1-t4:
  `mekmach-oracle` started headless by `tanks/server.sh`). World tests `chemical-tank`, `pressurized-tube`.
- Machines 2 (branch `feat/mekanism-machines-2`, hook points API 165): the Combiner and the Precision Sawmill, the
  factories of smelting, enriching, crushing, combining and sawing in four tiers (`mekanism_energy::factory`,
  `runtime/mek_factories.rs`, screens in `runtime/mek_factory_ui.rs` on `mek_ui`'s tabs and windows), tier installers
  (`native_use_item_on`, `/mek install`), energy cube items charging machines' energy slots. World tests
  `combiner-sawmill`, `factories`, `tier-installer`; their numbers were measured on a private copy of the NeoForge
  oracle (Mekanism 10.7.19, ATM10's configs, RCON 25611; local `mekm2-work/oracle`, `x1.py`, `x2.py`). Learned for
  the next ones: the infusing factory (`TileEntityItemStackChemicalToItemStackFactory`, shared with the injecting
  and purifying ones) keeps one chemical tank for all processes, an extra slot, and `used_so_far` (a long array of
  the chemical used a process, for per-tick usage) beside `progress`; the Osmium Compressor, Purification Chamber and
  Chemical Injection Chamber are `TileEntityAdvancedElectricMachine` (an item and a chemical in, the chemical used
  each tick, randomized by `randomizedConsumption`) with `ADVANCED_MACHINE` side configs (chemical sides
  `OUT_NO_EJECT`); their factories are the same class with another recipe type. Tier installers also upgrade energy
  cubes, fluid and chemical tanks and bins (`AttributeUpgradeable` on those blocks), not ported. The Configurator's
  modes (`ItemConfigurator`) would replace `/mek connection`; multiblocks stay out.
- Transmitters, part 1 (branch `feat/mekanism-transmitters`, no new hook point): the Mechanical Pipes
  (`mekanism_energy::transmitter` for their tiers and settings, `network` as the tubes', runtime `mek_pipes`) and the
  Fluid Tanks as their sources and sinks (`fluid_tank`, runtime `mek_fluid_tanks`); `mek_transmit` is where
  `mek_world`'s hooks hand pipes, tanks, transporters and the Configurator over, one line each. Measured on the
  private oracle (`mekm2-work/oracle`, x3 and x3b): pull 250/1000/8000/32 000 mB a tick by tier, two full basic pipes
  2000 each, an active tank's output split by a normal up side, push and none taking nothing, a pull side sending
  nothing back, one fluid a network. World test `mechanical-pipe`.
- Transmitters, part 2 (hook points API 166: `native_sneaking`, `native_insert_slots`, `native_pop_resource`): the
  Logistical Transporters (`mekanism_energy::transporter`: colours, stacks, requests, the predicted insert with the
  stacks on their way, A*, idle and home paths, the pull's delay, `round_robin`; runtime `mek_transporters`).
  Measured on the oracle (x4): first pull on the second tick then every 11, arrivals on ticks 59/29/14/5 over three
  transporters by tier, the near chest of one slot taking two pulls of 32 and the far one the rest, a blocked stack
  turning home at 50 in the last transporter (idle, westward, with no home), colours, the last slot pulled first.
  World test `logistical-transporter`. Learned: transporter stacks save no path (`initiatedPath` false, worked out
  again on the first tick), `delayCount` never resets, a stack moving into a transporter that already ticked waits
  a tick.
- Transmitters, part 3: the Configurator (`mekanism_energy::configurator`, runtime `mek_configurator`;
  `native_use_item_on` now with its face, `native_right_click_block` for the plain rotate), from Mekanism 10.7.19's
  code: a block with a screen opens it on a plain use (`BlockTile.useItemOn` passes, `openGui` opens), so a machine
  is configured, emptied and turned away with a sneaking use; a plain use of the rotate mode turns it clockwise
  (`tryWrenchRotate`); the wrench mode bypasses sneaking (`doesSneakBypassUse`) to dismantle. 10.7 removed the
  Configurator's energy (no settings, no `ItemEnergized`). World test `configurator`.
- Next: chemicals slices 4-7 (`docs/mekanism-chemicals.md`: more chemical machines, radiation, Applied Mekanistics
  after AE2's key type registry, the rest of the pack's chemicals); S10 (the
  NeoForge oracle, to confirm the generators' numbers tick by tick),
  the transmitters' rest (Thermodynamic Conductors, the Diversion and Restrictive Transporters, the Logistical
  Sorter on `transporter::round_robin`, hoppers into transporters, redstone sensitivity), the Fluid Tank's screen
  and item, the Configurator's mode key;
  the chemical machines and their factories (the Osmium Compressor, Purification Chamber, Chemical Injection Chamber,
  the infusing factory), other energy items charging machines.

## How to run

    source /c/PET/atm10-work/env.sh
    bash /c/PET/atm10-work/wait-slot.sh 3   # before each cargo command or game start
    export MEK_JOBS=3 ATM10_DIR='C:\Users\User\curseforge\minecraft\Instances\All the Mods 10 - ATM10' AE2_TEST_STARTUP_MS=600000
    bun run pack/scripts/mekanism.ts content units stage build world   # MEK_TESTS=energy-cube-tiers picks one

The chemicals come from the server oracle's chemicals-only run (`-Dhorizon.oracle.chemicals_only=true` in
`HORIZON_ORACLE_JVM` with `tools/neoforge_oracle.py pack`, about 5 minutes): pass its `facts.json` as `MEK_CHEMICALS`
to the `content` step, or the content mods declare none and the infuser and separator find no recipe.

The carries come from the player's jars: `horizon-port-pack carry <work> --resources <prepared 26.3> mekanism
mekanismgenerators` on a work folder imported from the ATM10 instance (see `pack/scripts/mekanism.ts` for the paths it
reads). The machines want the full pack's carries (`MEK_CARRY`, `MEKGEN_CARRY` from `final-data`, with ATM10's final
recipes) and its `atm10-pack` carry beside them (`MEK_PACK`), whose `minecraft:` and `c:` tags name Mekanism's blocks
and items: without them a survival pickaxe drops no cube. `mekanism-content` also takes the game's client assets (`MEK_VANILLA`) for the heat generator's lava gauge.
`chat-log` in a world test gives the newest line first.
