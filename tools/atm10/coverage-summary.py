"""coverage-summary.py: appends "Carry-over status" to ../coverage.md (after `horizon-port-pack report`, which
rewrites the file; a section from an earlier run is replaced). It sorts every row of coverage.json into:

- carried, content only: data tier; the carry is the whole mod (what is left is the harness);
- carried, needs code: logic, machines and deep tiers; blocks and items exist with their template's behaviour,
  the mod's own machines, menus, entities and so on need a native mod;
- carried, not classified yet (tier none, the report has no row for them);
- not carried: skip-plan.json's reasons, left-out\\ (content all in other mods' namespaces), and the rest
  (no content to carry: client, library and performance mods);
with per-group totals from each carry-report.json.
"""
import os
import collections
import json
from pathlib import Path

work = Path(os.environ.get("ATM10_WORK") or Path(__file__).resolve().parents[1])
MARK = '\n## Carry-over status'
rows = json.loads((work / 'coverage.json').read_text(encoding='utf-8'))
carried = set((work / 'all-keys.txt').read_text().split())
skip = json.loads((work / 'skip-plan.json').read_text(encoding='utf-8'))['skip']
left_out = {p.name for p in (work / 'left-out').iterdir() if p.is_dir()} if (work / 'left-out').is_dir() else set()


def report(key):
    p = work / 'mods' / key / 'carry-report.json'
    return json.loads(p.read_text(encoding='utf-8')) if p.exists() else None


groups = collections.defaultdict(list)
seen = set()
for r in rows:
    k = r['key']
    if k in seen:  # computercraft has two jars; the later one is carried
        continue
    seen.add(k)
    if k in carried:
        g = {'data': 'content only', 'none': 'not classified'}.get(r['tier'], 'needs code')
        groups[('carried', g, r['tier'])].append(k)
    elif k in skip:
        groups[('not carried', skip[k], '')].append(k)
    elif k in left_out:
        groups[('not carried', 'content all in other mods\' namespaces (left-out\\)', '')].append(k)
    elif r['status'] == 'not-needed':
        groups[('not carried', f"not needed ({r['class']})", '')].append(k)
    else:
        groups[('not carried', f"no content carried: {r['class']} / {r['tier']}", '')].append(k)


def totals(keys):
    t = collections.Counter()
    for k in keys:
        rep = report(k)
        if rep:
            t['blocks'] += len(rep['blocks'])
            t['left out'] += len(rep['blocks_unknown'])
            t['items'] += rep['items']
            t['recipes'] += rep['recipes_upgraded'] + len(rep['recipes_kept'])
            t['worldgen'] += rep['worldgen']
    return t


out = [MARK + ' (2026-11-01, `tools/coverage-summary.py`)', '',
       f'{len(carried)} mods carried (`all-keys.txt`) and loaded together with AE2 (load check: 288 on, 0 refused).',
       '"Content only" mods are complete as data once their harness passes; "needs code" mods have their blocks,',
       'items, recipes, loot and worldgen, with each block behaving as its vanilla template, and need a native mod',
       'for the rest (machines, menus, energy, entities, custom renderers). The engine side of that is the mod API',
       'tiers in `docs/modpacks.md` (0.3 behaviour callbacks, 0.4 block entities and menus, 0.5 entities and',
       'renderers).', '',
       '| group | tier | mods | blocks | blocks left out | items | recipes | worldgen files |',
       '|---|---|---:|---:|---:|---:|---:|---:|']
order = ['content only', 'needs code', 'not classified']
carried_groups = sorted((g for g in groups if g[0] == 'carried'), key=lambda g: (order.index(g[1]), g[2]))
for g in carried_groups:
    t = totals(groups[g])
    out.append(f"| carried, {g[1]} | {g[2]} | {len(groups[g])} | {t['blocks']:,} | {t['left out']:,} | "
               f"{t['items']:,} | {t['recipes']:,} | {t['worldgen']:,} |")
not_carried = sorted((g for g in groups if g[0] == 'not carried'), key=lambda g: -len(groups[g]))
out.append(f"| not carried | | {sum(len(groups[g]) for g in not_carried)} | | | | | |")
out += ['']
for g in carried_groups:
    out += [f"**Carried, {g[1]} ({g[2]}, {len(groups[g])}):** " + ', '.join(sorted(groups[g])), '']
out += ['**Not carried, by reason:**', '']
for g in not_carried:
    out.append(f"- {g[1]} ({len(groups[g])}): " + ', '.join(sorted(groups[g])))
out.append('')

md = work / 'coverage.md'
text = md.read_text(encoding='utf-8')
if MARK in text:
    text = text[:text.index(MARK)]
md.write_text(text.rstrip('\n') + '\n' + '\n'.join(out), encoding='utf-8')
print(f"{md}: {sum(len(groups[g]) for g in carried_groups)} carried, "
      f"{sum(len(groups[g]) for g in not_carried)} not carried")
