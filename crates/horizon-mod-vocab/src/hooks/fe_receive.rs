//! The `fe_receive` hook, one of the ATM10 fork's own: declarations the mod API generator reads (see
//! `hooks/mod.rs`). It has no `// catalog-order`: it follows the split hooks in file-name order.

pub trait Hooks {
    /// `IEnergyStorage.receiveEnergy` on the block entity at `pos`, a mod's: `max` FE (1 to
    /// `i32::MAX` unless `wide`) offered through `face`, as in `fe_storage`. Set `accepted` (0 to
    /// `max`) when `entity.id` is yours and `accepted` is none yet; with `simulate`, answer what
    /// you would take and change nothing. The entity is out of the world during the call, as in
    /// `native_block_entity_tick`; the engine puts it back and, when it took something for real,
    /// marks it changed (`setChanged`). A forwarder that holds nothing (a cable) may hand the
    /// energy on with `native_fe_offer` from here: the same block, side and direction asked again
    /// inside its own call gets 0, and so does a chain 16 calls deep.
    #[hook(
        side = "server",
        contract = "replace",
        out = "accepted",
        neoforge = "Capabilities.EnergyStorage.BLOCK (IEnergyStorage.receiveEnergy)"
    )]
    fn fe_receive(
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        entity: &mut ModBlockEntity,
        max: i64,
        simulate: bool,
        wide: bool,
        accepted: &mut Option<i64>,
    );
}
