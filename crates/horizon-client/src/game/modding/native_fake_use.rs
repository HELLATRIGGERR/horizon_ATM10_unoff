//! A machine's fake player (NeoForge's `FakePlayer` as a mod's machine holds one: Just Dire Things'
//! Clicker). `native_fake_use` gives a fake player its own UUID, stands it at the machine's face
//! looking the machine's way with the machine's stack in its main hand, and has it click as a
//! player does, through the same steps and hooks (`ServerPlayerGameMode.useItemOn`'s order in
//! `items/use_hand.rs`, the guest's packet handlers in `network/lan_actions`, swapped in as a
//! guest's turn), on the server: a right click (`Use`), a left click (`Attack`: an entity
//! attacked, a block hit as `START_DESTROY_BLOCK` hits it and dug), and a held use (`Hold`: one
//! tick of the use each call, then `releaseUsing`). Upstream's scoped fake action
//! (`native_server.rs`, `scoped_fake_turn`) keeps the fake's own state aside for the click, so
//! the stack the click left (a filled bucket, a worn sword) and what consumed it come back to the
//! machine. A use held over several calls keeps its per-action state here, by machine.
use super::Game;
use super::lan_actions::GuestPlay;
use crate::game::api_values::TryIntoApi;
use horizon_client::items::ItemStack;
use mod_api::{ActionOutcome, ActionRefused, Hand, PlayerAction, PlayerActionSetup};
use std::collections::HashMap;

/// What a machine's fake player does with the stack in its hand.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub(in crate::game) enum FakeClick {
    /// A right click: on a block, the block's use and the item's `useOn`
    /// (`ServerPlayerGameMode.useItemOn`), then, when that consumed nothing and the hand held
    /// something, the item's own use (`useItem`), as Click Machine's `clickBlockInDirection`
    /// does; in the air, the item's use; on an entity, `interactOn`.
    Use,
    /// A left click: an entity is attacked (`Player.attack`); a block is hit as
    /// `handleBlockBreakAction`'s `START_DESTROY_BLOCK` hits it (the left-click hook, the
    /// block's `attack`), then dug: it breaks once `dug` ticks of digging since that hit finish
    /// it (`incrementDestroyProgress`), else its crack shows. A fake that doesn't tick passes 0,
    /// so only what one hit breaks breaks.
    Attack {
        /// Ticks of digging since the first hit.
        dug: u32,
    },
    /// The hand's use held (`startUsingItem` at the first call, `updateUsingItem` at each),
    /// let go (`releaseUsingItem`) once it has been held `max` ticks. A use the item finishes
    /// on its own (food) ends there.
    Hold {
        /// The ticks a use is held before it is let go.
        max: u32,
    },
}

/// What the fake player clicks.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub(in crate::game) enum FakeTarget {
    /// A block, as a `BlockHitResult`: its position, the face, the point and whether the ray
    /// started inside it.
    Block {
        pos: [i32; 3],
        face: [i32; 3],
        point: [f64; 3],
        inside: bool,
    },
    /// Nothing in reach.
    Air,
    /// An entity of the level, hit at `location` relative to its origin.
    Entity { id: u32, location: [f64; 3] },
}

/// One click of a machine's fake player.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(in crate::game) struct FakeUse<'a> {
    /// The fake's own UUID and name (a valid player name: 1 to 16 characters).
    pub(in crate::game) uuid: u128,
    pub(in crate::game) name: &'a str,
    /// The machine, which holds a use held over several calls.
    pub(in crate::game) machine: [i32; 3],
    /// The eye, in world coordinates, and the look (vanilla's yaw and pitch, degrees).
    pub(in crate::game) eye: [f64; 3],
    pub(in crate::game) yaw: f32,
    pub(in crate::game) pitch: f32,
    /// Whether the fake sneaks (secondary use).
    pub(in crate::game) sneaking: bool,
    /// Whether its attacks land at full strength, as a player who waited
    /// (Click Machine's fake answers 1 for `getAttackStrengthScale`); else as one who didn't.
    pub(in crate::game) charged: bool,
    /// The stack in its main hand.
    pub(in crate::game) stack: Option<ItemStack>,
    pub(in crate::game) target: FakeTarget,
    pub(in crate::game) click: FakeClick,
}

/// What a fake player's click did.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub(in crate::game) struct FakeUsed {
    /// Consumed, passed or an effect stopped (a menu), or refused before anything ran (no
    /// server to act on, out of reach...).
    pub(in crate::game) outcome: Result<ActionOutcome, ActionRefused>,
    /// The stack left in the main hand: the machine's again. A refused click gives it back as
    /// it came.
    pub(in crate::game) stack: Option<ItemStack>,
    /// Other stacks the click left with the fake (a filled bucket from a stack of empty ones):
    /// Java's `cleanupFakePlayerFromUse` drops them.
    pub(in crate::game) other: Vec<ItemStack>,
    /// Whether a held use goes on (`isUsingItem`).
    pub(in crate::game) holding: bool,
}

/// The per-action state of each use held, by level and machine.
#[derive(Default)]
pub(in crate::game) struct FakeHolds(HashMap<(String, [i32; 3]), GuestPlay>);

// Native bundles use these; nothing in Horizon does.
#[allow(dead_code)]
impl Game<'_> {
    /// One click of a machine's fake player, on a server's level in place: see the module.
    /// Refused (the stack untouched) where no server runs this level (in-frame play), when the
    /// name is not a player's, or while another player's turn runs.
    pub(in crate::game) fn native_fake_use(&mut self, click: FakeUse<'_>) -> FakeUsed {
        let refused = |why, stack: Option<ItemStack>| FakeUsed {
            outcome: Err(why),
            stack,
            other: Vec::new(),
            holding: false,
        };
        let key = (self.dim.id().to_owned(), click.machine);
        let held = self.fake_holds.0.remove(&key);
        let Some(level) = self.level(self.dim.id()) else {
            return refused(ActionRefused::Stale, click.stack);
        };
        let Some(player) = self.fake_player(level, click.uuid, click.name) else {
            return refused(ActionRefused::Stale, click.stack);
        };
        let probe = PlayerAction::UseItem { hand: Hand::Main };
        let (gi, conn) = match self.fake_action_actor(player, probe) {
            Ok(found) => found,
            Err(why) => return refused(why, click.stack),
        };
        let Ok(stack) = click.stack.try_into_api() else {
            return refused(ActionRefused::InvalidSetup, click.stack);
        };
        let setup =
            PlayerActionSetup::new(stack, click.eye, click.yaw, click.pitch, click.sneaking);
        // A use held goes on from where the last call left it; any other click lets it go.
        let play = match click.click {
            FakeClick::Hold { .. } => held.unwrap_or_default(),
            _ => GuestPlay::default(),
        };
        let mut holding = false;
        let had_item = click.stack.is_some();
        let turn = self.scoped_fake_turn(gi, conn, setup, play, |game| {
            game.fake_acting(|game| match (click.click, click.target) {
                (
                    FakeClick::Use,
                    FakeTarget::Block {
                        pos,
                        face,
                        point,
                        inside,
                    },
                ) => {
                    let hit = super::Hit {
                        pos,
                        face,
                        point,
                        inside,
                    };
                    if game.guest_use_item_on(conn, gi, hit, Hand::Main)? {
                        return Ok(true);
                    }
                    if !had_item {
                        return Ok(false);
                    }
                    game.use_failed = false;
                    game.guest_use_item(conn, gi, Hand::Main, None)
                }
                (FakeClick::Use, FakeTarget::Air) => {
                    game.guest_use_item(conn, gi, Hand::Main, None)
                }
                (FakeClick::Use, FakeTarget::Entity { id, location }) => {
                    let id = i32::try_from(id).map_err(|_| ActionRefused::InvalidTarget)?;
                    game.guest_interact(conn, gi, id, Hand::Main, location)
                }
                (FakeClick::Attack { .. }, FakeTarget::Entity { id, .. }) => {
                    let id = i32::try_from(id).map_err(|_| ActionRefused::InvalidTarget)?;
                    if click.charged {
                        // The attack strength's clock, as a player who waited.
                        game.as_guest_at(gi, conn, |g| g.attack_ticks = 100)
                            .map_err(|_| ActionRefused::Stale)?;
                    }
                    game.guest_attack(conn, gi, id)
                }
                (FakeClick::Attack { dug }, FakeTarget::Block { pos, face, .. }) => {
                    game.fake_dig(gi, conn, pos, face, dug)
                }
                (FakeClick::Attack { .. }, FakeTarget::Air) => Ok(false),
                (FakeClick::Hold { max }, _) => {
                    let (used, going_on) = game.fake_hold(gi, conn, max)?;
                    holding = going_on;
                    Ok(used)
                }
            })
        });
        match turn {
            Ok(turn) => {
                if holding {
                    self.fake_holds.0.insert(key, turn.play);
                }
                FakeUsed {
                    outcome: Ok(turn.answer),
                    stack: turn.held,
                    other: turn.other,
                    holding,
                }
            }
            Err(why) => refused(why, click.stack),
        }
    }

    /// `handleBlockBreakAction`'s `START_DESTROY_BLOCK` by the fake at `gi`, `dug` ticks of
    /// digging on: reach, the world's height and border, the left-click hook, `blockActionRestricted`,
    /// the block's `attack`, then the break (`destroyBlock` through `guest_break_block`, with
    /// spawn protection) once the progress is whole, else the crack. True when the hit landed on
    /// a block (Click Machine's left click answers SUCCESS then).
    fn fake_dig(
        &mut self,
        gi: usize,
        conn: u32,
        pos: [i32; 3],
        face: [i32; 3],
        dug: u32,
    ) -> Result<bool, ActionRefused> {
        let host = self.lan.as_mut().ok_or(ActionRefused::Stale)?;
        if !host.hub.guests.update(gi, |g| g.may_reach_block(pos)) {
            return Err(ActionRefused::OutOfReach);
        }
        let (min_y, max_y) = (self.world.shape.min_y, self.world.shape.max_y() - 1);
        if !(min_y..=max_y).contains(&pos[1])
            || !self.border.contains(f64::from(pos[0]), f64::from(pos[2]))
        {
            return Err(ActionRefused::Protected);
        }
        let per_tick = self
            .as_guest_at(gi, conn, |g| {
                let state = g.world.block(pos[0], pos[1], pos[2]);
                let mut canceled = false;
                g.native_left_click_block(pos, face, state, &mut canceled);
                if canceled || g.block_action_restricted(pos) || g.res.facts().is_air(state) {
                    return None;
                }
                let _ = g.attack_block(pos);
                Some(g.destroy_progress(pos))
            })
            .map_err(|_| ActionRefused::Stale)?;
        let Some(per_tick) = per_tick else {
            return Ok(false);
        };
        let progress = per_tick * (dug as f32 + 1.0);
        if progress >= 1.0 {
            self.guest_break_block(conn, gi, pos)?;
            return Ok(true);
        }
        let id = self.lan.as_ref().ok_or(ActionRefused::Stale)?.hub.guests[gi].entity_id;
        self.destroy_block_progress(id, pos, (progress * 10.0) as i32);
        Ok(true)
    }

    /// Click Machine's hold: the use of the main hand started if none runs, one tick of it, and
    /// `releaseUsingItem` once held `max` ticks. Answers whether the click was used and whether
    /// the use goes on. An empty hand stops the use.
    fn fake_hold(&mut self, gi: usize, conn: u32, max: u32) -> Result<(bool, bool), ActionRefused> {
        self.as_guest_at(gi, conn, |g| {
            g.with_hand(Hand::Main, |g| {
                if g.held().is_none() {
                    g.stop_active_item_use();
                    return (false, false);
                }
                if g.using.is_none() && !g.start_using_item() {
                    return (false, false);
                }
                // The use is held: `updatingUsingItem` lets go of a use whose key is up.
                g.right = true;
                g.tick_used_item();
                if g.using.is_some_and(|(_, ticks)| ticks >= max) {
                    g.release_using_item();
                }
                (true, g.using.is_some())
            })
        })
        .map_err(|_| ActionRefused::Stale)
    }
}
