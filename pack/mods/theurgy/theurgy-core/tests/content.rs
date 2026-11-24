//! The recipes of the content mod (THEURGY_CONTENT_DIR: the carry of the player's jar and the jar's accumulation
//! recipes, as `pack/scripts/theurgy.ts content` makes it): every recipe of the five ported types parses, and the count
//! of those whose ingredients this port reads, per type. Skipped without the content mod.
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};
use theurgy_core::recipe::{self, Accumulation, Calcination, Incubation, Liquefaction, RecipeResult};

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

#[test]
fn every_recipe_of_the_ported_types_parses() {
    let Ok(content) = std::env::var("THEURGY_CONTENT_DIR") else {
        eprintln!("THEURGY_CONTENT_DIR unset: skipped");
        return;
    };
    let data = Path::new(&content).join("data");
    let mut files = Vec::new();
    for ns in fs::read_dir(&data).expect("the content mod's data").flatten() {
        walk(&ns.path().join("recipe"), &mut files);
    }
    // type -> (all, parsed, every ingredient read, tag results)
    let mut counts: BTreeMap<String, [usize; 4]> = BTreeMap::new();
    let mut errors = Vec::new();
    for path in &files {
        let json: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let Some(kind) = json.get("type").and_then(Value::as_str) else { continue };
        if !kind.starts_with("theurgy:") {
            continue;
        }
        let id = path.display().to_string();
        let row = counts.entry(kind.to_owned()).or_default();
        row[0] += 1;
        let (parsed, supported, tag) = match kind {
            recipe::CALCINATION | recipe::DISTILLATION => match Calcination::parse(&id, &json) {
                Ok(r) => (true, r.ingredient.ingredient.supported(), false),
                Err(e) => { errors.push(e); (false, false, false) }
            },
            recipe::LIQUEFACTION => match Liquefaction::parse(&id, &json) {
                Ok(r) => (true, r.ingredient.supported() && r.solvent.ingredient.supported(), false),
                Err(e) => { errors.push(e); (false, false, false) }
            },
            recipe::INCUBATION => match Incubation::parse(&id, &json) {
                Ok(r) => (true, r.mercury.supported() && r.salt.supported() && r.sulfur.supported(), matches!(r.result, RecipeResult::Tag { .. })),
                Err(e) => { errors.push(e); (false, false, false) }
            },
            recipe::ACCUMULATION => match Accumulation::parse(&id, &json) {
                Ok(r) => (true, r.solute.as_ref().is_none_or(|s| s.supported()) && r.evaporant.as_ref().is_none_or(|e| e.ingredient.supported()), false),
                Err(e) => { errors.push(e); (false, false, false) }
            },
            _ => continue,
        };
        row[1] += usize::from(parsed);
        row[2] += usize::from(supported);
        row[3] += usize::from(tag);
    }
    let mut ported = [0usize; 4];
    for (kind, row) in &counts {
        println!("THEURGY recipes {kind}: {} files, {} parsed, {} with every ingredient read, {} tag results", row[0], row[1], row[2], row[3]);
        if [recipe::CALCINATION, recipe::DISTILLATION, recipe::LIQUEFACTION, recipe::INCUBATION, recipe::ACCUMULATION].contains(&kind.as_str()) {
            for i in 0..4 {
                ported[i] += row[i];
            }
        }
    }
    println!("THEURGY recipes the ported apparatus run: {} of {} ({} of them with tag results)", ported[2], ported[0], ported[3]);
    for e in errors.iter().take(10) {
        println!("THEURGY parse error: {e}");
    }
    assert!(errors.is_empty(), "{} recipes don't parse", errors.len());
    assert!(ported[0] > 0, "no recipe of a ported type in {content}");
}
