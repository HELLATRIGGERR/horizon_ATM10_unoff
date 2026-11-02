# Mekanism bundle: working notes

Design and order of work: `.build/mekanism-native-mod-design.md`, section 6 (outside the checkout).

## State

- S0-S3 committed (2eeaed18d).
- S5 (tiers, creative semantics, config reload) and S4 (cube on Forge Energy): committed; see the log for what ran.
- S6 (engine: block entity load, unload, save, changed): committed as hooks API 64 on the old base (85020d24a), moved
  onto `int/atm10` as API 83 (Forge Energy, once 63, is 81 there).
- S7 (Universal Cables): committed. World test `universal-cable`.
- On `int/atm10` all six world tests (`mekanism.ts world`) pass, with ATM10 8.4's instance (Mekanism 10.7.19.85).
- Next after S7: S8 (the next hooks API, world reads) and the Solar Generator; S11's Configurator would replace `/mek connection`.

## How to run

    source /e/PROJECTS/horizon-ae2/env.sh
    bash /e/PROJECTS/horizon-ae2/.build/wait-slot.sh   # before each cargo command or game start
    export MEK_JOBS=3 ATM10_DIR='C:\Users\mpoka\curseforge\minecraft\Instances\All the Mods 10 - ATM10' AE2_TEST_STARTUP_MS=600000
    bun run pack/scripts/mekanism.ts content units stage build world   # MEK_TESTS=energy-cube-tiers picks one

`chat-log` in a world test gives the newest line first.
