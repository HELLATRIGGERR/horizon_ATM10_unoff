//! The tier upgrades (`ItemUpgrade*`): an item used on a furnace of one block turns it into another.

/// `(upgrade item, the block it works on, the block it makes)`, as the constructors of `ItemUpgrade*` name them.
/// Vanilla's furnace is the source of the Iron and Copper upgrades; the engine's furnace is not a mod block
/// entity, so those two are not applied here.
pub const UPGRADES: [(&str, &str, &str); 16] = [
    ("ironfurnaces:upgrade_copper", "minecraft:furnace", "ironfurnaces:copper_furnace"),
    ("ironfurnaces:upgrade_iron", "minecraft:furnace", "ironfurnaces:iron_furnace"),
    ("ironfurnaces:upgrade_iron2", "ironfurnaces:copper_furnace", "ironfurnaces:iron_furnace"),
    ("ironfurnaces:upgrade_silver", "ironfurnaces:copper_furnace", "ironfurnaces:silver_furnace"),
    ("ironfurnaces:upgrade_silver2", "ironfurnaces:iron_furnace", "ironfurnaces:silver_furnace"),
    ("ironfurnaces:upgrade_gold", "ironfurnaces:iron_furnace", "ironfurnaces:gold_furnace"),
    ("ironfurnaces:upgrade_gold2", "ironfurnaces:silver_furnace", "ironfurnaces:gold_furnace"),
    ("ironfurnaces:upgrade_diamond", "ironfurnaces:gold_furnace", "ironfurnaces:diamond_furnace"),
    ("ironfurnaces:upgrade_emerald", "ironfurnaces:diamond_furnace", "ironfurnaces:emerald_furnace"),
    ("ironfurnaces:upgrade_crystal", "ironfurnaces:diamond_furnace", "ironfurnaces:crystal_furnace"),
    ("ironfurnaces:upgrade_obsidian", "ironfurnaces:emerald_furnace", "ironfurnaces:obsidian_furnace"),
    ("ironfurnaces:upgrade_obsidian2", "ironfurnaces:crystal_furnace", "ironfurnaces:obsidian_furnace"),
    ("ironfurnaces:upgrade_netherite", "ironfurnaces:obsidian_furnace", "ironfurnaces:netherite_furnace"),
    ("ironfurnaces:upgrade_allthemodium", "ironfurnaces:netherite_furnace", "ironfurnaces:allthemodium_furnace"),
    ("ironfurnaces:upgrade_vibranium", "ironfurnaces:allthemodium_furnace", "ironfurnaces:vibranium_furnace"),
    ("ironfurnaces:upgrade_unobtainium", "ironfurnaces:vibranium_furnace", "ironfurnaces:unobtainium_furnace"),
];

/// The block `item` turns `block` into (`ItemUpgrade.useOn`: any other block passes the click on).
pub fn upgraded(item: &str, block: &str) -> Option<&'static str> {
    UPGRADES.iter().find(|(i, from, _)| *i == item && *from == block).map(|(_, _, to)| *to)
}

/// Whether `item` is one of Iron Furnaces' upgrades.
pub fn is_upgrade(item: &str) -> bool {
    UPGRADES.iter().any(|(i, ..)| *i == item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::Tier;

    #[test]
    fn each_upgrade_makes_a_tier_and_works_on_the_one_before() {
        for (item, from, to) in UPGRADES {
            assert!(Tier::of_block(to).is_some(), "{item} makes {to}");
            assert!(from == "minecraft:furnace" || Tier::of_block(from).is_some(), "{item} takes {from}");
            assert_eq!(upgraded(item, from), Some(to));
        }
    }

    #[test]
    fn an_upgrade_only_works_on_its_own_block() {
        assert_eq!(upgraded("ironfurnaces:upgrade_gold", "ironfurnaces:iron_furnace"), Some("ironfurnaces:gold_furnace"));
        assert_eq!(upgraded("ironfurnaces:upgrade_gold", "ironfurnaces:copper_furnace"), None);
        assert_eq!(upgraded("ironfurnaces:upgrade_diamond", "ironfurnaces:iron_furnace"), None);
        assert!(is_upgrade("ironfurnaces:upgrade_unobtainium") && !is_upgrade("ironfurnaces:augment_speed"));
    }

    #[test]
    fn the_chain_reaches_unobtainium_from_the_vanilla_furnace() {
        let mut block = "minecraft:furnace";
        for item in [
            "upgrade_iron", "upgrade_gold", "upgrade_diamond", "upgrade_emerald", "upgrade_obsidian", "upgrade_netherite",
            "upgrade_allthemodium", "upgrade_vibranium", "upgrade_unobtainium",
        ] {
            block = upgraded(&format!("ironfurnaces:{item}"), block).unwrap();
        }
        assert_eq!(block, "ironfurnaces:unobtainium_furnace");
    }
}
