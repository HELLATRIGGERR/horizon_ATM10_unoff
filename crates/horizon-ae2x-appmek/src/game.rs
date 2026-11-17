//! Applied Mekanistics 1.6.3 in the game: Mekanism's chemicals moved by AE2's storage, import and
//! export buses (`ChemicalExternalStorageStrategy`, `ChemicalImportStrategy`,
//! `ChemicalExportStrategy` over `ChemicalHandlerFacade`: AE2's fluid strategies with
//! `IChemicalHandler`), derived from Applied Mekanistics by ramidzkh, LGPL-3.0 (see
//! `ae2/addons/appmek/`). The chemical handler is Mekanism's, reached through the host
//! (`Ae2Host::chemical_tanks`, `chemical_insert`, `chemical_extract`: hook points API 435); without
//! Mekanism nothing answers, and nothing moves.
use crate::defs::{chemical_key, key_type};
use ae2_cell_core::ItemKey;
use horizon_client::game::ae2_ext::{Ae2Addon, Ae2Host, Ae2KeyStrategies, GridAddress};

/// The addon in one game: it keeps nothing.
struct AppMek;

pub(crate) fn make() -> Box<dyn Ae2Addon> {
    Box::new(AppMek)
}

impl Ae2Addon for AppMek {
    fn id(&self) -> &'static str {
        crate::defs::NAMESPACE
    }
    fn key_strategies(&self) -> Option<&dyn Ae2KeyStrategies> {
        Some(self)
    }
}

/// A chemical key's chemical: its id, with no components.
fn chemical(key: &ItemKey) -> Option<&str> {
    (key.components.is_empty() && key.item_id.contains(':')).then_some(key.item_id.as_str())
}

/// What the block beside `at` on `face` holds that can be taken out, by chemical, in the order
/// of its tanks (`ChemicalHandlerFacade.getAvailableStacks`, extractable only); none without a
/// chemical handler there.
fn extractable(host: &dyn Ae2Host, at: GridAddress, face: u8) -> Option<Vec<(ItemKey, u64)>> {
    let tanks = host.chemical_tanks(at, face)?;
    let mut out: Vec<(ItemKey, u64)> = Vec::new();
    for tank in tanks.into_iter().filter(|tank| tank.extractable && tank.amount > 0 && !tank.chemical.is_empty()) {
        let key = chemical_key(&tank.chemical);
        match out.iter_mut().find(|(known, _)| *known == key) {
            Some((_, amount)) => *amount = amount.saturating_add(tank.amount as u64),
            None => out.push((key, tank.amount as u64)),
        }
    }
    Some(out)
}

impl Ae2KeyStrategies for AppMek {
    fn import_offered(&self, host: &dyn Ae2Host, at: GridAddress, face: u8) -> Vec<(ItemKey, u64)> {
        extractable(host, at, face).unwrap_or_default()
    }

    /// `HandlerStrategy.transfer` (import): what the grid takes, simulated, then extracted for
    /// real and inserted; what the grid refused goes back.
    fn import(&self, host: &mut dyn Ae2Host, at: GridAddress, target: GridAddress, face: u8, key: &ItemKey, amount: u64) -> Result<u64, String> {
        let Some(id) = chemical(key) else { return Ok(0) };
        let kind = key_type();
        let room = host.grid_key_move(target, kind, key, amount, true, true)?;
        let to_take = host.chemical_extract(at, face, id, room, true);
        if to_take == 0 {
            return Ok(0);
        }
        let taken = host.chemical_extract(at, face, id, to_take, false);
        let added = host.grid_key_move(target, kind, key, taken, true, false)?;
        if added < taken {
            host.chemical_insert(at, face, id, taken - added, false);
        }
        Ok(added)
    }

    /// `HandlerStrategy.push` (export): what the block takes of what the grid has, simulated, then
    /// extracted from the grid and inserted; what the block refused goes back.
    fn export(&self, host: &mut dyn Ae2Host, at: GridAddress, source: GridAddress, face: u8, key: &ItemKey, amount: u64) -> Result<u64, String> {
        let Some(id) = chemical(key) else { return Ok(0) };
        if host.chemical_tanks(at, face).is_none() {
            return Ok(0);
        }
        let kind = key_type();
        let offered = host.grid_key_move(source, kind, key, amount, false, true)?;
        let to_add = host.chemical_insert(at, face, id, offered, true);
        if to_add == 0 {
            return Ok(0);
        }
        let taken = host.grid_key_move(source, kind, key, to_add, false, false)?;
        let added = host.chemical_insert(at, face, id, taken, false);
        if added < taken {
            host.grid_key_move(source, kind, key, taken - added, true, false)?;
        }
        Ok(added)
    }

    fn storage_entries(&self, host: &dyn Ae2Host, at: GridAddress, face: u8) -> Option<Vec<(ItemKey, u64)>> {
        extractable(host, at, face)
    }

    /// `ChemicalHandlerFacade.insert`/`extract`.
    fn storage_change(&self, host: &mut dyn Ae2Host, at: GridAddress, face: u8, key: &ItemKey, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
        let Some(id) = chemical(key) else { return Ok(0) };
        Ok(if insert { host.chemical_insert(at, face, id, amount, simulate) } else { host.chemical_extract(at, face, id, amount, simulate) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chemical_key_names_its_chemical_alone() {
        assert_eq!(chemical(&chemical_key("mekanism:hydrogen")), Some("mekanism:hydrogen"));
        assert_eq!(chemical(&ItemKey { item_id: "mekanism:hydrogen".into(), components: vec![1] }), None);
        assert_eq!(AppMek.id(), "appmek");
        assert!(AppMek.key_strategies().is_some());
    }
}
