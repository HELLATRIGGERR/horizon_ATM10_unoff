//! MEGA's ME Cell Dock in the game (`CellDockPart`, `CellDockMenu`): a cable
//! part that holds one storage cell. Its cell, priority and spin are a part
//! machine's (`ae2x::defs::Defs::add_cell_part`), which AE2's storage code
//! mounts on the grid as a Drive's cell (`IStorageProvider.mountInventories`
//! with its `DriveWatcher`), whose idle draw is 0.5 AE/t and the cell's, and
//! whose menu is AE2's machine menu over the part (`CellDockMenu`: the cell
//! slot, the priority tab); what is the dock's own is here: its shape, its
//! spin, where it draws its cell and LEDs, its menu's title and entries.
//! Derived from MEGA Cells 4.11.0, https://github.com/62832/MEGACells tag
//! 4.11.0, LGPL-3.0; see `ae2/addons/megacells/`.
use crate::game::{Game, ae2_parts::PartAddress, ae2_render::orientation};
use horizon_client::{ae2x::megacells::CELL_DOCK, items::ItemStack};
use horizon_world::nbt::Tag;

/// `CellDockPart.getBoxes`: in sixteenths, the part on the south face.
pub(in crate::game) fn boxes(id: &str) -> Option<Vec<[f64; 6]>> {
    (id == CELL_DOCK).then(|| vec![[3., 3., 12., 13., 13., 16.], [5., 5., 11., 11., 11., 12.]])
}

/// `SpinMapping.SPIN_DIRECTIONS`: the up direction of each spin, looking at a
/// block from `facing` (NORTH is up for UP and DOWN).
fn spin_directions(facing: &str) -> [&'static str; 4] {
    match facing {
        "down" => ["north", "west", "south", "east"],
        "up" => ["north", "east", "south", "west"],
        "north" => ["up", "west", "down", "east"],
        "south" => ["up", "east", "down", "west"],
        "west" => ["up", "south", "down", "north"],
        _ => ["up", "north", "down", "south"],
    }
}
/// `SpinMapping.getUpFromSpin`.
fn up_from_spin(facing: &str, spin: u8) -> &'static str {
    spin_directions(facing)[usize::from(spin % 4)]
}
/// `SpinMapping.getSpinFromUp`: 0 for a degenerate up direction.
fn spin_from_up(facing: &str, up: &str) -> u8 {
    spin_directions(facing).iter().position(|d| *d == up).map_or(0, |i| i as u8)
}

/// `CellDockPart.renderDynamic`, from the block's centre: where the cell's
/// model is drawn (`BlockOrientation.get(front, side)` with `front` the spin's
/// up direction, then (-3, 5, -4) sixteenths), its LED with it; and the
/// dock's own LED (`BlockOrientation.get(side, spin)`, then (-8, -3, -8)).
pub(in crate::game) fn poses(id: &str, side: &str, spin: u8) -> Option<(glam::Mat4, glam::Mat4)> {
    if id != CELL_DOCK {
        return None;
    }
    let front = up_from_spin(side, spin);
    let cell = orientation(front, spin_from_up(front, side)) * glam::Mat4::from_translation(glam::Vec3::new(-3.0, 5.0, -4.0) / 16.0);
    let led = orientation(side, spin) * glam::Mat4::from_translation(glam::Vec3::new(-8.0, -3.0, -8.0) / 16.0);
    Some((cell, led))
}

/// `CellDockPart.onPlacement`: on an UP or DOWN face, the spin the player's
/// yaw gives (`Mth.floor(yRot * 4 / 360 + 2.5) & 3`); none on the other faces,
/// which keep spin 0.
pub(in crate::game) fn placement_spin(id: &str, side: &str, yaw: f32) -> Option<u8> {
    if id != CELL_DOCK || !matches!(side, "up" | "down") {
        return None;
    }
    Some(((f64::from(yaw * 4.0 / 360.0) + 2.5).floor() as i32 & 3) as u8)
}

/// The window title `MenuOpener.open(CELL_DOCK, ..)` shows.
pub(in crate::game) fn menu_title(id: &str) -> Option<&'static str> {
    (id == CELL_DOCK).then_some("ME Cell Dock")
}

impl Game<'_> {
    /// `CellDockPart.onUseItemOn`: a wrench that can rotate
    /// (`InteractionUtil.canWrenchRotate`) turns the dock on face `face` of
    /// the cable bus at `pos` a quarter (`spin + 1`, mod 4), saved.
    pub(in crate::game) fn megacells_dock_wrench(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        if self.keys.contains(&crate::game::KeyCode::ShiftLeft) || !self.held().is_some_and(|s| self.ae2_wrench_can_rotate(s)) {
            return false;
        }
        let side = match face {
            [1, 0, 0] => 0,
            [-1, 0, 0] => 1,
            [0, 1, 0] => 2,
            [0, -1, 0] => 3,
            [0, 0, 1] => 4,
            [0, 0, -1] => 5,
            _ => return false,
        };
        let Some(host) = self.ae2_local_address((pos[0], pos[1], pos[2])) else { return false };
        let part = PartAddress { host, face: side };
        let Ok(Some(mut machine)) = self.ae2_part_machine(part) else { return false };
        if machine.id() != CELL_DOCK {
            return false;
        }
        let spin = match machine.config.get(&"spin".into()) {
            Some(Tag::Byte(spin)) => *spin as u8 % 4,
            _ => 0,
        };
        machine.config.insert("spin".into(), Tag::Byte(((spin + 1) % 4) as i8));
        self.ae2_store_part_machine(part, &machine).is_ok()
    }
}

/// `CellDockScreen`'s `addOpenPriorityButton`: the menu's last entry, as on
/// the ME Chest's window.
pub(in crate::game) fn menu_items(machine: &horizon_client::ae2_world::Machine, items: &mut [Option<ItemStack>]) {
    if machine.id() == CELL_DOCK && let Some(last) = items.last_mut() {
        *last = Some(crate::game::ae2_terminal::navigation("Priority: click to edit"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SpinMapping` round trips, and the cell faces the spin's up direction
    /// with its top out of the dock's face.
    #[test]
    fn the_cell_faces_the_spin_and_tops_out_of_the_face() {
        for side in ["down", "up", "north", "south", "west", "east"] {
            for spin in 0..4 {
                assert_eq!(spin_from_up(side, up_from_spin(side, spin)), spin, "{side} {spin}");
            }
        }
        // On a north face, spin 0: the cell's front is up, its top north.
        let (cell, led) = poses(CELL_DOCK, "north", 0).unwrap();
        let front = cell.transform_vector3(glam::Vec3::NEG_Z).round();
        let top = cell.transform_vector3(glam::Vec3::Y).round();
        assert_eq!((front, top), (glam::Vec3::Y, glam::Vec3::NEG_Z));
        // The dock's LED is on its front, at z = -8/16 from the centre (the block's north face).
        let at = led.transform_point3(glam::Vec3::ZERO);
        assert!((at.z + 0.5).abs() < 1e-6, "{at}");
        assert!(poses("ae2:terminal", "north", 0).is_none());
    }

    /// `CellDockPart.onPlacement`: only the floor and ceiling take the yaw.
    #[test]
    fn placement_spins_the_dock_on_up_and_down_faces_only() {
        // floor(yaw / 90 + 2.5) & 3: 0 -> 2, 90 -> 3, 180 -> 0, 270 -> 1, -45 -> 2.
        for (yaw, spin) in [(0.0, 2), (90.0, 3), (180.0, 0), (270.0, 1), (-45.0, 2), (44.0, 2), (46.0, 3)] {
            assert_eq!(placement_spin(CELL_DOCK, "up", yaw), Some(spin), "{yaw}");
            assert_eq!(placement_spin(CELL_DOCK, "down", yaw), Some(spin), "{yaw}");
        }
        assert_eq!(placement_spin(CELL_DOCK, "north", 90.0), None);
        assert_eq!(placement_spin("ae2:terminal", "up", 90.0), None);
    }

    /// A cable bus holding a dock on `face` with `cell` and `spin`.
    fn bus_with_dock(face: &str, cell: Option<ItemStack>, spin: i8) -> horizon_client::ae2_world::Machine {
        use horizon_client::ae2_world::{Machine, MachineKind};
        let mut bus = Machine::new(MachineKind::CableBus);
        let machine = Tag::compound([
            ("slots", horizon_client::save::encode_items(&[cell])),
            ("config", Tag::compound([("spin", Tag::Byte(spin)), ("priority", Tag::Int(7))])),
        ]);
        let part = Tag::compound([("id", Tag::String(CELL_DOCK.into())), ("face", Tag::String(face.into())), ("machine", machine)]);
        bus.config.insert("parts".into(), Tag::List(vec![part]));
        bus
    }

    /// `CellDockPart`: one cell slot that takes what `StorageCells.isCellHandled`
    /// takes; 0.5 AE/t and the cell's `getIdleDrain`; its cell among its drops.
    #[test]
    fn the_dock_holds_one_handled_cell_draws_its_drain_and_drops_it() {
        use crate::game::{ae2_parts, ae2_terminal::part_slot_allows};
        horizon_client::ae2x::install();
        let dock = ae2_parts::part_machine(CELL_DOCK).unwrap();
        assert_eq!((dock.slots.len(), dock.upgrades.len(), dock.id()), (1, 0, CELL_DOCK));
        assert!(ae2_parts::is_cell_part(CELL_DOCK) && !ae2_parts::is_cell_part("ae2:terminal"));
        for cell in ["ae2:item_storage_cell_1k", "ae2:fluid_storage_cell_256k", "megacells:item_storage_cell_1m", "megacells:fluid_storage_cell_256m", "ae2:portable_item_cell_4k", "megacells:portable_fluid_cell_1m"] {
            assert!(part_slot_allows(&dock, &ItemStack::new(cell, 1)), "{cell}");
        }
        for other in ["minecraft:dirt", "ae2:cell_component_1k", "megacells:mega_item_cell_housing", "ae2:view_cell"] {
            assert!(!part_slot_allows(&dock, &ItemStack::new(other, 1)), "{other}");
        }
        // The cells' drain, on top of the dock's 0.5 (the registry's part row).
        let drain = |cell: Option<&str>| {
            let bus = bus_with_dock("north", cell.map(|id| ItemStack::new(id, 1)), 0);
            let Some(Tag::List(parts)) = bus.config.get(&"parts".into()) else { unreachable!() };
            ae2_parts::cell_part_drain(CELL_DOCK, &parts[0])
        };
        assert_eq!(horizon_client::ae2x::defs::get().part(CELL_DOCK).map(|p| (p.idle_power, p.requires_channel, p.cells)), Some((0.5, true, 1)));
        assert_eq!(drain(None), 0.0);
        assert_eq!(drain(Some("ae2:item_storage_cell_1k")), 0.5);
        assert_eq!(drain(Some("megacells:item_storage_cell_1m")), 3.0);
        assert_eq!(drain(Some("megacells:fluid_storage_cell_256m")), 5.0);
        // Broken: the dock, then its cell (CellDockPart.addAdditionalDrops).
        let cell = ItemStack::new("megacells:item_storage_cell_4m", 1);
        let drops = crate::game::spill::ae2_physical_drops_with_max(&bus_with_dock("up", Some(cell.clone()), 0), &|_| 64);
        assert_eq!(drops.iter().map(|s| s.item.to_string()).collect::<Vec<_>>(), [CELL_DOCK, "megacells:item_storage_cell_4m"]);
    }

    /// `CellDockPart.writeToStream`: the client gets the cell's item, its
    /// `CellState` and the spin; the dock's shape is `getBoxes`'.
    #[test]
    fn the_client_sees_the_cell_its_state_and_the_spin() {
        use crate::game::ae2_visual_sync::visual;
        use horizon_client::containers::BlockEntity;
        horizon_client::ae2x::install();
        let bus = bus_with_dock("down", Some(ItemStack::new("megacells:fluid_storage_cell_1m", 1)), 3);
        let v = visual((0, 0, 0), &BlockEntity::Ae2(Box::new(bus)), true).unwrap();
        let part = &v.parts[0];
        assert_eq!((part.id.as_str(), part.face.as_str(), part.spin), (CELL_DOCK, "down", Some(3)));
        let cell = part.cell.as_ref().unwrap();
        assert_eq!((cell.id.as_str(), cell.state), ("megacells:fluid_storage_cell_1m", 1), "an empty cell is EMPTY");
        let none = visual((0, 0, 0), &BlockEntity::Ae2(Box::new(bus_with_dock("north", None, 0))), true).unwrap();
        assert_eq!((none.parts[0].cell.as_ref(), none.parts[0].spin), (None, Some(0)));
        // On a north face: (3, 3, 0) to (13, 13, 4) and (5, 5, 4) to (11, 11, 5) sixteenths.
        let boxes = crate::game::ae2_geometry::boxes(&none, true);
        let sixteenths = |b: [f64; 6]| b.map(|v| (v * 16.0).round());
        assert_eq!(boxes.into_iter().map(sixteenths).collect::<Vec<_>>(), [[3., 3., 0., 13., 13., 4.], [5., 5., 4., 11., 11., 5.]]);
    }
}
