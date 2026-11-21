//! `atm10-load-check <mod folder>...`: each content mod in the order given, read as the game's
//! `content_root` reads `--dev-mod` folders (manifest, content checks, namespace clashes; the
//! first of two mods owning a namespace stays on). Prints one line per mod, then the totals; with
//! `--entity-types`, also each living entity type a mod declares (`type <id> <template>`); with
//! `--rows <entity-types.json>` (the game's prepared table), each living type's template row must
//! be there with every attribute the type sets, as the content root's rows ask (`mod_entity_row`).
use horizon_mod_package::{Capability, Content, Package, content::clash};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list_types = args.iter().any(|a| a == "--entity-types");
    let rows_at = args.iter().position(|a| a == "--rows");
    let rows: Option<serde_json::Map<String, serde_json::Value>> = rows_at.map(|i| {
        let path = args.get(i + 1).expect("--rows names the game's entity-types.json");
        serde_json::from_slice(&std::fs::read(path).expect("entity-types.json reads")).expect("entity-types.json is an object")
    });
    let paths: Vec<PathBuf> = args
        .iter()
        .enumerate()
        .filter(|&(i, a)| a != "--entity-types" && a != "--rows" && rows_at.is_none_or(|r| i != r + 1))
        .map(|(_, a)| PathBuf::from(a))
        .collect();
    let mut on: Vec<(String, Vec<String>)> = Vec::new();
    let (mut blocks, mut items, mut files, mut refused, mut living) = (0, 0, 0, 0, 0);
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
            for t in content.entity_types.iter().filter(|t| t.is_living()) {
                let Some(rows) = &rows else { break };
                let Some(row) = rows.get(&t.template) else {
                    return Err(format!("{}: no row for its template {}", t.id, t.template));
                };
                if let Some(a) = t.attributes.keys().find(|a| row["attributes"].get(a.as_str()).is_none()) {
                    return Err(format!("{}: attribute {a} is not one of {}'s", t.id, t.template));
                }
            }
            Ok((m.id.clone(), content))
        });
        match result {
            Ok((id, c)) => {
                let mobs: Vec<_> = c.entity_types.iter().filter(|t| t.is_living()).collect();
                println!("ok {id}: {} files, {} blocks, {} items, {} living entity types, namespaces {:?}", c.files.len(), c.blocks.len(), c.items.len(), mobs.len(), c.namespaces);
                if list_types {
                    for t in &mobs {
                        println!("type {} {}", t.id, t.template);
                    }
                }
                living += mobs.len();
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
    println!("{} mods on, {refused} refused: {files} files, {blocks} blocks, {items} items, {living} living entity types", on.len());
}
