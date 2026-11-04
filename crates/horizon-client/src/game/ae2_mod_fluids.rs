//! AE2's fluid storage bus, import bus and export bus on the fluid handler of
//! a native mod's block (`Capabilities.FluidHandler.BLOCK`, API 160:
//! `machines/native_fluid_handler.rs`), as AE2 1.21.1 meets any
//! `IFluidHandler`: `ExternalStorageFacade.FluidHandlerFacade` (available
//! stacks from each tank, `fill` to insert, `drain(FluidStack)` to extract),
//! which `StorageImportStrategy` and `StorageExportStrategy` move through.
//! Native tanks keep no components, so a key with a component patch moves
//! nothing here.
use super::Game;
use super::ae2_global::GridAddress;
use ae2_cell_core::ItemKey;
use horizon_core::geometry::Direction;
use horizon_world::fluid_stack::FluidStack;

/// `AEFluidKey.toStack(amount)` for a key without components: AE2's network
/// encoding of an empty patch (`[10, 0]`, an empty compound) is none too.
fn key_stack(key: &ItemKey, amount: u64) -> Option<FluidStack> {
    if !key.components.is_empty() && key.components != [10, 0] {
        return None;
    }
    let amount = i32::try_from(amount).unwrap_or(i32::MAX);
    FluidStack::of(&key.item_id, amount).filter(|s| !s.is_empty())
}

impl Game<'_> {
    /// The level and position an AE2 address names.
    fn ae2_mod_fluid_target(&self, target: GridAddress) -> Option<(super::Dim, [i32; 3])> {
        let (dim, (x, y, z)) = self.ae2_address_parts(target)?;
        Some((dim, [x, y, z]))
    }

    /// Whether the block at `target` has a fluid handler on `face` (the side
    /// the bus touches): `getCapability(FluidHandler.BLOCK, pos, side)` not
    /// null.
    pub(super) fn ae2_mod_fluid_supports(&self, target: GridAddress, face: Direction) -> bool {
        self.ae2_mod_fluid_target(target)
            .is_some_and(|(dim, pos)| self.fluid_tanks_at(dim, pos, Some(face)).is_some())
    }

    /// `FluidHandlerFacade.getAvailableStacks`: each tank's fluid, the
    /// empty ones left out; with `extractable_only`, only those a simulated
    /// `drain` gives (`NativeFluidTankInfo::drainable`). None where the block
    /// has no handler on `face`.
    pub(super) fn ae2_mod_fluid_entries(
        &self,
        target: GridAddress,
        face: Direction,
        extractable_only: bool,
    ) -> Option<Vec<(ItemKey, u64)>> {
        let (dim, pos) = self.ae2_mod_fluid_target(target)?;
        let tanks = self.fluid_tanks_at(dim, pos, Some(face))?;
        let mut out: Vec<(ItemKey, u64)> = Vec::new();
        for tank in tanks {
            let stack = &tank.stack;
            if stack.is_empty() || !stack.components().is_empty() {
                continue;
            }
            if extractable_only && !tank.drainable {
                continue;
            }
            let key = super::ae2_fluids::fluid_key(stack.name());
            let amount = u64::try_from(stack.amount()).unwrap_or(0);
            match out.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += amount,
                None => out.push((key, amount)),
            }
        }
        Some(out)
    }

    /// `FluidHandlerFacade.insertExternal` (`fill`) or, with `!insert`,
    /// `extractExternal` (`drain(FluidStack)`) of `amount` of `key` on the
    /// block at `target` through `face`, as an `int`: what moved. None where
    /// the block has no handler on `face`, so the caller tries its next host.
    pub(super) fn ae2_mod_fluid_move(
        &mut self,
        target: GridAddress,
        face: Direction,
        key: &ItemKey,
        amount: u64,
        insert: bool,
        simulate: bool,
    ) -> Option<u64> {
        let (dim, pos) = self.ae2_mod_fluid_target(target)?;
        self.fluid_tanks_at(dim, pos, Some(face))?;
        let Some(stack) = key_stack(key, amount) else {
            return Some(0);
        };
        let moved = if insert {
            self.fluid_fill_at(dim, pos, Some(face), &stack, simulate)
        } else {
            self.fluid_drain_at(dim, pos, Some(face), Some(&stack), 0, simulate)
                .amount()
        };
        Some(u64::try_from(moved).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_keys_without_components_become_stacks() {
        let water = ItemKey {
            item_id: "minecraft:water".into(),
            components: Vec::new(),
        };
        assert_eq!(
            key_stack(&water, 1000),
            FluidStack::of("minecraft:water", 1000)
        );
        // A long amount saturates at Java's int.
        assert_eq!(
            key_stack(&water, u64::MAX).map(|s| s.amount()),
            Some(i32::MAX)
        );
        let empty_patch = ItemKey {
            components: vec![10, 0],
            ..water.clone()
        };
        assert!(key_stack(&empty_patch, 5).is_some());
        let patched = ItemKey {
            components: vec![10, 1, 0, 1, b'x', 1, 0],
            ..water.clone()
        };
        assert_eq!(key_stack(&patched, 5), None);
        let unknown = ItemKey {
            item_id: "nope:fluid".into(),
            components: Vec::new(),
        };
        assert_eq!(key_stack(&unknown, 5), None);
        assert_eq!(key_stack(&water, 0), None);
    }
}
