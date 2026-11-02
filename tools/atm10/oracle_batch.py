"""oracle_batch.py <batch name> <mod key>... [+<mod id>...]: copies the batch's jars (from <work>/jars) and their required
libraries (resolved through mods.toml, jar-in-jar included, from the ATM10 instance, read only) into
<work>/oracle/<batch>/jars and deps."""
import io, json, re, shutil, sys, tomllib, zipfile
from pathlib import Path
W = Path(r'E:\PROJECTS\horizon-ae2\ports\atm10')
I = Path(r'C:\Users\mpoka\curseforge\minecraft\Instances\All the Mods 10 - ATM10\mods')
BUILTIN = {'minecraft', 'neoforge', 'forge', 'java', 'fml', 'javafml'}

def tomls(z):
    for n in ('META-INF/neoforge.mods.toml', 'META-INF/mods.toml'):
        if n in z.namelist():
            try:
                yield tomllib.loads(z.read(n).decode('utf-8', 'replace'))
            except tomllib.TOMLDecodeError:
                pass
            return

def read(z, provides, requires, outer, nested=False):
    if 'META-INF/services/net.neoforged.neoforgespi.language.IModLanguageLoader' in z.namelist():
        languages.add(outer)
    for t in tomls(z):
        if str(t.get('modLoader', 'javafml')) not in ('javafml', 'lowcodefml'):
            foreign.add(outer)
        ids = [m.get('modId') for m in t.get('mods', [])]
        bundled.setdefault(outer, set()).update(ids)
        for i in ids:
            # A jar of its own wins over a copy bundled in another; of two jars, the later name (newer version).
            if nested:
                provides.setdefault(i, outer)
            else:
                provides[i] = outer
        dd = t.get('dependencies') or {}
        for deps in (dd.values() if isinstance(dd, dict) else [dd]):
            for d in (deps if isinstance(deps, list) else [deps]):
                if not isinstance(d, dict) or 'modId' not in d:
                    continue
                # NeoForge 21.1 reads `type` (default required) and ignores the old `mandatory`.
                kind = str(d.get('type', 'required')).lower()
                if kind == 'required' and d['modId'] not in BUILTIN:
                    for i in ids:
                        requires.setdefault(i, set()).add(d['modId'])
    for n in z.namelist():
        if n.startswith('META-INF/jarjar/') and n.endswith('.jar'):
            try:
                read(zipfile.ZipFile(io.BytesIO(z.read(n))), provides, requires, outer, True)
            except zipfile.BadZipFile:
                pass

provides, requires, bundled = {}, {}, {}
languages, foreign = set(), set()  # jars of language loaders (Scala, Kotlin); jars needing one
for jar in sorted(I.glob('*.jar')):
    read(zipfile.ZipFile(jar), provides, requires, jar)

def main():
    # `+<mod id>`: a library a mod needs without declaring it (Dyson Cube Project: titanium).
    batch = sys.argv[1]
    keys = [a for a in sys.argv[2:] if not a.startswith('+')]
    extra = [a[1:] for a in sys.argv[2:] if a.startswith('+')]
    recs = {r['key']: r for r in json.load(open(W / 'inventory.json', encoding='utf-8'))}
    out = W / 'oracle' / batch
    shutil.rmtree(out, ignore_errors=True)
    (out / 'jars').mkdir(parents=True)
    (out / 'deps').mkdir()
    chosen, want = set(), []
    for k in keys:
        jar = Path(recs[k]['jar'])
        shutil.copyfile(jar, out / 'jars' / jar.name)
        chosen.add(jar.name)
        want.append(recs[k]['info']['mod_id'])
        # Every mod the jar holds, bundled ones too, brings its own requirements.
        want.extend(bundled.get(I / jar.name, ()))
    want = list(dict.fromkeys(want))
    for d in extra:
        requires.setdefault(want[0], set()).add(d)
    todo, seen, missing = list(want), set(want), []
    deps = set()
    while todo:
        m = todo.pop()
        for d in sorted(requires.get(m, ())):
            if d in seen:
                continue
            seen.add(d)
            jar = provides.get(d)
            if jar is None:
                missing.append(f'{m} needs {d}')
                continue
            if jar.name not in chosen:
                deps.add(jar)
            for i, j in provides.items():
                if j == jar and i not in seen:
                    seen.add(i); todo.append(i)
            todo.append(d)
    # A mod written for another language loader (Quarry Plus: kotori_scala) needs that loader's jar.
    if any(I / name in foreign for name in chosen) or any(j in foreign for j in deps):
        deps |= languages
    for jar in sorted(deps):
        shutil.copyfile(jar, out / 'deps' / jar.name)
    print(f'{batch}: {len(keys)} jars, {len(deps)} libraries: {", ".join(sorted(j.name for j in deps))}')
    if missing:
        print('MISSING:', '; '.join(missing))

main()
