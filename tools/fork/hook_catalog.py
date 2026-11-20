#!/usr/bin/env python3
"""Writes tools/fork/native-hooks.json, the hook catalog the pack's bundles are applied with.

Upstream retired the JSON catalog: each hook is declared in `crates/horizon-mod-vocab/src/hooks/*.rs`
(`#[hook(...)] fn <name>();`) and fired by a `native_<name>` method in one of
`crates/horizon-client/src/game/native_hooks/*.rs`. horizon-mixins still resolves a bundle's
`hook = "<name>"` / `hook_version` through a `native-hooks.json` (name, version, target), so this
script writes one from the declarations, with the targets resolved as upstream's generator does
(`horizon-mod-api-gen/src/hook_targets.rs`: the file that defines the method names the module).
The pack scripts put a copy at the workspace root for `horizon-mixins apply` only (upstream's
build refuses a `native-hooks.json` there) and leave it out of the stage.

Usage: python3 tools/fork/hook_catalog.py [--check]
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VOCAB = ROOT / "crates/horizon-mod-vocab/src/hooks"
HOOKS = ROOT / "crates/horizon-client/src/game/native_hooks"
OUT = ROOT / "tools/fork/native-hooks.json"
PREFIX = "horizon_client::game::native_hooks::"
# The fork's compat hooks: hook points the fork's bundles inject into that upstream's vocabulary has no hook for
# (mapping.md, verdict `compat`). Their bodies are empty injection targets in fork_compat/compat.rs, which no trait
# mod hears; version 1 is the fork's contract.
COMPAT_FILE = ROOT / "crates/horizon-client/src/game/modding/fork_compat/compat.rs"
COMPAT_MODULE = "horizon_client::game::fork_compat::compat"
COMPAT_HOOKS = ["mods", "settings", "settings_click", "net_from_client", "net_from_server", "item_fluid_tank",
                "block_entity_types"]
# Upstream's `native_*` functions the fork's bundles inject into that its vocabulary declares no hook for (mapping.md:
# `block_entity_container` is a function only, its body reads the Parts' `block_container`; a bundle's HEAD
# injection runs before that assignment).
FN_ONLY_HOOKS = ["block_entity_container"]

ATTR = re.compile(r"#\[hook\((?P<args>.*?)\)\]\s*(?:#\[[^\]]*\]\s*)*fn\s+(?P<name>\w+)", re.S)
VERSION = re.compile(r"\bversion\s*=\s*(\d+)")
TARGET = re.compile(r"\btarget\s*=\s*\"([^\"]+)\"")
SIDE = re.compile(r"\bside\s*=\s*\"(\w+)\"")
IMPL = re.compile(r"^impl(?:<[^>{]*>)?\s+([A-Za-z_][\w:]*(?:<[^>{]*>)?)\s*(?:where[^{]*)?\{", re.M)
NATIVE_FN = re.compile(r"\bfn\s+(native_\w+)\s*[<(]")


def declarations():
    """(name, version, side, explicit target or None), in file-name order."""
    out = []
    for path in sorted(VOCAB.glob("*.rs")):
        if path.name == "mod.rs":
            continue
        text = re.sub(r"//[^\n]*", "", path.read_text(encoding="utf-8"))
        for match in ATTR.finditer(text):
            args = match.group("args")
            version = VERSION.search(args)
            target = TARGET.search(args)
            side = SIDE.search(args)
            out.append((match.group("name"), int(version.group(1)) if version else 1,
                        side.group(1) if side else None, target.group(1) if target else None))
    return out


def definitions():
    """`<Type>::native_<name>` -> the full target path, from the hook files' inherent impls."""
    targets = {}
    for path in sorted(HOOKS.glob("*.rs")):
        if path.name == "fire.rs":
            continue
        area = path.stem
        module = PREFIX[:-2] if area in ("mod", "shared") else PREFIX + area
        text = path.read_text(encoding="utf-8")
        impls = [(m.start(), m.group(1).replace(" ", "")) for m in IMPL.finditer(text)]
        for m in NATIVE_FN.finditer(text):
            owner = [ty for start, ty in impls if start < m.start()]
            if not owner or " for " in owner[-1]:
                continue
            ty = owner[-1]
            targets.setdefault(f"{ty}::{m.group(1)}", f"{module}::{ty}::{m.group(1)}")
    return targets


def catalog():
    targets = definitions()
    hooks, missing = [], []
    for name, version, side, target in declarations():
        target = target or f"{PREFIX}Game<'_>::native_{name}"
        if target.startswith(PREFIX):
            tail = "::".join(target[len(PREFIX):].split("::")[1:]) if target.count("::") > 4 else target[len(PREFIX):]
            resolved = targets.get(tail) or targets.get(f"Game<'_>::native_{name}")
            if resolved is None:
                missing.append(name)
                continue
            target = resolved
        entry = {"name": name, "version": version, "target": target}
        if side:
            entry["side"] = side
        hooks.append(entry)
    compat = COMPAT_FILE.read_text(encoding="utf-8")
    for name in COMPAT_HOOKS:
        if f"fn native_{name}(" not in compat:
            missing.append(name)
            continue
        hooks.append({"name": name, "version": 1, "target": f"{COMPAT_MODULE}::Game<'_>::native_{name}", "compat": True})
    for name in FN_ONLY_HOOKS:
        target = targets.get(f"Game<'_>::native_{name}")
        if target is None:
            missing.append(name)
            continue
        hooks.append({"name": name, "version": 1, "target": target, "compat": True})
    if missing:
        sys.exit(f"no native definition for: {', '.join(missing)}")
    return {"hooks": hooks}


def main():
    text = json.dumps(catalog(), indent=2) + "\n"
    if "--check" in sys.argv[1:]:
        if OUT.read_text(encoding="utf-8") != text:
            sys.exit(f"{OUT.relative_to(ROOT)} is stale: run python3 tools/fork/hook_catalog.py")
        return
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(f"{OUT.relative_to(ROOT)}: {len(json.loads(text)['hooks'])} hooks")


if __name__ == "__main__":
    main()
