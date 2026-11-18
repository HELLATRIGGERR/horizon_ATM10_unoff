//! The fork's AE2 code on upstream's LAN host model.
//!
//! Upstream keeps the [`LanHost`] in `Game::lan` while a guest's packet is handled; its helpers
//! take no host. The fork's AE2 code was written with the host out of `self.lan` and passed down
//! as `host: &mut LanHost`, beside `&mut self`. [`HostTurn`] keeps that borrow shape:
//!
//! - an AE2 function takes `host: &mut HostTurn` (or `&HostTurn`) where it took `&mut LanHost`;
//!   `host.hub`, `host.play` ... reach the host through `Deref`, and it passes to a function that
//!   still takes `&LanHost` / `&mut LanHost` by deref coercion;
//! - upstream code enters AE2 code with [`Game::with_lan_host`], which takes the host out of
//!   `self.lan` for the call and puts it back after;
//! - AE2 code calls an upstream helper that reads `self.lan` (and takes no host) through
//!   [`HostTurn::back`], which puts the host back into `self.lan` for that call.
use super::{Game, lan::LanHost};

/// A LAN host taken out of `Game::lan` for the fork's AE2 code, which can lend it back.
pub(in crate::game) struct HostTurn(Option<LanHost>);

impl HostTurn {
    /// `host`, taken out of `Game::lan` by the caller, for AE2 code.
    pub(in crate::game) fn lend(host: LanHost) -> Self {
        Self(Some(host))
    }

    /// The host again, for the caller to put back. None when an upstream helper run through
    /// [`Self::back`] closed the hub.
    pub(in crate::game) fn into_inner(self) -> Option<LanHost> {
        self.0
    }

    /// Run the upstream helper `f`, which reads `game.lan`, with this host back in place.
    pub(in crate::game) fn back<'g, R>(
        &mut self,
        game: &mut Game<'g>,
        f: impl FnOnce(&mut Game<'g>) -> R,
    ) -> R {
        std::mem::swap(&mut game.lan, &mut self.0);
        let result = f(game);
        std::mem::swap(&mut game.lan, &mut self.0);
        result
    }
}

impl std::ops::Deref for HostTurn {
    type Target = LanHost;
    fn deref(&self) -> &LanHost {
        self.0.as_ref().expect("LAN host closed during an AE2 turn")
    }
}

impl std::ops::DerefMut for HostTurn {
    fn deref_mut(&mut self) -> &mut LanHost {
        self.0.as_mut().expect("LAN host closed during an AE2 turn")
    }
}

#[allow(dead_code, reason = "only AE2 code enters through it")]
impl Game<'_> {
    /// Run the AE2 code `f` with the host out of `self.lan` (the fork's model); None when no
    /// LAN host is open.
    pub(in crate::game) fn with_lan_host<R>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut HostTurn) -> R,
    ) -> Option<R> {
        let mut turn = HostTurn::lend(self.lan.take()?);
        let result = f(self, &mut turn);
        // An upstream helper run through `back` may have closed the hub; a host the AE2 code
        // opened meanwhile (none does) would be dropped here.
        if self.lan.is_none() {
            self.lan = turn.into_inner();
        }
        Some(result)
    }
}
