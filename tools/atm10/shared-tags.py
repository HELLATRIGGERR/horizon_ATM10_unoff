#!/usr/bin/env python3
"""Shared tags (c: and minecraft:) that recipes and quests ask for, as the game would merge them from carries.

Reads only: carries (folder mods: horizon-mod.toml), the prepared 26.3 resources (vanilla's tags and items), the pack
run's dump (which registry each tag belongs to, and its members in Java) and the instance's FTB quests.
Counts, per world: the c: item tags recipes and quests use that are missing or empty in the game (but have members in
Java), and the recipes that cannot resolve an ingredient because of it.

  shared-tags.py --carries DIR --resources DIR --dump DIR [--instance DIR] [--extra MODDIR ...] [--each]
                 [--only a,b] [--out report.json]

World "full": every carry plus the extras. --each: also one world per carry (vanilla, that carry and the extras), as a
bundle's world test loads one carry; atm10-pack is never a world of its own.
"""
import argparse, json, os, re, sys
from collections import defaultdict

TAG_RE = re.compile(r"^#([a-z0-9_.-]+):([a-z0-9_./-]+)$")
SHARED = ("c", "minecraft")


def read_json(path):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def walk_files(root, suffix=".json"):
    for dp, _, fs in os.walk(root):
        for f in fs:
            if f.endswith(suffix):
                yield os.path.join(dp, f)


class Mod:
    """One folder mod: its item tag layers, declared items, recipes' tag references."""

    def __init__(self, folder):
        self.folder = folder
        self.name = os.path.basename(folder.rstrip("/\\"))
        self.tags = {}  # "ns:path" -> (replace, [entries])
        self.items = set()
        self.recipes = {}  # recipe id -> set of "ns:path" tags it names
        data = os.path.join(folder, "data")
        for ns in sorted(os.listdir(data)) if os.path.isdir(data) else []:
            base = os.path.join(data, ns)
            tdir = os.path.join(base, "tags", "item")
            for p in walk_files(tdir):
                rel = os.path.relpath(p, tdir).replace(os.sep, "/")[:-5]
                doc = read_json(p)
                if isinstance(doc, dict) and isinstance(doc.get("values"), list):
                    self.tags[f"{ns}:{rel}"] = (doc.get("replace") is True, doc["values"])
            # Declared items: every item, and every block with one (final-data's declared_items).
            for kind in ("block", "item"):
                kdir = os.path.join(base, "horizon", kind)
                for p in walk_files(kdir):
                    rel = os.path.relpath(p, kdir).replace(os.sep, "/")[:-5]
                    if kind == "block" and (read_json(p) or {}).get("item") is False:
                        continue
                    self.items.add(f"{ns}:{rel}")
            rdir = os.path.join(base, "recipe")
            for p in walk_files(rdir):
                rel = os.path.relpath(p, rdir).replace(os.sep, "/")[:-5]
                doc = read_json(p)
                refs = set()
                tag_refs(doc, refs)
                self.recipes[f"{ns}:{rel}"] = refs


def tag_refs(v, out):
    if isinstance(v, dict):
        t = v.get("tag")
        if isinstance(t, str) and ":" in t and not t.startswith("#"):
            out.add(t)
        for x in v.values():
            tag_refs(x, out)
    elif isinstance(v, list):
        for x in v:
            tag_refs(x, out)
    elif isinstance(v, str):
        m = TAG_RE.match(v)
        if m:
            out.add(f"{m.group(1)}:{m.group(2)}")


def vanilla(resources):
    items = set((read_json(os.path.join(resources, "items.json")) or {}).keys())
    items = {i if ":" in i else "minecraft:" + i for i in items}
    tags = {}
    tdir = os.path.join(resources, "data", "minecraft", "tags", "item")
    for p in walk_files(tdir):
        rel = os.path.relpath(p, tdir).replace(os.sep, "/")[:-5]
        doc = read_json(p)
        if isinstance(doc, dict) and isinstance(doc.get("values"), list):
            tags["minecraft:" + rel] = doc["values"]
    return items, tags


def entry_id(e):
    if isinstance(e, str):
        return e, True
    if isinstance(e, dict) and isinstance(e.get("id"), str):
        return e["id"], e.get("required", True) is not False
    return None, False


class World:
    def __init__(self, base_items, base_tags, mods):
        self.items = set(base_items)
        for m in mods:
            self.items |= m.items
        self.files = {k: list(v) for k, v in base_tags.items()}
        for m in mods:
            for tag, (replace, values) in m.tags.items():
                if replace or tag not in self.files:
                    self.files[tag] = []
                self.files[tag].extend(values)
        self.memo = {}

    def members(self, tag, seen=None):
        if tag in self.memo:
            return self.memo[tag]
        seen = seen or set()
        if tag in seen:
            return set()
        seen.add(tag)
        out = set()
        for e in self.files.get(tag, []):
            i, _ = entry_id(e)
            if not i:
                continue
            if i.startswith("#"):
                ref = i[1:] if ":" in i else "minecraft:" + i[1:]
                out |= self.members(ref, seen)
            elif i in self.items:
                out.add(i)
        self.memo[tag] = out
        return out


def dump_tags(dump):
    registries = defaultdict(set)
    java = {}
    with open(os.path.join(dump, "tags.jsonl"), encoding="utf-8") as f:
        for line in f:
            row = json.loads(line)
            registries[row["id"]].add(row["registry"])
            if row["registry"] == "minecraft:item":
                java[row["id"]] = len(row.get("values") or [])
    return registries, java


def quest_tags(instance):
    out = defaultdict(int)
    root = os.path.join(instance or "", "config", "ftbquests", "quests", "chapters")
    for p in walk_files(root, ".snbt"):
        with open(p, encoding="utf-8", errors="replace") as f:
            for m in re.finditer(r"item_tag\(([a-z0-9_.-]+:[a-z0-9_./-]+)\)", f.read()):
                out[m.group(1)] += 1
    return out


def measure(world, recipes, registries, java, quests, namespaces=SHARED):
    """Shared item tags (by Java's registries) named by recipes or quests: empty here, not empty in Java."""
    asked = defaultdict(int)
    for refs in recipes.values():
        for t in refs:
            asked[t] += 1
    is_item = lambda t: "minecraft:item" in registries.get(t, ()) or not registries.get(t)
    gaps = {}
    for t in set(asked) | set(quests):
        if t.split(":", 1)[0] not in namespaces or not is_item(t):
            continue
        if java.get(t, 0) == 0:
            continue  # empty in Java too: not this gap
        if not world.members(t):
            gaps[t] = {"missing": t not in world.files, "recipes": asked.get(t, 0), "quests": quests.get(t, 0),
                       "java": java[t]}
    blocked = [r for r, refs in recipes.items() if any(t in gaps for t in refs)]
    c_gaps = {t: g for t, g in gaps.items() if t.startswith("c:")}
    c_asked = {t for t in asked if t.startswith("c:") and is_item(t) and java.get(t, 0)}
    return {
        "recipes": len(recipes),
        "c_item_tags_asked": len(c_asked | {t for t in quests if t.startswith("c:")}),
        "c_tags_empty": len(c_gaps),
        "c_tags_missing": sum(1 for g in c_gaps.values() if g["missing"]),
        "minecraft_tags_empty": len(gaps) - len(c_gaps),
        "recipes_blocked": len(blocked),
        "recipes_blocked_by_c": sum(1 for r in blocked if any(t in c_gaps for t in recipes[r])),
        "quest_tags_empty": sorted(t for t, g in gaps.items() if g["quests"]),
        "worst": sorted(gaps.items(), key=lambda kv: -kv[1]["recipes"])[:25],
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--carries", required=True)
    ap.add_argument("--resources", required=True)
    ap.add_argument("--dump", required=True)
    ap.add_argument("--instance")
    ap.add_argument("--extra", action="append", default=[])
    ap.add_argument("--each", action="store_true")
    ap.add_argument("--only")
    ap.add_argument("--out")
    a = ap.parse_args()
    only = set(a.only.split(",")) if a.only else None
    folders = [os.path.join(a.carries, d) for d in sorted(os.listdir(a.carries))
               if os.path.isfile(os.path.join(a.carries, d, "horizon-mod.toml"))]
    extra_names = {os.path.basename(e.rstrip("/\\")) for e in a.extra}
    folders = [f for f in folders if os.path.basename(f) not in extra_names]
    if only:
        folders = [f for f in folders if os.path.basename(f) in only]
    mods = [Mod(f) for f in folders]
    extras = [Mod(e) for e in a.extra]
    print(f"read {len(mods)} carries, {len(extras)} extra", file=sys.stderr)
    base_items, base_tags = vanilla(a.resources)
    registries, java = dump_tags(a.dump)
    quests = quest_tags(a.instance)
    report = {"extras": sorted(extra_names)}
    everything = mods + extras
    recipes = {}
    for m in everything:
        recipes.update(m.recipes)
    report["full"] = measure(World(base_items, base_tags, everything), recipes, registries, java, quests)
    if a.each:
        per = {}
        totals = defaultdict(int)
        for m in mods:
            if m.name == "atm10-pack" or not m.recipes:
                continue
            r = measure(World(base_items, base_tags, [m] + extras), m.recipes, registries, java, {})
            r.pop("worst")
            per[m.name] = r
            for k in ("recipes", "recipes_blocked", "recipes_blocked_by_c", "c_tags_empty"):
                totals[k] += r[k]
            totals["carries_blocked"] += 1 if r["recipes_blocked_by_c"] else 0
        report["each_totals"] = dict(totals)
        report["each"] = per
    text = json.dumps(report, indent=1)
    if a.out:
        with open(a.out, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    s = {k: v for k, v in report.items() if k != "each"}
    s["full"] = {k: v for k, v in s["full"].items() if k != "worst"}
    print(json.dumps(s, indent=1))


if __name__ == "__main__":
    main()
