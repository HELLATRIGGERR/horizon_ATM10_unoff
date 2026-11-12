//! Advanced AE's Quantum Armor (`QuantumArmorBase`, `PoweredItem`,
//! `IUpgradeableItem`, `QuantumHelmet`, `QuantumChestplate`,
//! `QuantumLeggings`, `QuantumBoots`, `UpgradeType`, `UpgradeCards`,
//! `AAEPlayerEvents`, `AAELivingEntityEvents`, the quantum armor section of
//! `AAEConfig` at its defaults, which ATM10 keeps). Derived from Advanced
//! AE, https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! Each piece keeps AE in AE2's `ae2:stored_energy` (its capacity
//! `ae2:energy_capacity`, else the piece's own) and its cards in the mod's
//! components, per `UpgradeType`: `<type>_toggle` (present while the card is
//! installed, true while it is on), `<type>_value` and `<type>_filter`. The
//! server runs what the mod runs on the server each tick of a worn piece: the
//! passive cards and the buffs' draw (`tickUpgrades`), night vision and the
//! attribute cards (`ItemAttributeModifierEvent`), flight (`CREATIVE_FLIGHT`)
//! and water breathing; and the damage events of a hit on the wearer (Lava
//! Immunity, Evasion, the absorption into the AE, the boots' fall), through
//! `health::HURT_SERVER`. What the client moves itself (the speed, jump and
//! flight-speed cards) and what needs the grid link is listed in the README.
use super::super::super::{Game, lan::LanHost};
use horizon_client::{
    effects::EffectInstance,
    items::ItemStack,
    text::{GREEN, RED, Text},
};
use horizon_core::random::RandomSource;
use horizon_protocol::packets::play::{Clientbound, SystemChat};
use horizon_world::nbt::Tag;
use std::{cell::RefCell, collections::HashMap};

const STORED: &str = "ae2:stored_energy";
const CAPACITY: &str = "ae2:energy_capacity";
/// `PoweredItem.MIN_POWER`: less is no energy at all.
const MIN_POWER: f64 = 0.0001;
/// `PoweredItem.getChargeRate`, AE a tick.
pub(in crate::game) const CHARGE_RATE: f64 = 20_000.0;
/// `NIGHT_VISION_ACTIVATED`: the helmet gave the effect.
const NIGHT_VISION_ACTIVATED: &str = "advanced_ae:night_vision_activated";
/// `AAEConfig.getRenegerationPerTick` (10, used as 0.1 of it).
const REGENERATION: f32 = 1.0;
/// `AAEConfig.getEvasionChance`, in percent.
const EVASION_CHANCE: f64 = 30.0;
/// `AAEConfig.getPercentageDamageAbsorption`: the most of a hit the worn
/// pieces take into their AE, in percent.
const DAMAGE_ABSORPTION: i32 = 30;
/// `incomingDamage`: the AE a point of damage absorbed costs.
const AE_PER_DAMAGE: f32 = 1000.0;
/// More players than a server holds at once: past it, the worn sets are noted
/// afresh (an entity id is not reused, so a left player's set is only stale).
const MAX_WORN: usize = 4096;

/// `UpgradeType.ApplicationType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Apply {
    Passive,
    PassiveUse,
    Buff,
}

/// An `UpgradeType`: its name (the components' prefix), card, cost (FE a
/// use, drawn as AE), application and `UpgradeSettings` range.
pub(in crate::game) struct Upgrade {
    pub(in crate::game) name: &'static str,
    pub(in crate::game) card: &'static str,
    pub(in crate::game) cost: u32,
    pub(in crate::game) apply: Apply,
    pub(in crate::game) min: i32,
    pub(in crate::game) max: i32,
}

const fn up(name: &'static str, card: &'static str, cost: u32, apply: Apply, min: i32, max: i32) -> Upgrade {
    Upgrade { name, card, cost, apply, min, max }
}

/// `UpgradeType.values()` in order, with `getSettings` at ATM10's config.
pub(in crate::game) const UPGRADES: &[Upgrade] = &[
    up("empty", "advanced_ae:quantum_upgrade_base", 0, Apply::Passive, 1, 1),
    up("walk_speed", "advanced_ae:walk_speed_card", 10, Apply::Passive, 1, 60),
    up("sprint_speed", "advanced_ae:sprint_speed_card", 10, Apply::Passive, 1, 80),
    up("step_assist", "advanced_ae:step_assist_card", 5, Apply::PassiveUse, 1, 3),
    up("jump_height", "advanced_ae:jump_height_card", 10, Apply::PassiveUse, 1, 3),
    up("lava_immunity", "advanced_ae:lava_immunity_card", 10, Apply::PassiveUse, 1, 1),
    up("flight", "advanced_ae:flight_card", 10, Apply::PassiveUse, 1, 10),
    up("water_breathing", "advanced_ae:water_breathing_card", 10, Apply::PassiveUse, 1, 1),
    up("auto_feed", "advanced_ae:auto_feeding_card", 5, Apply::Passive, 1, 1),
    up("auto_stock", "advanced_ae:auto_stock_card", 5, Apply::Passive, 1, 1),
    up("magnet", "advanced_ae:magnet_card", 5, Apply::Passive, 3, 12),
    up("hp_buffer", "advanced_ae:hp_buffer_card", 10, Apply::Buff, 20, 20),
    up("evasion", "advanced_ae:evasion_card", 10, Apply::Buff, 30, 30),
    up("regeneration", "advanced_ae:regeneration_card", 10, Apply::Passive, 1, 1),
    up("strength", "advanced_ae:strength_card", 10, Apply::Buff, 10, 10),
    up("attack_speed", "advanced_ae:attack_speed_card", 10, Apply::Buff, 5, 5),
    up("luck", "advanced_ae:luck_card", 10, Apply::Buff, 2, 2),
    up("reach", "advanced_ae:reach_card", 10, Apply::Buff, 1, 5),
    up("swim_speed", "advanced_ae:swim_speed_card", 5, Apply::Passive, 1, 80),
    up("night_vision", "advanced_ae:night_vision_card", 10, Apply::Buff, 1, 1),
    up("flight_drift", "advanced_ae:flight_drift_card", 10, Apply::Buff, 0, 100),
    up("charging", "advanced_ae:recharging_card", 0, Apply::Passive, 1, 1),
    up("workbench", "advanced_ae:portable_workbench_card", 0, Apply::PassiveUse, 1, 1),
    up("pick_craft", "advanced_ae:pick_craft_card", 1000, Apply::PassiveUse, 1, 1),
    up("camo", "advanced_ae:camo_card", 0, Apply::Passive, 1, 1),
];

/// A piece: its item, its index in `armor` (head first), its
/// `MAX_POWER_STORAGE` and its `registerUpgrades` in order.
pub(in crate::game) struct Piece {
    pub(in crate::game) item: &'static str,
    pub(in crate::game) slot: usize,
    pub(in crate::game) capacity: f64,
    pub(in crate::game) upgrades: &'static [&'static str],
}

pub(in crate::game) const PIECES: &[Piece] = &[
    Piece {
        item: "advanced_ae:quantum_helmet",
        slot: 0,
        capacity: 200_000_000.0,
        upgrades: &["water_breathing", "auto_feed", "auto_stock", "magnet", "luck", "night_vision", "charging", "workbench", "camo"],
    },
    Piece {
        item: "advanced_ae:quantum_chestplate",
        slot: 1,
        capacity: 300_000_000.0,
        upgrades: &["flight", "hp_buffer", "lava_immunity", "regeneration", "strength", "attack_speed", "charging", "pick_craft", "camo"],
    },
    Piece {
        item: "advanced_ae:quantum_leggings",
        slot: 2,
        capacity: 250_000_000.0,
        upgrades: &["walk_speed", "sprint_speed", "swim_speed", "reach", "charging", "camo"],
    },
    Piece {
        item: "advanced_ae:quantum_boots",
        slot: 3,
        capacity: 200_000_000.0,
        upgrades: &["step_assist", "jump_height", "evasion", "flight_drift", "charging", "camo"],
    },
];

/// The attribute cards (`AAEPlayerEvents.itemAttributes`): the card, the
/// attribute, the modifier's id.
const ATTRIBUTES: &[(&str, &str, &str)] = &[
    ("step_assist", "minecraft:step_height", "advanced_ae:step_assist"),
    ("hp_buffer", "minecraft:max_health", "advanced_ae:hp_buffer"),
    ("strength", "minecraft:attack_damage", "advanced_ae:strength_boost"),
    ("attack_speed", "minecraft:attack_speed", "advanced_ae:attack_speed"),
    ("luck", "minecraft:luck", "advanced_ae:luck"),
    ("reach", "minecraft:block_interaction_range", "advanced_ae:reach_boost"),
    ("reach", "minecraft:entity_interaction_range", "advanced_ae:reach_boost"),
];

/// The hotkeys that turn a helmet's card (`registerToggleUpgradeAction`):
/// the action's id and the card.
pub(in crate::game) const TOGGLE_HOTKEYS: &[(&str, &str)] = &[
    ("advanced_ae:quantum_magnet_upgrade", "magnet"),
    ("advanced_ae:quantum_auto_stock_upgrade", "auto_stock"),
    ("advanced_ae:quantum_night_vision_upgrade", "night_vision"),
];

pub(in crate::game) fn piece(item: &str) -> Option<&'static Piece> {
    PIECES.iter().find(|p| p.item == item)
}
pub(in crate::game) fn upgrade(name: &str) -> Option<&'static Upgrade> {
    UPGRADES.iter().find(|u| u.name == name)
}
/// `QuantumUpgradeBaseItem.getType` of a card.
pub(in crate::game) fn card_upgrade(card: &str) -> Option<&'static Upgrade> {
    UPGRADES.iter().find(|u| u.card == card)
}

fn component<'a>(stack: &'a ItemStack, id: &str) -> Option<&'a Tag> {
    stack.data().and_then(|d| d.other(id))
}
fn id(upgrade: &str, kind: &str) -> String {
    format!("advanced_ae:{upgrade}_{kind}")
}
fn truth(tag: Option<&Tag>) -> Option<bool> {
    match tag? {
        Tag::Byte(b) => Some(*b != 0),
        Tag::Int(i) => Some(*i != 0),
        _ => None,
    }
}

/// `hasUpgrade`: the toggle is there.
pub(in crate::game) fn has(stack: &ItemStack, upgrade: &str) -> bool {
    component(stack, &id(upgrade, "toggle")).is_some()
}
/// `isUpgradeEnabled`: the toggle, false when absent.
pub(in crate::game) fn enabled(stack: &ItemStack, upgrade: &str) -> bool {
    truth(component(stack, &id(upgrade, "toggle"))).unwrap_or(false)
}
/// `UPGRADE_VALUE`, or `default`.
pub(in crate::game) fn value(stack: &ItemStack, upgrade: &str, default: i32) -> i32 {
    match component(stack, &id(upgrade, "value")) {
        Some(Tag::Int(v)) => *v,
        _ => default,
    }
}
/// `getAECurrentPower` and `getAEMaxPower`.
pub(in crate::game) fn energy(stack: &ItemStack) -> Option<(f64, f64)> {
    let piece = piece(&stack.item)?;
    let read = |id: &str, default: f64| match component(stack, id) {
        Some(Tag::Double(bits)) => f64::from_bits(*bits),
        _ => default,
    };
    Some((read(STORED, 0.0).max(0.0), read(CAPACITY, piece.capacity)))
}
fn set_stored(stack: &mut ItemStack, power: f64) {
    let tag = (power >= MIN_POWER).then(|| Tag::Double(power.to_bits()));
    stack.data_mut().set_other(STORED, tag);
}
/// `injectAEPower(.., MODULATE)`: the AE it took.
pub(in crate::game) fn inject(stack: &mut ItemStack, amount: f64) -> f64 {
    let Some((current, max)) = energy(stack) else { return 0.0 };
    let taken = amount.max(0.0).min((max - current).max(0.0));
    if taken > 0.0 {
        set_stored(stack, current + taken);
    }
    taken
}
/// `extractAEPower(.., MODULATE)`: the AE it gave.
pub(in crate::game) fn extract(stack: &mut ItemStack, amount: f64) -> f64 {
    let Some((current, _)) = energy(stack) else { return 0.0 };
    let given = amount.max(0.0).min(current);
    if given > 0.0 {
        set_stored(stack, current - given);
    }
    given
}
/// `isUpgradePowered`: the item's FE (AE2's `PoweredItemCapabilities`, two
/// FE an AE, as an int) reaches the card's cost.
pub(in crate::game) fn powered(stack: &ItemStack, upgrade: &Upgrade) -> bool {
    energy(stack).is_some_and(|(current, _)| fe_reaches(current, upgrade.cost))
}
fn fe_reaches(ae: f64, cost: u32) -> bool {
    (ae * 2.0) as i64 >= i64::from(cost)
}
/// `isUpgradeEnabledAndPowered`.
pub(in crate::game) fn active(stack: &ItemStack, upgrade: &str) -> bool {
    self::upgrade(upgrade).is_some_and(|u| enabled(stack, u.name) && powered(stack, u))
}
/// `consumeEnergy`: the cost at `PowerMultiplier.CONFIG`, nothing in creative.
fn consume(stack: &mut ItemStack, cost: u32, creative: bool) {
    if !creative {
        extract(stack, cost_ae(cost));
    }
}
fn cost_ae(cost: u32) -> f64 {
    f64::from(cost) * horizon_client::ae2_common_config::gameplay().usage_multiplier
}

/// A worn set as a hit on its wearer sees it (`health::HURT_SERVER` names the
/// player by entity id alone, in the middle of the hit): each slot's Quantum
/// piece's AE, head first, the damage cards that are on, and what the hits
/// took since the armor's last tick, which then takes it from the pieces.
#[derive(Clone, Debug, Default, PartialEq)]
struct Worn {
    creative: bool,
    stored: [Option<f64>; 4],
    /// The chestplate's Lava Immunity and the boots' Evasion are on (their
    /// power is read from `stored` at the hit).
    lava_immunity: bool,
    evasion: bool,
    /// The AE the hits took from each slot.
    drawn: [f64; 4],
    /// Lava Immunity put the fire out (`setRemainingFireTicks(0)`).
    extinguish: bool,
}

impl Worn {
    fn of(armor: &[Option<ItemStack>; 4], creative: bool) -> Self {
        let worn = |slot: usize| armor[slot].as_ref().filter(|s| piece(&s.item).is_some_and(|p| p.slot == slot));
        Self {
            creative,
            stored: std::array::from_fn(|slot| worn(slot).and_then(energy).map(|(current, _)| current)),
            lava_immunity: worn(1).is_some_and(|s| enabled(s, "lava_immunity")),
            evasion: worn(3).is_some_and(|s| enabled(s, "evasion")),
            ..Self::default()
        }
    }
    /// `extractAEPower(.., MODULATE)` of slot `slot`'s piece: the AE it gave.
    fn extract(&mut self, slot: usize, amount: f64) -> f64 {
        let Some(current) = self.stored[slot] else { return 0.0 };
        let given = amount.max(0.0).min(current);
        let left = current - given;
        self.stored[slot] = Some(if left >= MIN_POWER { left } else { 0.0 });
        self.drawn[slot] += given;
        given
    }
    /// `isUpgradePowered` of slot `slot`'s piece for a card.
    fn powered(&self, slot: usize, upgrade: &str) -> bool {
        self::upgrade(upgrade).is_some_and(|u| self.stored[slot].is_some_and(|ae| fe_reaches(ae, u.cost)))
    }
    /// `consumeEnergy` of a card.
    fn consume(&mut self, slot: usize, upgrade: &str) {
        if let Some(u) = self::upgrade(upgrade)
            && !self.creative
        {
            self.extract(slot, cost_ae(u.cost));
        }
    }
}

thread_local! {
    /// The worn sets by the wearer's entity id, on the thread that ticks the world.
    static WORN: RefCell<HashMap<i32, Worn>> = RefCell::new(HashMap::new());
}

/// Notes what player `entity` wears after the armor's tick: none forgets it.
fn note_worn(entity: i32, worn: Option<Worn>) {
    WORN.with(|sets| {
        let mut sets = sets.borrow_mut();
        match worn {
            Some(worn) => {
                if sets.len() >= MAX_WORN && !sets.contains_key(&entity) {
                    sets.clear();
                }
                sets.insert(entity, worn);
            }
            None => {
                sets.remove(&entity);
            }
        }
    });
}

/// What the hits on player `entity` took from each slot since the last call,
/// and whether Lava Immunity put the fire out.
fn take_hits(entity: i32) -> ([f64; 4], bool) {
    WORN.with(|sets| {
        sets.borrow_mut()
            .get_mut(&entity)
            .map_or(([0.0; 4], false), |w| (std::mem::take(&mut w.drawn), std::mem::take(&mut w.extinguish)))
    })
}

/// A hit of `damage_type` on player `entity` by `amount`, where
/// `LivingEntity.hurtServer` asks its incoming-damage hooks: whether the worn
/// Quantum Armor called it off. The mod's events in their order:
/// `LivingFallDamage` (boots with any AE take the fall's distance to 0, so a
/// fall hurts nothing), `invulnerability` (Lava Immunity against lava and
/// fire, then Evasion's chance against any hit, each at its card's cost) and
/// `incomingDamage` (each worn piece takes up to a quarter of 30% of the hit
/// into its AE, 1,000 AE a point; what is left under 1 is no hit).
pub(in crate::game) fn player_hurt(entity: i32, damage_type: &str, amount: &mut f32) -> bool {
    WORN.with(|sets| {
        let mut sets = sets.borrow_mut();
        let Some(worn) = sets.get_mut(&entity) else { return false };
        let kind = damage_type.strip_prefix("minecraft:").unwrap_or(damage_type);
        // `extractAEPower(stack, 10, SIMULATE) > 0`: no AE is drawn.
        if kind == "fall" && worn.stored[3].is_some_and(|ae| ae > 0.0) {
            return true;
        }
        let mut invulnerable = false;
        if matches!(kind, "lava" | "in_fire" | "on_fire") && worn.lava_immunity && worn.powered(1, "lava_immunity") {
            worn.extinguish = true;
            invulnerable = true;
            worn.consume(1, "lava_immunity");
        }
        // `new Random().nextDouble(100)`: a random of its own each hit.
        if worn.evasion && worn.powered(3, "evasion") && horizon_client::random_source::create().next_double() * 100.0 < EVASION_CHANCE {
            invulnerable = true;
            worn.consume(3, "evasion");
        }
        if invulnerable {
            return true;
        }
        if *amount <= 0.0 {
            return false;
        }
        let per_piece = *amount * DAMAGE_ABSORPTION as f32 / 100.0 / 4.0;
        let mut absorbed = 0.0f32;
        for slot in 0..4 {
            if worn.stored[slot].is_some() {
                absorbed += worn.extract(slot, f64::from(per_piece * AE_PER_DAMAGE)) as f32 / AE_PER_DAMAGE;
            }
        }
        if absorbed > 0.0 {
            *amount = (*amount - absorbed).max(0.0);
        }
        *amount < 1.0
    })
}

/// `applyUpgrade`: a card the piece takes and lacks goes in, on, at its
/// setting's most, with an empty filter (and the magnet's list a black list).
pub(in crate::game) fn apply(stack: &mut ItemStack, upgrade: &Upgrade) -> bool {
    let Some(piece) = piece(&stack.item) else { return false };
    if !piece.upgrades.contains(&upgrade.name) || has(stack, upgrade.name) {
        return false;
    }
    let data = stack.data_mut();
    data.set_other(&id(upgrade.name, "toggle"), Some(Tag::Byte(1)));
    data.set_other(&id(upgrade.name, "value"), Some(Tag::Int(upgrade.max)));
    data.set_other(&id(upgrade.name, "filter"), Some(Tag::List(Vec::new())));
    if upgrade.name == "magnet" {
        data.set_other(&id(upgrade.name, "extra"), Some(Tag::Byte(1)));
    }
    true
}
/// `removeUpgrade`: the toggle, value and filter go (the extra stays).
pub(in crate::game) fn remove(stack: &mut ItemStack, upgrade: &str) -> bool {
    if !has(stack, upgrade) {
        return false;
    }
    for kind in ["toggle", "value", "filter"] {
        stack.data_mut().set_other(&id(upgrade, kind), None);
    }
    true
}
/// `toggleUpgrade`: the new state, none when the card is not installed.
pub(in crate::game) fn toggle(stack: &mut ItemStack, upgrade: &str) -> Option<bool> {
    let now = truth(component(stack, &id(upgrade, "toggle")))?;
    stack.data_mut().set_other(&id(upgrade, "toggle"), Some(Tag::Byte(i8::from(!now))));
    Some(!now)
}
/// The value a configuration sets, within the card's range.
pub(in crate::game) fn set_value(stack: &mut ItemStack, upgrade: &Upgrade, value: i32) -> bool {
    if !has(stack, upgrade.name) {
        return false;
    }
    stack.data_mut().set_other(&id(upgrade.name, "value"), Some(Tag::Int(value.clamp(upgrade.min, upgrade.max))));
    true
}
/// The installed cards, in the piece's order (`getAppliedUpgrades`).
pub(in crate::game) fn applied(stack: &ItemStack) -> Vec<&'static Upgrade> {
    piece(&stack.item).map_or_else(Vec::new, |p| p.upgrades.iter().filter(|u| has(stack, u)).filter_map(|u| upgrade(u)).collect())
}
/// `PoweredItem.isBarVisible`, `getBarWidth` and `getBarColor`.
pub(in crate::game) fn bar(stack: &ItemStack) -> Option<Option<(i32, u32)>> {
    let (current, max) = energy(stack)?;
    let filled = if max > 0.0 { current / max } else { 0.0 };
    Some((filled < 0.99).then(|| (((filled * 13.0) as i32).clamp(0, 13), 0x00ff00)))
}

/// What a tick of the worn armor asks of the server beside the player.
#[derive(Default)]
pub(in crate::game) struct Outcome {
    /// `CREATIVE_FLIGHT` from the chestplate.
    pub(in crate::game) flight: bool,
    /// The magnet's range around the player.
    pub(in crate::game) magnet: Option<(f64, Vec<ItemStack>, bool)>,
}

impl Game<'_> {
    /// A guest's tick with the armor (`inventoryTick` of each worn piece,
    /// `PlayerTickEvent.Pre`, `ItemAttributeModifierEvent`,
    /// `LivingBreatheEvent`), then what the server does around them: flight
    /// and the magnet's pickup.
    pub(in crate::game) fn lan_advancedae_armor_tick(&mut self, host: &mut LanHost, gi: usize) {
        let Some(g) = host.hub.guests.get(gi) else { return };
        let (conn, entity) = (g.conn, g.entity_id);
        let worn = g.armor.iter().flatten().any(|s| piece(&s.item).is_some());
        let granted = self.ae2x.advancedae.armor_flight.contains(&conn);
        if !worn {
            note_worn(entity, None);
        }
        if !worn && !granted && !self.ae2x.advancedae.armor_modifiers.contains(&conn) {
            return;
        }
        let outcome = self.as_guest(host, conn, |game| game.advancedae_armor_turn(entity)).flatten();
        let Some(outcome) = outcome else { return };
        if worn {
            self.ae2x.advancedae.armor_modifiers.insert(conn);
        } else {
            self.ae2x.advancedae.armor_modifiers.remove(&conn);
        }
        if outcome.flight != granted {
            let flying = outcome.flight && host.hub.guests[gi].flying;
            host.hub.set_flight(conn, outcome.flight, flying);
            if outcome.flight {
                self.ae2x.advancedae.armor_flight.insert(conn);
            } else {
                self.ae2x.advancedae.armor_flight.remove(&conn);
            }
        }
        if let Some((range, filter, blacklist)) = outcome.magnet {
            self.advancedae_magnet(host, gi, range, &filter, blacklist);
        }
    }

    /// The acting player's turn with the armor, `entity` theirs; none without
    /// a body.
    fn advancedae_armor_turn(&mut self, entity: i32) -> Option<Outcome> {
        let creative = self.creative;
        let spectator = self.spectator();
        let flying = self.player.as_ref()?.flying;
        self.advancedae_armor_hits(entity);
        let mut out = Outcome::default();
        for piece in PIECES {
            let Some(mut stack) = self.armor[piece.slot].clone().filter(|s| s.item.as_ref() == piece.item) else { continue };
            let before = stack.clone();
            // `tick`: the piece in its own slot, with a passive or buff card.
            if applied(&stack).iter().any(|u| u.apply != Apply::PassiveUse) {
                for upgrade in applied(&stack) {
                    let on = enabled(&stack, upgrade.name);
                    if upgrade.apply == Apply::Passive && on {
                        if self.advancedae_ability(upgrade, &stack, spectator, &mut out) {
                            consume(&mut stack, upgrade.cost, creative);
                        }
                    } else if upgrade.apply == Apply::Buff && on {
                        consume(&mut stack, upgrade.cost, creative);
                    }
                    if upgrade.name == "flight" && flying {
                        consume(&mut stack, upgrade.cost, creative);
                    }
                }
            }
            if piece.slot == 0 {
                self.advancedae_night_vision(&mut stack, creative);
                // `breath`: the player breathes, at the card's cost a tick.
                if active(&stack, "water_breathing") {
                    if let Some(p) = self.player.as_mut()
                        && p.air < 300
                    {
                        p.air = (p.air.max(0) + 4).min(300);
                    }
                    consume(&mut stack, 10, creative);
                }
            }
            if piece.slot == 1 && active(&stack, "flight") {
                out.flight = true;
            }
            if stack != before {
                self.armor[piece.slot] = Some(stack);
            }
        }
        self.advancedae_armor_attributes();
        note_worn(entity, Some(Worn::of(&self.armor, creative)));
        Some(out)
    }

    /// What the hits on the acting player (`entity`) took since the last
    /// time, from their pieces, and the fire Lava Immunity put out.
    fn advancedae_armor_hits(&mut self, entity: i32) {
        let (drawn, extinguish) = take_hits(entity);
        for (slot, ae) in drawn.into_iter().enumerate() {
            if ae > 0.0
                && let Some(stack) = self.armor[slot].as_mut().filter(|s| piece(&s.item).is_some())
            {
                extract(stack, ae);
            }
        }
        if extinguish && let Some(p) = self.player.as_mut() {
            p.set_remaining_fire_ticks(0);
        }
    }

    /// A passive card's `Ability`: whether it acted (and so draws its cost).
    fn advancedae_ability(&mut self, upgrade: &Upgrade, stack: &ItemStack, spectator: bool, out: &mut Outcome) -> bool {
        match upgrade.name {
            "regeneration" if active(stack, "regeneration") => {
                let Some(p) = self.player.as_mut() else { return false };
                let health = p.health.health();
                let max = p.attributes.cached_value_of("minecraft:max_health").unwrap_or(20.0) as f32;
                if health > 0.0 {
                    let _ = p.set_health((health + REGENERATION).min(max), false);
                }
                true
            }
            "magnet" if !spectator && active(stack, "magnet") => {
                let filter = match component(stack, &id("magnet", "filter")) {
                    Some(Tag::List(list)) => list.iter().filter_map(filter_stack).collect(),
                    _ => Vec::new(),
                };
                let blacklist = truth(component(stack, &id("magnet", "extra"))).unwrap_or(true);
                out.magnet = Some((f64::from(value(stack, "magnet", 5)), filter, blacklist));
                true
            }
            // The speed cards move the client's player, Auto Feed, Auto Stock
            // and ME Recharging need the grid link: see the README.
            _ => false,
        }
    }

    /// `AAEPlayerEvents.playerTick`'s night vision: while the card is on and
    /// powered the effect (210 ticks, no particles, no icon) is renewed below
    /// 210 ticks at the card's cost; the helmet takes off what it gave.
    fn advancedae_night_vision(&mut self, stack: &mut ItemStack, creative: bool) {
        let Some(def) = horizon_client::brewing::effect("minecraft:night_vision") else { return };
        let duration = self.potions.effects.get(def.id).map(|e| e.duration);
        if active(stack, "night_vision") && duration.is_none_or(|d| d < 210) {
            stack.data_mut().set_other(NIGHT_VISION_ACTIVATED, Some(Tag::Byte(1)));
            let mut effect = EffectInstance::new(def.id, 210, 0);
            effect.ambient = false;
            effect.visible = false;
            effect.show_icon = false;
            self.add_player_effect(effect);
            consume(stack, 10, creative);
            return;
        }
        if truth(component(stack, NIGHT_VISION_ACTIVATED)) == Some(true) && duration.is_some_and(|d| d < 210) {
            self.potions.effects.list.retain(|e| e.id != def.id);
            self.apply_effect_status();
            stack.data_mut().set_other(NIGHT_VISION_ACTIVATED, None);
        }
    }

    /// The attribute cards of the pieces worn in their slots, as transient
    /// modifiers of the cards' values; the others taken off.
    fn advancedae_armor_attributes(&mut self) {
        let wanted: Vec<Option<f64>> = ATTRIBUTES
            .iter()
            .map(|(card, _, _)| {
                PIECES.iter().find_map(|piece| {
                    let stack = self.armor[piece.slot].as_ref().filter(|s| s.item.as_ref() == piece.item)?;
                    (piece.upgrades.contains(card) && active(stack, card)).then(|| f64::from(value(stack, card, 0)))
                })
            })
            .collect();
        let Some(p) = self.player.as_mut() else { return };
        for ((_, attribute, modifier), amount) in ATTRIBUTES.iter().zip(wanted) {
            let Some(key) = p.attributes.key(attribute) else { continue };
            let current = p.attributes.modifier_value(Some(key.clone()), modifier).ok();
            let has = p.attributes.has_modifier(Some(key), modifier).unwrap_or(false);
            match amount {
                Some(amount) if !has || current != Some(amount) => {
                    let _ = p.set_modifier(attribute, modifier, amount, "add_value", true);
                }
                None if has => {
                    let _ = p.set_modifier(attribute, modifier, 0.0, "add_value", false);
                }
                _ => {}
            }
        }
    }

    /// `UpgradeCards.magnet`: the item entities in reach of the player
    /// (`MagnetHelpers.getBoundingBox`, `validEntities`) that the filter
    /// allows are touched (`playerTouch`).
    fn advancedae_magnet(&mut self, host: &mut LanHost, gi: usize, range: f64, filter: &[ItemStack], blacklist: bool) {
        let g = &host.hub.guests[gi];
        let [x, y, z] = g.position;
        let (min, max) = ([x - range, y - range, z - range], [x + range, y + range, z + range]);
        let uuid = horizon_client::lan::playerdata::uuid_ints(g.uuid);
        let items: Vec<u32> = self
            .dropped
            .iter()
            .filter(|d| {
                let b = d.bounds();
                let (lo, hi) = (b.min_position(), b.max_position());
                lo.x < max[0] && hi.x > min[0] && lo.y < max[1] && hi.y > min[1] && lo.z < max[2] && hi.z > min[2]
            })
            .filter(|d| d.pickup_delay == 0 && d.target.is_none_or(|t| t == uuid) && d.stack.count > 0)
            .filter(|d| {
                if filter.is_empty() {
                    return true;
                }
                let listed = filter.iter().any(|f| f.item == d.stack.item);
                listed != blacklist
            })
            .map(|d| d.id)
            .collect();
        if !items.is_empty() {
            self.guest_touch_items(host, gi, items);
        }
    }

    /// A helmet card's hotkey for guest `conn` (`toggleUpgrade` with the
    /// player): the card's name and ON or OFF, or that it is not installed,
    /// in the action bar. False when the guest wears no Quantum Helmet.
    pub(in crate::game) fn lan_advancedae_armor_hotkey(&mut self, host: &mut LanHost, conn: u32, id: &str) -> bool {
        let Some(&(_, name)) = TOGGLE_HOTKEYS.iter().find(|(hotkey, _)| *hotkey == id) else { return false };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn && !g.spectator()) else { return false };
        let Some(helmet) = host.hub.guests[gi].armor[0].as_mut().filter(|s| s.item.as_ref() == PIECES[0].item) else { return false };
        let Some(card) = upgrade(name) else { return false };
        let card_name = Text::translate(&format!("item.{}", card.card.replace(':', ".")), &[]);
        let text = match toggle(helmet, name) {
            Some(on) => card_name.append(Text::literal(if on { " ON" } else { " OFF" }).with_color(if on { GREEN } else { RED })),
            None => Text::translate("gui.advanced_ae.UpgradeNotInstalledMessage", &[card_name]),
        };
        host.hub.send_to(conn, &Clientbound::SystemChat(SystemChat { content: horizon_client::lan::data::component_nbt(&text.component()), overlay: true }));
        true
    }

    /// The configuration in place of `QuantumArmorConfigScreen`
    /// (`/ae2 advae armor <slot> ...`): the acting player's piece in armor
    /// slot `slot` (0 head .. 3 feet).
    pub(in crate::game) fn advancedae_armor_command(&mut self, words: &[&str]) -> Result<serde_json::Value, String> {
        let [slot, rest @ ..] = words else { return Err("armor <slot> [install|remove|toggle|value ...]".into()) };
        let slot: usize = slot.parse().ok().filter(|s| *s < 4).ok_or("bad armor slot")?;
        // The hits since the last tick first, so the AE shown is the pieces' now.
        let entity = self.lan_acting_entity.map(|e| e as i32);
        if let Some(entity) = entity {
            self.advancedae_armor_hits(entity);
        }
        let mut stack = self.armor[slot].clone().filter(|s| piece(&s.item).is_some()).ok_or("no Quantum Armor in that slot")?;
        let named = |name: &str| upgrade(name).ok_or_else(|| format!("no upgrade {name}"));
        match rest {
            [] => {}
            // `QuantumArmorMenuHost.tick`: the card in the input goes in.
            ["install", card] => {
                let upgrade = card_upgrade(card).filter(|u| u.name != "empty").ok_or("not an upgrade card")?;
                let from = self.inventory.iter().position(|s| s.as_ref().is_some_and(|s| s.item.as_ref() == *card && s.count > 0)).ok_or("no such card in the inventory")?;
                if !apply(&mut stack, upgrade) {
                    return Err("the piece does not take that card or has it".into());
                }
                let taken = self.inventory[from].as_mut().expect("found above");
                taken.count -= 1;
                if taken.count == 0 {
                    self.inventory[from] = None;
                }
            }
            // `QuantumArmorConfigMenu.requestUninstall`: the card back.
            ["remove", name] => {
                let upgrade = named(name)?;
                if !remove(&mut stack, upgrade.name) {
                    return Err("not installed".into());
                }
                let card = ItemStack::new(upgrade.card, 1);
                if let Some(free) = self.inventory.iter().take(36).position(Option::is_none) {
                    self.inventory[free] = Some(card);
                }
            }
            ["toggle", name] => {
                toggle(&mut stack, named(name)?.name).ok_or("not installed")?;
            }
            ["value", name, amount] => {
                let amount: i32 = amount.parse().map_err(|_| "bad value")?;
                if !set_value(&mut stack, named(name)?, amount) {
                    return Err("not installed".into());
                }
            }
            _ => return Err("armor <slot> [install <card>|remove <upgrade>|toggle <upgrade>|value <upgrade> <n>]".into()),
        }
        let (stored, capacity) = energy(&stack).unwrap_or_default();
        let upgrades: Vec<serde_json::Value> = applied(&stack)
            .iter()
            .map(|u| serde_json::json!({"upgrade": u.name, "on": enabled(&stack, u.name), "value": value(&stack, u.name, 0), "powered": powered(&stack, u)}))
            .collect();
        let item = stack.item.to_string();
        self.armor[slot] = Some(stack);
        if let Some(entity) = entity {
            note_worn(entity, Some(Worn::of(&self.armor, self.creative)));
        }
        Ok(serde_json::json!({"item": item, "stored": stored, "capacity": capacity, "upgrades": upgrades}))
    }
}

/// A magnet filter entry (`GenericStack`) as the item it names.
fn filter_stack(tag: &Tag) -> Option<ItemStack> {
    let Tag::Compound(c) = tag else { return None };
    match c.get(&"id".into()) {
        Some(Tag::String(id)) => Some(ItemStack::new(id.to_string().ok()?.as_str(), 1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn charged(item: &str, ae: f64) -> ItemStack {
        let mut stack = ItemStack::new(item, 1);
        set_stored(&mut stack, ae);
        stack
    }

    #[test]
    fn each_piece_takes_its_own_cards_at_their_most() {
        let mut helmet = charged(PIECES[0].item, 0.0);
        assert!(apply(&mut helmet, upgrade("magnet").unwrap()));
        assert!(!apply(&mut helmet, upgrade("magnet").unwrap()), "already installed");
        assert!(!apply(&mut helmet, upgrade("flight").unwrap()), "a chestplate card");
        assert!(has(&helmet, "magnet") && enabled(&helmet, "magnet"));
        assert_eq!(value(&helmet, "magnet", 5), 12);
        assert_eq!(component(&helmet, "advanced_ae:magnet_extra"), Some(&Tag::Byte(1)));
        assert_eq!(toggle(&mut helmet, "magnet"), Some(false));
        assert!(!enabled(&helmet, "magnet") && has(&helmet, "magnet"));
        assert_eq!(toggle(&mut helmet, "luck"), None, "not installed");
        assert!(set_value(&mut helmet, upgrade("magnet").unwrap(), 1));
        assert_eq!(value(&helmet, "magnet", 5), 3, "clamped to the range");
        assert!(remove(&mut helmet, "magnet"));
        assert!(!has(&helmet, "magnet"));
        assert_eq!(applied(&helmet).len(), 0);
        assert_eq!(card_upgrade("advanced_ae:auto_feeding_card").unwrap().name, "auto_feed");
    }

    #[test]
    fn the_buffer_is_ae2s_energy_of_the_pieces_capacity() {
        let mut chest = charged(PIECES[1].item, 0.0);
        assert_eq!(energy(&chest), Some((0.0, 300_000_000.0)));
        assert_eq!(inject(&mut chest, 400_000_000.0), 300_000_000.0);
        assert_eq!(extract(&mut chest, 100.0), 100.0);
        assert_eq!(energy(&chest).unwrap().0, 299_999_900.0);
        assert_eq!(extract(&mut chest, 1e9), 299_999_900.0);
        assert!(component(&chest, STORED).is_none(), "empty removes the component");
        assert_eq!(bar(&chest), Some(Some((0, 0x00ff00))));
        inject(&mut chest, 300_000_000.0);
        assert_eq!(bar(&chest), Some(None), "full hides the bar");
        assert!(energy(&ItemStack::new("minecraft:diamond_helmet", 1)).is_none());
    }

    #[test]
    fn a_card_is_powered_by_twice_its_cost_in_fe() {
        let flight = upgrade("flight").unwrap();
        let mut chest = charged(PIECES[1].item, 4.9);
        apply(&mut chest, flight);
        assert!(!powered(&chest, flight), "9 FE < 10");
        set_stored(&mut chest, 5.0);
        assert!(powered(&chest, flight) && active(&chest, "flight"));
        toggle(&mut chest, "flight");
        assert!(!active(&chest, "flight"));
    }

    fn wear(entity: i32, armor: [Option<ItemStack>; 4], creative: bool) {
        note_worn(entity, Some(Worn::of(&armor, creative)));
    }

    #[test]
    fn a_hit_goes_30_percent_into_the_pieces_ae_and_a_fall_into_charged_boots() {
        let e = 1_000_001;
        wear(e, [Some(charged(PIECES[0].item, 1e6)), Some(charged(PIECES[1].item, 1e6)), None, Some(charged(PIECES[3].item, 1e6))], false);
        let mut hit = 4.0;
        assert!(!player_hurt(e, "minecraft:generic", &mut hit));
        assert!((hit - 3.1).abs() < 1e-5, "three pieces took 0.3 each: {hit}");
        assert_eq!(take_hits(e), ([300.0, 300.0, 0.0, 300.0], false));
        let mut small = 1.2;
        assert!(player_hurt(e, "minecraft:generic", &mut small), "0.93 left is no hit");
        assert_eq!(take_hits(e).0, [90.0, 90.0, 0.0, 90.0]);
        let mut fall = 7.0;
        assert!(player_hurt(e, "minecraft:fall", &mut fall), "charged boots: no fall damage");
        assert_eq!(take_hits(e).0, [0.0; 4], "the fall draws nothing");
        let mut other = 4.0;
        assert!(!player_hurt(e + 1, "minecraft:generic", &mut other));
        assert_eq!(other, 4.0, "a player wearing nothing noted");
        // Empty boots: the fall hurts, and the others absorb it.
        wear(e, [Some(charged(PIECES[0].item, 1e6)), None, None, Some(charged(PIECES[3].item, 0.0))], false);
        let mut fall = 8.0;
        assert!(!player_hurt(e, "minecraft:fall", &mut fall));
        assert_eq!(take_hits(e).0, [600.0, 0.0, 0.0, 0.0]);
        note_worn(e, None);
        assert_eq!(take_hits(e), ([0.0; 4], false));
    }

    #[test]
    fn lava_immunity_and_evasion_call_the_hit_off_at_their_cost() {
        let e = 1_000_002;
        let mut chest = charged(PIECES[1].item, 1000.0);
        apply(&mut chest, upgrade("lava_immunity").unwrap());
        wear(e, [None, Some(chest.clone()), None, None], false);
        for kind in ["minecraft:lava", "minecraft:in_fire", "minecraft:on_fire"] {
            let mut lava = 4.0;
            assert!(player_hurt(e, kind, &mut lava), "{kind}");
            assert_eq!(take_hits(e), ([0.0, 10.0, 0.0, 0.0], true), "{kind}: the card's cost, the fire out");
        }
        let mut hit = 4.0;
        assert!(!player_hurt(e, "minecraft:generic", &mut hit), "not a fire");
        assert_eq!(take_hits(e), ([0.0, 300.0, 0.0, 0.0], false));
        wear(e, [None, Some(chest.clone()), None, None], true);
        let mut lava = 4.0;
        assert!(player_hurt(e, "minecraft:lava", &mut lava));
        assert_eq!(take_hits(e).0, [0.0; 4], "nothing drawn in creative");
        toggle(&mut chest, "lava_immunity");
        wear(e, [None, Some(chest), None, None], false);
        let mut lava = 4.0;
        assert!(!player_hurt(e, "minecraft:lava", &mut lava), "the card off");
        let mut boots = charged(PIECES[3].item, 1e9);
        apply(&mut boots, upgrade("evasion").unwrap());
        wear(e, [None, None, None, Some(boots)], false);
        let evaded = (0..1000)
            .filter(|_| {
                let mut hit = 2.0;
                player_hurt(e, "minecraft:generic", &mut hit)
            })
            .count();
        assert!((200..400).contains(&evaded), "30% of 1000: {evaded}");
        assert_eq!(take_hits(e).0[3], evaded as f64 * 10.0 + (1000 - evaded) as f64 * 150.0, "an evaded hit costs 10, a taken one gives 150");
        note_worn(e, None);
    }
}
