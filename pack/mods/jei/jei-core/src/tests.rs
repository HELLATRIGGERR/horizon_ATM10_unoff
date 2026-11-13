use super::*;

fn item(id: &str, name: &str, tags: &[&str]) -> Item {
    Item { id: id.into(), name: name.into(), tags: tags.iter().map(|t| t.to_string()).collect(), ..Item::default() }
}

fn list() -> List {
    List::new(vec![
        item("mekanism:osmium_ingot", "Osmium Ingot", &["c:ingots", "c:ingots/osmium"]),
        item("ae2:fluix_crystal", "Fluix Crystal", &["ae2:all_fluix"]),
        item("minecraft:oak_log", "Oak Log", &["minecraft:logs"]),
        item("minecraft:iron_ingot", "Iron Ingot", &["c:ingots", "c:ingots/iron"]),
        item("minecraft:red_wool", "Red Wool", &["minecraft:wool"]),
        Item { tooltip: vec!["Stores energy".into()], colors: vec!["red".into()], creative_tabs: vec!["Mekanism".into()], ..item("mekanism:basic_energy_cube", "Basic Energy Cube", &[]) },
    ])
}

fn ids(l: &List, q: &str, modes: &Modes) -> Vec<String> {
    l.filter(q, modes).into_iter().map(|n| l.id(n).to_owned()).collect()
}

#[test]
fn order_is_minecraft_then_mods_by_name() {
    let l = list();
    let all: Vec<&str> = (0..l.len()).map(|n| l.id(n)).collect();
    assert_eq!(all, ["minecraft:oak_log", "minecraft:iron_ingot", "minecraft:red_wool", "ae2:fluix_crystal", "mekanism:osmium_ingot", "mekanism:basic_energy_cube"]);
}

#[test]
fn words_and_alternatives() {
    let (l, m) = (list(), Modes::default());
    assert_eq!(ids(&l, "", &m).len(), 6);
    assert_eq!(ids(&l, "INGOT", &m), ["minecraft:iron_ingot", "mekanism:osmium_ingot"]);
    assert_eq!(ids(&l, "iron ingot", &m), ["minecraft:iron_ingot"]);
    assert_eq!(ids(&l, "oak | fluix", &m), ["minecraft:oak_log", "ae2:fluix_crystal"]);
    assert_eq!(ids(&l, "ingot -iron", &m), ["mekanism:osmium_ingot"]);
    assert_eq!(ids(&l, "\"oak log\"", &m), ["minecraft:oak_log"]);
    assert_eq!(ids(&l, "oak\\ log", &m), ["minecraft:oak_log"]);
    assert!(ids(&l, "\"log oak\"", &m).is_empty());
}

#[test]
fn prefixes_with_jei_defaults() {
    let (l, m) = (list(), Modes::default());
    assert_eq!(ids(&l, "@mekanism", &m), ["mekanism:osmium_ingot", "mekanism:basic_energy_cube"]);
    assert_eq!(ids(&l, "@applied", &m), ["ae2:fluix_crystal"]);
    assert_eq!(ids(&l, "@appliedenergistics", &m), ["ae2:fluix_crystal"]);
    assert_eq!(ids(&l, "-@minecraft", &m).len(), 3);
    assert_eq!(ids(&l, "#c:ingots", &m), ["minecraft:iron_ingot", "mekanism:osmium_ingot"]);
    assert_eq!(ids(&l, "#logs", &m), ["minecraft:oak_log"]);
    assert_eq!(ids(&l, "@minecraft ingot", &m), ["minecraft:iron_ingot"]);
    // Mod names and tags need their prefix by default; the tooltip is searched without one.
    assert!(ids(&l, "mekanism", &m).is_empty());
    assert_eq!(ids(&l, "stores", &m), ["mekanism:basic_energy_cube"]);
    assert_eq!(ids(&l, "$energy", &m), ["mekanism:basic_energy_cube"]);
    // Colour, identifier and creative tab are off by default: the prefix is plain text and matches no name.
    assert!(ids(&l, "^red", &m).is_empty());
    assert!(ids(&l, "&oak", &m).is_empty());
    assert!(ids(&l, "%mekanism", &m).is_empty());
}

#[test]
fn modes_change_what_a_prefix_does() {
    let l = list();
    let mut m = Modes::default();
    m.set(Prefix::Color, Mode::RequirePrefix);
    m.set(Prefix::Identifier, Mode::RequirePrefix);
    m.set(Prefix::CreativeTab, Mode::RequirePrefix);
    assert_eq!(ids(&l, "^red", &m), ["mekanism:basic_energy_cube"]);
    assert_eq!(ids(&l, "&minecraft:oak", &m), ["minecraft:oak_log"]);
    assert_eq!(ids(&l, "%mekanism", &m), ["mekanism:basic_energy_cube"]);
    m.set(Prefix::ModName, Mode::Enabled);
    assert_eq!(ids(&l, "mekanism", &m), ["mekanism:osmium_ingot", "mekanism:basic_energy_cube"]);
    m.set(Prefix::Tooltip, Mode::Disabled);
    assert!(ids(&l, "stores", &m).is_empty());
    assert!(ids(&l, "$energy", &m).is_empty());
}

#[test]
fn modes_round_trip_and_cycle() {
    for mode in [Mode::Enabled, Mode::RequirePrefix, Mode::Disabled] {
        assert_eq!(Mode::parse(mode.name()), Some(mode));
    }
    assert_eq!(Mode::Disabled.next(), Mode::Enabled);
    assert_eq!(Modes::default().get(Prefix::Tooltip), Mode::Enabled);
    assert_eq!(give_count(false, false), 1);
    assert_eq!(give_count(true, false), 64);
}
