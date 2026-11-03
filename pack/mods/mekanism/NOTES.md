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
- Next: S10 (the NeoForge oracle, to confirm the generators' numbers tick by tick), mod fluids for the heat generator's
  tank, S11's Configurator in place of `/mek connection`; the machines' next tier (factories, the Osmium Compressor,
  Combiner, Purification Chamber) on the same `machine` crate module, energy items charging machines, chemicals as a
  capability for the infuser's tank.

## How to run

    source /c/PET/atm10-work/env.sh
    bash /c/PET/atm10-work/wait-slot.sh 3   # before each cargo command or game start
    export MEK_JOBS=3 ATM10_DIR='C:\Users\User\curseforge\minecraft\Instances\All the Mods 10 - ATM10' AE2_TEST_STARTUP_MS=600000
    bun run pack/scripts/mekanism.ts content units stage build world   # MEK_TESTS=energy-cube-tiers picks one

The carries come from the player's jars: `horizon-port-pack carry <work> --resources <prepared 26.3> mekanism
mekanismgenerators` on a work folder imported from the ATM10 instance (see `pack/scripts/mekanism.ts` for the paths it
reads). The machines want the full pack's carries (`MEK_CARRY`, `MEKGEN_CARRY` from `final-data`, with ATM10's final
recipes) and its `atm10-pack` carry beside them (`MEK_PACK`), whose `minecraft:` and `c:` tags name Mekanism's blocks
and items: without them a survival pickaxe drops no cube. `mekanism-content` also takes the game's client assets (`MEK_VANILLA`) for the heat generator's lava gauge.
`chat-log` in a world test gives the newest line first.
