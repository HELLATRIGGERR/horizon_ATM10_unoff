//! `DynamicEnergyStorage`: a machine's RF buffer. Oritech's energy is Forge Energy one to one.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Energy {
    pub amount: i64,
    pub capacity: i64,
    pub max_insert: i64,
    pub max_extract: i64,
}

impl Energy {
    /// `insert`: up to `maxInsert` a call and the room left. Java's can go negative for a buffer over its capacity
    /// (a capacity lowered under it); this takes nothing then.
    pub fn insert(&mut self, amount: i64, simulate: bool) -> i64 {
        let inserted = self.max_insert.min(amount).min(self.capacity - self.amount).max(0);
        if !simulate {
            self.amount += inserted;
        }
        inserted
    }

    /// `extract`: up to `maxExtract` a call and what is held.
    pub fn extract(&mut self, amount: i64, simulate: bool) -> i64 {
        let extracted = amount.min(self.max_extract).min(self.amount).max(0);
        if !simulate {
            self.amount -= extracted;
        }
        extracted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_is_capped_by_rate_and_room() {
        let mut e = Energy { amount: 24_900, capacity: 25_000, max_insert: 256, max_extract: 0 };
        assert_eq!(e.insert(1000, true), 100);
        assert_eq!(e.amount, 24_900, "a simulation changes nothing");
        e.amount = 0;
        assert_eq!(e.insert(1000, false), 256);
        assert_eq!(e.amount, 256);
        assert_eq!(e.extract(50, false), 0, "a machine gives none (maxExtract 0)");
    }
}
