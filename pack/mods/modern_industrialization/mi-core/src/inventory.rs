//! `ConfigurableItemStack` and `ConfigurableFluidStack` (inventory/), as far as the crafter reads them:
//! the resource, its amount, the slot's capacity and its locks. Player locks are kept but no screen sets
//! them yet.

/// One item slot. `max_stack` is the item's own stack size, which the caller looks up (`getMaxStackSize`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemSlot {
    pub item: Option<String>,
    pub count: u32,
    /// The item's stack size; 64 for an empty slot.
    pub max_stack: u32,
    /// `adjustedCapacity`: 64 for MI's standard slots.
    pub capacity: u32,
    pub locked: Option<String>,
    pub machine_locked: bool,
    pub player_locked: bool,
}

impl ItemSlot {
    pub fn new(item: Option<&str>, count: u32, max_stack: u32) -> Self {
        let item = item.filter(|_| count > 0).map(str::to_owned);
        ItemSlot { count: if item.is_some() { count } else { 0 }, item, max_stack, capacity: 64, ..Self::default() }
    }
    pub fn is_blank(&self) -> bool {
        self.item.is_none() || self.count == 0
    }
    /// `isResourceAllowedByLock`.
    pub fn allowed_by_lock(&self, item: &str) -> bool {
        self.locked.as_deref().is_none_or(|l| l == item)
    }
    /// `getRemainingCapacityFor(key)`: the room for `item`, whose stack size is `max_stack`.
    pub fn room_for(&self, max_stack: u32) -> u32 {
        if self.capacity < self.count {
            return 0;
        }
        max_stack.min(self.capacity) - self.count
    }
    pub fn decrement(&mut self, n: u32) {
        self.count -= n;
        if self.count == 0 {
            self.item = None;
        }
    }
    /// `enableMachineLock`. MI throws on a different lock; here the old one stays.
    pub fn enable_machine_lock(&mut self, item: &str) {
        if self.locked.as_deref().is_some_and(|l| l != item) {
            return;
        }
        self.machine_locked = true;
        self.locked = Some(item.to_owned());
    }
    /// `disableMachineLock`.
    pub fn disable_machine_lock(&mut self) {
        self.machine_locked = false;
        if !self.player_locked {
            self.locked = None;
        }
    }
}

/// One fluid slot, in millibuckets (MI's `ConfigurableFluidStack` counts droplets on Fabric, mB here as on
/// NeoForge).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FluidSlot {
    pub fluid: Option<String>,
    pub amount: i64,
    pub capacity: i64,
    pub locked: Option<String>,
    pub machine_locked: bool,
    pub player_locked: bool,
}

impl FluidSlot {
    /// `standardInputSlot(capacity)` / `standardOutputSlot(capacity)`.
    pub fn standard(capacity: i64) -> Self {
        FluidSlot { capacity, ..Self::default() }
    }
    pub fn is_blank(&self) -> bool {
        self.fluid.is_none()
    }
    pub fn allowed_by_lock(&self, fluid: &str) -> bool {
        self.locked.as_deref().is_none_or(|l| l == fluid)
    }
    pub fn remaining_space(&self) -> i64 {
        self.capacity - self.amount
    }
    /// `decrement`: an empty slot forgets its fluid.
    pub fn decrement(&mut self, n: i64) {
        self.amount -= n;
        if self.amount <= 0 {
            self.amount = 0;
            self.fluid = None;
        }
    }
    pub fn enable_machine_lock(&mut self, fluid: &str) {
        if self.locked.as_deref().is_some_and(|l| l != fluid) {
            return;
        }
        self.machine_locked = true;
        self.locked = Some(fluid.to_owned());
    }
    pub fn disable_machine_lock(&mut self) {
        self.machine_locked = false;
        if !self.player_locked {
            self.locked = None;
        }
    }
}

/// `MachineInventoryComponent`'s four lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub item_inputs: Vec<ItemSlot>,
    pub item_outputs: Vec<ItemSlot>,
    pub fluid_inputs: Vec<FluidSlot>,
    pub fluid_outputs: Vec<FluidSlot>,
}

impl Inventory {
    /// `MIInventory.hash()`: changes when any slot's resource or amount does, which is all the crafter
    /// asks of it (`shouldUpdateActiveRecipe`).
    pub fn hash(&self) -> i32 {
        let mut h: i32 = 1;
        let mut mix = |s: &str, n: i64| {
            for b in s.bytes() {
                h = h.wrapping_mul(31).wrapping_add(i32::from(b));
            }
            h = h.wrapping_mul(31).wrapping_add(n as i32);
        };
        for s in self.item_inputs.iter().chain(&self.item_outputs) {
            mix(s.item.as_deref().unwrap_or(""), i64::from(s.count));
        }
        for s in self.fluid_inputs.iter().chain(&self.fluid_outputs) {
            mix(s.fluid.as_deref().unwrap_or(""), s.amount);
        }
        h
    }
}
