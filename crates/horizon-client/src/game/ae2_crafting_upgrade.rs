//! Source in-world crafting-unit component exchange, AE2 19.2.18 (LGPL-3.0+).
//! AbstractCraftingUnitBlock and active CraftingUnitTransformRecipe records,
//! with MEGA Cells 4.11.0's `AbstractCraftingUnitBlockMixin` (LGPL-3.0) over
//! every crafting block: a block changes only into blocks of its own
//! namespace, and MEGA's crafting unit takes a component as AE2's does.
use super::{Game, KeyCode};
use horizon_client::{
    ae2_world::{self, MachineKind},
    items::ItemStack,
    text::Text,
};
use serde_json::Value;
type Recipes = [(String, Value)];
fn namespace(id: &str) -> &str {
    id.split_once(':').map_or("minecraft", |(ns, _)| ns)
}
/// `AbstractCraftingUnitBlockMixin.removeMegaUpgrade`: the plain unit an
/// alternate use leaves, MEGA's crafting unit for MEGA's blocks and AE2's
/// for the rest.
fn plain_unit(block: &str) -> &'static str {
    ae2_world::crafting_units()
        .find(|unit| unit.kind == MachineKind::CraftingUnit && namespace(unit.id) == namespace(block))
        .map_or("ae2:crafting_unit", |unit| unit.id)
}
struct Change {
    next: &'static str,
    returned: Option<ItemStack>,
    consume: bool,
}
/// `AbstractCraftingUnitBlock.upgrade`, and `useWithoutItem`'s
/// `removeUpgrade`, on block `old`.
fn change(
    old: &str,
    held: Option<&ItemStack>,
    alternate: bool,
    hands_full: bool,
    recipes: &Recipes,
) -> Option<Change> {
    let unit = ae2_world::crafting_unit(old)?;
    if alternate && hands_full {
        return None;
    }
    // `type == UNIT`, and the mixin's `isMegaUnit`: a plain unit has no
    // upgrade to give back.
    let plain = unit.kind == MachineKind::CraftingUnit;
    // CraftingUnitTransformRecipe.getRemovedUpgrade.
    let old_part = || {
        recipes
            .iter()
            .find(|(_, r)| r["upgraded_block"].as_str() == Some(old))
            .and_then(|(_, r)| r["upgrade_item"].as_str())
            .map(|id| ItemStack::new(id, 1))
    };
    if let Some(held) = held.filter(|s| s.count > 0) {
        // getUpgradedBlock, and the mixin's `isMegaUpgrade`: an upgrade into
        // a block of another namespace is none. MEGA takes the first recipe
        // of the held item the recipe manager lists, in no fixed order, and
        // refuses it when it is the other mod's; the port takes the first of
        // the block's own namespace (README, slice 2).
        let (_, recipe) = recipes.iter().find(|(_, r)| {
            r["upgrade_item"].as_str() == Some(held.item.as_ref())
                && r["upgraded_block"].as_str().is_some_and(|next| namespace(next) == namespace(old))
        })?;
        let next = ae2_world::crafting_unit(recipe["upgraded_block"].as_str()?)?;
        if next.id == old {
            return None;
        }
        let returned = if plain { None } else { Some(old_part()?) };
        Some(Change {
            next: next.id,
            returned,
            consume: true,
        })
    } else if alternate && !plain {
        Some(Change {
            next: plain_unit(old),
            returned: Some(old_part()?),
            consume: false,
        })
    } else {
        None
    }
}
impl Game<'_> {
    pub(super) fn ae2_predict_crafting_upgrade(
        &self,
        pos: [i32; 3],
        held: Option<&ItemStack>,
    ) -> bool {
        let block = self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]));
        let hands_full = self.inventory[self.selected].is_some() || self.offhand.is_some();
        change(
            &block.name,
            held,
            self.keys.contains(&KeyCode::ShiftLeft),
            hands_full,
            self.res
                .recipes()
                .ae2_custom
                .get("ae2:crafting_unit_transform")
                .map_or(&[][..], Vec::as_slice),
        )
        .is_some()
    }
    pub(super) fn ae2_try_crafting_upgrade(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        if self.remote.is_some() || self.spectator() {
            return false;
        }
        let old = self.world.block(pos[0], pos[1], pos[2]);
        let name = self.res.facts().get(old).name.clone();
        let Some(unit) = ae2_world::crafting_unit(&name) else {
            return false;
        };
        let hands_full = self.inventory[self.selected].is_some() || self.offhand.is_some();
        let Some(change) = change(
            &name,
            self.held(),
            self.keys.contains(&KeyCode::ShiftLeft),
            hands_full,
            self.res
                .recipes()
                .ae2_custom
                .get("ae2:crafting_unit_transform")
                .map_or(&[][..], Vec::as_slice),
        ) else {
            return false;
        };
        let key = (pos[0], pos[1], pos[2]);
        self.ae2_ensure_machine(key);
        if unit.kind != MachineKind::CraftingUnit && self.ae2_crafting_cluster_busy(key) {
            self.system_message(Text::translate("chat.ae2.CraftingCpuBusy", &[]));
            return true;
        }
        if change
            .returned
            .as_ref()
            .is_some_and(|s| self.res.items().get(&s.item).is_none())
        {
            return false;
        }
        let registry = self.pack.assets.registry;
        let Some(base) = registry.default_state(change.next) else {
            return false;
        };
        let Some(mut target) = registry.state(base).cloned() else {
            return false;
        };
        if target.properties.contains_key("facing") {
            let facing = match face {
                [0, -1, 0] => "down",
                [0, 1, 0] => "up",
                [0, 0, -1] => "north",
                [0, 0, 1] => "south",
                [-1, 0, 0] => "west",
                _ => "east",
            };
            target.properties.insert("facing".into(), facing.into());
        }
        let Ok(next) = registry.resolve(&target) else {
            return false;
        };
        if let Err(error) = self.ae2_break_crafting_cluster(key) {
            self.command_failure(Text::literal(format!("AE2 crafting block: {error}")));
            return true;
        }
        let previous = self.world.block_entities.get(&key).cloned();
        let name = self.world.block_names.get(&key).cloned();
        let changed = self.level_set(pos, self.world.air).is_ok()
            && self.level_set(pos, next).is_ok()
            && self.world.block(pos[0], pos[1], pos[2]) == next;
        if !changed {
            let _ = self.level_set(pos, old);
            if let Some(previous) = previous {
                self.world.block_entities.insert(key, previous);
            }
            if let Some(name) = name {
                self.world.block_names.insert(key, name);
            }
            return false;
        }
        self.ae2_ensure_machine(key);
        self.audio.play(
            "minecraft:entity.item_frame.remove_item",
            Some(pos.map(|p| f64::from(p) + 0.5)),
            0.5,
            1.0,
        );
        // Source removeUpgrade returns the old component before consuming the
        // held one. With a full inventory that ordering can cause overflow.
        if let Some(returned) = change.returned {
            if let Some(rest) = self.give(returned) {
                self.player_drop(rest);
            }
        }
        if change.consume {
            self.consume_held();
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn recipe(item: &str, block: &str) -> (String, Value) {
        (block.into(), serde_json::json!({"upgrade_item": item, "upgraded_block": block}))
    }
    #[test]
    fn source_crafting_component_exchange_uses_active_recipe_and_alternate_rules() {
        let recipes = vec![
            recipe("ae2:cell_component_1k", "ae2:1k_crafting_storage"),
            recipe("ae2:cell_component_4k", "ae2:4k_crafting_storage"),
        ];
        let c = change(
            "ae2:crafting_unit",
            Some(&ItemStack::new("ae2:cell_component_1k", 2)),
            false,
            true,
            &recipes,
        )
        .unwrap();
        assert_eq!(c.next, "ae2:1k_crafting_storage");
        assert!(c.returned.is_none() && c.consume);
        let c = change(
            "ae2:1k_crafting_storage",
            Some(&ItemStack::new("ae2:cell_component_4k", 1)),
            false,
            true,
            &recipes,
        )
        .unwrap();
        assert_eq!(c.returned, Some(ItemStack::new("ae2:cell_component_1k", 1)));
        let c = change("ae2:4k_crafting_storage", None, true, false, &recipes).unwrap();
        assert_eq!(c.next, "ae2:crafting_unit");
        assert!(!c.consume);
        assert_eq!(c.returned, Some(ItemStack::new("ae2:cell_component_4k", 1)));
        assert!(
            change(
                "ae2:crafting_unit",
                Some(&ItemStack::new("ae2:cell_component_1k", 1)),
                true,
                true,
                &recipes
            )
            .is_none()
        );
        assert!(
            change(
                "ae2:1k_crafting_storage",
                Some(&ItemStack::new("ae2:cell_component_1k", 1)),
                false,
                true,
                &recipes
            )
            .is_none()
        );
    }

    /// MEGA's `AbstractCraftingUnitBlockMixin`: MEGA's unit upgrades in place
    /// into MEGA's blocks only, AE2's into AE2's only, and a MEGA block goes
    /// back to MEGA's unit. The engineering processor and the storage monitor
    /// upgrade both mods' units, each into its own mod's block.
    #[cfg(feature = "ae2-megacells")]
    #[test]
    fn mega_crafting_blocks_change_within_their_own_namespace() {
        horizon_client::ae2x::install();
        let recipes = vec![
            recipe("ae2:engineering_processor", "ae2:crafting_accelerator"),
            recipe("ae2:storage_monitor", "ae2:crafting_monitor"),
            recipe("ae2:cell_component_1k", "ae2:1k_crafting_storage"),
            recipe("megacells:cell_component_1m", "megacells:1m_crafting_storage"),
            recipe("megacells:cell_component_256m", "megacells:256m_crafting_storage"),
            recipe("ae2:engineering_processor", "megacells:mega_crafting_accelerator"),
            recipe("ae2:storage_monitor", "megacells:mega_crafting_monitor"),
        ];
        let held = |id: &str| ItemStack::new(id, 1);
        let c = change("megacells:mega_crafting_unit", Some(&held("megacells:cell_component_1m")), false, false, &recipes).unwrap();
        assert_eq!((c.next, c.returned, c.consume), ("megacells:1m_crafting_storage", None, true));
        // AE2's unit takes no MEGA component, MEGA's no AE2 one.
        assert!(change("ae2:crafting_unit", Some(&held("megacells:cell_component_1m")), false, false, &recipes).is_none());
        assert!(change("megacells:mega_crafting_unit", Some(&held("ae2:cell_component_1k")), false, false, &recipes).is_none());
        for (unit, accelerator, monitor) in [
            ("ae2:crafting_unit", "ae2:crafting_accelerator", "ae2:crafting_monitor"),
            ("megacells:mega_crafting_unit", "megacells:mega_crafting_accelerator", "megacells:mega_crafting_monitor"),
        ] {
            assert_eq!(change(unit, Some(&held("ae2:engineering_processor")), false, false, &recipes).unwrap().next, accelerator);
            assert_eq!(change(unit, Some(&held("ae2:storage_monitor")), false, false, &recipes).unwrap().next, monitor);
        }
        // A 1M storage swapped for a 256M one gives its component back.
        let c = change("megacells:1m_crafting_storage", Some(&held("megacells:cell_component_256m")), false, false, &recipes).unwrap();
        assert_eq!((c.next, c.returned), ("megacells:256m_crafting_storage", Some(held("megacells:cell_component_1m"))));
        // Alternate use: back to MEGA's own unit, not AE2's.
        let c = change("megacells:mega_crafting_accelerator", None, true, false, &recipes).unwrap();
        assert_eq!((c.next, c.returned, c.consume), ("megacells:mega_crafting_unit", Some(held("ae2:engineering_processor")), false));
        assert_eq!(change("ae2:1k_crafting_storage", None, true, false, &recipes).unwrap().next, "ae2:crafting_unit");
        // MEGA's plain unit has nothing to give back.
        assert!(change("megacells:mega_crafting_unit", None, true, false, &recipes).is_none());
    }
}
