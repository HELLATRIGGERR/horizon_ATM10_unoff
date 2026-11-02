r"""pack-leave-out.py <instance> > oracle/pack-leave-out.txt: the jars a server cannot load, for `neoforge_oracle.py
pack --leave-out`: mods Modrinth marks as unsupported on a server (inventory.json), plus the ones named below, except
those another mod needs on a server (its mods.toml dependency with side BOTH or SERVER and type required)."""
import os
import json
import sys
import tomllib
import zipfile
from pathlib import Path

here = Path(os.environ.get("ATM10_WORK") or Path(__file__).resolve().parents[1])
instance = Path(sys.argv[1])
# Client-only by what they are, which Modrinth doesn't say for CurseForge-only mods.
CLIENT_ONLY = """drippyloadingscreen fancymenu konkrete melody enchdesc extremesoundmuffler overloadedarmorbar toastcontrol
trashslot betteradvancedtooltips keybindbundles journeymap appleskin jade""".split()

records = json.loads((here / 'inventory.json').read_text(encoding='utf-8'))
out = {r['key'] for r in records if r.get('server') == 'unsupported'} | set(CLIENT_ONLY)
by_id = {}
needs = {}
for jar in sorted((instance / 'mods').glob('*.jar')):
    with zipfile.ZipFile(jar) as z:
        if 'META-INF/neoforge.mods.toml' not in z.namelist():
            continue
        try:
            toml = tomllib.loads(z.read('META-INF/neoforge.mods.toml').decode('utf-8', 'replace'))
        except tomllib.TOMLDecodeError:
            continue
    for m in toml.get('mods', []):
        by_id[m.get('modId', '')] = jar.name
    table = toml.get('dependencies', {})
    for source, deps in table.items() if isinstance(table, dict) else ():
        for d in deps if isinstance(deps, list) else []:
            if d.get('type', 'required').lower() == 'required' and d.get('side', 'BOTH').upper() != 'CLIENT':
                needs.setdefault(d.get('modId', ''), set()).add(source)
# A mod needed by one that stays, stays too.
changed = True
while changed:
    changed = False
    for key in sorted(out):
        if {s for s in needs.get(key, ())} - out - {key}:
            out.discard(key)
            changed = True
for key in sorted(out):
    if key in by_id:
        print(key)
