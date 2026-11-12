//! `Game` as the host of the AE2 crates above horizon-client (`ae2_ext::Ae2Host`): the one place
//! they reach the game.
use super::{Game, ae2_ext::Ae2Host};

impl Ae2Host for Game<'_> {}
