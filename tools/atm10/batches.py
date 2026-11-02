"""batches.py [LIMIT]: groups the carried keys ($ATM10_WORK/all-keys.txt) into oracle batches of about LIMIT blockstate files, writes
work/oracle/<batch>.txt (keys) and work/oracle/batches.txt (`<batch> <namespace>,...`: the mod ids of each batch's jars,
as `neoforge_oracle.py export` takes them). The mods sharing a namespace no pack mod owns go in one batch, so one
carry run sees all their claims."""
import json, os, sys, tomllib, zipfile
from pathlib import Path
W = Path(os.environ['ATM10_WORK'])
LIMIT = int(sys.argv[1]) if len(sys.argv) > 1 else 5000
SHARED = 'amendments create farmingforblockheads supplementaries dyenamics xycraft_world xycraft_machines'.split()
recs = {r['key']: r for r in json.load(open(W / 'inventory.json', encoding='utf-8'))}
keys = (W / 'all-keys.txt').read_text().split()

def ids_and_states(jar):
    with zipfile.ZipFile(jar) as z:
        names = z.namelist()
        ids = []
        for n in ('META-INF/neoforge.mods.toml', 'META-INF/mods.toml'):
            if n in names:
                try:
                    ids = [m.get('modId', '') for m in tomllib.loads(z.read(n).decode('utf-8', 'replace')).get('mods', [])]
                except tomllib.TOMLDecodeError:
                    pass
                break
        states = sum(1 for n in names if n.startswith('assets/') and '/blockstates/' in n and n.endswith('.json'))
    return [i for i in ids if i], states

info = {}
for k in keys:
    ids, states = ids_and_states(recs[k]['jar'])
    info[k] = (ids or [recs[k]['info']['mod_id']], states)
batches, cur, size = [], [], 0
shared = [k for k in keys if k in SHARED]
for k in sorted((k for k in keys if k not in SHARED), key=lambda k: -info[k][1]):
    s = info[k][1]
    if cur and size + s > LIMIT:
        batches.append(cur); cur, size = [], 0
    cur.append(k); size += s
if cur:
    batches.append(cur)
batches.append(shared)
oracle = W / 'oracle'
oracle.mkdir(exist_ok=True)
lines = []
for i, b in enumerate(batches, 1):
    name = 'shared' if b is shared else f'b{i:02d}'
    # Shell scripts read these: no CR at the end of a key.
    (oracle / f'{name}.txt').write_text('\n'.join(sorted(b)) + '\n', newline='\n')
    ns = sorted({i for k in b for i in info[k][0]})
    lines.append(f'{name} {",".join(ns)}')
    print(f'{name}: {len(b)} mods, {sum(info[k][1] for k in b)} blockstate files')
(oracle / 'batches.txt').write_text('\n'.join(lines) + '\n', newline='\n')
