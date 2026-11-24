# Mod mobs and bosses: mob types as data

ATM10's mods register 1,299 entity types of their own (the pack oracle's `minecraft:entity_type`, 86 namespaces). The
engine can't run their Java classes, so a mod's mob becomes one of the engine's vanilla mobs under its own id: a
**template** whose model, AI, sounds and saved data it takes, with its own attributes, texture, spawns and loot. Mod
blocks already work this way (a block copies the vanilla block its class extends). Coverage counts "Mobs and bosses" at
8 points, 0 so far (roadmap §6).

## State

Slices 1 and 2 are on upstream's content-mod `entity_type/` (`horizon-mod-package`), the mob types as data of the
release nightly-source/2026-11-18-0b14f8a.

- **Engine.** A content mod's living type copies one of 81 vanilla mobs (`LIVING_TEMPLATES`,
  `horizon-mod-package/src/content.rs`). Upstream's declaration (`content/entity_type.rs`) has `template`,
  `attributes`, `sounds`, `shadow_radius`, `shadow_strength` and `passenger_attachments`. This fork adds `texture`
  (slice 1) and `category`, `width`, `height`, `eye_height` and `fire_immune` (slice 2), each optional and the
  template's when left out (upstream keeps the template's size, eyes and category in every mod row).
  - The content root checks them when it reads the mod (`EntityType::validate_own_kind`): a vanilla `MobCategory`
    (`MOB_CATEGORIES`), a box larger than nothing, eyes at 0 or above. A thrown type has none of them.
  - `mod_entity_row` (`horizon-client/src/mod_content/entity_sounds.rs`) writes them over the template's row of
    `entity-types.json`. `MobTypes` (`mobs.rs`) makes the mob from that row: the template's species, model, AI and
    sounds; the row's attributes, texture, category (natural spawn caps, the spawners' categories), box, eyes and fire
    immunity.
  - The HUD atlas always packs these textures, as it packs vanilla's (`mod_mob_textures`,
    `horizon-client-render/src/hud.rs`); the content mods' other entity textures are optional.
- **Port-pack.** The carries declare the mobs themselves (`horizon-port-pack carry` with the oracle's entity facts):
  - Oracle: `EntityFacts.java` (`mods/horizon/src/neoforge-oracle/`) writes every entity type of the export's
    namespaces and vanilla's: `category`, `width`, `height`, `eye_height`, `fire_immune`, the class chain of an
    instance (`EntityType.create`, never added to the world), `living`, and the default attributes
    (`DefaultAttributes`, 1.21.1 ids), then each spawn egg and the type it spawns (`SpawnEggItem.getType`). It is part
    of every batch's facts, and `-Dhorizon.oracle.entities_only=true` writes it alone for the whole pack: 528 s on
    this PC (1,299 mod types, 718 of them living instances, none failed to create; 499 eggs).
  - `classes.rs` reads code (`parse_code`: string constants, field reads, `new`s, constructor references and lambdas
    per method), and `mob_code` finds a type's class from its registration or name, its renderer from its field's
    registration or name, the render kind, and a texture from the renderer's strings, then its **own render layers'**
    strings (Shiny draws its texture as a layer over the vanilla renderer), then a PNG named for the type.
  - `mobs.rs` turns that and the facts into `data/<ns>/horizon/entity_type/<path>.json`: the template from the
    oracle's class chain (the jar's own chain without one), the texture unless the type is drawn by its template's own
    vanilla renderer, and the attributes (26.3 ids, only those of the template's 26.3 row), category, size and fire
    immunity that differ from the template's 1.21.1 facts.
  - Each declaration is read back as the game's `EntityType` and checked with `validate_own_kind` before it is
    written, so no declaration can stop its carry from loading: one the game would refuse is left, with why.
  - A `MobCategory` of the mod's own (NeoForge's `MobCategory.create`: Aether's `aether:aether_surface_monster`) isn't
    one the game has. Such a type keeps its template's category, and the carry report lists it
    (`mobs.categories_of_their_own`).
  - `carry.rs` calls it for each carried mod (the report's `mobs`: declared, left with why, eggs named otherwise,
    categories of their own); `pipeline.rs` reads `oracle/entity-facts.json` over the batch facts and the game's
    `entity-types.json`. `carry-with-facts.sh` copies `ATM10_ENTITY_FACTS` (else `dump/entity-facts.json`).
- Audit: `tools/atm10/mob-audit.py` (below), report `atm10-work/mobs-work/mob-audit.json`, the pack-wide census.
- `tools/atm10/load-check` reads carries as the content root does; `--rows <entity-types.json>` also checks each living
  type's template row and attributes, `--entity-types` lists the living types.
- World test: `tools/atm10/mob-world.ts` (Tests).

**Numbers.** ATM10 8.4: all 287 carries carried again by this port-pack, with the entity facts and the merged
engine's resources (horizon-prepare v5), into `atm10-work/mobs-work/work-up`.

| | Types |
|---|--:|
| The audit's living types | 630 |
| Declared by the carries (all among the 630) | **128** |
| Loaded by `load-check --rows` with the game's 26.3 rows | 128 (287 carries on, 0 refused) |
| Spawned by `/summon` in a world, every declaring carry loaded (`MOB_ALL=1`) | 124 of the 124 tried (Twilight Forest's 4 not: below) |
| Left with a reason | 497 |
| Not living in the oracle's facts (`ars_nouveau:bubble`, `linger`, `scryer_camera`, `tombstone:spectral_bite`, `twilightforest:slider`) | 5 |

- By mod: Shiny 66, Occultism 36, Neo Vitae 6, Twilight Forest 4, Ars Nouveau 3, Aether 3, Iron's Spellbooks 2, one
  each for 8 more (Merrymaking, Ice and Fire, Draconic Evolution, Sushi Go Crafting, MineColonies, Everything Copper,
  Dyenamics, Roots Classic).
- Of the declarations, 85 name a texture, 43 have attributes of their own (`max_health` 40, `movement_speed` 18,
  `attack_damage` 18, `armor` 17, `knockback_resistance` 14, `follow_range` 10...), 50 an eye height, 14 a width,
  13 a height, 8 a category and 10 fire immunity. Aether's blue and golden swets have a category of their own and keep
  the slime's.
- Left, of the 630: no template 397 (an abstract base: `Monster`, `PathfinderMob`, `Animal`,
  `AbstractSchoolingFish`, `TamableAnimal`...), no renderer found 40, GeckoLib 26, a model of its own code 22, a
  computed texture 8, drawn by a renderer of its own 2, drawn by another vanilla renderer 2.
- 28 eggs of declared types are named otherwise (Occultism's `spawn_egg/<type>`, Ice and Fire's `spawn_egg_<type>`):
  the game finds an egg's type by its `_spawn_egg` name, so these spawn nothing yet (slice 3).
- In a world (`MOB_ALL=1`, 21 November 2026): the 14 other declaring carries loaded together, every declared type
  summoned in turn: **124 of 124 spawned**, with their declared `max_health` except where the template sets health at
  spawn (Aether's swets and sentry on the slime: `setSize`) and float rounding (`occultism:possessed_blaze` 66.6 is
  66.5999984741211, as in Java). The run loads copies without the carries' worldgen, biome and structure modifiers and
  dimensions, and Dyenamics' and Everything Copper's recipes without their own `lazy` load condition: loaded as a
  subset, this tree refuses those (`invalid world generation field: type`; datapack validation of an unknown load
  condition), which isn't the mobs'. Twilight Forest's 4 types load (load-check) but its carry stops this tree's
  integrated server (`invalid world generation field: material identifier`), and its block tag
  `druid_projectile_replaceable` names `#minecraft:nether_carver_replaceables`, which 26.3 lacks and which fails the
  whole block tag bundle (upstream fixed that after the release, #18226).
## The audit

    python tools/atm10/mob-audit.py --pack <work>/pack.json --dump <dump> --carries <carries> \
        --species crates/horizon-client/src/mobs/species.rs --out mob-audit.json

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
| Declared by the carries (`horizon-port-pack carry` with the entity facts; State) | **128** |
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
{"template": "minecraft:skeleton",
 "attributes": {"minecraft:attack_damage": 4.0, "minecraft:max_health": 30.0}, "eye_height": 1.6915001}
```

It is upstream's content-mod declaration (`horizon-mod-package`'s `EntityType`) with this fork's `texture` and
`category`, `width`, `height`, `eye_height` and `fire_immune`; upstream's `sounds`, `shadow_radius`,
`shadow_strength` and `passenger_attachments` are there too, which the carries don't write yet.

- `template`: one of `LIVING_TEMPLATES` (the 1.21.1 mobs a mod's class can extend, 81). A mob of the template's
  species, under the mod's id: the model, goals or brain, sounds, saved data and synced data of the template's class
  (`MobType::class_id`). Everything keyed by the type's own id (commands, tags, loot, spawn eggs, the network id)
  sees the mod's id.
- `texture`: the renderer's `getTextureLocation`, drawn on the template's model. If it is missing, the template's
  texture is used (a type registered with the vanilla renderer itself).
- `attributes`: `EntityAttributeCreationEvent`'s values that differ from the template's. Only the template's own
  attributes are accepted. The jars don't hold these as data: the oracle gives them (`EntityFacts`).
- `category`, `width`, `height`, `eye_height`, `fire_immune`: the `EntityType`'s `MobCategory`, `EntityDimensions` and
  `fireImmune`, where they differ from the template's (the oracle's). `category` is one of vanilla's eight
  (`MOB_CATEGORIES`); a mod's own category leaves the template's. Placement on magma blocks and code mods'
  `entity_fire_immune` still read the template's species, not `fire_immune`.

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

1. **Done:** any vanilla mob as a template, the type's own texture, its texture packed for sure; the audit.
2. **Done:** upstream's `entity_type/` with the type's own category, size and fire immunity; the carries write the
   declarations (`classes.rs`, `mobs.rs`), with attributes, `MobCategory`, size and fire immunity from the oracle
   (`EntityFacts`), each checked as the game reads it; Shiny's textures come from their render layers' code. 128
   types. Eggs named otherwise (28) wait for slice 3.
3. Spawns: the eggs named otherwise (the game takes an egg's type from its `minecraft:entity_data` `id`, and the carry
   writes it from `spawn_eggs`), then the mod modifier codecs (Aquaculture, Cataclysm, Living Things, Variants and
   Ventures). A probe that counts natural spawns of mod types per biome against the jars' entries.
4. Abstract bases to templates by goal set and navigation (397 of the 630).
5. Models from bytecode (`createBodyLayer`) and 1.21.1 layers of changed vanilla models.
6. Bosses, one at a time, starting with the Star chain's.
7. GeckoLib models and animations.

## Tests

- Units:
  - `horizon-mod-package`: `content::tests::living_entity_types_copy_any_mob_a_mod_class_extends_with_its_own_texture`,
    `content::tests::living_entity_types_have_their_own_category_size_and_fire_immunity` (and the other
    `entity_type` tests);
  - `horizon-client-lib` (`mod_content` is in its lib): `mod_content::entity_sounds::tests::a_living_mod_types_row_is_its_templates_with_its_attributes`
    (texture, attributes, category, size and fire immunity in the row) and
    `a_living_mod_type_spawns_with_its_own_category_size_and_fire_immunity` (the row through `MobTypes`);
  - the game's (`horizon-client --lib`) `mods::tests::a_mod_living_type_is_its_templates_mob_with_its_own_attributes`.
- Port-pack: `classes::tests::mob_types_are_read_from_their_registration_and_renderers`,
  `mobs::tests::a_mob_is_declared_on_its_template_with_what_differs_from_it` (the game's form, a refused size left out,
  a mod's own category), `mobs::tests::declare_writes_the_namespaces_living_types`.
- Carries: `atm10-load-check --rows <resources>/entity-types.json --entity-types <work>/mods/*/carry`.
- World: `MOB_CARRIES=<work>/mods bun run tools/atm10/mob-world.ts` (`AE2_TEST_BINARY` the game, `AE2_RESOURCES` the
  v5 resources); look at the screenshot it prints. The pen has a glowstone roof: under glass the zombie and the
  skeleton burn in the daylight and hide the picture. `MOB_MODS=all MOB_ALL=1` (or a list) summons every declared
  type of the loaded carries and prints how many spawned; a `MOB_MODS` entry may be a carry's own folder.
- Parity: `python tools/parity.py mob_attributes mob_ai natural_spawn finalize_spawn` (vanilla mobs must not move).
