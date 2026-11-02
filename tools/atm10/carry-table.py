"""carry-table.py [--keys FILE] [--md]: one row per carried mod from <work>/mods/<id>/carry-report.json and
<work>/inventory.json (version, class, tier, licence), plus totals. With --md, a Markdown table."""
import os
import json, sys
from pathlib import Path

work = Path(os.environ.get("ATM10_WORK") or Path(__file__).resolve().parents[1])
recs = {}
for r in json.loads((work / 'inventory.json').read_text(encoding='utf-8')):
    if r.get('jar'):
        recs[r['key']] = r  # the later of two jars with one key (computercraft) is the one carried
keys = sorted(p.parent.name for p in (work / 'mods').glob('*/carry-report.json'))
if '--keys' in sys.argv:
    keys = sorted(set(keys) & set((work / sys.argv[sys.argv.index('--keys') + 1]).read_text().split()))
rows = []
for k in keys:
    rep = json.loads((work / 'mods' / k / 'carry-report.json').read_text(encoding='utf-8'))
    r = recs.get(k, {})
    info = r.get('info') or {}
    rows.append({
        'key': k, 'name': r.get('name', k), 'version': info.get('version') or r.get('version', ''),
        'class': r.get('class', ''), 'tier': r.get('tier', ''), 'licence': info.get('license') or r.get('license', ''),
        'blocks': len(rep['blocks']), 'left_out': len(rep['blocks_unknown']), 'items': rep['items'],
        'recipes_up': rep['recipes_upgraded'], 'recipes_kept': len(rep['recipes_kept']),
        'loot': rep['loot_tables_upgraded'] + rep['loot_tables_added'], 'worldgen': rep['worldgen'],
        'outside': len(rep['outside']), 'unreadable': len(rep['unreadable']), 'copied': rep['copied'],
        'oracle': rep['oracle'], 'equipment': rep['equipment'], 'namespaces': rep['namespaces'],
    })
total = {f: sum(r[f] for r in rows) for f in ('blocks', 'left_out', 'items', 'recipes_up', 'recipes_kept', 'loot',
                                              'worldgen', 'outside', 'unreadable', 'copied', 'equipment')}
if '--md' in sys.argv:
    print('| mod | version | class / tier | licence | blocks | left out | items | recipes up/kept | loot | worldgen | outside |')
    print('|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|')
    for r in rows:
        lic = r['licence'] if len(r['licence']) < 40 else r['licence'][:37] + '...'
        name = f"{r['key']} ({r['name']})" if r['name'] != r['key'] else r['key']
        print(f"| {name} | {r['version']} | {r['class']} / {r['tier']} | {lic} | {r['blocks']} | {r['left_out']} | "
              f"{r['items']} | {r['recipes_up']}/{r['recipes_kept']} | {r['loot']} | {r['worldgen']} | {r['outside']} |")
    print(f"| **total {len(rows)}** | | | | **{total['blocks']:,}** | **{total['left_out']:,}** | **{total['items']:,}** | "
          f"**{total['recipes_up']:,}/{total['recipes_kept']:,}** | **{total['loot']:,}** | **{total['worldgen']:,}** | "
          f"**{total['outside']:,}** |")
else:
    print(json.dumps({'mods': len(rows), **total}))
