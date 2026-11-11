//! ExtendedAE's upgrade kits (`ItemUpgrade` and its `ItemPatternProviderUpgrade`,
//! `ItemInterfaceUpgrade`, `ItemDriveUpgrade`, `ItemIOBusUpgrade`,
//! `ItemWirelessConnectorUpgrade`), derived from ExtendedAE
//! 1.21-2.2.42, https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//!
//! A kit used on AE2's own device replaces it with the extended one, where it
//! stands and with all it holds (the addons' `kits`).
use super::super::kits::Kit;
use crate::game::Game;

pub(in crate::game) const KITS: &[Kit] = &[
    Kit {
        item: "extendedae:pattern_provider_upgrade",
        blocks: &[("ae2:pattern_provider", "extendedae:ex_pattern_provider")],
        parts: &[("ae2:cable_pattern_provider", "extendedae:ex_pattern_provider_part")],
    },
    Kit {
        item: "extendedae:interface_upgrade",
        blocks: &[("ae2:interface", "extendedae:ex_interface")],
        parts: &[("ae2:cable_interface", "extendedae:ex_interface_part")],
    },
    Kit { item: "extendedae:drive_upgrade", blocks: &[("ae2:drive", "extendedae:ex_drive")], parts: &[] },
    Kit {
        item: "extendedae:io_bus_upgrade",
        blocks: &[],
        parts: &[("ae2:export_bus", "extendedae:ex_export_bus_part"), ("ae2:import_bus", "extendedae:ex_import_bus_part")],
    },
    // ItemPatternAccessTerminalUpgrade.
    Kit { item: horizon_client::ae2x::extendedae::PATTERN_TERMINAL_KIT, blocks: &[], parts: &[("ae2:pattern_access_terminal", horizon_client::ae2x::extendedae::EX_PATTERN_TERMINAL)] },
    // The mod's own blocks: the hub loads the connector's `freq` as its first link.
    Kit {
        item: horizon_client::ae2x::extendedae::WIRELESS_HUB_KIT,
        blocks: &[(horizon_client::ae2x::extendedae::WIRELESS_CONNECTOR, horizon_client::ae2x::extendedae::WIRELESS_HUB)],
        parts: &[],
    },
];

impl Game<'_> {
    /// `ItemUpgrade.useOn` for the held item at block `pos`, side `face`.
    /// True when it replaced a device.
    pub(in crate::game) fn extendedae_kit_use(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        self.ae2x_kit_use(KITS, pos, face)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::kits::{upgraded, upgraded_part};
    use super::*;
    use horizon_client::{ae2_world::{Machine, variant}, items::ItemStack};
    use horizon_world::nbt::Tag;

    fn kit(item: &str) -> Option<&'static Kit> {
        KITS.iter().find(|kit| kit.item == item)
    }

    #[test]
    fn kits_name_the_mods_devices() {
        horizon_client::ae2x::install();
        for kit in KITS {
            for (from, to) in kit.blocks {
                match variant(to) {
                    Some(row) => assert_eq!(row.base, *from),
                    // The mod's own blocks, the Wireless Connector into the Hub.
                    None => assert!(Machine::for_block(from).unwrap().kind.ext().is_some() && Machine::for_block(to).unwrap().kind.ext().is_some()),
                }
            }
            for (from, to) in kit.parts {
                assert_eq!(variant(to).unwrap().base, *from);
            }
        }
        // ItemDriveUpgrade has no part, ItemIOBusUpgrade no block.
        assert!(kit("extendedae:drive_upgrade").unwrap().parts.is_empty());
        let buses = kit("extendedae:io_bus_upgrade").unwrap();
        assert!(buses.blocks.is_empty());
        assert_eq!(buses.parts.len(), 2);
    }

    #[test]
    fn a_kit_keeps_what_the_device_holds() {
        horizon_client::ae2x::install();
        let mut provider = Machine::for_block("ae2:pattern_provider").unwrap();
        provider.slots[8] = Some(ItemStack::new("ae2:crafting_pattern", 1));
        provider.config.insert("priority".into(), Tag::Int(7));
        let next = upgraded(&provider, "extendedae:ex_pattern_provider").unwrap();
        assert_eq!(next.slots.len(), 36);
        assert_eq!(next.slots[8], provider.slots[8]);
        assert_eq!(next.config.get(&"priority".into()), Some(&Tag::Int(7)));
        assert_eq!(next.id(), "extendedae:ex_pattern_provider");

        let mut drive = Machine::for_block("ae2:drive").unwrap();
        drive.slots[9] = Some(ItemStack::new("ae2:item_storage_cell_1k", 1));
        let next = upgraded(&drive, "extendedae:ex_drive").unwrap();
        assert_eq!((next.slots.len(), next.slots[9].clone()), (20, drive.slots[9].clone()));

        let mut interface = Machine::for_block("ae2:interface").unwrap();
        interface.upgrades[0] = Some(ItemStack::new("ae2:fuzzy_card", 1));
        let next = upgraded(&interface, "extendedae:ex_interface").unwrap();
        assert_eq!((next.slots.len(), next.upgrades.clone()), (36, interface.upgrades.clone()));

        // Never into another kind of machine.
        assert!(upgraded(&drive, "extendedae:ex_interface").is_none());
        assert!(upgraded(&drive, horizon_client::ae2x::extendedae::WIRELESS_HUB).is_none());

        // ItemWirelessConnectorUpgrade: the hub keeps the connector's cards and its link, as its first.
        use horizon_client::ae2x::extendedae as rows;
        let mut connector = Machine::for_block(rows::WIRELESS_CONNECTOR).unwrap();
        connector.upgrades[2] = Some(ItemStack::new("ae2:energy_card", 1));
        rows::set_wireless_frequency(rows::WIRELESS_CONNECTOR, &mut connector.config, 0, 99);
        let hub = upgraded(&connector, rows::WIRELESS_HUB).unwrap();
        assert_eq!((hub.id(), hub.upgrades.clone()), (rows::WIRELESS_HUB, connector.upgrades.clone()));
        assert_eq!(rows::wireless_frequencies(hub.id(), &hub.config), vec![99, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_part_keeps_its_record_under_the_new_id() {
        let mut stack = ItemStack::new("ae2:cable_interface", 1);
        stack.damage = 0;
        let part = Tag::compound([
            ("id", Tag::String("ae2:cable_interface".into())),
            ("face", Tag::String("east".into())),
            ("ae2_part_identity", Tag::IntArray(vec![1, 2, 3, 4])),
            ("stack", horizon_client::save::item_to_nbt(&stack)),
            ("machine", Tag::compound([("config", Tag::compound([("priority", Tag::Int(5))]))])),
        ]);
        let next = upgraded_part(&part, "extendedae:ex_interface_part").unwrap();
        assert_eq!(next.get("id"), Some(&Tag::String("extendedae:ex_interface_part".into())));
        assert_eq!(next.get("face"), part.get("face"));
        assert_eq!(next.get("machine"), part.get("machine"));
        assert_ne!(next.get("ae2_part_identity"), part.get("ae2_part_identity"));
        assert_eq!(horizon_client::save::item_from_nbt(next.get("stack").unwrap()).unwrap().item.as_ref(), "extendedae:ex_interface_part");
    }
}
