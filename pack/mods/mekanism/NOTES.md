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
- Next: chemicals slices 4-7 (`docs/mekanism-chemicals.md`: more chemical machines, radiation, Applied Mekanistics
  after AE2's key type registry, the rest of the pack's chemicals); S10 (the
  NeoForge oracle, to confirm the generators' numbers tick by tick), S11's Configurator in place of `/mek connection`;
  the machines' next tier (factories, the Osmium Compressor, Combiner, Purification Chamber) on the same `machine`
  crate module, energy items charging machines.

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
