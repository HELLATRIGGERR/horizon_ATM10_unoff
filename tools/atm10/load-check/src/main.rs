//! `atm10-load-check <mod folder>...`: each content mod in the order given, read as the game's
//! `content_root` reads `--dev-mod` folders (manifest, content checks, namespace clashes; the
//! first of two mods owning a namespace stays on). Prints one line per mod, then the totals.
use horizon_mods::{Capability, Content, Package, content::clash};
use std::path::PathBuf;

fn main() {
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let mut on: Vec<(String, Vec<String>)> = Vec::new();
    let (mut blocks, mut items, mut files, mut refused) = (0, 0, 0, 0);
    for path in &paths {
        let result = Package::read(path).and_then(|p| {
            let m = p.manifest;
            if !m.capabilities.contains(&Capability::Registries) {
                return Err("no capability registries".into());
            }
            let content = Content::read(path, &m)?;
            if let Some(e) = on.iter().find_map(|(id, ns)| clash((id, ns), (&m.id, &content.namespaces))) {
                return Err(e);
            }
            Ok((m.id.clone(), content))
        });
        match result {
            Ok((id, c)) => {
                println!("ok {id}: {} files, {} blocks, {} items, namespaces {:?}", c.files.len(), c.blocks.len(), c.items.len(), c.namespaces);
                blocks += c.blocks.len();
                items += c.items.len();
                files += c.files.len();
                on.push((id, c.namespaces.clone()));
            }
            Err(e) => {
                refused += 1;
                println!("REFUSED {}: {e}", path.display());
            }
        }
    }
    println!("{} mods on, {refused} refused: {files} files, {blocks} blocks, {items} items", on.len());
}
