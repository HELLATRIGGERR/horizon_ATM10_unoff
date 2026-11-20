// Mekanism's frequency list (`FrequencyManager`, `FrequencyManagerWrapper`, `SecurityFrequency`'s trusted
// players): every frequency of every kind, saved with the world in `mekanism/frequencies.json` beside its
// `level.dat` (Mekanism's `mekanism_frequencies` saved data), so a frequency outlives the blocks that use it. QIO uses
// it (`mek_qio`); the Teleporter and the Quantum Entangloporter take the same list with their own kind. The rules
// are `mekanism_energy::frequency`.
use super::{Game, mek_world::NativeServerPlayer};
use mekanism_energy::frequency::{Frequencies, Identity, Security};
use serde_json::{Value, json};
use std::cell::RefCell;

#[derive(Default)]
struct Registry {
    freqs: Frequencies,
    loaded: bool,
    saved: u64,
    /// Frequencies deleted since the world opened: their holders drop them (`Frequency.isRemoved`).
    removed: Vec<(String, Identity)>,
}

thread_local! {
    static REG: RefCell<Registry> = RefCell::new(Registry::default());
}

/// The world closed.
pub(super) fn clear() {
    REG.with(|r| *r.borrow_mut() = Registry::default());
}

fn path(game: &Game<'_>) -> Option<std::path::PathBuf> {
    Some(game.native_world_dir()?.join("mekanism").join("frequencies.json"))
}

/// The list, read from the world the first time; written back when `f` changed it.
pub(super) fn with<R>(game: &mut Game<'_>, f: impl FnOnce(&mut Frequencies) -> R) -> R {
    let file = path(game);
    let (out, write) = REG.with(|r| {
        let mut r = r.borrow_mut();
        if !r.loaded {
            r.loaded = true;
            if let Some(v) = file.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
                r.freqs = Frequencies::from_json(&v);
            }
            r.saved = r.freqs.revision;
        }
        let before: Vec<(String, Identity)> = r.freqs.list.iter().map(|q| (q.kind.clone(), q.identity())).collect();
        let out = f(&mut r.freqs);
        let changed = r.freqs.revision != r.saved;
        if changed {
            r.saved = r.freqs.revision;
            let gone: Vec<(String, Identity)> =
                before.into_iter().filter(|(k, id)| r.freqs.find(k, id).is_none()).collect();
            r.removed.extend(gone);
        }
        (out, changed.then(|| r.freqs.to_json()))
    });
    if let (Some(v), Some(p)) = (write, file) {
        let tmp = p.with_extension("json.tmp");
        let ok = p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok())
            && std::fs::write(&tmp, serde_json::to_string_pretty(&v).unwrap_or_default()).is_ok()
            && std::fs::rename(&tmp, &p).is_ok();
        if !ok {
            log::warn!("[mekanism] the frequencies weren't saved to {}", p.display());
        }
    }
    out
}

/// The list as it is (loaded by a `with` before).
pub(super) fn read<R>(f: impl FnOnce(&Frequencies) -> R) -> R {
    REG.with(|r| f(&r.borrow().freqs))
}

/// Whether a holder's frequency was deleted since the world opened (it drops it).
pub(super) fn was_removed(kind: &str, id: &Identity) -> bool {
    REG.with(|r| r.borrow().removed.iter().any(|(k, i)| k == kind && i.same(id)) && r.borrow().freqs.find(kind, id).is_none())
}

/// A holder's frequency at its tick (`FrequencyManager.validateAndUpdate`): false once deleted (the holder drops
/// it); one the list lacks (a world from before the list) is put back on it.
pub(super) fn validate(game: &mut Game<'_>, kind: &str, id: &Identity) -> bool {
    if was_removed(kind, id) {
        return false;
    }
    let known = REG.with(|r| {
        let r = r.borrow();
        r.loaded && r.freqs.find(kind, id).is_some()
    });
    if !known {
        let name = player_name(game, id.owner);
        with(game, |f| f.restore(kind, id, &name));
    }
    true
}

/// The name of a player the server knows; empty when it knows none.
pub(super) fn player_name(game: &Game<'_>, id: u128) -> String {
    game.native_profile_by_uuid(id).unwrap_or_default()
}

/// A player by name: one the server knows, else the development environment's fake player of that name (as
/// FTB Teams' `/ftbteams_add_fake_player`), whose id is made from the name.
pub(super) fn player_by_name(game: &Game<'_>, name: &str) -> (u128, String) {
    if let Some(p) = game.native_profile_by_name(name) {
        return p;
    }
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    for b in format!("OfflinePlayer:{name}").bytes() {
        h ^= u128::from(b);
        h = h.wrapping_mul(0x0000000001000000000000000000013B);
    }
    (h, name.to_owned())
}

/// `/mek freq list <kind> <public|private|trusted> [as <player>]`: the frequencies a player's selector lists;
/// `/mek freq trust <player>` and `/mek freq untrust <player>`: the acting player's trusted list (the Security
/// Desk's).
pub(super) fn command(game: &mut Game<'_>, player: &NativeServerPlayer, words: &[&str]) -> Value {
    match words {
        ["list", kind, mode, rest @ ..] => {
            let Some(security) = Security::parse(mode) else { return json!({ "error": "mode: public, private or trusted" }) };
            let who = match rest {
                ["as", name] => player_by_name(game, name).0,
                _ => player.uuid,
            };
            with(game, |_| ());
            read(|f| {
                let list: Vec<Value> = f
                    .visible(kind, security, who)
                    .iter()
                    .map(|q| json!({ "name": q.name, "owner": q.owner_name, "security": q.security.name(), "color": q.color }))
                    .collect();
                json!({ "kind": kind, "security": security.name(), "frequencies": list })
            })
        }
        [verb @ ("trust" | "untrust"), name] => {
            let (id, name) = player_by_name(game, name);
            let owner = player.uuid;
            let done = with(game, |f| if *verb == "trust" { f.trust(owner, id, &name) } else { f.untrust(owner, id) });
            json!({ "verb": *verb, "player": name, "done": done })
        }
        _ => json!({ "error": "usage: /mek freq list <kind> <mode> [as <player>] | trust <player> | untrust <player>" }),
    }
}
