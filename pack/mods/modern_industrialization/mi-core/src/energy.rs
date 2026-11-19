//! `EnergyComponent` (EU) and the Forge Energy bridge of `api/energy/EnergyApi.java`: with
//! `bidirectionalEnergyCompat` (ATM10's startup config has it on) every MI storage is also an
//! `IEnergyStorage` in FE, at `forgeEnergyPerEu` FE to the EU (ATM10's server config: 16; MI's default 10).

/// MI's default `forgeEnergyPerEu`.
pub const DEFAULT_FE_PER_EU: i64 = 10;

/// An EU buffer (`EnergyComponent`): what `storedEu` holds and the casing's capacity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnergyStore {
    pub eu: i64,
    pub capacity: i64,
}

impl EnergyStore {
    /// `readNbt`: capped at the capacity.
    pub fn new(eu: i64, capacity: i64) -> Self {
        EnergyStore { eu: eu.clamp(0, capacity.max(0)), capacity }
    }
    /// `consumeEu`.
    pub fn consume(&mut self, max: i64, act: bool) -> i64 {
        let ext = max.max(0).min(self.eu);
        if act {
            self.eu -= ext;
        }
        ext
    }
    /// `insertEu`.
    pub fn insert(&mut self, max: i64, act: bool) -> i64 {
        let ins = max.max(0).min(self.capacity - self.eu);
        if act {
            self.eu += ins;
        }
        ins
    }
    /// `WrappedMIStorage.receive`: FE offered, in whole EU; the FE taken.
    pub fn receive_fe(&mut self, max_fe: i64, ratio: i64, act: bool) -> i64 {
        let ratio = ratio.max(1);
        self.insert(max_fe / ratio, act) * ratio
    }
    /// `WrappedMIStorage.getAmount`, `getCapacity`.
    pub fn fe(&self, ratio: i64) -> (i64, i64) {
        (self.eu.saturating_mul(ratio), self.capacity.saturating_mul(ratio))
    }
}

/// `forgeEnergyPerEu` from `modern_industrialization-server.toml` (a `key = value` line), MI's default
/// without one; MI's range is 1 to 1000.
pub fn fe_per_eu(server_toml: Option<&str>) -> i64 {
    server_toml
        .and_then(|t| {
            t.lines().find_map(|l| {
                let (k, v) = l.split_once('=')?;
                (k.trim() == "forgeEnergyPerEu").then(|| v.trim().parse::<i64>().ok()).flatten()
            })
        })
        .map_or(DEFAULT_FE_PER_EU, |n| n.clamp(1, 1000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fe_bridge_in_whole_eu() {
        // An LV casing's 3,200 EU are 51,200 FE at ATM10's 16.
        let mut e = EnergyStore::new(0, 3200);
        assert_eq!(e.fe(16), (0, 51_200));
        // 1,000 FE offered: 62 EU, 992 FE taken; 15 FE are less than an EU and nothing is taken.
        assert_eq!(e.receive_fe(1000, 16, false), 992);
        assert_eq!(e.eu, 0);
        assert_eq!(e.receive_fe(1000, 16, true), 992);
        assert_eq!(e.eu, 62);
        assert_eq!(e.receive_fe(15, 16, true), 0);
        // Full: the rest of the room only.
        assert_eq!(e.receive_fe(i64::from(i32::MAX), 16, true), (3200 - 62) * 16);
        assert_eq!(e.eu, 3200);
        assert_eq!(e.consume(40, false), 40);
        assert_eq!(e.consume(5000, true), 3200);
        assert_eq!(EnergyStore::new(9999, 3200).eu, 3200);
    }

    #[test]
    fn reads_atm10_server_config() {
        let atm10 = "#comment\nforgeEnergyPerEu = 16\nbaseItemPipeTransfer = 16\n";
        assert_eq!(fe_per_eu(Some(atm10)), 16);
        assert_eq!(fe_per_eu(None), 10);
        assert_eq!(fe_per_eu(Some("forgeEnergyPerEu = 5000")), 1000);
    }
}
