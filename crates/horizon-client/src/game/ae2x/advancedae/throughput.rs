//! Advanced AE's ME Throughput Monitor (`ThroughputMonitorPart`): AE2's
//! Storage Monitor (`AbstractMonitorPart`, its models, lock and key) that also
//! shows, under the amount, how fast the amount changes (`ThroughputCache`):
//! the mean of the changes between its readings over its routine's span, per
//! tick, second, minute or ten minutes (`WorkRoutine`), which the Throughput
//! Monitor Configurator turns (`onUseItemOn`). A reading every 20 to 100 ticks
//! (`TickingRequest(20, 100, ..)`, `SLOWER` each time). Derived from Advanced
//! AE, https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
use crate::game::{Game, ae2_tick_tracker::Modulation, ae2_visual_sync::RateVisual};
use ae2_cell_core::{AmountFormat, ItemKey, KeyType};
use horizon_client::{ae2x::advancedae as rows, containers::BlockEntity};
use horizon_world::nbt::Tag;
use std::collections::VecDeque;

type Pos = (i32, i32, i32);

/// `WorkRoutine`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) enum Routine {
    Tick,
    #[default]
    Second,
    Minute,
    TenMinute,
}
impl Routine {
    /// `fromInt`, of what `writeToNBT` saves (`ordinal`).
    pub(in crate::game) fn from_int(value: i32) -> Self {
        match value {
            0 => Self::Tick,
            2 => Self::Minute,
            3 => Self::TenMinute,
            _ => Self::Second,
        }
    }
    pub(in crate::game) fn ordinal(self) -> i32 {
        match self {
            Self::Tick => 0,
            Self::Second => 1,
            Self::Minute => 2,
            Self::TenMinute => 3,
        }
    }
    pub(in crate::game) fn cycle(self) -> Self {
        match self {
            Self::Tick => Self::Second,
            Self::Second => Self::Minute,
            Self::Minute => Self::TenMinute,
            Self::TenMinute => Self::Tick,
        }
    }
    /// `ticks`: the value is a change per this many ticks.
    fn ticks(self) -> f64 {
        match self {
            Self::Tick => 1.0,
            Self::Second => 20.0,
            Self::Minute => 1200.0,
            Self::TenMinute => 12000.0,
        }
    }
    /// `timeLimit_s`: the span of the readings averaged.
    fn time_limit_s(self, cache_size: u32) -> u64 {
        match self {
            Self::Tick => 10,
            Self::Second => 20,
            Self::Minute => u64::from(cache_size / 2),
            Self::TenMinute => u64::from(cache_size) * 5,
        }
    }
    /// The line's `AAEText`: `%s%s/t`, `/s`, `/m`, `/10m`.
    fn text(self) -> &'static str {
        match self {
            Self::Tick => "gui.advanced_ae.OverdriveThroughputMonitorValue",
            Self::Second => "gui.advanced_ae.ThroughputMonitorValue",
            Self::Minute => "gui.advanced_ae.SlowThroughputMonitorValue",
            Self::TenMinute => "gui.advanced_ae.SlowerThroughputMonitorValue",
        }
    }
}
/// `writeToNBT`'s `routine`, in the part's record.
pub(in crate::game) const ROUTINE: &str = "routine";

/// What a monitor keeps while it runs (`ThroughputCache`, `lastReportedValue`,
/// `lastHumanReadableValue`, `lastWorkRoutine`); none of it is saved.
#[derive(Clone, Debug)]
pub(in crate::game) struct Monitor {
    /// The readings, the newest first: an amount and its tick.
    readings: VecDeque<(i64, u64)>,
    last_routine: Routine,
    reported: f64,
    text: String,
    key: Option<(ItemKey, bool)>,
}
impl Default for Monitor {
    fn default() -> Self {
        Self { readings: VecDeque::new(), last_routine: Routine::Second, reported: -1.0, text: String::new(), key: None }
    }
}
impl Monitor {
    /// `ThroughputCache.push`: one reading a tick, at most `max`.
    fn push(&mut self, amount: i64, tick: u64, max: usize) {
        if tick == 0 || self.readings.front().is_some_and(|(_, at)| *at == tick) {
            return;
        }
        self.readings.push_front((amount, tick));
        if self.readings.len() > max {
            self.readings.pop_back();
        }
    }
    /// `averagePerTick`: the mean of the changes per tick between the
    /// readings of the last `time_limit_s` seconds.
    fn average_per_tick(&self, now: u64, time_limit_s: u64) -> f64 {
        let limit = now as i64 - time_limit_s as i64 * 20;
        let mut last: Option<(i64, u64)> = None;
        let mut averages = Vec::new();
        for &(amount, at) in &self.readings {
            if (at as i64) < limit {
                break;
            }
            if let Some((last_amount, last_at)) = last {
                averages.push((last_amount - amount) as f64 / (last_at as f64 - at as f64));
            }
            last = Some((amount, at));
        }
        let size = averages.len() as f64;
        averages.iter().fold(0.0, |sum, average| sum + average / size)
    }
    /// `updateState`.
    fn update(&mut self, amount: i64, now: u64, routine: Routine, cache_size: u32) {
        self.push(amount, now, cache_size as usize);
        self.last_routine = routine;
    }
    /// `tickingRequest` of an active monitor with a key: true for `URGENT`
    /// (no reading yet), else `SLOWER`.
    fn step(&mut self, routine: Routine, now: u64, amount: i64, key_type: KeyType, cache_size: u32) -> bool {
        if self.readings.is_empty() {
            self.update(amount, now, routine, cache_size);
            self.text = "-".into();
            return true;
        }
        if routine == self.last_routine {
            self.reported = self.average_per_tick(now, routine.time_limit_s(cache_size)) * routine.ticks();
            let value = self.reported.abs();
            self.text = if value > 10.0 || self.reported == 0.0 {
                // Math.round, then formatAmount(.., SLOT).
                key_type.format_amount((value + 0.5).floor() as u64, AmountFormat::Slot)
            } else {
                format!("{value:.2}")
            };
        } else {
            self.text.clear();
        }
        self.update(amount, now, routine, cache_size);
        false
    }
    /// `resetState`.
    fn reset(&mut self) {
        self.readings.clear();
        self.text.clear();
    }
    /// The line `renderDynamic` draws: a sign, the value, green when it
    /// grows, red when it falls (`AEColor.GREEN`, `RED.mediumVariant`).
    fn line(&self, routine: Routine) -> RateVisual {
        let (sign, color) = if self.reported > 0.0 {
            ("+", Some(0x17b86d))
        } else if self.reported == 0.0 {
            ("", None)
        } else {
            ("-", Some(0xd73e42))
        };
        RateVisual { key: routine.text().into(), sign: sign.into(), value: self.text.clone(), color }
    }
}

impl Game<'_> {
    /// The Throughput Monitor on `face` of the cable bus at `pos`: its routine.
    fn advancedae_throughput_routine(&self, pos: Pos, face: &str) -> Option<Routine> {
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&pos) else { return None };
        let Some(Tag::List(parts)) = machine.config.get(&"parts".into()) else { return None };
        let part = parts.iter().find(|p| p.get("face") == Some(&Tag::String(face.into())))?;
        if part.get("id") != Some(&Tag::String(rows::THROUGHPUT_MONITOR.into())) {
            return None;
        }
        Some(Routine::from_int(match part.get(ROUTINE) {
            Some(Tag::Int(n)) => *n,
            _ => 1,
        }))
    }
    /// The monitors on this level's cable buses: where, and their routines.
    fn advancedae_throughput_monitors(&self) -> Vec<(Pos, String, Routine)> {
        let mut found = Vec::new();
        for (pos, entity) in self.world.block_entities.iter() {
            let BlockEntity::Ae2(machine) = entity else { continue };
            let Some(Tag::List(parts)) = machine.config.get(&"parts".into()) else { continue };
            for part in parts {
                if part.get("id") != Some(&Tag::String(rows::THROUGHPUT_MONITOR.into())) {
                    continue;
                }
                let Some(Tag::String(face)) = part.get("face") else { continue };
                let Ok(face) = face.to_string() else { continue };
                let routine = Routine::from_int(match part.get(ROUTINE) {
                    Some(Tag::Int(n)) => *n,
                    _ => 1,
                });
                found.push((*pos, face, routine));
            }
        }
        found.sort_unstable_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        found
    }
    /// What the grid of the cable at `pos` holds of `key` (`getAmount`).
    fn advancedae_monitor_amount(&self, pos: Pos, key: &(ItemKey, bool)) -> i64 {
        let stored = if key.1 { self.ae2_grid_fluids(pos) } else { self.ae2_grid_inventory(pos) };
        let amount = stored.unwrap_or_default().into_iter().find_map(|(k, n)| (k == key.0).then_some(n)).unwrap_or(0);
        amount.min(i64::MAX as u64) as i64
    }

    /// Each Throughput Monitor's `tickingRequest` on the server, and
    /// `configureWatchers`: a new key's reading, and the monitor wakes.
    pub(in crate::game) fn tick_advancedae_throughput(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let monitors = self.advancedae_throughput_monitors();
        if monitors.is_empty() {
            return;
        }
        let now = self.ae2_server_tick();
        let network = self.ae2_network();
        let services = self.ae2_event_service_ids(&network);
        let size = rows::QuantumConfig::get().throughput_cache_size;
        for (pos, face, routine) in monitors {
            let (Some(address), Some(side)) = (self.ae2_local_address(pos), crate::game::ae2_grid::face_index(&face)) else { continue };
            let Some(identity) = network.part_identities.get(&(address.network_pos(), side)).copied() else { continue };
            let key = self.ae2_monitor_state(pos, &face).and_then(|state| state.key);
            let online = self.ae2_part_online(pos, &face);
            let mut monitor = self.ae2x.advancedae.throughput.remove(&identity).unwrap_or_default();
            match key.as_ref().filter(|_| online) {
                // Inactive or showing nothing: resetState and SLEEP.
                None => {
                    monitor.reset();
                    monitor.key = key;
                }
                Some(shown) => {
                    let amount = self.advancedae_monitor_amount(pos, shown);
                    let changed = monitor.key.as_ref() != Some(shown);
                    if changed {
                        monitor.key = Some(shown.clone());
                        monitor.update(amount, now, routine, size);
                    }
                    // The configurator's turn alerts the monitor (`cycleWorkRoutine`).
                    let alert = changed || routine != monitor.last_routine;
                    let service = services.as_ref().and_then(|s| s.get(&identity).copied());
                    if self.ae2_device_scheduler.borrow_mut().due(identity, service, now, 20, 100, false, alert) {
                        let key_type = if shown.1 { KeyType::FLUIDS } else { KeyType::ITEMS };
                        let urgent = monitor.step(routine, now, amount, key_type, size);
                        self.ae2_device_scheduler.borrow_mut().completed(identity, now, if urgent { Modulation::Urgent } else { Modulation::Slower });
                    }
                }
            }
            self.ae2x.advancedae.throughput.insert(identity, monitor);
        }
    }

    /// The line under the Throughput Monitor's amount on `face` of the
    /// cable at `pos`; none for any other part.
    pub(in crate::game) fn advancedae_monitor_line(&self, network: &ae2_network_core::Network, pos: Pos, face: &str) -> Option<RateVisual> {
        let routine = self.advancedae_throughput_routine(pos, face)?;
        let identity = self.ae2_local_address(pos).zip(crate::game::ae2_grid::face_index(face)).and_then(|(address, side)| network.part_identities.get(&(address.network_pos(), side)).copied());
        let monitor = identity.and_then(|id| self.ae2x.advancedae.throughput.get(&id).cloned()).unwrap_or_default();
        Some(monitor.line(routine))
    }

    /// `onUseItemOn` with the Throughput Monitor Configurator on a Throughput
    /// Monitor: its routine turns (`cycleWorkRoutine`). False for any other
    /// item or part.
    pub(in crate::game) fn advancedae_configurator_use(&mut self, pos: Pos, face: &str) -> bool {
        if self.held().is_none_or(|held| held.item.as_ref() != rows::MONITOR_CONFIGURATOR) {
            return false;
        }
        let Some(routine) = self.advancedae_throughput_routine(pos, face) else { return false };
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&pos) else { return false };
        let Some(Tag::List(parts)) = machine.config.get_mut(&"parts".into()) else { return false };
        let Some(Tag::Compound(part)) = parts.iter_mut().find(|p| p.get("face") == Some(&Tag::String(face.into()))) else { return false };
        part.insert(ROUTINE.into(), Tag::Int(routine.cycle().ordinal()));
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        true
    }

    /// `/ae2 advae throughput`: the monitor's routine and line.
    pub(in crate::game) fn advancedae_throughput_status(&self, pos: Pos, face: &str) -> Result<serde_json::Value, String> {
        let routine = self.advancedae_throughput_routine(pos, face).ok_or("no Throughput Monitor on that face")?;
        let network = self.ae2_network();
        let line = self.advancedae_monitor_line(&network, pos, face).ok_or("no Throughput Monitor on that face")?;
        let identity = self.ae2_local_address(pos).zip(crate::game::ae2_grid::face_index(face)).and_then(|(address, side)| network.part_identities.get(&(address.network_pos(), side)).copied());
        let readings = identity.and_then(|id| self.ae2x.advancedae.throughput.get(&id)).map_or(0, |m| m.readings.len());
        Ok(serde_json::json!({"routine": routine.ordinal(), "key": line.key, "sign": line.sign, "value": line.value, "readings": readings}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_is_the_mean_change_per_tick_over_the_routine_span() {
        let mut m = Monitor::default();
        assert!(m.step(Routine::Second, 100, 0, KeyType::ITEMS, 80), "no reading yet: URGENT");
        assert_eq!(m.text, "-");
        // 40 items in 20 ticks, then 20 in 20: 1.5 a tick, 30 a second.
        assert!(!m.step(Routine::Second, 120, 40, KeyType::ITEMS, 80));
        assert!(!m.step(Routine::Second, 140, 60, KeyType::ITEMS, 80));
        assert_eq!(m.average_per_tick(140, 20), 1.5);
        assert!(!m.step(Routine::Second, 160, 60, KeyType::ITEMS, 80));
        assert_eq!((m.reported, m.text.as_str()), (30.0, "30"));
        assert_eq!(m.line(Routine::Second), RateVisual { key: "gui.advanced_ae.ThroughputMonitorValue".into(), sign: "+".into(), value: "30".into(), color: Some(0x17b86d) });
        m.step(Routine::Tick, 180, 60, KeyType::ITEMS, 80);
        assert_eq!(m.text, "", "the routine just turned: no value");
        // Small values have two decimals; a fall is red.
        let mut e = Monitor::default();
        e.update(0, 20, Routine::Tick, 80);
        e.update(3, 29, Routine::Tick, 80);
        e.step(Routine::Tick, 30, 3, KeyType::ITEMS, 80);
        assert_eq!(e.text, "0.33", "3 in 9 ticks");
        let mut d = Monitor::default();
        d.update(100, 20, Routine::Second, 80);
        d.update(60, 40, Routine::Second, 80);
        d.step(Routine::Second, 60, 60, KeyType::ITEMS, 80);
        let line = d.line(Routine::Second);
        assert_eq!((line.sign.as_str(), line.value.as_str(), line.color), ("-", "40", Some(0xd73e42)));
        // Fluids in buckets; readings older than the span left out.
        let mut f = Monitor::default();
        f.update(9_999, 100, Routine::Minute, 80);
        f.update(0, 500, Routine::Minute, 80);
        f.update(120_000, 1100, Routine::Minute, 80);
        assert!(!f.step(Routine::Minute, 1240, 120_000, KeyType::FLUIDS, 80));
        assert_eq!(f.reported, 240_000.0, "200 mB a tick over the last 40 s, a minute of it");
        assert_eq!(f.text, "240");
    }

    #[test]
    fn the_cache_keeps_one_reading_a_tick_and_at_most_its_size() {
        let mut m = Monitor::default();
        m.push(1, 0, 3);
        assert!(m.readings.is_empty(), "tick 0 is never kept");
        for tick in [1, 1, 2, 3, 4] {
            m.push(tick as i64, tick, 3);
        }
        assert_eq!(m.readings.iter().map(|r| r.1).collect::<Vec<_>>(), [4, 3, 2]);
        assert_eq!((Routine::from_int(7), Routine::from_int(0).cycle().cycle().cycle().cycle()), (Routine::Second, Routine::Tick));
        assert_eq!(Routine::TenMinute.time_limit_s(80), 400);
        m.reset();
        assert!(m.readings.is_empty() && m.text.is_empty());
    }
}
