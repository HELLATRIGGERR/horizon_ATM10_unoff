# Mekanism bundle: working notes

Design and order of work: `.build/mekanism-native-mod-design.md`, section 6 (outside the checkout).

## State

- S0-S3 committed (2eeaed18d).
- S5 (tiers, creative semantics, config reload) and S4 (cube on Forge Energy): committed; see the log for what ran.
- S6 (engine: block entity load, unload, save, changed): committed as hooks API 64 on the old base (85020d24a), moved
  onto `int/atm10` as API 83 (Forge Energy, once 63, is 81 there). Since the upstream merge at 6e1635c1e (upstream
  API 153) they are API 418 and Forge Energy is API 416.
- S7 (Universal Cables): committed. World test `universal-cable`.
- S8 (Solar Generator) and S9 (Heat Generator): on `feat/mekanism-generators`. The world reads S8 needed are hooks API 84 (419 since the upstream merge):
  the helpers `native_biome` (`native_biome_climate` since the merge of 21a750533) and `native_dimension_type`; light, weather and the day time were there (`native_light`,
  `native_weather`, `native_day_time`), and lava beside a block is `native_block`. World tests `solar-generator`,
  `heat-generator`.
- S12 (the Enrichment Chamber, Crusher, Energized Smelter and Metallurgic Infuser): on `feat/mekanism-machines`, with
  hook points API 421 (`native_cooking_recipes`, the world's smelting recipes for the smelter). World tests
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
- Mod fluids, slice 5 (branch `feat/mod-fluids-5`, hook points API 422): the heat generator's lava tank is a fluid
  handler (`fluid_handler_tanks`, `fluid_handler_fill`, `fluid_handler_drain`): every side but the front, lava from any
  block, none out; its fuel slot fills it through the item's fluid handler; `fluid_tanks` is read and written as a
  `FluidStack`. World test `heat-generator-tank` (AE2's export, storage and import buses against it).
- Chemicals, slice 1 (`docs/mekanism-chemicals.md`, branch `feat/mek-chemicals-1`, hook points API 423): the
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
- Machines 2 (branch `feat/mekanism-machines-2`, hook points API 424): the Combiner and the Precision Sawmill, the
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
- Transmitters, part 2 (hook points API 425: `native_insert_slots`, `native_pop_resource`; `native_sneaking` is upstream's): the
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
- Chemicals, slice 4 (branch `feat/mek-chemicals-4`, no new hook point): the Gas-Burning Generator
  (`mekanism_energy::gas`, runtime `mek_gas`; `GeneratorKind::GasBurning` in `mek_world`'s generators), measured on a
  NeoForge oracle with Mekanism Generators as well (local `chem-work/oracle4`, driven by `chem-work/m4/*.py`). World
  test `gas-burning-generator`. The Chemical Oxidizer and Chemical Infuser (`mekanism_energy::chemical_machine`,
  runtime `mek_chem_machines`, two `MachineKind`s sharing `mek_machines`' block entity, energy, config, handler and
  probe). World test `chemical-oxidizer-infuser`. The Osmium Compressor, Purification Chamber and Chemical Injection
  Chamber (`mekanism_energy::advanced`: the per-tick chemical use, constant or drawn at random; three more
  `MachineKind`s in `mek_machines`). World test `advanced-machines` (loads All the Ores' carry beside `MEK_CARRY`,
  or `MEK_ATO_CARRY`, for the chambers' clumps and shards).
- Chemicals, slice 5 (branch `feat/mek-chemicals-5`, on machines 2, no new hook point): the Chemical Dissolution
  Chamber, Chemical Washer and Chemical Crystallizer (`mekanism_energy::ore`) and the Rotary Condensentrator
  (`mekanism_energy::rotary`), four `MachineKind`s run by `runtime/mek_ore_machines.rs` (the rotary's by
  `runtime/mek_rotary.rs`) on `mek_machines`' block
  entity, energy, config, chemical and fluid handlers (the washer's and the rotary's fluid tanks beside the
  separator's) and probe. All the Ores' slurries: the content step makes a content mod of its carry with them declared
  (`mekanism-content` with `--chemicals` and no overlay of its own; `MEK_ATO_CONTENT_DIR`). Measured on the oracle
  with Mekanism's own iron recipes (local `chem-work/m5/c1.py`). World tests `ore-chain` (All the Ores' aluminium:
  needs `MEK_ATO_CONTENT_DIR`) and `rotary-condensentrator`.
- Chemicals, slice 6 (branch `feat/mek-chemicals-6`, on the transmitters, no new hook point): the compressing,
  purifying, injecting and infusing factories (`mekanism_energy::factory`'s four more `FactoryType`s with their
  tank per process and use, run by `runtime/mek_factories.rs` on `advanced::process`; their tank a chemical handler
  through `mek_chemicals::with_handler`, their screen's chemical bar in `mek_factory_ui`), tier installers from the
  Osmium Compressor, Purification Chamber, Chemical Injection Chamber and Metallurgic Infuser. Measured on the oracle
  (local `chem-work/m6/f1.py` to `f3.py`; blocks at x 62 and beyond in f1 sat outside its force-loaded chunks and
  never ticked, so f3 repeats them). World test `chemical-factories` (needs All the Ores' carry beside `MEK_CARRY`).
  The Rotary Condensentrator's fluid ejector (`mek_rotary::eject_fluid`): API 422's `native_fluid_fill` already
  fills another block's fluid handler, so it needed no new hook point (slice 5's note said otherwise); measured
  with `chem-work/m6/f4.py`, checked in `rotary-condensentrator`.
- Multiblocks, part 1: the framework (branch `feat/mekanism-multiblocks`, hook points API 426: `native_level_particles`,
  `ServerLevel.sendParticles` from a hook outside a block's tick, for the sparkles): `mekanism_energy::multiblock`
  (cuboid, planes, the check of its blocks, the cache merge, ids) and runtime `mek_multiblock` (managers, nodes from
  the block entity lifecycle, formations in the level tick, caches in the level's attachments as Mekanism's saved
  data, the slots mirrored into the blocks that show them, the Configurator's reason). Measured on the oracle
  (`mekm2-work/oracle`, x6): a hole in a side or an edge, stone in a side, glass or a valve on an edge, dirt inside, a
  casing touching it from outside and 3x2x3 or 19 long don't form; glass in a side or beside it, 5x5x5 and 18x18x18
  do; dirt put into the middle of a formed 5x5x5 goes unseen, beside a wall it breaks it; a 3x3x3 forms 11-12 ticks
  after it is built; broken, the cache keeps the contents under the old id; formed again, every block names a new id.
  The saved data is `{cache: [{inventory_id, fluid_tanks_stored, fluid_tanks, chemical_tanks_stored, chemical_tanks,
  items_stored, items, edit_mode}]}` (the matrix's: `energy_containers_stored`, `energy_containers`, `items_stored`).
  Learned for the next multiblocks: the Thermal Evaporation Plant's validator takes `fetchCuboid`'s other form (sides
  and a tolerance: an open top), a controller block (`invalid_no_controller`, `invalid_controller_conflict`) and
  `IGNORED` positions; the Boiler adds heat capacitors to the cache (`heat_capacitors`, not in `Cache` yet), a layer
  of dispersers splitting two volumes and superheating elements inside; the SPS and the Fusion Reactor are shapes with
  their corners left out (per-position requirements over the cuboid's); the Fission Reactor's and the Turbine's
  inner columns (fuel assemblies, rotors, coils, condensers) are checked after the walls (`postcheck`), their ports
  and vents have modes, and reactor glass is the only glass that interfaces with a "reactor" manager. Radiation of
  rejected chemicals, comparators and glass opening the multiblock's screen aren't there yet.
- Multiblocks, part 2: the Dynamic Tank (`mekanism_energy::dynamic_tank`, runtime `mek_dynamic_tank`). Measured on the
  oracle (x6 D1-D6): 9 450 000 mB of water for a 3x3x3 (27 * 350 000), 43 750 000 for a 5x5x5, 2 041 200 000 for an
  18x18x18; an ultimate pipe pulling from a valve takes 32 000 a tick; a basic chemical tank putting hydrogen out of
  every side fills it by 1000 a tick through a valve, and water isn't taken then. World test `dynamic-tank`. The
  Dynamic Valve has no output mode in 10.7.19 (no `ACTIVE` attribute, no ejector): what a valve gives, others take.
- Multiblocks, part 3: the Induction Matrix (`mekanism_energy::induction`, runtime `mek_matrix`, its tick in
  `native_level_tick_pre`). Measured on the oracle (x6 M1-M4): with one basic provider a creative cube against the
  input port and an ultimate cube against the output port move 256 000 J a tick each, the cell staying put; with the
  source gone the cell loses 256 000 a tick, the last 232 000; two basic providers take in 512 000 a tick, landing in
  the cell a tick after the cube gives them; broken, what was queued lands in the cell, which keeps it; a port placed
  with `active=true` comes up in input mode (its saved `active_state`); a cell emptied saves no `energy_containers`.
  World test `induction-matrix`.
- Multiblocks, part 4: the Thermal Evaporation Plant (branch `feat/mekanism-multiblocks-2`, no new hook point:
  `mekanism_energy::evaporation`, runtime `mek_evaporation`; the framework gained `fetch_cuboid_sides` (the open
  top, tolerance 8), `validate_with` (a validator's requirements, `IGNORED` corners, a check before
  `validateFrame`), `Node::Other` (the controller), the controller failures and heat capacitors in `Cache`).
  Measured on the oracle (local `mekmb2-work/e1.py` on `mekm2-work/oracle`; `fillbiome` takes at most 32 768
  blocks, so its slabs): forms 12 ticks after it is built at 3 and 18 high, with the top corners gone, glass in a
  side or on the ring, a valve or the controller on the ring; not at 2 or 19, with two controllers, a controller on
  an edge, dirt inside, a block in the top's middle or one touching the ring from above. The cache is
  `{fluid_tanks_stored: 2, items_stored: 4, heat_capacitors_stored: 1, heat_capacitors: [{stored, heat_capacity}]}`;
  formed anew it holds its capacity times its air (a new cache's capacitor has no heat yet: `storedHeat` -1):
  540000.0005364418 of 1800 in plains (3 high; 0.8f), 594000.0 in a desert (330 K). Fed by an active creative tank above a ring valve (the brine there a tick later, the first lithium a
  tick after it): 9 mB of lithium a tick 3 high, 55 18 high, 9 of brine from water, none in plains, the input kept
  full (768 000, 4 608 000). World test `thermal-evaporation`. Not measured: the Advanced Solar Generators' heat
  (`solarMultiplier` 0.2 K a tick each, from Mekanism's code). Two fixes on the new main: the machines read an
  item ingredient's `items` (upstream's `Ingredient` is no longer a tuple, so the bundle didn't build), and a
  mechanical pipe taken out no longer pulls once more before its removal commits at the tick's end (the
  `dynamic-tank` suite saw 32 000 mB more leave after the pipe went).
- Multiblocks, part 5: the Thermoelectric Boiler and the Resistive Heater (branch `feat/mekanism-multiblocks-2`, no new
  hook point: `mekanism_energy::boiler` and `heater`, runtime `mek_boiler` and `mek_heater`; the framework gained the
  boiler's failures, the shape a formation works out (`Formed.boiler`) and the heat handed to a multiblock's capacitor
  (`heat_sink`, `hand_heat`, `take_heat`), which the evaporation plant now takes as well). Measured on the oracle
  (local `mekboil-work/b1.py` to `b3.py` on `mekm2-work/oracle`, plains): forms 12 ticks after it is built; a 3x5x3
  holds 2100 of heat capacity (50 a wall block), 544 000 mB of water (17 blocks: the bottom wall's 9 and the 8 walls
  beside the element), 18 blocks of steam; no disperser, no element, a hole in the disperser layer, a disperser below
  it, an element above it, elements apart, a valve on an edge and dirt inside don't form. A heater of 10 000 J/t
  against its wall: 300.476 K three ticks after it came, 374.390 at forty, then 19 126 mB of steam from tick 40 to
  200 out of an output valve (about 120 mB a tick), the heater at 658.71 K. Superheated sodium from a basic tank:
  399 mB cooled on the second tick (`round(0.4 * 1000 * (1 - 300 / 100000))`). One element under a creative tank:
  320 000 mB of steam a tick, its cooled tank full (25 600 000) on the fourth. A heater against a thermal
  evaporation valve (`b3.py`): the plant's heat 541 000 on the third tick, 616 604.83 on the twentieth. World test
  `boiler`. Learned:
  `renderLocation` is a block above the cuboid's corner, so the water volume's explore takes in the bottom wall that
  the air count leaves out (a pocket of air fails only once it outnumbers the bottom wall), and the steam volume runs
  from the disperser layer up through the top wall; heat a heater hands a boiler lands at the boiler's next update,
  the boil's and the coolant's own at the end of its tick; `hotMap` is read as the tick begins, so the elements light
  a tick after it passes 373.14 K; `ITileHeatHandler.simulateAdjacent` takes the heater's temperature over the air's,
  not over the sink's; an unformed casing is no heat handler, so a heater beside it keeps its heat.
- Multiblocks, part 6: the Industrial Turbine (branch `feat/mekanism-multiblocks-2`, no new hook point:
  `mekanism_energy::turbine`, runtime `mek_turbine`; the framework gained `Kind::Turbine` (vents its `OTHER` block, a
  cache of steam, vent water and Joules saved with `dumping` and no `edit_mode`), the turbine's failures and the
  shape a formation works out (`Formed.turbine`); the turbine blocks' declarations take the beacon template like the
  boiler's). Measured on a private copy of the oracle with Mekanism Generators 10.7.19 added (local
  `mekturb-work/oracle`, RCON 25631, ATM10's `generators.toml`; `t1.py`, `t2.py`): forms 12 ticks after it is built;
  5x5x5 with one rotor, a vent at the complex's height or in the top, glass and valves in the sides, three
  condensers, and 5x9x5 with five rotors form; no blade, an even length, the complex off the middle, no coil, no
  vent, a vent below the layer or on an edge, a condenser below it, a gap in the rotors, a rotor off the middle, a
  missing or an extra disperser, a coil apart from the others, six rotors 5 wide and dirt inside don't. The cache is
  `{chemical_tanks_stored: 1, fluid_tanks_stored: 1, energy_containers_stored: 1, dumping}` (no `edit_mode`).
  Creative steam into a 5x5x5 of two blades: 43 478 mB a tick, 31 055 J into an ultimate cube against a valve,
  43 478 mB of water (32 000 into a basic fluid tank against the vent, the vent tank full at 128 000 on the fifth
  tick); a 7x7x7 of four blades, two coils, condensers and vents 124 223 J and 86 956 mB; ten blades on one coil
  124 223 J; a basic tank's 1000 mB a tick 1994 mB and 4 J on the second tick, 7213 and 561 on the eighth (the flow
  is `min(stored, rate) * stored / capacity`); a boiler's steam valve into a turbine valve 31 055 J a tick from the
  boiler's third tick until its cooled tank stopped it. The dump modes come only from the screen (a client packet),
  so `t2.py` wrote them into the saved cache with the server stopped: dumping from 160 000 mB 114 192 and 81 499
  after two ticks, dumping excess from 800 000 741 392 and 687 078, idle 116 522 and 84 859; a dumping turbine fed
  1000 mB a tick keeps the last 1000 (twice the last input goes). World test `turbine` (its own copy of the configs:
  `generators.toml` too). Learned: the multiblock's tick runs before a tank placed after it, so steam pushed in lands
  for the next tick and counts as that tick's `lastSteamInput`; a turbine filled in one tick and dumping loses all of
  it on the next.
- Generators 2 and the fluid world machines (branch `feat/mekanism-generators-2`, on chemicals 6, no new hook point):
  the Bio-Generator, Wind Generator and Advanced Solar Generator (`mekanism_energy::bio`, `wind`, `solar`'s
  `AdvancedSolar`; three more `GeneratorKind`s run by `runtime/mek_gen2.rs`, which also places and removes their
  bounding blocks) and the Electric Pump and Fluidic Plenisher (`mekanism_energy::pump`, runtime `mek_pump`, their own
  block entity beside the machines'). Their Generators sources were decompiled from the jar into the local
  `chem-work/g2/mekgen-src` (the shared `src-mekgen` held only the gas generator). Measured on the oracle (local
  `chem-work/oracle4`, `chem-work/g2/w1.py`, `w1b.py`, `w2.py`): bio 350 J and one mB a tick, bio fuel 200 mB a tick
  into the tank before the burn; wind 150/150/353/607/900 J a tick at y -60/0/100/200/315, a blocked top stopping it
  at the next 20-tick look; advanced solar 1056 J at noon in plains, 2400 J out on its first tick then 2112 split
  front and bottom, three shaded checks 704, four 586, five none; the pump idle and free for 18 ticks, a source on
  the 19th, 100 J a tick after, 256 mB a tick up, water endless, 50 mB of heavy water with a filter; the plenisher's
  walk (middle, down, north, south, west, east) a block every 20 ticks with 100 J every tick while it walks. World
  tests `generators-2`, `electric-pump-plenisher`. Learned: Mekanism 10.7's wind ignores the weather; the pump and
  plenisher never `setActive`; `/setblock` places no bounding blocks; Java's `/setblock` cube has every side `NONE`,
  so the oracle's generator-to-cube runs need the cube's `component_config` merged (w1b); this engine's sky light
  under a block set while the world is held settles a tick or more later, so the tests let it settle before the
  generators look again.
- Next: the SPS and the reactors (the Fission Reactor's ports and the Fusion Reactor's frame); the turbine's rest
  (blades drawn and turning, the rotors' `position`, the statistics graphs, comparators, cables pulling from its
  valves, a rotor dropping its blades however it goes); more heat handlers (the fuelwood heater, the heat generator's
  down side and its taking heat, thermodynamic conductors) and the resistive heater's usage field (a text field in
  the bundle's screens); chemicals slices 7-9 (`docs/mekanism-chemicals.md`: radiation, Applied Mekanistics after AE2's key type
  registry, the rest of the pack's chemicals); S10 (the
  NeoForge oracle, to confirm the generators' numbers tick by tick),
  the transmitters' rest (Thermodynamic Conductors, the Diversion and Restrictive Transporters, the Logistical
  Sorter on `transporter::round_robin`, hoppers into transporters, redstone sensitivity), the Fluid Tank's screen
  and item, the Configurator's mode key;
  bounding blocks with the main block's shape that pass uses and breaks on to it; the pump's and plenisher's reset
  (the Configurator) and a game-rule read for `waterSourceConversion`;
  other energy items charging machines.

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
