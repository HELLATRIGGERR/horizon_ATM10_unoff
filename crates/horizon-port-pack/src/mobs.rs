//! A mod's mobs as the game's entity type declarations
//! (`data/<ns>/horizon/entity_type/<path>.json`, docs/mobs.md): a type whose
//! class extends a vanilla mob the game has becomes that mob, its
//! **template**, under its own id, drawn with its own texture when its
//! renderer draws the template's model. The class, renderer and texture come
//! from the jar's code ([`crate::classes::mob_code`]), or the class from the
//! oracle's instance; the attributes, category, size and fire immunity from
//! the oracle (`EntityFacts`), written where they differ from the template's.
use crate::classes::{ClassCode, MobCode, Render};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// What the oracle says of an entity type (`EntityFacts`).
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
pub struct EntityFacts {
    pub category: String,
    pub width: f64,
    pub height: f64,
    pub eye_height: f64,
    pub fire_immune: bool,
    /// An instance's class and its superclasses, binary names; empty when
    /// `EntityType.create` gave none.
    #[serde(default)]
    pub class: Vec<String>,
    #[serde(default)]
    pub living: bool,
    /// Default attributes by 1.21.1 id (`minecraft:generic.max_health`).
    #[serde(default)]
    pub attributes: BTreeMap<String, f64>,
}

/// An entities-only run's facts (`horizon.oracle.entities_only`,
/// `entity-facts.json`): every namespace's types and spawn eggs.
#[derive(Debug, Default, Deserialize)]
pub struct EntityProbe {
    #[serde(default)]
    pub entity_types: BTreeMap<String, EntityFacts>,
    #[serde(default)]
    pub spawn_eggs: BTreeMap<String, String>,
}

impl EntityProbe {
    /// `entity-facts.json`, or none.
    pub fn read(path: &std::path::Path) -> Result<Option<EntityProbe>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Its types and eggs over `facts`' own.
    pub fn lay_over(self, facts: &mut crate::oracle::Facts) {
        facts.entity_types.extend(self.entity_types);
        facts.spawn_eggs.extend(self.spawn_eggs);
    }
}

/// 1.21.1's concrete mob classes under `net.minecraft.world.entity.`, and the
/// vanilla type each is.
const VANILLA_MOBS: &[(&str, &str)] = &[
    ("monster.Zombie", "zombie"),
    ("monster.Husk", "husk"),
    ("monster.Drowned", "drowned"),
    ("monster.ZombieVillager", "zombie_villager"),
    ("monster.ZombifiedPiglin", "zombified_piglin"),
    ("monster.Skeleton", "skeleton"),
    ("monster.Stray", "stray"),
    ("monster.WitherSkeleton", "wither_skeleton"),
    ("monster.Bogged", "bogged"),
    ("monster.Creeper", "creeper"),
    ("monster.Spider", "spider"),
    ("monster.CaveSpider", "cave_spider"),
    ("monster.EnderMan", "enderman"),
    ("monster.Endermite", "endermite"),
    ("monster.Silverfish", "silverfish"),
    ("monster.Slime", "slime"),
    ("monster.MagmaCube", "magma_cube"),
    ("monster.Blaze", "blaze"),
    ("monster.Ghast", "ghast"),
    ("monster.Witch", "witch"),
    ("monster.Vindicator", "vindicator"),
    ("monster.Evoker", "evoker"),
    ("monster.Pillager", "pillager"),
    ("monster.Illusioner", "illusioner"),
    ("monster.Ravager", "ravager"),
    ("monster.Vex", "vex"),
    ("monster.Guardian", "guardian"),
    ("monster.ElderGuardian", "elder_guardian"),
    ("monster.Shulker", "shulker"),
    ("monster.Phantom", "phantom"),
    ("monster.Strider", "strider"),
    ("monster.Zoglin", "zoglin"),
    ("monster.hoglin.Hoglin", "hoglin"),
    ("monster.piglin.Piglin", "piglin"),
    ("monster.piglin.PiglinBrute", "piglin_brute"),
    ("monster.warden.Warden", "warden"),
    ("monster.breeze.Breeze", "breeze"),
    ("monster.Giant", "giant"),
    ("animal.Cow", "cow"),
    ("animal.MushroomCow", "mooshroom"),
    ("animal.Pig", "pig"),
    ("animal.Sheep", "sheep"),
    ("animal.Chicken", "chicken"),
    ("animal.Rabbit", "rabbit"),
    ("animal.Wolf", "wolf"),
    ("animal.Cat", "cat"),
    ("animal.Ocelot", "ocelot"),
    ("animal.Fox", "fox"),
    ("animal.Panda", "panda"),
    ("animal.PolarBear", "polar_bear"),
    ("animal.Bee", "bee"),
    ("animal.Parrot", "parrot"),
    ("animal.Turtle", "turtle"),
    ("animal.Dolphin", "dolphin"),
    ("animal.Squid", "squid"),
    ("GlowSquid", "glow_squid"),
    ("animal.Cod", "cod"),
    ("animal.Salmon", "salmon"),
    ("animal.Pufferfish", "pufferfish"),
    ("animal.TropicalFish", "tropical_fish"),
    ("animal.IronGolem", "iron_golem"),
    ("animal.SnowGolem", "snow_golem"),
    ("animal.horse.Horse", "horse"),
    ("animal.horse.Donkey", "donkey"),
    ("animal.horse.Mule", "mule"),
    ("animal.horse.SkeletonHorse", "skeleton_horse"),
    ("animal.horse.ZombieHorse", "zombie_horse"),
    ("animal.horse.Llama", "llama"),
    ("animal.horse.TraderLlama", "trader_llama"),
    ("animal.goat.Goat", "goat"),
    ("animal.frog.Frog", "frog"),
    ("animal.frog.Tadpole", "tadpole"),
    ("animal.axolotl.Axolotl", "axolotl"),
    ("animal.camel.Camel", "camel"),
    ("animal.sniffer.Sniffer", "sniffer"),
    ("animal.allay.Allay", "allay"),
    ("animal.armadillo.Armadillo", "armadillo"),
    ("ambient.Bat", "bat"),
    ("npc.Villager", "villager"),
    ("npc.WanderingTrader", "wandering_trader"),
    ("boss.wither.WitherBoss", "wither"),
];

/// The template a class chain reaches (binary or internal names): its first
/// game class, when that is a mob the game's content mods may copy.
pub fn template_of(chain: &[String]) -> Option<String> {
    let game = chain
        .iter()
        .map(|c| c.replace('/', "."))
        .find(|c| c.starts_with("net.minecraft."))?;
    let rest = game.strip_prefix("net.minecraft.world.entity.")?;
    let (_, name) = VANILLA_MOBS.iter().find(|(c, _)| *c == rest)?;
    let id = format!("minecraft:{name}");
    horizon_mod_package::content::LIVING_TEMPLATES
        .contains(&id.as_str())
        .then_some(id)
}

/// A 1.21.1 attribute id as 26.3 names it (1.21.2 dropped the `generic.`,
/// `player.` and `zombie.` prefixes).
pub fn attribute_id(id: &str) -> String {
    match id.split_once(':') {
        Some(("minecraft", path)) => {
            let path = ["generic.", "player.", "zombie."]
                .iter()
                .find_map(|p| path.strip_prefix(p))
                .unwrap_or(path);
            format!("minecraft:{path}")
        }
        _ => id.to_owned(),
    }
}

/// Why a type was or wasn't declared, for the carry's report.
#[derive(Debug, Default, Serialize)]
pub struct Mobs {
    /// Declared types and their templates.
    pub declared: BTreeMap<String, String>,
    /// Living types left out, and why.
    pub left: BTreeMap<String, String>,
    /// Spawn eggs whose name isn't `<type>_spawn_egg`, and the type each spawns
    /// (`SpawnEggItem.getType`): the game finds an egg's type by its name.
    pub eggs_named_otherwise: BTreeMap<String, String>,
    /// Declared types whose `MobCategory` is one of the mod's own, which the
    /// game doesn't have, and that category: they take their template's.
    pub categories_of_their_own: BTreeMap<String, String>,
}

/// A type's declaration, or why there is none: a template, the texture
/// its renderer draws on the template's model, and what differs from the
/// template's facts. `rows` are the game's `entity-types.json` (the
/// template's 26.3 attributes); empty, any of the template's is kept.
pub fn declaration(
    facts: &EntityFacts,
    code: &MobCode,
    all: &BTreeMap<String, EntityFacts>,
    rows: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    let chain = if facts.class.is_empty() {
        &code.chain
    } else {
        &facts.class
    };
    let Some(template) = template_of(chain) else {
        let base = chain
            .iter()
            .find(|c| c.replace('/', ".").starts_with("net.minecraft."))
            .map_or("no class found".to_owned(), |c| {
                format!("extends {}", c.rsplit(['.', '/']).next().unwrap_or(c))
            });
        return Err(format!("no template: {base}"));
    };
    let own = template.strip_prefix("minecraft:").unwrap_or(&template);
    let texture = match code.render {
        Render::Vanilla => {
            let r = code.renderer.as_deref().unwrap_or("");
            let name = r
                .rsplit('/')
                .next()
                .unwrap_or(r)
                .strip_suffix("Renderer")
                .unwrap_or("");
            if name.to_ascii_lowercase() != own.replace('_', "") {
                return Err(format!("drawn by another vanilla renderer: {r}"));
            }
            None
        }
        Render::VanillaSubclass | Render::VanillaModel => match &code.texture {
            Some(t) => Some(t.clone()),
            None => return Err("its renderer's texture is computed".into()),
        },
        Render::NoneFound => return Err("no renderer found".into()),
        Render::OwnModel => return Err("a model of its own code".into()),
        Render::GeckoLib => return Err("a GeckoLib model".into()),
        Render::Other => return Err("drawn by a renderer of its own".into()),
    };
    let mut v = json!({"template": template});
    if let Some(t) = texture {
        v["texture"] = json!(t);
    }
    let base = all.get(&template);
    let row = rows
        .get(&template)
        .and_then(|r| r["attributes"].as_object());
    let mut attributes = serde_json::Map::new();
    for (attribute, value) in &facts.attributes {
        let same = base
            .and_then(|b| b.attributes.get(attribute))
            .is_some_and(|b| b == value);
        let attribute = attribute_id(attribute);
        // Only the template's own: the mob reads what its template's class has.
        if same || row.is_some_and(|r| !r.contains_key(&attribute)) {
            continue;
        }
        if row.is_none() && !attribute.starts_with("minecraft:") {
            continue;
        }
        attributes.insert(attribute, json!(value));
    }
    if !attributes.is_empty() {
        v["attributes"] = Value::Object(attributes);
    }
    let differs = |f: &dyn Fn(&EntityFacts) -> Value| base.is_none_or(|b| f(b) != f(facts));
    // A category of the mod's own (NeoForge's `MobCategory.create`: Aether's
    // `aether:aether_surface_monster`) isn't one the game has: the template's stays.
    let own_category =
        horizon_mod_package::content::MOB_CATEGORIES.contains(&facts.category.as_str());
    for (key, f) in [
        (
            "category",
            &(|e: &EntityFacts| json!(e.category)) as &dyn Fn(&EntityFacts) -> Value,
        ),
        ("width", &|e: &EntityFacts| json!(e.width)),
        ("height", &|e: &EntityFacts| json!(e.height)),
        ("eye_height", &|e: &EntityFacts| json!(e.eye_height)),
        ("fire_immune", &|e: &EntityFacts| json!(e.fire_immune)),
    ] {
        if differs(f) && (key != "category" || own_category) {
            v[key] = f(facts);
        }
    }
    // The game's own form (a content mod's `entity_type/`): what its content
    // root reads and checks, so no declaration stops the carry from loading.
    let read: horizon_mod_package::content::EntityType = serde_json::from_value(v.clone())
        .map_err(|e| format!("not a declaration the game reads: {e}"))?;
    read.validate_own_kind("its declaration")?;
    Ok(v)
}

/// Declares `owned` namespaces' living types into `inside`
/// (`data/<ns>/horizon/entity_type/<path>.json`) from the jar's `classes`
/// and `pngs` and the oracle's facts.
pub fn declare(
    facts: &crate::oracle::Facts,
    owned: &[&str],
    classes: &BTreeMap<String, ClassCode>,
    pngs: &[String],
    inside: &std::path::Path,
) -> Result<Mobs, String> {
    let ours = |id: &str| {
        id.split_once(':')
            .is_some_and(|(ns, _)| owned.contains(&ns))
    };
    let types: Vec<String> = facts
        .entity_types
        .iter()
        .filter(|(id, f)| ours(id) && (f.living || !f.attributes.is_empty()))
        .map(|(id, _)| id.clone())
        .collect();
    let mut mobs = Mobs::default();
    if types.is_empty() {
        return Ok(mobs);
    }
    let code = crate::classes::mob_code(classes, pngs, &types);
    for id in &types {
        let (ns, path) = id.split_once(':').unwrap_or_default();
        match declaration(
            &facts.entity_types[id],
            &code[id],
            &facts.entity_types,
            &facts.vanilla_entities,
        ) {
            Ok(v) => {
                let file = inside.join(format!("data/{ns}/horizon/entity_type/{path}.json"));
                if let Some(dir) = file.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                }
                let text = serde_json::to_vec_pretty(&v).unwrap_or_default();
                std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
                mobs.declared
                    .insert(id.clone(), v["template"].as_str().unwrap_or("").to_owned());
                let category = &facts.entity_types[id].category;
                if !horizon_mod_package::content::MOB_CATEGORIES.contains(&category.as_str()) {
                    mobs.categories_of_their_own
                        .insert(id.clone(), category.clone());
                }
            }
            Err(why) => {
                mobs.left.insert(id.clone(), why);
            }
        }
    }
    for (egg, kind) in &facts.spawn_eggs {
        if ours(egg) && mobs.declared.contains_key(kind) && format!("{kind}_spawn_egg") != *egg {
            mobs.eggs_named_otherwise.insert(egg.clone(), kind.clone());
        }
    }
    Ok(mobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(class: &[&str], attributes: &[(&str, f64)]) -> EntityFacts {
        EntityFacts {
            category: "monster".into(),
            width: 0.6,
            height: 1.99,
            eye_height: 1.74,
            fire_immune: false,
            class: class.iter().map(|c| c.to_string()).collect(),
            living: true,
            attributes: attributes
                .iter()
                .map(|(a, v)| (a.to_string(), *v))
                .collect(),
        }
    }

    #[test]
    fn a_mob_is_declared_on_its_template_with_what_differs_from_it() {
        let skeleton = "net.minecraft.world.entity.monster.Skeleton";
        let base = [
            ("minecraft:generic.max_health", 20.0),
            ("minecraft:generic.movement_speed", 0.25),
            ("neoforge:swim_speed", 1.0),
        ];
        let mut all = BTreeMap::new();
        all.insert("minecraft:skeleton".to_owned(), facts(&[skeleton], &base));
        let mut ours = facts(
            &["m.PossessedSkeleton", skeleton],
            &[
                ("minecraft:generic.max_health", 40.0),
                ("minecraft:generic.movement_speed", 0.25),
                ("neoforge:swim_speed", 2.0),
                ("minecraft:generic.flying_speed", 0.4),
            ],
        );
        ours.category = "creature".into();
        ours.width = 1.2;
        ours.fire_immune = true;
        let rows: BTreeMap<String, Value> = [(
            "minecraft:skeleton".to_owned(),
            json!({"attributes": {"minecraft:max_health": 20.0, "minecraft:movement_speed": 0.25}}),
        )]
        .into();
        let code = MobCode {
            render: Render::VanillaSubclass,
            renderer: Some("m/PossessedSkeletonRenderer".into()),
            texture: Some("m:entity/possessed_skeleton".into()),
            ..MobCode::default()
        };
        let v = declaration(&ours, &code, &all, &rows).unwrap();
        // The same speed stays the template's; an attribute the template's 26.3 row lacks is left out.
        assert_eq!(
            v,
            json!({"template": "minecraft:skeleton", "texture": "m:entity/possessed_skeleton",
                   "attributes": {"minecraft:max_health": 40.0}, "category": "creature", "width": 1.2,
                   "fire_immune": true})
        );
        // It is the game's form, and a size the game refuses is left out.
        let read: horizon_mod_package::content::EntityType =
            serde_json::from_value(v.clone()).unwrap();
        assert_eq!((read.width, read.fire_immune), (Some(1.2), Some(true)));
        let flat = EntityFacts {
            height: 0.0,
            ..ours.clone()
        };
        let e = declaration(&flat, &code, &all, &rows).unwrap_err();
        assert!(e.contains("width and height are more than 0"), "{e}");
        // A category of the mod's own (`MobCategory.create`) is not the game's: the template's stays.
        let sky = EntityFacts {
            category: "m:sky_monster".into(),
            ..ours.clone()
        };
        let v = declaration(&sky, &code, &all, &rows).unwrap();
        assert!(v.get("category").is_none(), "{v}");
        // A vanilla renderer itself: the template's own texture, when it is the template's renderer.
        let own = MobCode {
            render: Render::Vanilla,
            renderer: Some("net/minecraft/client/renderer/entity/SkeletonRenderer".into()),
            ..MobCode::default()
        };
        let same = facts(&[skeleton], &base);
        assert_eq!(
            declaration(&same, &own, &all, &rows).unwrap(),
            json!({"template": "minecraft:skeleton"})
        );
        let other = MobCode {
            renderer: Some("net/minecraft/client/renderer/entity/ZombieRenderer".into()),
            ..own.clone()
        };
        let e = declaration(&same, &other, &all, &rows).unwrap_err();
        assert!(e.contains("another vanilla renderer"), "{e}");
        // No template: an abstract base, or the oracle had no instance and the jar no class.
        let monster = facts(
            &["m.Imp", "net.minecraft.world.entity.monster.Monster"],
            &[],
        );
        let e = declaration(&monster, &own, &all, &rows).unwrap_err();
        assert_eq!(e, "no template: extends Monster");
        let e = declaration(&facts(&[], &[]), &MobCode::default(), &all, &rows).unwrap_err();
        assert_eq!(e, "no template: no class found");
        // The jar's own chain when the oracle gave no instance; the texture must be named.
        let from_jar = MobCode {
            chain: vec![
                "m/Imp".into(),
                "net/minecraft/world/entity/monster/Zombie".into(),
            ],
            render: Render::VanillaSubclass,
            ..MobCode::default()
        };
        let e = declaration(&facts(&[], &[]), &from_jar, &all, &rows).unwrap_err();
        assert!(e.contains("computed"), "{e}");
        assert_eq!(
            attribute_id("minecraft:zombie.spawn_reinforcements"),
            "minecraft:spawn_reinforcements"
        );
    }

    #[test]
    fn declare_writes_the_namespaces_living_types() {
        let dir = std::env::temp_dir().join(format!("port-pack-mobs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut f = crate::oracle::Facts::default();
        let zombie = "net.minecraft.world.entity.monster.Zombie";
        f.entity_types
            .insert("minecraft:zombie".into(), facts(&[zombie], &[]));
        f.entity_types
            .insert("m:ghoul".into(), facts(&["m.Ghoul", zombie], &[]));
        let mut arrow = facts(&["m.Bolt"], &[]);
        arrow.living = false;
        f.entity_types.insert("m:bolt".into(), arrow);
        f.entity_types
            .insert("other:ghoul".into(), facts(&["o.Ghoul", zombie], &[]));
        f.spawn_eggs.insert("m:ghoul_egg".into(), "m:ghoul".into());
        f.spawn_eggs
            .insert("m:ghoul_spawn_egg".into(), "m:ghoul".into());
        let mut classes = BTreeMap::new();
        for (name, sup) in [
            ("m/Ghoul", zombie.replace('.', "/")),
            (
                "m/GhoulRenderer",
                "net/minecraft/client/renderer/entity/ZombieRenderer".into(),
            ),
        ] {
            let bytes = crate::classes::tests_support::class_file(name, &sup);
            classes.insert(name.to_owned(), crate::classes::parse_code(&bytes).unwrap());
        }
        let pngs = ["assets/m/textures/entity/ghoul.png".to_owned()];
        let mobs = declare(&f, &["m"], &classes, &pngs, &dir).unwrap();
        assert_eq!(mobs.declared.keys().collect::<Vec<_>>(), ["m:ghoul"]);
        assert!(mobs.left.is_empty(), "{:?}", mobs.left);
        assert_eq!(
            mobs.eggs_named_otherwise.keys().collect::<Vec<_>>(),
            ["m:ghoul_egg"]
        );
        let written: Value = serde_json::from_slice(
            &std::fs::read(dir.join("data/m/horizon/entity_type/ghoul.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            written,
            json!({"template": "minecraft:zombie", "texture": "m:entity/ghoul"})
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
