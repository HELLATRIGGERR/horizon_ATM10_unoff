//! What the book calls things that have no title of their own
//! (`QuestObjectBase.getAltTitle` of each type), and how the files' image paths
//! map to the game's sprites.

use crate::book::{ItemRef, Reward, RewardKind, Task, TaskKind};
use crate::filter::Filter;
use crate::snbt::Compound;

/// What the game knows: a language key's text, an item's or entity's name.
pub trait Names {
    /// The key's text, or `fallback`.
    fn tr(&self, key: &str, fallback: &str) -> String;
    fn item(&self, id: &str) -> String;
}

/// `namespace:path` to the language key `kind.namespace.path` (`entity.minecraft.zombie`).
pub fn id_key(kind: &str, id: &str) -> String {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("{kind}.{namespace}.{}", path.replace('/', "."))
}

/// `a_long_id` as a name, for ids the language has no key for.
pub fn pretty(id: &str) -> String {
    let path = id.rsplit(':').next().unwrap_or(id);
    let path = path.rsplit('/').next().unwrap_or(path);
    path.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn thing(names: &dyn Names, kind: &str, id: &str) -> String {
    names.tr(&id_key(kind, id), &pretty(id))
}

fn field<'a>(fields: &'a Compound, key: &str) -> &'a str {
    fields.str(key).unwrap_or_default()
}

/// An item, with its count, as `ItemTask.getAltTitle` writes it.
fn item_title(names: &dyn Names, item: &ItemRef, count: u64, filter: Option<&Filter>) -> String {
    let name = match filter {
        Some(f) => filter_name(names, f),
        None => names.item(&item.id),
    };
    if count > 1 { format!("{count}x {name}") } else { name }
}

/// A filter in words: its items, or `Any #tag`.
pub fn filter_name(names: &dyn Names, filter: &Filter) -> String {
    match filter {
        Filter::Item(id) => names.item(id),
        Filter::Tag(tag) => format!("#{tag}"),
        Filter::Mod(name) => format!("@{name}"),
        Filter::Block => names.tr("ftbquests.filter.block", "Any block"),
        Filter::Component { json, .. } => json.clone(),
        Filter::Or(all) | Filter::And(all) => {
            let parts: Vec<String> = all.iter().take(3).map(|f| filter_name(names, f)).collect();
            let more = if all.len() > 3 { ", ..." } else { "" };
            format!("{}{more}", parts.join(", "))
        }
        Filter::Not(inner) => format!("not {}", filter_name(names, inner)),
        Filter::Unknown(text) => text.clone(),
    }
}

/// A task's title: its own, or `getAltTitle` of its type.
pub fn task_title(names: &dyn Names, task: &Task) -> String {
    if !task.title.is_empty() {
        return task.title.clone();
    }
    match &task.kind {
        TaskKind::Item { item, count, filter, .. } => item_title(names, item, *count, filter.as_ref()),
        TaskKind::Checkmark => names.tr("ftbquests.task.ftbquests.checkmark", "Checkmark"),
        TaskKind::Other { kind, fields } => other_task_title(names, kind, fields),
    }
}

fn other_task_title(names: &dyn Names, kind: &str, fields: &Compound) -> String {
    let labelled = |key: &str, fallback: &str, value: String| {
        let label = names.tr(key, fallback);
        if value.is_empty() { label } else { format!("{label}: {value}") }
    };
    match kind {
        "kill" => {
            let count = fields.i64("value").unwrap_or(1);
            let entity = fields.str("entity").map_or_else(
                || fields.str("entity_type_tag").map_or_else(String::new, |t| format!("#{t}")),
                |e| thing(names, "entity", e),
            );
            let template = names.tr("ftbquests.task.ftbquests.kill.title", "Kill: %sx %s");
            template.replacen("%s", &count.to_string(), 1).replacen("%s", &entity, 1)
        }
        "dimension" => {
            let id = field(fields, "dimension");
            labelled("ftbquests.task.ftbquests.dimension", "Visit Dimension", thing(names, "dimension", id))
        }
        "biome" => {
            let id = field(fields, "biome");
            labelled("ftbquests.task.ftbquests.biome", "Visit Biome", thing(names, "biome", id))
        }
        "structure" => {
            let id = field(fields, "structure");
            labelled("ftbquests.task.ftbquests.structure", "Find Structure", pretty(id))
        }
        "advancement" => {
            let id = field(fields, "advancement");
            labelled("ftbquests.task.ftbquests.advancement", "Advancement", pretty(id))
        }
        "observation" => {
            let id = field(fields, "to_observe");
            labelled("ftbquests.task.ftbquests.observation", "Observation", pretty(id))
        }
        "forge_energy" => {
            let value = fields.i64("value").unwrap_or(0);
            names.tr("ftbquests.task.ftbquests.forge_energy.text", "%d FE").replacen("%d", &value.to_string(), 1)
        }
        "xp" => {
            let value = fields.i64("value").unwrap_or(0);
            labelled("ftbquests.task.ftbquests.xp", "XP Levels", value.to_string())
        }
        "location" => names.tr("ftbquests.task.ftbquests.location", "Location"),
        "stat" => labelled("ftbquests.task.ftbquests.stat", "Stat", pretty(field(fields, "stat"))),
        "fluid" => labelled("ftbquests.task.ftbquests.fluid", "Fluid", pretty(field(fields, "fluid"))),
        other => names.tr(&format!("ftbquests.task.ftbquests.{other}"), &pretty(other)),
    }
}

/// A reward's title: its own, or `getAltTitle` of its type.
pub fn reward_title(names: &dyn Names, reward: &Reward, table: &dyn Fn(i64) -> Option<String>) -> String {
    if !reward.title.is_empty() {
        return reward.title.clone();
    }
    match &reward.kind {
        RewardKind::Item { item, count, .. } => {
            let name = names.item(&item.id);
            if *count > 1 { format!("{count}x {name}") } else { name }
        }
        RewardKind::Other { kind, fields } => other_reward_title(names, kind, fields, table),
    }
}

fn other_reward_title(names: &dyn Names, kind: &str, fields: &Compound, table: &dyn Fn(i64) -> Option<String>) -> String {
    match kind {
        "xp" => format!("{}: +{}", names.tr("ftbquests.reward.ftbquests.xp", "XP"), fields.i64("xp").unwrap_or(100)),
        "xp_levels" => {
            format!("{}: +{}", names.tr("ftbquests.reward.ftbquests.xp_levels", "XP Levels"), fields.i64("xp_levels").unwrap_or(5))
        }
        "random" | "choice" | "loot" | "all_table" => {
            let label = names.tr(&format!("ftbquests.reward.ftbquests.{kind}"), &pretty(kind));
            match fields.i64("table_id").and_then(table) {
                Some(title) if !title.is_empty() => format!("{label}: {title}"),
                _ => label,
            }
        }
        "command" => format!("{}: {}", names.tr("ftbquests.reward.ftbquests.command", "Command"), field(fields, "command")),
        "toast" => names.tr("ftbquests.reward.ftbquests.toast", "Toast"),
        other => names.tr(&format!("ftbquests.reward.ftbquests.{other}"), &pretty(other)),
    }
}

/// The game's GUI sprite for a path in the files: `atm:textures/a/b.png` is
/// `atm:gui/a/b` once something copied it under `textures/gui`, FTB Quests' own
/// `ftbquests:textures/shapes/x/y.png` is the GUI mod's
/// `ftbquests_gui:gui/shapes/x/y`, `minecraft:block/stone` is vanilla's
/// `block/stone`, and another namespace's `ns:item/x` stays as it is.
pub fn sprite_name(path: &str) -> String {
    let path = path.strip_suffix(".png").unwrap_or(path);
    let (namespace, rest) = path.split_once(':').unwrap_or(("minecraft", path));
    // What `ftbquests-content` copies goes under its own namespace.
    if let "ftbquests" | "ftblibrary" = namespace {
        if let Some(inner) = rest.strip_prefix("textures/gui/").or_else(|| rest.strip_prefix("textures/")) {
            return format!("ftbquests_gui:gui/{inner}");
        }
    }
    let rest = match rest.strip_prefix("textures/") {
        Some(inner) if !inner.starts_with("gui/") && !inner.starts_with("item/") && !inner.starts_with("block/") => format!("gui/{inner}"),
        Some(inner) => inner.to_owned(),
        None => rest.to_owned(),
    };
    if namespace == "minecraft" { rest } else { format!("{namespace}:{rest}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::Tristate;
    use crate::snbt::parse_compound;

    struct Lang;

    impl Names for Lang {
        fn tr(&self, key: &str, fallback: &str) -> String {
            match key {
                "ftbquests.task.ftbquests.kill.title" => "Kill: %sx %s".into(),
                "entity.minecraft.zombie" => "Zombie".into(),
                _ => fallback.into(),
            }
        }
        fn item(&self, id: &str) -> String {
            pretty(id)
        }
    }

    fn other_task(text: &str) -> Task {
        let fields = parse_compound(text).unwrap();
        let kind = fields.str("type").unwrap().to_owned();
        Task { id: 1, title: String::new(), icon: None, optional: false, kind: TaskKind::Other { kind, fields } }
    }

    #[test]
    fn a_tasks_own_title_wins() {
        let mut t = other_task(r#"{ type: "kill", entity: "minecraft:zombie", value: 3L }"#);
        assert_eq!(task_title(&Lang, &t), "Kill: 3x Zombie");
        t.title = "Slay".into();
        assert_eq!(task_title(&Lang, &t), "Slay");
    }

    #[test]
    fn unsupported_tasks_read_as_the_mod_writes_them() {
        assert_eq!(task_title(&Lang, &other_task(r#"{ type: "dimension", dimension: "aether:the_aether" }"#)), "Visit Dimension: The Aether");
        assert_eq!(task_title(&Lang, &other_task(r#"{ type: "biome", biome: "minecraft:warm_ocean" }"#)), "Visit Biome: Warm Ocean");
        assert_eq!(task_title(&Lang, &other_task(r#"{ type: "structure", structure: "aether:bronze_dungeon" }"#)), "Find Structure: Bronze Dungeon");
        assert_eq!(task_title(&Lang, &other_task(r#"{ type: "forge_energy", value: 1000L }"#)), "1000 FE");
        assert_eq!(task_title(&Lang, &other_task(r#"{ type: "location" }"#)), "Location");
        assert_eq!(task_title(&Lang, &other_task(r#"{ type: "strange" }"#)), "Strange");
    }

    #[test]
    fn items_read_with_their_count() {
        let t = Task {
            id: 1,
            title: String::new(),
            icon: None,
            optional: false,
            kind: TaskKind::Item {
                item: ItemRef { id: "minecraft:oak_log".into(), count: 1, components: None },
                count: 16,
                consume: Tristate::Default,
                only_from_crafting: Tristate::Default,
                match_components: crate::book::MatchComponents::None,
                task_screen_only: false,
                filter: None,
            },
        };
        assert_eq!(task_title(&Lang, &t), "16x Oak Log");
    }

    #[test]
    fn filters_in_words() {
        let f = Filter::parse("or(item(a:one)item(a:two)item(a:three)item(a:four))");
        assert_eq!(filter_name(&Lang, &f), "One, Two, Three, ...");
        assert_eq!(filter_name(&Lang, &Filter::parse("item_tag(c:ingots/iron)")), "#c:ingots/iron");
    }

    #[test]
    fn rewards_read_as_the_mod_writes_them() {
        let reward = |text: &str| {
            let fields = parse_compound(text).unwrap();
            let kind = fields.str("type").unwrap().to_owned();
            Reward { id: 1, title: String::new(), icon: None, team: Tristate::Default, auto: String::new(), exclude_from_claim_all: false, kind: RewardKind::Other { kind, fields } }
        };
        let none = |_: i64| None;
        let table = |_: i64| Some("Starter Loot".to_owned());
        assert_eq!(reward_title(&Lang, &reward(r#"{ type: "xp", xp: 100 }"#), &none), "XP: +100");
        assert_eq!(reward_title(&Lang, &reward(r#"{ type: "xp_levels", xp_levels: 5 }"#), &none), "XP Levels: +5");
        assert_eq!(reward_title(&Lang, &reward(r#"{ type: "random", table_id: 5L }"#), &table), "Random: Starter Loot");
        assert_eq!(reward_title(&Lang, &reward(r#"{ type: "command", command: "/say hi" }"#), &none), "Command: /say hi");
    }

    #[test]
    fn image_paths_map_to_sprites() {
        assert_eq!(sprite_name("atm:textures/questpics/a/b.png"), "atm:gui/questpics/a/b");
        assert_eq!(sprite_name("ftbquests:textures/shapes/square/outline.png"), "ftbquests_gui:gui/shapes/square/outline");
        assert_eq!(sprite_name("minecraft:block/torchflower"), "block/torchflower");
        assert_eq!(sprite_name("alltheores:item/iron_dust"), "alltheores:item/iron_dust");
        assert_eq!(sprite_name("ftbquests:textures/gui/close.png"), "ftbquests_gui:gui/close");
        assert_eq!(sprite_name("minecraft:textures/item/apple.png"), "item/apple");
    }

    #[test]
    fn id_keys_and_pretty_names() {
        assert_eq!(id_key("entity", "minecraft:zombie"), "entity.minecraft.zombie");
        assert_eq!(id_key("biome", "a:b/c"), "biome.a.b.c");
        assert_eq!(pretty("aether:bronze_dungeon"), "Bronze Dungeon");
    }
}
