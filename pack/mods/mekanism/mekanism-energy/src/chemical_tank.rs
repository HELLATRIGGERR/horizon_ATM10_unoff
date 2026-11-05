//! Mekanism's Chemical Tanks and chemical items: the block's tank
//! (`TileEntityChemicalTank` with `ChemicalTankChemicalTank`), the item that
//! keeps its contents (`ComponentBackedChemicalTankTank` behind the item's
//! `IChemicalHandler`, `Capabilities.CHEMICAL.item()`), and the slots that
//! move chemicals between an item and a machine's tank
//! (`ChemicalInventorySlot.fillChemicalTank` and `drainChemicalTank`), as
//! Mekanism 10.7.19 has them (MIT, Copyright (c) 2017-2025 Aidan C. Brady).
//!
//! A tank's own moves (`INTERNAL`: its ejector, its slots) go at most its
//! tier's output a tick; other blocks (`EXTERNAL`) move any amount. An item's
//! tank moves at most its tier's output a call, its handler being the item's
//! own (`INTERNAL`). The creative tank keeps what it holds whatever is taken
//! out or put in, and only its own slot (not another block) fills it, to its
//! whole capacity.
use crate::chemical::{ChemicalHandler, ChemicalStack, ChemicalTank, TankRules, always};
use crate::container::Automation;
use crate::separator::GasMode;
use crate::sides::{DataType, RelativeSide};
use crate::tier::ChemicalTankTier;

/// The drain slot (`ChemicalInventorySlot.drain`, `+`: fills an item from the
/// tank) and the fill slot (`fill`, `-`: empties an item into the tank).
pub const DRAIN_SLOT: usize = 0;
pub const FILL_SLOT: usize = 1;

/// A chemical tank block as its tier and `tiers.toml` size it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TankBlock {
    pub tier: ChemicalTankTier,
    pub capacity: i64,
    pub output: i64,
}

impl TankBlock {
    pub fn creative(&self) -> bool {
        self.tier == ChemicalTankTier::Creative
    }

    /// `ChemicalTankChemicalTank.getInsertRate` and `getExtractRate`: the
    /// tier's output for its own moves, unlimited for the rest.
    pub fn rate(&self, automation: Automation) -> i64 {
        if automation == Automation::Internal { self.output } else { i64::MAX }
    }

    /// `ChemicalTankChemicalTank.insert`: an empty creative tank that its own
    /// slot fills (not another block, not a simulation) is filled to its
    /// capacity; a creative tank changes no other way.
    pub fn insert(&self, tank: &mut ChemicalTank, stack: &ChemicalStack, simulate: bool, automation: Automation, rules: &TankRules<'_>) -> ChemicalStack {
        let rate = self.rate(automation);
        if self.creative() && tank.is_empty() && !simulate && automation != Automation::External {
            let rest = tank.insert_rated(stack, true, automation, rules, rate);
            if rest.is_empty() {
                tank.stored = stack.copy_with_amount(tank.capacity);
            }
            return rest;
        }
        tank.insert_rated(stack, simulate || self.creative(), automation, rules, rate)
    }

    /// `ChemicalTankChemicalTank.extract`: a creative tank gives without losing.
    pub fn extract(&self, tank: &mut ChemicalTank, amount: i64, simulate: bool, automation: Automation, rules: &TankRules<'_>) -> ChemicalStack {
        tank.extract_rated(amount, simulate || self.creative(), automation, rules, self.rate(automation))
    }

    /// `onUpdateServer`'s dump: `DUMPING` loses a 400th of the tier's
    /// storage a tick; `DUMPING_EXCESS` brings it down to
    /// `dumpExcessKeepRatio` of its capacity, at most the output a tick. The
    /// creative tank dumps nothing.
    pub fn dump(&self, tank: &mut ChemicalTank, mode: GasMode, keep_ratio: f64) {
        if self.creative() || tank.is_empty() {
            return;
        }
        match mode {
            GasMode::Idle => {}
            GasMode::Dumping => {
                tank.shrink_stack(self.capacity / 400, false);
            }
            GasMode::DumpingExcess => {
                let target = crate::clamp_to_long(tank.capacity as f64 * keep_ratio);
                let stored = tank.stored();
                if target < stored {
                    tank.shrink_stack((stored - target).min(self.output), false);
                }
            }
        }
    }

    /// `setCanEject`: the ejector runs unless the tank is dumping (the
    /// creative tank always).
    pub fn ejects(&self, mode: GasMode) -> bool {
        self.creative() || mode != GasMode::Dumping
    }
}

/// The block tank's rules (`alwaysTrueBi`, `alwaysTrueBi`, `alwaysTrue`) with
/// `valid` the attribute validator: the default one, the creative tank's
/// `ALWAYS_ALLOW`.
pub fn block_rules<'a>(valid: &'a dyn Fn(&ChemicalStack) -> bool) -> TankRules<'a> {
    TankRules { can_extract: &always, can_insert: &always, valid }
}

/// `setupIOConfig(CHEMICAL, tank, FRONT)` and the item's `setupIOConfig(ITEM,
/// drainSlot, fillSlot, FRONT, true)`: none, input, output and both.
pub fn supports(data: DataType) -> bool {
    matches!(data, DataType::None | DataType::Input | DataType::Output | DataType::InputOutput)
}

/// The config its item comes with (`ItemBlockChemicalTank.SIDE_CONFIG`):
/// `FRONT_OUT_EJECT` for chemicals, `FRONT_OUT_NO_EJECT` for items.
pub fn default_sides() -> [DataType; 6] {
    RelativeSide::ALL.map(|s| if s == RelativeSide::Front { DataType::Output } else { DataType::Input })
}

/// A side's chemical handler (`ConfigChemicalTankHolder` with
/// `ChemicalSlotInfo`): whether it takes in and gives out; none for `NONE`.
pub fn side_io(data: DataType) -> Option<(bool, bool)> {
    crate::sides::io(data)
}

/// The block's tank as one side sees it (`ProxyChemicalHandler`): another
/// block (`EXTERNAL`) on a side that takes in or gives out, the null side
/// reading only.
pub struct TankSide<'t, 'r> {
    pub tank: &'t mut ChemicalTank,
    pub block: TankBlock,
    pub rules: &'r TankRules<'r>,
    pub side: bool,
    pub can_insert: bool,
    pub can_extract: bool,
}

impl ChemicalHandler for TankSide<'_, '_> {
    fn tanks(&self) -> usize {
        1
    }

    fn in_tank(&self, _: usize) -> ChemicalStack {
        self.tank.stored.clone()
    }

    fn capacity(&self, _: usize) -> i64 {
        self.tank.capacity
    }

    fn is_valid(&self, _: usize, stack: &ChemicalStack) -> bool {
        (self.rules.valid)(stack)
    }

    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if tank != 0 || !self.side || !self.can_insert {
            return stack.clone();
        }
        self.block.insert(self.tank, stack, simulate, Automation::External, self.rules)
    }

    fn extract_tank(&mut self, tank: usize, amount: i64, simulate: bool) -> ChemicalStack {
        if tank != 0 || !self.side || !self.can_extract {
            return ChemicalStack::EMPTY;
        }
        self.block.extract(self.tank, amount, simulate, Automation::External, self.rules)
    }
}

/// A chemical tank item's tank (`ComponentBackedChemicalTankTank`): what it
/// holds, its tier's storage, and its output as the most a call moves. Its
/// handler is the item's own, so every move is `INTERNAL`. A creative item
/// changes no way. Only a stack of one has a handler (`ContainerType`'s
/// capability provider).
pub struct ItemTank<'r> {
    pub stored: ChemicalStack,
    pub capacity: i64,
    pub rate: i64,
    pub creative: bool,
    /// The attribute validator: the default one, the creative tank's `ALWAYS_ALLOW`.
    pub valid: &'r dyn Fn(&ChemicalStack) -> bool,
}

impl ItemTank<'_> {
    pub fn of_tier(tier: ChemicalTankTier, (capacity, output): (i64, i64), stored: ChemicalStack, valid: &dyn Fn(&ChemicalStack) -> bool) -> ItemTank<'_> {
        ItemTank { stored: stored.normalized(), capacity, rate: output, creative: tier == ChemicalTankTier::Creative, valid }
    }
}

impl ChemicalHandler for ItemTank<'_> {
    fn tanks(&self) -> usize {
        1
    }

    fn in_tank(&self, _: usize) -> ChemicalStack {
        self.stored.clone()
    }

    fn capacity(&self, _: usize) -> i64 {
        self.capacity
    }

    fn is_valid(&self, _: usize, stack: &ChemicalStack) -> bool {
        (self.valid)(stack)
    }

    /// `ComponentBackedChemicalTank.insert`.
    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if tank != 0 || stack.is_empty() || !(self.valid)(stack) {
            return stack.clone();
        }
        let needed = self.rate.min((self.capacity - self.stored.amount()).max(0));
        if needed <= 0 || !self.stored.is_empty() && !ChemicalStack::is_same_chemical(&self.stored, stack) {
            return stack.clone();
        }
        let add = stack.amount().min(needed);
        if !simulate && !self.creative {
            self.stored = stack.copy_with_amount(self.stored.amount() + add);
        }
        stack.copy_with_amount(stack.amount() - add)
    }

    /// `ComponentBackedChemicalTank.extract`.
    fn extract_tank(&mut self, tank: usize, amount: i64, simulate: bool) -> ChemicalStack {
        if tank != 0 || amount < 1 || self.stored.is_empty() {
            return ChemicalStack::EMPTY;
        }
        let size = self.rate.min(self.stored.amount()).min(amount);
        let out = self.stored.copy_with_amount(size);
        if !simulate && !self.creative && !out.is_empty() {
            self.stored = self.stored.copy_with_amount(self.stored.amount() - size).normalized();
        }
        out
    }
}

/// A machine's tank as its own chemical slot reaches it (`INTERNAL`): a
/// chemical tank's with its rate and creative rules, or a plain
/// `BasicChemicalTank`'s.
pub struct OwnTank<'t, 'r> {
    pub tank: &'t mut ChemicalTank,
    pub rules: &'r TankRules<'r>,
    pub block: Option<TankBlock>,
}

impl OwnTank<'_, '_> {
    fn insert(&mut self, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        match self.block {
            Some(b) => b.insert(self.tank, stack, simulate, Automation::Internal, self.rules),
            None => self.tank.insert(stack, simulate, Automation::Internal, self.rules),
        }
    }

    fn extract(&mut self, amount: i64, simulate: bool) -> ChemicalStack {
        match self.block {
            Some(b) => b.extract(self.tank, amount, simulate, Automation::Internal, self.rules),
            None => self.tank.extract(amount, simulate, Automation::Internal, self.rules),
        }
    }

    fn valid(&self, stack: &ChemicalStack) -> bool {
        (self.rules.valid)(stack)
    }
}

/// `ChemicalInventorySlot.fillChemicalTank`: what the item's handler gives up
/// (up to the tank's capacity, or the tank's chemical up to what it needs) and
/// the tank takes, moved; whether anything did.
pub fn fill_from_item(item: &mut dyn ChemicalHandler, tank: &mut OwnTank<'_, '_>) -> bool {
    if tank.tank.needed() <= 0 {
        return false;
    }
    let mut offered = if tank.tank.is_empty() {
        item.extract(tank.tank.capacity, true)
    } else {
        item.extract_stack(&tank.tank.stored.copy_with_amount(tank.tank.needed()), true)
    };
    if offered.is_empty() {
        return false;
    }
    let rest = tank.insert(&offered, true);
    offered.shrink(rest.amount());
    if offered.is_empty() {
        return false;
    }
    let got = item.extract_stack(&offered, false);
    if got.is_empty() {
        return false;
    }
    tank.insert(&got, false);
    true
}

/// `ChemicalInventorySlot.drainChemicalTank`: what the item takes of the
/// tank's chemical, out of the tank (`INTERNAL`) into the item; whether
/// anything moved.
pub fn drain_into_item(item: &mut dyn ChemicalHandler, tank: &mut OwnTank<'_, '_>) -> bool {
    if tank.tank.is_empty() {
        return false;
    }
    let stored = tank.tank.stored.clone();
    let rest = item.insert(&stored, true).amount();
    if rest >= stored.amount() {
        return false;
    }
    let got = tank.extract(stored.amount() - rest, false);
    if got.is_empty() {
        return false;
    }
    item.insert(&got, false);
    true
}

/// `fillInsertCheck`: the fill slot takes an item holding a chemical the
/// tank would take some of.
pub fn fill_accepts(item: &dyn ChemicalHandler, tank: &mut OwnTank<'_, '_>) -> bool {
    (0..item.tanks()).any(|i| {
        let held = item.in_tank(i);
        !held.is_empty() && tank.insert(&held, true).amount() < held.amount()
    })
}

/// `getDrainInsertPredicate`: the drain slot takes an item with room, for the
/// tank's chemical when it holds one.
pub fn drain_accepts(item: &mut dyn ChemicalHandler, tank: &OwnTank<'_, '_>) -> bool {
    if tank.tank.is_empty() {
        return (0..item.tanks()).any(|i| item.in_tank(i).amount() < item.capacity(i));
    }
    item.insert(&tank.tank.stored, true).amount() < tank.tank.stored()
}

/// `getFillExtractPredicate`: an item leaves the fill slot by automation when
/// it holds nothing the tank takes.
pub fn fill_done(item: &dyn ChemicalHandler, tank: &OwnTank<'_, '_>) -> bool {
    !(0..item.tanks()).any(|i| {
        let held = item.in_tank(i);
        !held.is_empty() && tank.valid(&held)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chemical::any;

    const H: &str = "mekanism:hydrogen";
    const O: &str = "mekanism:oxygen";

    fn block(tier: ChemicalTankTier) -> TankBlock {
        TankBlock { tier, capacity: tier.default_storage(), output: tier.default_output() }
    }

    fn tank(tier: ChemicalTankTier, chemical: &str, amount: i64) -> ChemicalTank {
        ChemicalTank::with(tier.default_storage(), ChemicalStack::new(chemical, amount))
    }

    #[test]
    fn a_tank_moves_its_output_on_its_own_and_anything_from_outside() {
        // The NeoForge oracle (chem-work/tanks t1): a basic tank ejects 1000 a tick, an advanced 16 000,
        // an elite 128 000, an ultimate 512 000; another tank's insert into it is unlimited.
        let rules = block_rules(&any);
        for (tier, out) in [(ChemicalTankTier::Basic, 1000), (ChemicalTankTier::Advanced, 16_000), (ChemicalTankTier::Elite, 128_000), (ChemicalTankTier::Ultimate, 512_000)] {
            let b = block(tier);
            let mut t = tank(tier, H, 1_000_000.min(tier.default_storage()));
            assert_eq!(b.extract(&mut t, i64::MAX, true, Automation::Internal, &rules).amount(), out);
        }
        let b = block(ChemicalTankTier::Basic);
        let mut t = ChemicalTank::new(b.capacity);
        assert_eq!(b.insert(&mut t, &ChemicalStack::new(H, 50_000), false, Automation::External, &rules), ChemicalStack::EMPTY);
        assert_eq!(b.insert(&mut t, &ChemicalStack::new(H, 50_000), false, Automation::Internal, &rules).amount(), 49_000);
        assert_eq!(t.stored(), 51_000);
    }

    #[test]
    fn dumping_as_the_oracle_measured() {
        // t1: a basic tank of 64 000 dumping loses 160 a tick; dumping excess 1000 a tick down to 54 400
        // (85 %); an ultimate one dumping 20 480; the creative tank keeps all.
        let b = block(ChemicalTankTier::Basic);
        let mut t = tank(ChemicalTankTier::Basic, H, 64_000);
        b.dump(&mut t, GasMode::Dumping, 0.85);
        assert_eq!(t.stored(), 63_840);
        let mut t = tank(ChemicalTankTier::Basic, H, 64_000);
        b.dump(&mut t, GasMode::DumpingExcess, 0.85);
        assert_eq!(t.stored(), 63_000);
        let mut t = tank(ChemicalTankTier::Basic, H, 54_500);
        b.dump(&mut t, GasMode::DumpingExcess, 0.85);
        assert_eq!(t.stored(), 54_400);
        let u = block(ChemicalTankTier::Ultimate);
        let mut t = tank(ChemicalTankTier::Ultimate, H, 8_192_000);
        u.dump(&mut t, GasMode::Dumping, 0.85);
        assert_eq!(t.stored(), 8_171_520);
        let c = block(ChemicalTankTier::Creative);
        let mut t = ChemicalTank::with(i64::MAX, ChemicalStack::new(H, 100));
        c.dump(&mut t, GasMode::Dumping, 0.85);
        assert_eq!(t.stored(), 100);
        assert!(!b.ejects(GasMode::Dumping) && b.ejects(GasMode::DumpingExcess) && c.ejects(GasMode::Dumping));
    }

    #[test]
    fn the_creative_tank_keeps_what_it_holds() {
        // t1: one holding 5 mB gives 5 a tick and keeps 5; a tank ejecting into it loses what it gives
        // (an empty creative tank stays empty); its own fill slot fills an empty one to Long.MAX (t2).
        let rules = block_rules(&any);
        let c = block(ChemicalTankTier::Creative);
        let mut t = ChemicalTank::with(i64::MAX, ChemicalStack::new(H, 5));
        assert_eq!(c.extract(&mut t, i64::MAX, false, Automation::Internal, &rules).amount(), 5);
        assert_eq!(t.stored(), 5);
        let mut empty = ChemicalTank::new(i64::MAX);
        assert!(c.insert(&mut empty, &ChemicalStack::new(H, 1000), false, Automation::External, &rules).is_empty(), "taken");
        assert!(empty.is_empty(), "and lost");
        assert!(c.insert(&mut empty, &ChemicalStack::new(H, 100), false, Automation::Internal, &rules).is_empty());
        assert_eq!(empty.stored(), i64::MAX);
    }

    fn item(tier: ChemicalTankTier, chemical: &str, amount: i64) -> ItemTank<'static> {
        ItemTank::of_tier(tier, (tier.default_storage(), tier.default_output()), ChemicalStack::new(chemical, amount), &any)
    }

    #[test]
    fn slots_fill_and_drain_items_at_the_slower_rate() {
        // t2: a basic tank drains 1000 a tick into an empty basic item, fills 1000 a tick from one;
        // an advanced tank takes 16 000 from an ultimate item; an elite tank gives an ultimate item 128 000;
        // an item of hydrogen takes no oxygen.
        let rules = block_rules(&any);
        let b = block(ChemicalTankTier::Basic);
        let mut t = tank(ChemicalTankTier::Basic, H, 30_000);
        let mut it = item(ChemicalTankTier::Basic, H, 0);
        assert!(drain_into_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(b) }));
        assert_eq!((t.stored(), it.stored.amount()), (29_000, 1000));
        let mut t = ChemicalTank::new(b.capacity);
        let mut it = item(ChemicalTankTier::Basic, H, 5000);
        assert!(fill_from_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(b) }));
        assert_eq!((t.stored(), it.stored.amount()), (1000, 4000));
        let a = block(ChemicalTankTier::Advanced);
        let mut t = ChemicalTank::new(a.capacity);
        let mut it = item(ChemicalTankTier::Ultimate, H, 600_000);
        fill_from_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(a) });
        assert_eq!((t.stored(), it.stored.amount()), (16_000, 584_000));
        let e = block(ChemicalTankTier::Elite);
        let mut t = tank(ChemicalTankTier::Elite, H, 600_000);
        let mut it = item(ChemicalTankTier::Ultimate, H, 0);
        drain_into_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(e) });
        assert_eq!((t.stored(), it.stored.amount()), (472_000, 128_000));
        let mut t = tank(ChemicalTankTier::Basic, O, 3000);
        let mut it = item(ChemicalTankTier::Basic, H, 10);
        assert!(!drain_into_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(b) }));
        assert!(!drain_accepts(&mut it, &OwnTank { tank: &mut t, rules: &rules, block: Some(b) }));
        // A creative item gives 100 a tick and keeps it; a basic item empties into a creative tank, which fills.
        let mut t = ChemicalTank::new(b.capacity);
        let mut it = item(ChemicalTankTier::Creative, H, 100);
        fill_from_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(b) });
        fill_from_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(b) });
        assert_eq!((t.stored(), it.stored.amount()), (200, 100));
        let c = block(ChemicalTankTier::Creative);
        let mut t = ChemicalTank::new(i64::MAX);
        let mut it = item(ChemicalTankTier::Basic, H, 100);
        fill_from_item(&mut it, &mut OwnTank { tank: &mut t, rules: &rules, block: Some(c) });
        assert_eq!((t.stored(), it.stored.amount()), (i64::MAX, 0));
        assert!(it.stored.is_empty() && fill_done(&it, &OwnTank { tank: &mut t, rules: &rules, block: Some(c) }));
    }

    #[test]
    fn a_side_of_the_block_follows_its_config() {
        // t1: a side set to none or output takes nothing from a tank beside it, input or both take.
        let rules = block_rules(&any);
        let b = block(ChemicalTankTier::Basic);
        let mut t = ChemicalTank::new(b.capacity);
        for (data, takes) in [(DataType::Input, true), (DataType::Output, false), (DataType::InputOutput, true)] {
            let (i, o) = side_io(data).unwrap();
            let mut side = TankSide { tank: &mut t, block: b, rules: &rules, side: true, can_insert: i, can_extract: o };
            assert_eq!(side.insert(&ChemicalStack::new(H, 10), true).is_empty(), takes, "{data:?}");
        }
        assert!(side_io(DataType::None).is_none());
        let mut null = TankSide { tank: &mut t, block: b, rules: &rules, side: false, can_insert: false, can_extract: false };
        assert_eq!(null.insert(&ChemicalStack::new(H, 10), false).amount(), 10, "the null side reads only");
        assert_eq!(default_sides()[0], DataType::Output);
    }
}
