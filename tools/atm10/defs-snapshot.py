"""defs-snapshot.py <out.json> <mod key>...: each carried mod's block and item definitions
(data/<ns>/horizon/{block,item}/*.json) and its carry-report counts, to compare a carry before and after the
oracle's facts."""
import os
import json, sys
from pathlib import Path

work = Path(os.environ.get("ATM10_WORK") or Path(__file__).resolve().parents[1])
out = {}
for key in sys.argv[2:]:
    carry = work / 'mods' / key / 'carry'
    rep = json.loads((work / 'mods' / key / 'carry-report.json').read_text(encoding='utf-8'))
    defs = {}
    for kind in ('block', 'item'):
        for f in sorted(carry.glob(f'data/*/horizon/{kind}/**/*.json')):
            ns = f.relative_to(carry).parts[1]
            name = f.relative_to(carry / 'data' / ns / 'horizon' / kind).with_suffix('').as_posix()
            defs[f'{kind} {ns}:{name}'] = json.loads(f.read_text(encoding='utf-8'))
    out[key] = {'defs': defs, 'blocks': len(rep['blocks']), 'left_out': len(rep['blocks_unknown']),
                'items': rep['items'], 'items_unknown': len(rep['items_unknown']), 'differences': len(rep['differences']),
                'equipment': rep['equipment'], 'loot_added': rep['loot_tables_added'], 'oracle': rep['oracle'],
                'recipes_up': rep['recipes_upgraded'], 'recipes_kept': len(rep['recipes_kept'])}
Path(sys.argv[1]).write_text(json.dumps(out), encoding='utf-8')
print(sys.argv[1], sum(len(v['defs']) for v in out.values()), 'definitions')
