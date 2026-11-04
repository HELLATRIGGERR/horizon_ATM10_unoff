#!/usr/bin/env python3
"""Dev-only NeoForge oracle for Horizon's mod-port harnesses (docs/modpacks.md).

    neoforge_oracle.py export --jars DIR [--deps DIR] [--namespaces a,b] --out FACTS.json
    neoforge_oracle.py pack --instance DIR --run DIR --out DIR [--leave-out FILE] [--namespaces a,b] [--batches FILE]
    neoforge_oracle.py client-pack --instance DIR --run DIR --out FILE [--leave-out FILE]

installs a NeoForge server under .local/neoforge-oracle (once), builds the exporter mod in
mods/horizon/src/neoforge-oracle, starts the server headless with the given mod jars, and writes every
block, item, recipe and tag of the mods' namespaces (and the vanilla blocks, for templates) to
FACTS.json. The jars are the user's own; nothing here is shipped or committed.

`pack` starts a whole modpack instance the same way: every jar of its mods/ (but the client-only ones in --leave-out),
with its config, KubeJS scripts and global data packs copied to the run folder (the instance is only read), and writes
the final recipes, tags and loot tables to --out as JSON lines (FactsExporter's PackDump), plus the facts of
--namespaces, and with --batches (lines of `<name> <namespace>,...`) the facts of each batch in <out>/facts-<name>.json:
a whole pack's facts in one file are too big for `horizon-port-pack carry` to read at once. KubeJS and Almost Unified have then had their say, so the result stands in for both. KubeJS scripts may be
All Rights Reserved: the output stays local.

`client-pack` starts the same instance on the NeoForge *client* (every jar of its mods/, with the client exporter in
mods/horizon/src/neoforge-oracle-client) and writes what only a client holds to --out once the mods have loaded:
each fluid type's client extension (textures, tint), each fluid's render layer and each bucket's item colours
(docs/mod-fluids.md). Its window is never shown, the game halts before the title screen, mods that open windows of
their own (CLIENT_NEVER: Crash Assistant) are left out, and a Java process a mod leaves behind is stopped.

Environment: HORIZON_NEOFORGE_HOME, a NeoForge server already installed (its libraries/ and args files), used
instead of installing one; HORIZON_NEOFORGE_CLIENT_HOME, the folder the NeoForge client is installed into (default
.local/neoforge-oracle/client-<version>); HORIZON_ORACLE_JAVA, the Java that runs NeoForge (1.21.1 wants Java 21; default
`java`); HORIZON_ORACLE_XMX, the server's heap (default 4G); HORIZON_ORACLE_JVM, more Java options. On Windows the server reads win_args.txt, and a run
folder gets a junction to libraries/ and copies of the jars: symbolic links need Developer Mode there.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCAL = ROOT / '.local' / 'neoforge-oracle'
SOURCE = ROOT / 'mods' / 'horizon' / 'src' / 'neoforge-oracle'
# ATM10 8.4: Minecraft 1.21.1 on NeoForge 21.1.265 (docs/modpacks.md); HORIZON_NEOFORGE follows another instance.
NEOFORGE = os.environ.get('HORIZON_NEOFORGE', '21.1.265')
MAVEN = 'https://maven.neoforged.net/releases/net/neoforged/neoforge'
HOME = os.environ.get('HORIZON_NEOFORGE_HOME')
JAVA = os.environ.get('HORIZON_ORACLE_JAVA', 'java')
XMX = os.environ.get('HORIZON_ORACLE_XMX', '4G')
# unix_args.txt separates its module path with `:`, which Java on Windows refuses.
ARGS = 'win_args.txt' if os.name == 'nt' else 'unix_args.txt'


def installer_jar():
    """NeoForge's installer, fetched once per version."""
    installer = LOCAL / f'neoforge-{NEOFORGE}-installer.jar'
    if not installer.exists():
        LOCAL.mkdir(parents=True, exist_ok=True)
        url = f'{MAVEN}/{NEOFORGE}/neoforge-{NEOFORGE}-installer.jar'
        with urllib.request.urlopen(url, timeout=120) as r:
            installer.write_bytes(r.read())
    return installer


def install():
    """The NeoForge server, installed once per version (or the one HORIZON_NEOFORGE_HOME names)."""
    home = Path(HOME) if HOME else LOCAL / NEOFORGE
    args = home / f'libraries/net/neoforged/neoforge/{NEOFORGE}/{ARGS}'
    if args.exists():
        return home
    if HOME:
        sys.exit(f'HORIZON_NEOFORGE_HOME: {args} does not exist')
    home.mkdir(parents=True, exist_ok=True)
    installer = installer_jar()
    with (LOCAL / f'install-{NEOFORGE}.log').open('w') as log:
        subprocess.run([JAVA, '-jar', str(installer), '--installServer', str(home)],
                       cwd=LOCAL, stdout=log, stderr=subprocess.STDOUT, check=True)
    return home


def client():
    """The NeoForge client (tools/perf_bench.py --game neoforge), installed once per version: the installer's
    own files, then the vanilla libraries its version inherits, fetched as the launcher does (sha1-checked)."""
    sys.path.insert(0, str(ROOT / 'tools'))
    import hashlib
    import lab
    home = Path(os.environ.get('HORIZON_NEOFORGE_CLIENT_HOME') or LOCAL / f'client-{NEOFORGE}')
    profile = home / 'versions' / f'neoforge-{NEOFORGE}' / f'neoforge-{NEOFORGE}.json'
    if not profile.exists():
        installer = installer_jar()
        home.mkdir(parents=True, exist_ok=True)
        (home / 'launcher_profiles.json').write_text('{"profiles":{}}')
        with (home / f'install-client-{NEOFORGE}.log').open('w') as log:
            subprocess.run([JAVA, '-jar', str(installer), '--installClient', str(home)], cwd=home, stdout=log,
                           stderr=subprocess.STDOUT, check=True)
    neo = json.loads(profile.read_text())
    base = json.loads((home / 'versions' / neo['inheritsFrom'] / f'{neo["inheritsFrom"]}.json').read_text())
    jars = []
    for manifest in (neo, base):
        names = {library['name'] for library in manifest['libraries']}
        for library in manifest['libraries']:
            artifact = library.get('downloads', {}).get('artifact')
            if not artifact or not lab.library_allowed(library) or not lab.native_library_matches(library, None, names):
                continue
            path = home / 'libraries' / artifact['path']
            if not path.exists():
                with urllib.request.urlopen(artifact['url'], timeout=120) as r:
                    data = r.read()
                if hashlib.sha1(data).hexdigest() != artifact['sha1']:
                    sys.exit(f'{artifact["url"]}: sha1 mismatch')
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            if path not in jars:
                jars.append(path)
    return home, neo, base, jars


def client_command(java, heap, game_dir, natives, assets, jvm=(), game=()):
    """The command the launcher would run for the NeoForge client profile, with no account."""
    home, neo, base, jars = client()
    import lab
    values = {'library_directory': str(home / 'libraries'), 'classpath_separator': os.pathsep,
              'version_name': f'neoforge-{NEOFORGE}', 'natives_directory': str(natives),
              'launcher_name': 'horizon-bench', 'launcher_version': '1'}
    expand = lambda s: s.replace('${', '{').format(**values)
    command = [str(java), f'-Xmx{heap}']
    for argument in base['arguments']['jvm']:
        if isinstance(argument, dict):
            if lab.library_allowed(argument):
                command += [argument['value']] if isinstance(argument['value'], str) else argument['value']
        elif argument == '${classpath}':
            # Every library; not the vanilla jar, which NeoForge's patched client replaces.
            command.append(os.pathsep.join(map(str, jars)))
        else:
            command.append(expand(argument))
    command += [expand(a) for a in neo['arguments']['jvm']]
    command += [*jvm, neo['mainClass']]
    command += ['--username', 'Bench', '--uuid', '00000000000000000000000000000000', '--accessToken', '0',
                '--version', f'neoforge-{NEOFORGE}', '--gameDir', str(game_dir), '--assetsDir', str(assets),
                '--assetIndex', base['assetIndex']['id'], '--userType', 'msa', '--versionType', 'release',
                *neo['arguments']['game'], *game]
    return home, command


def build_bench(home):
    """The NeoForge benchmark driver (mods/horizon/src/neoforge-bench), against the client."""
    return build(home, ROOT / 'mods/horizon/src/neoforge-bench', 'horizon-bench-neoforge.jar', client_classpath(home))


def client_classpath(home):
    libs = home / 'libraries'
    first = [libs / f'net/neoforged/neoforge/{NEOFORGE}/neoforge-{NEOFORGE}-client.jar',
             libs / f'net/neoforged/neoforge/{NEOFORGE}/neoforge-{NEOFORGE}-universal.jar']
    rest = sorted(p for p in libs.rglob('*.jar') if p not in first and 'slim' not in p.name
                  and 'extra' not in p.name and not p.name.endswith('-unpacked.jar'))
    return os.pathsep.join(map(str, first + rest))


def classpath(home):
    libs = home / 'libraries'
    first = [libs / f'net/neoforged/neoforge/{NEOFORGE}/neoforge-{NEOFORGE}-server.jar',
             libs / f'net/neoforged/neoforge/{NEOFORGE}/neoforge-{NEOFORGE}-universal.jar']
    rest = sorted(p for p in libs.rglob('*.jar') if p not in first and 'slim' not in p.name
                  and 'extra' not in p.name and not p.name.endswith('-unpacked.jar'))
    return os.pathsep.join(map(str, first + rest))


def build(home, source=SOURCE, name='horizon-oracle.jar', cp=None):
    """A dev-only mod jar (by default the exporter), rebuilt when its sources change."""
    out = LOCAL / 'build'
    jar = out / name
    sources = sorted((source / 'java').rglob('*.java'))
    resources = sorted(p for p in (source / 'resources').rglob('*') if p.is_file())
    newest = max(p.stat().st_mtime for p in sources + resources)
    if jar.exists() and jar.stat().st_mtime > newest:
        return jar
    classes = out / f'classes-{Path(name).stem}'
    shutil.rmtree(classes, ignore_errors=True)
    classes.mkdir(parents=True)
    javac = subprocess.run(['javac', '--release', '21', '-proc:none', '-nowarn', '-cp', cp or classpath(home),
                            '-d', str(classes), *map(str, sources)], capture_output=True, text=True)
    if javac.returncode:
        sys.exit(f'{name} does not compile:\n{javac.stdout}{javac.stderr}')
    with zipfile.ZipFile(jar, 'w', zipfile.ZIP_DEFLATED) as z:
        for p in sorted(classes.rglob('*.class')):
            z.write(p, p.relative_to(classes).as_posix())
        for p in resources:
            z.write(p, p.relative_to(source / 'resources').as_posix())
    return jar


SERVER_PROPERTIES = """level-type=minecraft\\:flat
online-mode=false
server-port=0
spawn-protection=0
view-distance=2
simulation-distance=2
generate-structures=false
spawn-monsters=false
spawn-animals=false
"""


def link(path, target, directory=False):
    """A symbolic link to `target`. Windows makes those only in Developer Mode, so there a directory gets a
    junction and a file a copy."""
    if os.name != 'nt':
        path.symlink_to(target, target_is_directory=directory)
    elif directory:
        import _winapi
        _winapi.CreateJunction(str(target), str(path))
    else:
        shutil.copyfile(target, path)


def remove_run(run):
    """Deletes a run folder; its libraries/ link or junction goes first, so the install is never touched."""
    libraries = run / 'libraries'
    if libraries.is_symlink():
        libraries.unlink()
    elif libraries.is_junction():
        os.rmdir(libraries)
    shutil.rmtree(run, ignore_errors=True)


def instance(name, jars, extra=()):
    """A server folder with `jars` (and the exporter) in mods/."""
    home = install()
    run = LOCAL / 'runs' / name
    remove_run(run)
    (run / 'mods').mkdir(parents=True)
    link(run / 'libraries', home / 'libraries', directory=True)
    (run / 'eula.txt').write_text('eula=true\n')
    (run / 'server.properties').write_text(SERVER_PROPERTIES)
    for jar in [*jars, *extra]:
        link(run / 'mods' / Path(jar).name, Path(jar).resolve())
    return home, run


def start(home, run, props, log_name='server.log', timeout=900):
    """Runs the server to its end; returns (seconds, the log)."""
    args = home / f'libraries/net/neoforged/neoforge/{NEOFORGE}/{ARGS}'
    extra = os.environ.get('HORIZON_ORACLE_JVM', '').split()
    command = [JAVA, f'-Xmx{XMX}', *extra, *[f'-D{k}={v}' for k, v in props.items()], f'@{args}', 'nogui']
    began = time.monotonic()
    with (run / log_name).open('w') as log:
        subprocess.run(command, cwd=run, stdin=subprocess.DEVNULL, stdout=log,
                       stderr=subprocess.STDOUT, timeout=timeout)
    return time.monotonic() - began, (run / log_name).read_text(errors='replace')


def mod_ids(jar):
    """The mod ids a jar declares (its neoforge.mods.toml)."""
    import tomllib
    with zipfile.ZipFile(jar) as z:
        for name in ('META-INF/neoforge.mods.toml', 'META-INF/mods.toml'):
            if name in z.namelist():
                try:
                    toml = tomllib.loads(z.read(name).decode('utf-8', 'replace'))
                except tomllib.TOMLDecodeError:
                    return []
                return [m.get('modId', '') for m in toml.get('mods', [])]
    return []


def export(args):
    jars = sorted(Path(args.jars).glob('*.jar'))
    deps = sorted(Path(args.deps).glob('*.jar')) if args.deps else []
    namespaces = args.namespaces or ','.join(i for j in jars for i in mod_ids(j))
    home = install()
    exporter = build(home)
    home, run = instance('export', jars, [*deps, exporter])
    out = Path(args.out).resolve()
    out.parent.mkdir(parents=True, exist_ok=True)
    out.unlink(missing_ok=True)
    seconds, log = start(home, run, {'horizon.oracle.out': out, 'horizon.oracle.namespaces': namespaces})
    if not out.exists():
        tail = '\n'.join(log.splitlines()[-40:])
        sys.exit(f'the oracle wrote nothing; the end of {run / "server.log"}:\n{tail}')
    facts = json.loads(out.read_text(encoding='utf-8'))
    mods = sum(1 for b in facts['blocks'] if not b.startswith('minecraft:'))
    skipped = facts.get('skipped', [])
    print(f'{out}: {mods} mod blocks, {len(facts["items"])} items, {len(facts["recipes"])} recipes '
          f'({namespaces}); {len(skipped)} left out because their facts threw; server ran {seconds:.1f} s')


# What a server folder takes from a modpack instance, besides mods/.
PACK_FOLDERS = ('config', 'defaultconfigs', 'kubejs', 'datapacks', 'moonlight-global-datapacks', 'local', 'moddata')


def pack(args):
    instance_dir, run, dump = Path(args.instance), Path(args.run).resolve(), Path(args.out).resolve()
    leave_out = set()
    if args.leave_out:
        for text in Path(args.leave_out).read_text(encoding='utf-8').splitlines():
            text = text.split('#')[0].strip()
            if text:
                leave_out.add(text)
    home = install()
    exporter = build(home)
    remove_run(run)
    (run / 'mods').mkdir(parents=True)
    link(run / 'libraries', home / 'libraries', directory=True)
    (run / 'eula.txt').write_text('eula=true\n')
    # The dump runs before the first tick, over the watchdog's minute.
    (run / 'server.properties').write_text(SERVER_PROPERTIES + 'max-tick-time=-1\n')
    used = left = 0
    for jar in sorted((instance_dir / 'mods').glob('*.jar')):
        ids = mod_ids(jar)
        if jar.name in leave_out or any(i in leave_out for i in ids):
            left += 1
            continue
        # Copied, not linked: the instance is only read, and nothing may write through a link into it.
        shutil.copyfile(jar, run / 'mods' / jar.name)
        used += 1
    shutil.copyfile(exporter, run / 'mods' / exporter.name)
    for folder in PACK_FOLDERS:
        if (instance_dir / folder).is_dir():
            shutil.copytree(instance_dir / folder, run / folder)
    dump.mkdir(parents=True, exist_ok=True)
    out = dump / 'facts.json'
    for old in (out, dump / 'summary.json'):
        old.unlink(missing_ok=True)
    print(f'{used} mods, {left} left out; server starting with -Xmx{XMX}', flush=True)
    props = {'horizon.oracle.out': out, 'horizon.oracle.dump': dump, 'horizon.oracle.namespaces': args.namespaces or ''}
    if args.batches:
        props['horizon.oracle.batches'] = Path(args.batches).resolve()
        for old in dump.glob('facts-*.json'):
            old.unlink()
    seconds, log = start(home, run, props, timeout=args.timeout)
    if not (dump / 'summary.json').exists():
        tail = '\n'.join(log.splitlines()[-40:])
        sys.exit(f'the oracle wrote no summary; the end of {run / "server.log"}:\n{tail}')
    print(f'{dump}: server ran {seconds:.1f} s: {(dump / "summary.json").read_text(encoding="utf-8")[:400]}')


CLIENT_SOURCE = ROOT / 'mods' / 'horizon' / 'src' / 'neoforge-oracle-client'
# Mods a client oracle never loads, whatever --leave-out says: Crash Assistant starts a process of its own with a
# window (its integrated GPU warning) that a hidden game can't hide, and that outlives the game.
CLIENT_NEVER = {'crash_assistant'}


def natives_of(jars, target):
    """The native libraries of the client's natives jars, as the launcher extracts them."""
    target.mkdir(parents=True, exist_ok=True)
    for jar in jars:
        if 'natives-' not in jar.name:
            continue
        with zipfile.ZipFile(jar) as archive:
            for name in archive.namelist():
                if name.endswith(('.so', '.dylib', '.jnilib', '.dll')) and not (target / Path(name).name).exists():
                    (target / Path(name).name).write_bytes(archive.read(name))


def no_early_window(fml_toml):
    """FML's early loading window off (`earlyWindowControl`), so the game's own, hidden window is the only one."""
    text = fml_toml.read_text(encoding='utf-8') if fml_toml.exists() else ''
    lines = [line for line in text.splitlines() if not line.strip().startswith('earlyWindowControl')]
    fml_toml.parent.mkdir(parents=True, exist_ok=True)
    fml_toml.write_text('\n'.join(['earlyWindowControl = false', *lines]) + '\n', encoding='utf-8')


def stop_strays(run):
    """Ends any Java process a mod of the run started that outlived the game (one whose command line names the run
    folder), so nothing of the oracle stays on a shared machine."""
    if os.name != 'nt':
        return
    folder = str(run).replace("'", "''")
    script = (f"Get-CimInstance Win32_Process -Filter \"Name='java.exe' OR Name='javaw.exe'\" | "
              f"Where-Object {{ $_.CommandLine -like '*{folder}*' }} | "
              f"ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force; $_.ProcessId }}")
    stopped = subprocess.run(['powershell', '-NoProfile', '-Command', script], capture_output=True, text=True).stdout.split()
    if stopped:
        print(f'stopped {len(stopped)} Java process(es) the run left behind: {", ".join(stopped)}')


def client_pack(args):
    instance_dir, run, out = Path(args.instance), Path(args.run).resolve(), Path(args.out).resolve()
    leave_out = set(CLIENT_NEVER)
    if args.leave_out:
        for text in Path(args.leave_out).read_text(encoding='utf-8').splitlines():
            text = text.split('#')[0].strip()
            if text:
                leave_out.add(text)
    home, _, _, jars = client()
    exporter = build(home, CLIENT_SOURCE, 'horizon-oracle-client.jar', client_classpath(home))
    shutil.rmtree(run, ignore_errors=True)
    (run / 'mods').mkdir(parents=True)
    used = left = 0
    for jar in sorted((instance_dir / 'mods').glob('*.jar')):
        if jar.name in leave_out or any(i in leave_out for i in mod_ids(jar)):
            left += 1
            continue
        # Copied, not linked: the instance is only read.
        shutil.copyfile(jar, run / 'mods' / jar.name)
        used += 1
    shutil.copyfile(exporter, run / 'mods' / exporter.name)
    for folder in PACK_FOLDERS:
        if (instance_dir / folder).is_dir():
            shutil.copytree(instance_dir / folder, run / folder)
    no_early_window(run / 'config' / 'fml.toml')
    natives, assets = run / 'natives', run / 'assets'
    natives_of(jars, natives)
    (assets / 'indexes').mkdir(parents=True, exist_ok=True)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.unlink(missing_ok=True)
    extra = os.environ.get('HORIZON_ORACLE_JVM', '').split()
    _, command = client_command(JAVA, XMX, run, natives, assets,
                                jvm=[*extra, f'-Dhorizon.oracle.client.out={out}'],
                                game=['--width', '854', '--height', '480'])
    print(f'{used} mods, {left} left out; client starting with -Xmx{XMX}', flush=True)
    began = time.monotonic()
    env = dict(os.environ, ALSOFT_DRIVERS='null')
    with (run / 'client.log').open('w') as log:
        try:
            subprocess.run(command, cwd=run, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                           env=env, timeout=args.timeout)
        except subprocess.TimeoutExpired:
            print(f'the client ran past {args.timeout} s and was stopped')
    stop_strays(run)
    seconds = time.monotonic() - began
    if not out.exists():
        tail = '\n'.join((run / 'client.log').read_text(errors='replace').splitlines()[-40:])
        sys.exit(f'the client oracle wrote nothing; the end of {run / "client.log"}:\n{tail}')
    facts = json.loads(out.read_text(encoding='utf-8'))
    print(f'{out}: {len(facts["fluid_types"])} fluid types, {len(facts["fluids"])} fluids, {len(facts["items"])} '
          f'buckets; client ran {seconds:.1f} s')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='command', required=True)
    e = sub.add_parser('export', help='write the facts of the mods in --jars')
    e.add_argument('--jars', required=True, help='the mods to export')
    e.add_argument('--deps', help='their required libraries (loaded, not exported)')
    e.add_argument('--namespaces', help='comma-separated; default: the jars\' mod ids')
    e.add_argument('--out', required=True)
    p = sub.add_parser('pack', help='write the final recipes, tags and loot tables of a whole modpack instance')
    p.add_argument('--instance', required=True, help='the modpack folder (mods/, config/, kubejs/...); only read')
    p.add_argument('--run', required=True, help='the server folder to make; deleted first')
    p.add_argument('--out', required=True, help='the folder for recipes.jsonl, tags.jsonl, loot_tables.jsonl...')
    p.add_argument('--leave-out', help='a file of jar names or mod ids not to load (client-only), one a line')
    p.add_argument('--namespaces', help='comma-separated: also write these namespaces\' facts to <out>/facts.json')
    p.add_argument('--batches', help='a file of `<name> <namespace>,...` lines: facts of each into <out>/facts-<name>.json')
    p.add_argument('--timeout', type=int, default=1500, help='seconds before the server is stopped')
    c = sub.add_parser('client-pack', help="write a whole modpack's client facts of its fluids")
    c.add_argument('--instance', required=True, help='the modpack folder; only read')
    c.add_argument('--run', required=True, help='the game folder to make; deleted first')
    c.add_argument('--out', required=True, help='the JSON file to write')
    c.add_argument('--leave-out', help='a file of jar names or mod ids not to load, one a line')
    c.add_argument('--timeout', type=int, default=1500, help='seconds before the client is stopped')
    args = parser.parse_args()
    {'export': export, 'pack': pack, 'client-pack': client_pack}[args.command](args)


if __name__ == '__main__':
    main()
