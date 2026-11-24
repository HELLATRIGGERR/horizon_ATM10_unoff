//! A multiblock's machine cores: where they go around the controller (`Geometry.rotatePosition`), which blocks are
//! cores and of what quality (`MachineCoreBlock`), and the assembly check (`MultiblockMachineController.initMultiblock`,
//! `getNextMissingCore`).

/// A controller's `HORIZONTAL_FACING`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    North,
    South,
    West,
    East,
}

impl Facing {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "north" => Some(Self::North),
            "south" => Some(Self::South),
            "west" => Some(Self::West),
            "east" => Some(Self::East),
            _ => None,
        }
    }
}

/// `Geometry.rotatePosition(Vec3i, Direction)`: a machine-relative offset (x back, y up, z left) to a world offset.
pub fn rotate(rel: [i32; 3], facing: Facing) -> [i32; 3] {
    let [x, y, z] = rel;
    match facing {
        Facing::North => [z, y, x],
        Facing::West => [x, y, -z],
        Facing::South => [-z, y, -x],
        Facing::East => [-x, y, z],
    }
}

/// The world positions of the cores of a controller at `pos`.
pub fn core_positions(pos: [i32; 3], facing: Facing, cores: &[[i32; 3]]) -> Vec<[i32; 3]> {
    cores
        .iter()
        .map(|c| {
            let r = rotate(*c, facing);
            [pos[0] + r[0], pos[1] + r[1], pos[2] + r[2]]
        })
        .collect()
}

/// `MachineCoreBlock.getCoreQuality` of a block: the seven cores are 1 to 7, the hidden one 1.
pub fn core_quality(block: &str) -> Option<f32> {
    match block {
        "oritech:machine_core_hidden" => Some(1.0),
        _ => {
            let n: u8 = block.strip_prefix("oritech:machine_core_")?.parse().ok()?;
            (1..=7).contains(&n).then_some(f32::from(n))
        }
    }
}

/// `initMultiblock`: every core position holds an unused core; the quality is their average (the float sum over
/// the count, as Java's). `found` has each position's core quality, `None` for no core (or a used one).
pub fn assemble(found: &[Option<f32>]) -> Option<f32> {
    if found.is_empty() || found.iter().any(Option::is_none) {
        return None;
    }
    let mut sum = 0f32;
    for q in found.iter().flatten() {
        sum += q;
    }
    Some(sum / found.len() as f32)
}

/// `getNextMissingCore`: the first core position that is air or replaceable.
pub fn next_missing(positions: &[[i32; 3]], free: impl Fn([i32; 3]) -> bool) -> Option<[i32; 3]> {
    positions.iter().copied().find(|p| free(*p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{ASSEMBLER, FRAGMENT_FORGE};

    #[test]
    fn cores_turn_with_the_controller() {
        // A controller facing north (the player stood north of it): its back is south (+z), its left is east (+x).
        assert_eq!(rotate([1, 0, 0], Facing::North), [0, 0, 1]);
        assert_eq!(rotate([0, 0, 1], Facing::North), [1, 0, 0]);
        assert_eq!(rotate([1, 0, 0], Facing::South), [0, 0, -1]);
        assert_eq!(rotate([0, 0, 1], Facing::South), [-1, 0, 0]);
        assert_eq!(rotate([1, 0, 0], Facing::East), [-1, 0, 0]);
        assert_eq!(rotate([0, 0, 1], Facing::West), [0, 0, -1]);
        assert_eq!(rotate([0, 1, 0], Facing::West), [0, 1, 0], "up stays up");
        let at = core_positions([10, 64, 10], Facing::North, ASSEMBLER.cores);
        assert_eq!(at, vec![[11, 64, 10], [10, 65, 10], [11, 65, 10]]);
        assert_eq!(core_positions([0, 0, 0], Facing::South, FRAGMENT_FORGE.cores).len(), 7);
    }

    #[test]
    fn quality_is_the_average_of_the_cores() {
        assert_eq!(core_quality("oritech:machine_core_1"), Some(1.0));
        assert_eq!(core_quality("oritech:machine_core_7"), Some(7.0));
        assert_eq!(core_quality("oritech:machine_core_8"), None);
        assert_eq!(core_quality("oritech:machine_core_hidden"), Some(1.0));
        assert_eq!(core_quality("minecraft:iron_block"), None);
        assert_eq!(assemble(&[Some(1.0), Some(2.0), Some(3.0)]), Some(2.0));
        assert_eq!(assemble(&[Some(1.0), None, Some(3.0)]), None, "a missing core");
        assert_eq!(assemble(&[]), None);
        let free = |p: [i32; 3]| p == [10, 65, 10];
        assert_eq!(next_missing(&core_positions([10, 64, 10], Facing::North, ASSEMBLER.cores), free), Some([10, 65, 10]));
    }
}
