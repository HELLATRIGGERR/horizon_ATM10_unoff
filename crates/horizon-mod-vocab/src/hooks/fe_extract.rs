//! The `fe_extract` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// `IEnergyStorage.extractEnergy` on the block entity at `pos`, a mod's, as `fe_receive`: set
    /// `given` (0 to `max`).
    #[hook(
        side = "server",
        contract = "replace",
        out = "given",
        neoforge = "Capabilities.EnergyStorage.BLOCK (IEnergyStorage.extractEnergy)"
    )]
    fn fe_extract(
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        entity: &mut ModBlockEntity,
        max: i64,
        simulate: bool,
        wide: bool,
        given: &mut Option<i64>,
    );
}
