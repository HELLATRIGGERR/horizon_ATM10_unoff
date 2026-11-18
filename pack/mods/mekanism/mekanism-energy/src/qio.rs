//! Quantum Item Orchestration (Mekanism 10.7.19, `mekanism.common.content.qio`): the drives' tiers
//! (`QIODriveTier`), a drive's items (`QIODriveData`) and a frequency's view over its drives
//! (`QIOFrequency.massInsert`, `massExtract`, `QIOItemTypeData.add` and `remove`).
//!
//! `K` is an item type (Mekanism's `HashedItem`: an item and its components); the runtime compares
//! stacks with `same_item_same_components`, so types are looked up in order, as the frequency's
//! `SequencedMap` keeps them.

/// `QIODriveTier`: a drive's item count and type limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveTier {
    Base,
    HyperDense,
    TimeDilating,
    Supermassive,
}

impl DriveTier {
    pub const ALL: [Self; 4] = [Self::Base, Self::HyperDense, Self::TimeDilating, Self::Supermassive];

    /// `mekanism:qio_drive_<tier>`.
    pub fn item(self) -> &'static str {
        match self {
            Self::Base => "mekanism:qio_drive_base",
            Self::HyperDense => "mekanism:qio_drive_hyper_dense",
            Self::TimeDilating => "mekanism:qio_drive_time_dilating",
            Self::Supermassive => "mekanism:qio_drive_supermassive",
        }
    }

    pub fn of_item(item: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.item() == item)
    }

    /// `getMaxCount`: items of every type together.
    pub fn max_count(self) -> u64 {
        match self {
            Self::Base => 16_000,
            Self::HyperDense => 128_000,
            Self::TimeDilating => 1_048_000,
            Self::Supermassive => 16_000_000_000,
        }
    }

    /// `getMaxTypes`.
    pub fn max_types(self) -> usize {
        match self {
            Self::Base => 128,
            Self::HyperDense => 256,
            Self::TimeDilating => 1_024,
            Self::Supermassive => 8_192,
        }
    }
}

/// `QIODriveData`: the types a drive holds, in the order it took them, and their counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Drive<K> {
    pub tier: DriveTier,
    pub items: Vec<(K, u64)>,
    pub count: u64,
}

impl<K: PartialEq + Clone> Drive<K> {
    pub fn new(tier: DriveTier, items: Vec<(K, u64)>) -> Self {
        let mut items = items;
        items.retain(|(_, n)| *n > 0);
        let count = items.iter().map(|(_, n)| n).sum();
        Self { tier, items, count }
    }

    pub fn stored(&self, key: &K) -> u64 {
        self.items.iter().find(|(k, _)| k == key).map_or(0, |(_, n)| *n)
    }

    /// `QIODriveData.add`: what is left of `amount`. A full drive takes none; a new type none once it holds its
    /// tier's types.
    fn add(&mut self, key: &K, amount: u64, execute: bool) -> u64 {
        let at = self.items.iter().position(|(k, _)| k == key);
        if self.count == self.tier.max_count() || (at.is_none() && self.items.len() == self.tier.max_types()) {
            return amount;
        }
        let to_add = amount.min(self.tier.max_count() - self.count);
        if execute {
            match at {
                Some(i) => self.items[i].1 += to_add,
                None => self.items.push((key.clone(), to_add)),
            }
            self.count += to_add;
        }
        amount - to_add
    }

    /// `QIODriveData.remove`: what it gave.
    fn remove(&mut self, key: &K, amount: u64, execute: bool) -> u64 {
        let Some(i) = self.items.iter().position(|(k, _)| k == key) else {
            return 0;
        };
        let taken = amount.min(self.items[i].1);
        if execute {
            self.items[i].1 -= taken;
            if self.items[i].1 == 0 {
                self.items.remove(i);
            }
            self.count -= taken;
        }
        taken
    }
}

/// `QIOFrequency`'s storage: its drives in the order they joined (`driveMap`), each type's total and the drives
/// that hold it (`QIOItemTypeData`), in the order the types came (`itemDataMap`).
#[derive(Clone, Debug, Default)]
pub struct Storage<D, K> {
    pub drives: Vec<(D, Drive<K>)>,
    pub types: Vec<TypeData<D, K>>,
    pub total: u64,
    /// Drives whose items changed since `take_changed`: their stacks are written again.
    changed: Vec<D>,
    /// Bumped on every change: the item viewers pack the list again.
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub struct TypeData<D, K> {
    pub key: K,
    pub count: u64,
    pub drives: Vec<D>,
}

impl<D: PartialEq + Clone, K: PartialEq + Clone> Storage<D, K> {
    pub fn new() -> Self {
        Self { drives: Vec::new(), types: Vec::new(), total: 0, changed: Vec::new(), revision: 0 }
    }

    /// `getTotalItemCountCapacity`.
    pub fn count_capacity(&self) -> u64 {
        self.drives.iter().map(|(_, d)| d.tier.max_count()).sum()
    }

    /// `getTotalItemTypeCapacity`.
    pub fn type_capacity(&self) -> usize {
        self.drives.iter().map(|(_, d)| d.tier.max_types()).sum()
    }

    pub fn stored(&self, key: &K) -> u64 {
        self.types.iter().find(|t| &t.key == key).map_or(0, |t| t.count)
    }

    pub fn has_drive(&self, id: &D) -> bool {
        self.drives.iter().any(|(d, _)| d == id)
    }

    pub fn drive(&self, id: &D) -> Option<&Drive<K>> {
        self.drives.iter().find(|(d, _)| d == id).map(|(_, d)| d)
    }

    /// `addDrive`: its items join the frequency's.
    pub fn add_drive(&mut self, id: D, drive: Drive<K>) {
        if self.has_drive(&id) {
            return;
        }
        for (key, n) in &drive.items {
            match self.types.iter_mut().find(|t| &t.key == key) {
                Some(t) => {
                    t.count += n;
                    t.drives.push(id.clone());
                }
                None => self.types.push(TypeData { key: key.clone(), count: *n, drives: vec![id.clone()] }),
            }
            self.total += n;
        }
        self.drives.push((id, drive));
        self.revision += 1;
    }

    /// `removeDrive`: its items leave the frequency; the drive as it is now.
    pub fn remove_drive(&mut self, id: &D) -> Option<Drive<K>> {
        let at = self.drives.iter().position(|(d, _)| d == id)?;
        let (_, drive) = self.drives.remove(at);
        for (key, n) in &drive.items {
            if let Some(i) = self.types.iter().position(|t| &t.key == key) {
                let t = &mut self.types[i];
                t.count -= n;
                t.drives.retain(|d| d != id);
                if t.count == 0 {
                    self.types.remove(i);
                }
            }
            self.total -= n;
        }
        self.changed.retain(|d| d != id);
        self.revision += 1;
        Some(drive)
    }

    /// `massInsert`: how many of `amount` it took. None once full, nor a new type once it holds its types; the
    /// drives that hold the type first, then the others in order (`QIOItemTypeData.add`).
    pub fn insert(&mut self, key: &K, amount: u64, execute: bool) -> u64 {
        if amount == 0 {
            return 0;
        }
        let at = self.types.iter().position(|t| &t.key == key);
        if self.total == self.count_capacity() || (at.is_none() && self.types.len() == self.type_capacity()) {
            return 0;
        }
        let holding: Vec<D> = at.map_or(Vec::new(), |i| self.types[i].drives.clone());
        let mut left = amount;
        let mut joined = Vec::new();
        let mut touched = Vec::new();
        for id in &holding {
            if let Some((_, drive)) = self.drives.iter_mut().find(|(d, _)| d == id) {
                let rest = drive.add(key, left, execute);
                if rest != left {
                    touched.push(id.clone());
                }
                left = rest;
            }
            if left == 0 {
                break;
            }
        }
        if left > 0 {
            for (id, drive) in self.drives.iter_mut() {
                if holding.contains(id) {
                    continue;
                }
                let rest = drive.add(key, left, execute);
                if rest != left {
                    joined.push(id.clone());
                    touched.push(id.clone());
                }
                left = rest;
                if left == 0 {
                    break;
                }
            }
        }
        let taken = amount - left;
        if execute && taken > 0 {
            let i = at.unwrap_or_else(|| {
                self.types.push(TypeData { key: key.clone(), count: 0, drives: Vec::new() });
                self.types.len() - 1
            });
            self.types[i].count += taken;
            self.types[i].drives.extend(joined);
            self.total += taken;
            self.mark(touched);
        }
        taken
    }

    /// `massExtract`: how many of `amount` it gave, from the drives that hold the type in order.
    pub fn extract(&mut self, key: &K, amount: u64, execute: bool) -> u64 {
        let Some(at) = self.types.iter().position(|t| &t.key == key) else {
            return 0;
        };
        let want = amount.min(self.types[at].count);
        let mut left = want;
        let mut touched = Vec::new();
        let mut emptied = Vec::new();
        for id in self.types[at].drives.clone() {
            if left == 0 {
                break;
            }
            if let Some((_, drive)) = self.drives.iter_mut().find(|(d, _)| *d == id) {
                let took = drive.remove(key, left, execute);
                if took > 0 {
                    touched.push(id.clone());
                }
                if drive.stored(key) == 0 {
                    emptied.push(id);
                }
                left -= took;
            }
        }
        let given = want - left;
        if execute && given > 0 {
            let t = &mut self.types[at];
            t.count -= given;
            t.drives.retain(|d| !emptied.contains(d));
            if t.count == 0 {
                self.types.remove(at);
            }
            self.total -= given;
            self.mark(touched);
        }
        given
    }

    fn mark(&mut self, touched: Vec<D>) {
        for id in touched {
            if !self.changed.contains(&id) {
                self.changed.push(id);
            }
        }
        self.revision += 1;
    }

    /// The drives whose items changed since the last call.
    pub fn take_changed(&mut self) -> Vec<D> {
        std::mem::take(&mut self.changed)
    }

    /// Takes a drive's write back: it was written by its holder.
    pub fn forget_changed(&mut self, id: &D) {
        self.changed.retain(|d| d != id);
    }
}

/// `TextUtils.getAmountString`-like short counts the viewer prints on its slots (`QIO`'s `1.2K`, `16M`).
pub fn short_count(n: u64) -> String {
    const UNITS: [(u64, &str); 4] = [(1_000_000_000_000, "T"), (1_000_000_000, "G"), (1_000_000, "M"), (1_000, "K")];
    for (unit, suffix) in UNITS {
        if n >= unit {
            let whole = n / unit;
            if whole >= 100 {
                return format!("{whole}{suffix}");
            }
            let tenth = (n % unit) * 10 / unit;
            return if tenth == 0 { format!("{whole}{suffix}") } else { format!("{whole}.{tenth}{suffix}") };
        }
    }
    n.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    type S = Storage<u8, &'static str>;

    #[test]
    fn tiers_hold_mekanisms_counts_and_types() {
        let got: Vec<(u64, usize)> = DriveTier::ALL.iter().map(|t| (t.max_count(), t.max_types())).collect();
        assert_eq!(got, vec![(16_000, 128), (128_000, 256), (1_048_000, 1_024), (16_000_000_000, 8_192)]);
        assert_eq!(DriveTier::of_item("mekanism:qio_drive_hyper_dense"), Some(DriveTier::HyperDense));
    }

    #[test]
    fn insert_fills_the_holding_drive_first_then_the_others() {
        let mut s = S::new();
        s.add_drive(0, Drive::new(DriveTier::Base, vec![]));
        s.add_drive(1, Drive::new(DriveTier::Base, vec![("stone", 10)]));
        assert_eq!(s.insert(&"stone", 15_995, true), 15_995);
        assert_eq!(s.drive(&1).unwrap().stored(&"stone"), 16_000);
        assert_eq!(s.drive(&0).unwrap().stored(&"stone"), 5);
        assert_eq!(s.stored(&"stone"), 16_005);
        assert_eq!(s.take_changed(), vec![1, 0]);
        assert_eq!(s.insert(&"dirt", 20_000, false), 15_995, "a simulation only counts");
        assert_eq!(s.total, 16_005);
    }

    #[test]
    fn a_full_frequency_takes_none_and_types_are_limited() {
        let mut s = S::new();
        assert_eq!(s.insert(&"stone", 1, true), 0, "no drive, no room");
        s.add_drive(0, Drive::new(DriveTier::Base, vec![("stone", 16_000)]));
        assert_eq!(s.insert(&"stone", 1, true), 0, "a full drive");
        let mut d = Drive::new(DriveTier::Base, vec![]);
        for i in 0..128u8 {
            let k: &'static str = Box::leak(format!("t{i}").into_boxed_str());
            assert_eq!(d.add(&k, 1, true), 0);
        }
        assert_eq!(d.add(&"new", 5, true), 5, "a drive holding its 128 types takes no new one");
        assert_eq!(d.add(&"t3", 5, true), 0, "but more of a type it holds");
    }

    #[test]
    fn extract_and_remove_drive_keep_the_totals() {
        let mut s = S::new();
        s.add_drive(0, Drive::new(DriveTier::Base, vec![("stone", 40), ("dirt", 3)]));
        s.add_drive(1, Drive::new(DriveTier::Base, vec![("stone", 30)]));
        assert_eq!(s.extract(&"stone", 64, true), 64);
        assert_eq!((s.drive(&0).unwrap().stored(&"stone"), s.drive(&1).unwrap().stored(&"stone")), (0, 6));
        assert_eq!(s.types.iter().find(|t| t.key == "stone").unwrap().drives, vec![1]);
        assert_eq!(s.extract(&"dirt", 64, true), 3);
        assert!(s.types.iter().all(|t| t.key != "dirt"));
        let gone = s.remove_drive(&1).unwrap();
        assert_eq!(gone.count, 6);
        assert_eq!((s.total, s.types.len()), (0, 0));
    }

    #[test]
    fn short_counts() {
        assert_eq!([999, 1_000, 1_250, 16_000, 128_000, 16_000_000_000].map(short_count), ["999", "1K", "1.2K", "16K", "128K", "16G"]);
    }
}
