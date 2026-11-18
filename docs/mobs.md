# Mod mobs and bosses: mob types as data

## Handoff (18 November 2026, branch `feat/sys-mobs`)

- **Audit numbers (ATM10 8.4):** 1,299 mod entity types registered, class found 1,165, living 630; template 228,
  abstract base only 402; natural spawns 182; spawn egg 290; texture found 632; loot carried 385; declared today 0;
  slice 1 (ready to declare) 63. Report: `atm10-work/mobs-work/mob-audit.json`.
- **Slice 1 is done, built and tested:** the audit (`tools/atm10/mob-audit.py`), this plan, and the engine side: 81
  `LIVING_TEMPLATES` and the `texture` field (`crates/horizon-mods/src/content.rs`), `texture` in the type's row
  (`crates/horizon-client/src/mod_content.rs`), the row's texture drawn (`crates/horizon-client/src/mobs.rs`), those
  textures packed for sure in the HUD atlas (`crates/horizon-client-render/src/hud.rs`, `mod_mob_textures`), the unit
  tests under Tests, and the world test `tools/atm10/mob-world.ts`. Vanilla rows of `entity-types.json` have no
  `texture` key, so vanilla mobs keep their textures.
- **Checked:** the game builds (iter); units pass (`horizon-mods` 3, `horizon-client-lib` `mod_content mobs::` 830,
  `horizon-client` lib `mods:: lan:: mod_content` 22); the world test passes and its screenshot shows Merrymaking's
  Christmas creepers in their own Santa-hat texture on the creeper model, and Occultism's spider, zombie and skeleton
  on their templates' models. Parity `mob_attributes`, `natural_spawn` and `finalize_spawn` pass. **`mob_ai` has
  not run:** it needs 26.3's client libraries (gson 2.14.0 and others), which this PC's launcher install doesn't
  have, and nothing is downloaded. Run it where `tools/lab.py prepare` works.
- **Exact next step: slice 2** (below): `horizon-port-pack` writes the declarations into the carries instead of
  `mob-audit.py --declare` writing them beside copies. The audit's class reading moves into `classes.rs`; the oracle's
  `FactsExporter` gives attributes, `MobCategory`, size and fire immunity, and the egg's `entity_data` id; the
  oracle's client run names the computed textures (Shiny's 71).
- **How to run, in `atm10-work/mobs-work`:** `build.sh` (iter game build, own target, via wait-slot, `-j 4`);
  `units.sh`; `world.sh N` (two carries from `carries-mobs`, so no `fullpack-game.lock`; it queues for the render
  lease, 3 to 10 minutes). Parity: the worktree's `.local/minecraft/26.3` is imported from the local launcher install
  and server jar without the decompiler and client libraries (no downloads), `.local/horizon-client` links to the
  main checkout's; `parity-build.sh` builds `client_probe` with `-j 4`, `parity-run.sh` runs the harnesses.
- **Test overlay:** `atm10-work/mobs-work/carries-mobs/{occultism,merrymaking}`: copies of the carries with
  `mob-audit.py --declare` output (`mobs-work/declared`) added. The generated carries are not touched.
- **Left:** slices 2-7 below.

ATM10's mods register 1,299 entity types of their own (the pack oracle's `minecraft:entity_type`, 86 namespaces). The
engine can't run their Java classes, so a mod's mob becomes one of the engine's vanilla mobs under its own id: a
**template** whose model, AI, sounds and saved data it takes, with its own attributes, texture, spawns and loot. Mod
blocks already work this way (a block copies the vanilla block its class extends). Coverage counts "Mobs and bosses" at
8 points, 0 so far (roadmap §6).

## State

- Audit: `tools/atm10/mob-audit.py` (below), report `atm10-work/mobs-work/mob-audit.json`.
- Slice 1 in the engine: a content mod's living type may copy any of 81 vanilla mobs (`LIVING_TEMPLATES` in
  `horizon-mods/src/content.rs`) and name its own `texture`. The texture goes in its row of `entity-types.json`
  (`mod_content.rs`), and the mob is drawn with it (`mobs.rs`). The HUD atlas always packs these textures, as it packs
  vanilla's (`hud.rs`); the content mods' other entity textures are optional.
- The carries don't declare entity types yet: `mob-audit.py --declare DIR` writes the declarations beside copies of
  the carries (test overlays). The next step is to have `horizon-port-pack` write them (slice 2).
- World test: `tools/atm10/mob-world.ts` runs Merrymaking and Occultism with their declarations.

## The audit

    python tools/atm10/mob-audit.py --pack <work>/pack.json --dump <dump> --carries <carries> \
        --species crates/horizon-client/src/mobs/species.rs --out mob-audit.json [--declare DIR]

The audit reads the jars only: their class files, data and assets. It also reads the oracle's `registries.json` and the
carries, and runs no Java. A type's class comes from the bytecode of its registration
(`EntityType.Builder.of(Ghoul::new, …)` next to the name it is registered under), or from its name when that fails
(`GhoulEntity`). The template is the first game class in the class's superclass chain, when that is a mob the engine
has. The renderer is the class registered with the type's field (`registerEntityRenderer(GHOUL, GhoulRenderer::new)`),
else the one named `<Name>Renderer`. 16 s on 4 processes.

ATM10 8.4, 18 November 2026:

| State | Types |
|---|--:|
| Registered by mods | 1,299 |
| Class found | 1,165 |
| Living (a mob, not a projectile, item or vehicle) | 630 |
| Template: the chain reaches a vanilla mob the engine has | 228 |
| Abstract base only (`Monster` 153, `PathfinderMob` 122, `Animal` 52, `TamableAnimal` 30, …) | 402 |
| Natural spawns in jar data (`add_spawns`, mod biomes, Aquaculture's own modifiers) | 182 (living 132) |
| A `<type>_spawn_egg` item | 290 (living 270) |
| Renderer found / texture found | 1,037 / 632 |
| Loot table carried | 385 (living 338) |
| Declared in the carries today (what the engine knows) | **0** |
| Slice 1: template, and either the template's own renderer or a vanilla renderer or model with a texture found | 63 |

What draws the 630 living types: a model in the mod's own code (`LayerDefinition` built in Java) 244, GeckoLib 98, a
vanilla renderer with its own texture 81, the template's own renderer 43, a vanilla model 38, other or not found 126.
The templates: bee 28 (Productive Bees' bee classes), enderman 23 (Enderman Overhaul), skeleton 12, zombie 11, slime 8,
spider 6… By mod: Shiny 80 of 81 have a template, Occultism 38 of 63, Enderman Overhaul 21 of 32, Ars Elemental 10.
Cataclysm (103 types, 38 bosses and monsters on abstract bases, 0 templates) and Eternal Starlight (84) are the big
abstract-base mods.

Before slice 1 the engine knew none of these. Their spawn eggs (carried as sticks) spawned nothing. Their `add_spawns`
modifiers added spawners naming types it didn't have. Their 385 loot tables were never used.

## Mob types as data

A carry declares each type in `data/<ns>/horizon/entity_type/<path>.json`:

```json
{"template": "minecraft:creeper", "texture": "merrymaking:entity/christmas_creeper",
 "attributes": {"minecraft:max_health": 30.0}}
```

- `template`: one of `LIVING_TEMPLATES` (the 1.21.1 mobs a mod's class can extend, 81). A mob of the template's
  species, under the mod's id: the model, goals or brain, sounds, saved data and synced data of the template's class
  (`MobType::class_id`). Everything keyed by the type's own id (commands, tags, loot, spawn eggs, the network id)
  sees the mod's id.
- `texture`: the renderer's `getTextureLocation`, drawn on the template's model. If it is missing, the template's
  texture is used (a type registered with the vanilla renderer itself).
- `attributes`: `EntityAttributeCreationEvent`'s values that differ from the template's. Only the template's own
  attributes are accepted. The jars don't hold these as data: the oracle has to give them (slice 2).

### Class to template

| The class chain reaches | Template | Count |
|---|---|--:|
| A concrete vanilla mob (`Zombie`, `Spider`, `Bee`…) | that mob | 228 |
| `Monster` with a melee goal set, `AbstractSkeleton`, `AbstractIllager`, `Raider` | the nearest vanilla mob of that family, chosen by the goal classes the constructor adds (`MeleeAttackGoal` → zombie, `RangedBowAttackGoal` → skeleton) | slice 4 |
| `Animal`, `TamableAnimal`, `AbstractHorse`, `WaterAnimal`, `AbstractSchoolingFish` | cow, wolf, horse, squid, cod, chosen by size and the goals | slice 4 |
| `FlyingMob`, `PathfinderMob` with flying navigation | phantom, ghast or bee, chosen by its `MoveControl` | slice 4 |
| A boss (`ServerBossEvent` in its fields, Cataclysm, Twilight Forest's bosses) | code per boss, as `wither.rs` and `dragon.rs` | slice 6 |

The goal classes a constructor adds are in its bytecode (`registerGoals`: `goalSelector.addGoal(n, new X(...))`). The
audit can read them the way it reads registrations: 762 AI goal classes in 38 jars, most of them extending vanilla
goals.

### Spawns

- Natural: the engine already applies NeoForge's `add_spawns` biome modifiers (`mod_content_biomes.rs`) and mod
  biomes' `spawners`. With the type registered, those spawners spawn it, using the template's placement
  (`SpawnPlacements`: on the ground, in water, in lava, or anywhere for flyers) and its spawn rules
  (`checkMonsterSpawnRules`). Mod placements registered in code (`RegisterSpawnPlacementsEvent`) are classes, so they
  take the template's. Aquaculture (`aquaculture:fish_spawn`, 36 files), Cataclysm, Living Things and Variants and
  Ventures add spawns through modifier types of their own: each needs a reader for its codec (slice 3). ATM10's In
  Control rules (`config/incontrol`) come after that.
- Spawn eggs: a mod's `<type>_spawn_egg` (carried as a stick) is used like vanilla's (`throw_item`, by the `_spawn_egg`
  suffix), and its type comes from its id. Eggs named differently need the egg's `minecraft:entity_data` `id` written by
  the carry (slice 2).
- Spawners and trial spawners take the id as is (`spawn_egg_on_spawner`).

### Rendering

1. Vanilla model layers (slice 1): the template's own model with the template's or the mod's texture. One catch: a
   26.3 model must match the 1.21.1 texture layout. Humanoids, skeletons, spiders, creepers, slimes and golems do; the
   cow, pig, chicken and the other variant-textured farm animals changed their UV layout in 1.21.5. A mod texture on
   one of those needs the 1.21.1 layer, carried as a model of the mod (slice 5).
2. Models in the mod's code (244 living types): `createBodyLayer()` is a chain of constant calls
   (`addOrReplaceChild`, `CubeListBuilder.texOffs/addBox`, `PartPose.offset`). Interpreting that bytecode gives the
   same JSON as vanilla's `entity models` (slice 5). Their animation (`setupAnim`) is code: it takes the template's
   `Anim` (biped, quadruped, spider…) by part names.
3. GeckoLib (98 living, 13 jars depend on it): `geo/*.geo.json` (Bedrock geometry) and `animations/*.animation.json`
   are data. A GeckoLib model kind in `mobs/render.rs` plus an animation player for idle, walk and attack (slice 7).

### Loot

Carried already: 385 entity tables (`data/<ns>/loot_table/entities/<path>.json`). The engine's death loot reads
`<ns>:entities/<path>` from the mob's own id (`drops.rs`), so a declared type drops its table with no further work.
Tables that name a mod's own loot functions or conditions are the loot work's (docs/atm10-port.md, defects table).

### Bosses

The Star chain needs the Snow Queen, Cataclysm's bosses and the Piglich (roadmap §2). Each boss is code: phases,
attacks and arenas. They come last, one boss per slice, on a template that gives them movement and a boss bar
(`ServerBossEvent`: the wither's), with their attacks written natively as `wither.rs` does.

## Slices

1. **Done (this branch):** any vanilla mob as a template, the type's own texture, its texture packed for sure; the
   audit with its `--declare` overlay; a world test (Merrymaking's Christmas creeper by its egg, Occultism's
   possessed skeleton, wild zombie and wild spider by `/summon`: they tick, walk, are drawn, and drop their loot).
2. The carry writes the declarations (`horizon-port-pack`: the audit's class reading moves into `classes.rs`).
   Attributes, `MobCategory`, size and fire immunity come from the oracle (`FactsExporter`: `DefaultAttributes`,
   `EntityType` fields), and eggs with other names get an `entity_data` id. This covers the 63 slice-1 types. 70 more have a
   template and a vanilla renderer subclass whose texture the audit doesn't find, because it is computed (Shiny's
   71): the oracle's client run can name it (`getTextureLocation` on an instance).
3. Spawns: the mod modifier codecs (Aquaculture, Cataclysm, Living Things, Variants and Ventures). A probe that counts
   natural spawns of mod types per biome against the jars' entries.
4. Abstract bases to templates by goal set and navigation (402 types).
5. Models from bytecode (`createBodyLayer`) and 1.21.1 layers of changed vanilla models.
6. Bosses, one at a time, starting with the Star chain's.
7. GeckoLib models and animations.

## Tests

- Units (`horizon-client`'s `game::mods` tests are in its lib, `--lib`, not the bin):
  `horizon-mods` `living_entity_types_copy_any_mob_a_mod_class_extends_with_its_own_texture`;
  `mod_content_tests` `a_living_mod_types_row_is_its_templates_with_its_attributes` (the texture in the row);
  the game's `mods::tests::a_mod_living_type_is_its_templates_mob_with_its_own_attributes` (a creeper copy drawn with
  its own texture).
- World: `MOB_CARRIES=<overlay> bun run tools/atm10/mob-world.ts`; look at the screenshot it prints. The pen has a
  glowstone roof: under glass the zombie and the skeleton burn in the daylight and hide the picture.
- Parity: `python tools/parity.py mob_attributes mob_ai natural_spawn finalize_spawn` (vanilla mobs must not move).
