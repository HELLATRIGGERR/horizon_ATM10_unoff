//! The Energy Transmitter (`EnergyTransmitterBE`, `TransmitterEnergyStorage`): every 50 ticks it scans its area for blocks
//! that take Forge Energy and for other transmitters; each tick it sends up to `rf_per_tick` to each block (losing a
//! percent of it for each block of distance) from the energy of the whole network of transmitters it sees, and then balances
//! that network. A transmitter's storage answers for its network: what it holds is the network's total.
use crate::{config::TransmitterSettings, machines::Clock};

/// `tickSpeed = 50`: how often the area is scanned again.
pub const SCAN_TICKS: i32 = 50;

/// A new transmitter's clock (`tickSpeed` 50).
pub fn new_clock() -> Clock {
    Clock { tick_speed: SCAN_TICKS, ..Clock::default() }
}

/// `calculateLoss`: what of `amount` arrives after `distance` blocks (Manhattan) at `loss_per_block` percent a block.
pub fn after_loss(amount: i32, distance: i32, loss_per_block: f64) -> i32 {
    let loss = (loss_per_block * f64::from(distance.abs())) / 100.0;
    amount - (f64::from(amount) * loss).floor() as i32
}

/// The transmitters one transmitter sees (itself among them, at `me`), each with its own energy; `cap` is each one's buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub energies: Vec<i32>,
    pub cap: i32,
    pub me: usize,
}

impl Network {
    /// `getTotalEnergyStored`: what this transmitter's storage says it holds.
    pub fn total(&self) -> i32 {
        self.energies.iter().map(|e| i64::from(*e)).sum::<i64>().clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }

    /// `getTotalMaxEnergyStored`.
    pub fn max_total(&self) -> i32 {
        (i64::from(self.cap) * self.energies.len() as i64).min(i64::from(i32::MAX)) as i32
    }

    /// `distributeEnergy`: `realReceiveEnergy` into each transmitter in turn until all is given.
    pub fn distribute(&mut self, mut energy: i32) -> i32 {
        let mut inserted = 0;
        for e in &mut self.energies {
            let got = (self.cap - *e).min(self.cap.min(energy)).max(0);
            *e += got;
            energy -= got;
            inserted += got;
            if energy <= 0 {
                break;
            }
        }
        inserted
    }

    /// `TransmitterEnergyStorage.receiveEnergy`: what the network takes (and gives out among its transmitters).
    pub fn receive(&mut self, amount: i32, simulate: bool) -> i32 {
        let got = (self.max_total() - self.total()).min(self.cap.min(amount)).max(0);
        if got > 0 && !simulate {
            self.distribute(got);
        }
        got
    }

    /// `TransmitterEnergyStorage.extractEnergy`: up to what the network holds is asked, and it comes out of this
    /// transmitter's own energy (which can go below 0 while the others hold it, until a balance).
    pub fn extract(&mut self, amount: i32, simulate: bool) -> i32 {
        let got = self.total().min(self.cap.min(amount)).max(0);
        if !simulate {
            self.energies[self.me] -= got;
        }
        got
    }

    /// `balanceEnergy`: with two or more transmitters, each is given the average (the remainder one more each, in order)
    /// unless they are all within one of it. Whether anything changed.
    pub fn balance(&mut self) -> bool {
        let n = self.energies.len() as i32;
        if n <= 1 {
            return false;
        }
        let total = self.total();
        let (average, remainder) = (total / n, total % n);
        let top = average + i32::from(remainder > 0);
        if self.energies.iter().all(|e| *e == average || *e == top) {
            return false;
        }
        for (i, e) in self.energies.iter_mut().enumerate() {
            *e = average + i32::from((i as i32) < remainder);
        }
        true
    }

    /// `transmitPowerWithLoss`: `offer` gives a machine what of an amount it takes (with `simulate`, would take); what it was
    /// given. `distance`: the Manhattan distance to the machine.
    pub fn send(&mut self, s: &TransmitterSettings, distance: i32, mut offer: impl FnMut(i32, bool) -> i32) -> i32 {
        let fit = offer(s.rf_per_tick, true);
        if fit <= 0 {
            return 0;
        }
        let taken = self.extract(fit, false);
        offer(after_loss(taken, distance, s.loss_per_block), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> TransmitterSettings {
        TransmitterSettings::default()
    }

    #[test]
    fn the_loss_is_a_percent_a_block_rounded_down() {
        assert_eq!(after_loss(1000, 1, 1.0), 990);
        assert_eq!(after_loss(1000, 10, 1.0), 900);
        assert_eq!(after_loss(99, 1, 1.0), 99, "less than a unit of loss is none");
        assert_eq!(after_loss(1000, 0, 1.0), 1000);
        assert_eq!(after_loss(1000, 200, 1.0), -1000, "a very distant machine is Java's overflow of the percent");
    }

    #[test]
    fn a_network_balances_to_the_average_with_the_remainder_in_order() {
        let mut n = Network { energies: vec![10, 0, 0], cap: 1000, me: 0 };
        assert!(n.balance());
        assert_eq!(n.energies, vec![4, 3, 3]);
        assert!(!n.balance(), "within one of the average");
        let mut one = Network { energies: vec![7], cap: 1000, me: 0 };
        assert!(!one.balance());
    }

    #[test]
    fn receiving_fills_the_transmitters_in_turn_up_to_the_network() {
        let mut n = Network { energies: vec![900, 0], cap: 1000, me: 1 };
        assert_eq!(n.max_total(), 2000);
        assert_eq!(n.receive(500, true), 500);
        assert_eq!(n.energies, vec![900, 0], "simulated");
        assert_eq!(n.receive(500, false), 500);
        assert_eq!(n.energies, vec![1000, 400], "the first fills first");
        assert_eq!(n.receive(5000, false), 600, "the network's room, at most a buffer a call");
        assert_eq!(n.total(), 2000);
        assert_eq!(n.receive(5, false), 0);
    }

    #[test]
    fn sending_takes_from_this_transmitter_and_arrives_less_the_loss() {
        let mut n = Network { energies: vec![0, 5000], cap: 1_000_000, me: 1 };
        let mut got = Vec::new();
        let sent = n.send(&s(), 2, |amount, simulate| {
            if !simulate {
                got.push(amount);
            }
            amount.min(300)
        });
        assert_eq!(n.energies, vec![0, 4700]);
        assert_eq!((sent, got), (294, vec![294]), "300 taken, two percent lost");
        let mut none = Network { energies: vec![100, 0], cap: 1000, me: 1 };
        assert_eq!(none.extract(60, false), 60, "the network holds it, this one doesn't");
        assert_eq!(none.energies, vec![100, -60]);
        none.balance();
        assert_eq!(none.energies, vec![20, 20]);
        assert_eq!(n.send(&s(), 1, |_, _| 0), 0);
    }
}
