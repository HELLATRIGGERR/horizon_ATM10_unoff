#!/usr/bin/env python3
"""What ATM10's KubeJS does, by kind, and whether the port has each effect today. Reads the instance's `kubejs/`
scripts for their effects only (calls are counted and the ids they name are checked; no script text is copied or
written), the pack oracle's dump (the game with KubeJS on: final recipes, tags, loot tables, registries), the carries
`final-data` wrote, the native content folders beside them and the game's 26.3 resources. Runs no JavaScript.

    python tools/atm10/kubejs-audit.py --instance <ATM10> --dump <dump> --carries <carries> \
        --resources <26.3 resources> --also <content/ae2> --also <addon content root> --out kubejs-audit.json

Kinds, and how "applied" is decided:
- recipes (`ServerEvents.recipes`): removals, additions by type, replacements. A removal named by id is applied when
  the dump lacks the id and no carry, native content folder or vanilla file holds it; an addition named by id when the
  dump and a carry hold it. Every jar recipe the dump lacks is a removal of the pack's (KubeJS's, a condition's or
  Almost Unified's), applied unless a vanilla or native file still stands for it; a vanilla or native recipe the pack
  changed (other ids in it) is applied only if the game reads the dump's form.
- tags (`ServerEvents.tags`, and `kubejs/data/*/tags`): each literal add or remove, against the dump and against the
  union of the carries', native folders' and vanilla's tag files (as the game merges them).
- registrations (`StartupEvents.registry`): KubeJS's items, blocks, fluids in the oracle's registries against the pack
  mod's declarations, with a model and texture for each item; other registries' entries (chemicals, MI machines).
- the data pack (`kubejs/data`) and data generators (`ServerEvents.generateData`): recipes, tags and loot tables reach
  the game through the dump; every other file is applied when its owner carry holds the same JSON at its path.
- assets (`kubejs/assets`): applied when a carry holds the same bytes at the path.
- event handlers (players, items, blocks, levels, network, commands, NeoForge events) and client-only scripts (recipe
  viewer hiding, tooltips, ponder scenes): counted; nothing applies them.
"""
import argparse, json, os, re, sys, zipfile
from collections import Counter, defaultdict

STRING = r'''("(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'|`(?:\\.|[^`\\])*`)'''
COMMENT = re.compile(STRING + r'|//[^\n]*|/\*.*?\*/', re.S)
EVENT = re.compile(r'\b([A-Z]\w*Events)\.(\w+)\(\s*(?:(["\'])([^"\']+)\3)?')
LIT = re.compile(r'''(["'])([^"'`$\n]+)\1''')
VANILLA_CALLS = {'shaped': 'crafting_shaped', 'shapeless': 'crafting_shapeless', 'smelting': 'smelting',
                 'blasting': 'blasting', 'smoking': 'smoking', 'campfireCooking': 'campfire_cooking',
                 'stonecutting': 'stonecutting', 'smithing': 'smithing_transform',
                 'smithingTransform': 'smithing_transform', 'smithingTrim': 'smithing_trim'}
TAG_DIR = {'item': 'item', 'block': 'block', 'fluid': 'fluid', 'entity_type': 'entity_type',
           'enchantment': 'enchantment', 'mob_effect': 'mob_effect', 'block_entity_type': 'block_entity_type',
           'worldgen/biome': 'worldgen/biome', 'worldgen/structure': 'worldgen/structure'}
HANDLERS = ('PlayerEvents', 'ItemEvents', 'BlockEvents', 'LevelEvents', 'NativeEvents', 'NetworkEvents',
            'EntityEvents', 'ClientEvents')
DUMP_KINDS = ('recipe', 'tags', 'loot_table')


def snake(name):
    return re.sub(r'(?<!^)([A-Z])', r'_\1', name).lower()


def strip_comments(text):
    return COMMENT.sub(lambda m: m.group(1) or '', text)


def args_of(text, start):
    """The text inside the call's parentheses that open at `start`."""
    depth, i = 0, start
    while i < len(text):
        c = text[i]
        if c in '"\'`':
            m = re.compile(STRING).match(text, i)
            i = m.end() if m else i + 1
            continue
        if c in '([{':
            depth += 1
        elif c in ')]}':
            depth -= 1
            if depth == 0:
                return text[start + 1:i]
        i += 1
    return text[start + 1:]


def calls(text, name):
    for m in re.finditer(r'\.' + name + r'\(', text):
        yield m, args_of(text, m.end() - 1)


def scripts(kubejs):
    for side in ('startup_scripts', 'server_scripts', 'client_scripts'):
        for root, _, files in os.walk(os.path.join(kubejs, side)):
            for f in sorted(files):
                if f.endswith('.js'):
                    path = os.path.join(root, f)
                    with open(path, encoding='utf-8', errors='replace') as fh:
                        yield side, os.path.relpath(path, kubejs).replace('\\', '/'), strip_comments(fh.read())


def blocks(text):
    """Each event's text: from its call to the next event's."""
    found = list(EVENT.finditer(text))
    for i, m in enumerate(found):
        end = found[i + 1].start() if i + 1 < len(found) else len(text)
        yield m.group(1), m.group(2), m.group(4), text[m.start():end]


def walk(root):
    for base, _, files in os.walk(root):
        for f in files:
            full = os.path.join(base, f)
            yield os.path.relpath(full, root).replace('\\', '/'), full


def read_json(path):
    try:
        with open(path, 'rb') as fh:
            return json.loads(fh.read())
    except (OSError, ValueError):
        return None


def jsonl(path):
    with open(path, encoding='utf-8') as fh:
        for line in fh:
            if line.strip():
                yield json.loads(line)


def recipe_ids(root, out):
    """`data/<ns>/recipe/<path>.json` under a content folder; the pack mod's `atm10_pack:<ns>/<path>` as `<ns>:<path>`."""
    data = os.path.join(root, 'data')
    if not os.path.isdir(data):
        return
    for ns in os.listdir(data):
        rdir = os.path.join(data, ns, 'recipe')
        for rel, _ in walk(rdir):
            if not rel.endswith('.json'):
                continue
            path = rel[:-5]
            if ns == 'atm10_pack' and '/' in path:
                mod, rest = path.split('/', 1)
                out.add(f'{mod}:{rest}')
            else:
                out.add(f'{ns}:{path}')


def tag_files(root, out):
    data = os.path.join(root, 'data')
    if not os.path.isdir(data):
        return
    for ns in os.listdir(data):
        for rel, full in walk(os.path.join(data, ns, 'tags')):
            if not rel.endswith('.json'):
                continue
            parts = rel[:-5].split('/')
            # tags/<registry dir, one or two parts>/<path>: a registry dir of two parts starts with worldgen or a mod's.
            n = 2 if parts[0] == 'worldgen' or parts[0] not in TAG_DIR.values() and len(parts) > 2 else 1
            reg, path = '/'.join(parts[:n]), '/'.join(parts[n:])
            v = read_json(full)
            if not isinstance(v, dict):
                continue
            entry = out.setdefault((reg, f'{ns}:{path}'), set())
            if v.get('replace'):
                entry.clear()
            for x in v.get('values', []):
                x = x.get('id') if isinstance(x, dict) else x
                if isinstance(x, str):
                    entry.add(x)


def resolve(tags, reg, tag, seen=None):
    seen = seen or set()
    if (reg, tag) in seen:
        return set()
    seen.add((reg, tag))
    out = set()
    for v in tags.get((reg, tag), ()):
        out |= resolve(tags, reg, v[1:], seen) if v.startswith('#') else {v}
    return out


def ids_in(v, out):
    if isinstance(v, dict):
        for k, x in v.items():
            if k != 'type':
                ids_in(x, out)
    elif isinstance(v, list):
        for x in v:
            ids_in(x, out)
    elif isinstance(v, str) and re.fullmatch(r'#?[a-z0-9_.-]+:[a-z0-9_./-]+', v):
        out.add(v.lstrip('#'))
    return out


def numbers(v):
    """The numbers in a JSON value, sorted."""
    if isinstance(v, dict):
        return sorted(n for x in v.values() for n in numbers(x))
    if isinstance(v, list):
        return sorted(n for x in v for n in numbers(x))
    return [float(v)] if isinstance(v, (int, float)) and not isinstance(v, bool) else []


def owners(carries):
    """Namespace -> carry folder, from each carry's manifest (`id`, `namespaces`)."""
    out = {}
    for name in os.listdir(carries):
        toml = os.path.join(carries, name, 'horizon-mod.toml')
        if not os.path.exists(toml):
            continue
        with open(toml, encoding='utf-8') as fh:
            text = fh.read()
        mid = re.search(r'^id\s*=\s*"([^"]+)"', text, re.M)
        spaces = re.search(r'^namespaces\s*=\s*\[([^\]]*)\]', text, re.M)
        for ns in ([mid.group(1)] if mid else []) + (re.findall(r'"([^"]+)"', spaces.group(1)) if spaces else []):
            out.setdefault(ns, os.path.join(carries, name))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--instance', required=True)
    ap.add_argument('--dump', required=True)
    ap.add_argument('--carries', required=True)
    ap.add_argument('--resources', required=True)
    ap.add_argument('--also', action='append', default=[], help='a native content folder, or a folder of them')
    ap.add_argument('--vanilla-jar', help="Minecraft 1.21.1's jar (default: the instance's install)")
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    kubejs = os.path.join(a.instance, 'kubejs')
    report = {}

    # --- the scripts, by kind
    count = Counter()
    rec_types, removal_keys = Counter(), Counter()
    removed_ids, named_adds = set(), set()
    tag_ops = []  # (registry, op, tag, value)
    tag_events, registrations = Counter(), Counter()
    handlers, generators, client = Counter(), Counter(), Counter()
    gen_paths = []
    handler_files = defaultdict(set)
    other_startup = Counter()
    for side, rel, text in scripts(kubejs):
        count['files ' + side] += 1
        recipe_text = []
        for group, name, arg, body in blocks(text):
            key = f'{group}.{name}'
            if key == 'ServerEvents.recipes':
                recipe_text.append(body)
            elif key == 'ServerEvents.tags':
                reg = arg or '?'
                tag_events[reg] += 1
                for op in ('add', 'remove', 'removeAll', 'removeAllTagsFrom'):
                    for m, inner in calls(body, op):
                        count[f'tag {op}'] += 1
                        first = LIT.match(inner.strip())
                        if op in ('add', 'remove') and first:
                            rest = inner.strip()[first.end():]
                            for v in LIT.finditer(rest):
                                tag_ops.append((reg, op, first.group(2), v.group(2)))
            elif key == 'StartupEvents.registry':
                for m, inner in calls(body, 'create'):
                    registrations[arg] += 1
            elif key == 'ServerEvents.generateData':
                generators[rel] += 1
                for m, inner in calls(body, 'json'):
                    first = re.match(r'\s*(["\'`])([^"\'`]+)\1', inner)
                    path = first.group(2) if first else '?'
                    gen_paths.append((rel, path))
            elif group == 'RecipeViewerEvents':
                for op in ('remove', 'add'):
                    client[f'{key} .{op}'] += sum(1 for _ in calls(body, op))
            elif key == 'ItemEvents.modifyTooltips':
                client[key + ' .add'] += sum(1 for _ in calls(body, 'add'))
            elif group in ('MIMachineEvents', 'MIRegistrationEvents', 'MMREvents') or key in (
                    'StartupEvents.postInit', 'StartupEvents.modifyCreativeTab', 'ItemEvents.modification'):
                other_startup[key] += 1
            elif group in HANDLERS or key in ('ServerEvents.loaded', 'ServerEvents.basicPublicCommand'):
                handlers[key] += 1
                handler_files[key].add(rel)
        if '/ponder/' in rel:
            client['ponder scenes'] += len(re.findall(r'\.scene\(', text))
        joined = '\n'.join(recipe_text)
        if not joined:
            continue
        typed = re.compile(r'\.recipes\.(\w+)\.(\w+)\(')
        for m in typed.finditer(joined):
            rec_types[f'{m.group(1)}:{snake(m.group(2))}'] += 1
        rest = typed.sub('.x(', joined)
        for call, kind in VANILLA_CALLS.items():
            rec_types['minecraft:' + kind] += sum(1 for _ in calls(rest, call))
        for m, inner in calls(rest, 'custom'):
            t = re.search(r'type\s*:\s*(["\'])([^"\']+)\1', inner)
            rec_types[t.group(2) if t else '(custom, type not literal)'] += 1
        for m, inner in calls(joined, 'remove'):
            count['recipe removals'] += 1
            keys = set(re.findall(r'\b(id|output|input|mod|type|not)\s*:', inner)) or {'(not literal)'}
            for k in keys:
                removal_keys[k] += 1
            for idm in re.finditer(r'\bid\s*:\s*(\[[^\]]*\]|(["\'])[^"\']+\2)', inner):
                removed_ids.update(v.group(2) for v in LIT.finditer(idm.group(1)))
        count['recipe replaceInput'] += sum(1 for _ in calls(joined, 'replaceInput'))
        count['recipe replaceOutput'] += sum(1 for _ in calls(joined, 'replaceOutput'))
        for m, inner in calls(joined, 'id'):
            v = LIT.fullmatch(inner.strip())
            if v and ':' in v.group(2):
                named_adds.add(v.group(2))
    removed_ids = {i for i in removed_ids if ':' in i and '*' not in i}

    # --- the dump: the game on NeoForge with KubeJS on
    dump_recipes = {}
    for row in jsonl(os.path.join(a.dump, 'recipes.jsonl')):
        dump_recipes[row['id']] = row.get('json')
    dump_tags = {}
    for row in jsonl(os.path.join(a.dump, 'tags.jsonl')):
        dump_tags[(row['registry'], row['id'])] = set(row.get('values') or [])
    dump_loot = {row['id'] for row in jsonl(os.path.join(a.dump, 'loot_tables.jsonl'))}
    registries = read_json(os.path.join(a.dump, 'registries.json')) or {}

    # --- what the port loads: carries, native content, vanilla
    also = []
    for d in a.also:
        if os.path.exists(os.path.join(d, 'horizon-mod.toml')):
            also.append(d)
        elif os.path.isdir(d):
            also += [os.path.join(d, n) for n in os.listdir(d) if os.path.exists(os.path.join(d, n, 'horizon-mod.toml'))]
    carried, native, vanilla = set(), set(), set()
    carry_dirs = [os.path.join(a.carries, n) for n in os.listdir(a.carries)
                  if os.path.exists(os.path.join(a.carries, n, 'horizon-mod.toml'))]
    for d in carry_dirs:
        recipe_ids(d, carried)
    parked = set()
    outside = a.carries.rstrip('/\\') + '-outside'
    for rel, _ in walk(os.path.join(outside, 'recipe')):
        if rel.endswith('.json') and '/' in rel:
            ns, path = rel[:-5].split('/', 1)
            parked.add(f'{ns}:{path}')
    for d in also:
        recipe_ids(d, native)
    recipe_ids(a.resources, vanilla)
    port_tags = {}
    for d in carry_dirs + also + [a.resources]:
        tag_files(d, port_tags)

    # Jar recipes: every recipe file of the instance's jars and of vanilla 1.21.1.
    vjar = a.vanilla_jar or os.path.join(os.path.dirname(os.path.dirname(a.instance.rstrip('/\\'))),
                                         'Install', 'versions', '1.21.1', '1.21.1.jar')
    jars = [os.path.join(a.instance, 'mods', j) for j in os.listdir(os.path.join(a.instance, 'mods')) if j.endswith('.jar')]
    jar_recipes = {}
    for jar in jars + ([vjar] if os.path.exists(vjar) else []):
        try:
            with zipfile.ZipFile(jar) as z:
                for n in z.namelist():
                    m = re.fullmatch(r'data/([^/]+)/recipes?/(.+)\.json', n)
                    if m:
                        jar_recipes.setdefault(f'{m.group(1)}:{m.group(2)}', (jar, n))
        except (zipfile.BadZipFile, OSError):
            pass
    report['vanilla_jar'] = os.path.exists(vjar)

    # --- recipes
    stands = native | vanilla
    gone = set(jar_recipes) - set(dump_recipes)
    gone_standing = sorted(gone & stands)
    gone_carried = sorted(gone & carried)
    changed_standing = []
    for rid in sorted(set(dump_recipes) & stands & set(jar_recipes)):
        jar, name = jar_recipes[rid]
        try:
            with zipfile.ZipFile(jar) as z:
                original = json.loads(z.read(name))
        except (OSError, ValueError, KeyError):
            continue
        if dump_recipes[rid] is not None and ids_in(original, set()) != ids_in(dump_recipes[rid], set()):
            changed_standing.append(rid)
    rem_bad = sorted(i for i in removed_ids if i in dump_recipes or i in carried or i in stands)
    rem_standing = sorted(i for i in removed_ids if i in stands)
    add_missing = sorted(i for i in named_adds if i not in dump_recipes)
    add_in_port = {i for i in named_adds if i in carried or i in parked}
    kubejs_ns = [i for i in dump_recipes if i.startswith('kubejs:')]
    report['recipes'] = {
        'calls': {'removals': count['recipe removals'], 'removal filters': dict(removal_keys),
                  'replaceInput': count['recipe replaceInput'], 'replaceOutput': count['recipe replaceOutput'],
                  'additions by type': dict(rec_types.most_common()), 'additions': sum(rec_types.values()),
                  'named additions': len(named_adds)},
        'removals by id': len(removed_ids), 'removals by id still somewhere': rem_bad,
        'removals by id that a vanilla or native file still holds': rem_standing,
        'jar recipes the dump lacks': len(gone), 'of them still in a carry': gone_carried[:50],
        'of them a vanilla or native file still holds': len(gone_standing),
        'sample still held': gone_standing[:40],
        'vanilla or native recipes the pack changed': len(changed_standing), 'sample changed': changed_standing[:40],
        'named additions not in the dump': add_missing, 'named additions in the carries': len(add_in_port),
        'kubejs-namespace recipes in the dump': len(kubejs_ns),
        'kubejs-namespace recipes in the carries': sum(1 for i in kubejs_ns if i in carried),
        'kubejs-namespace recipes parked (result not ours)': sum(1 for i in kubejs_ns if i in parked),
    }

    # --- tags
    tag_rows = Counter()
    tag_misses = []
    for reg, op, tag, value in tag_ops:
        rdir = TAG_DIR.get(reg, reg)
        dreg = 'minecraft:' + rdir if ':' not in rdir else rdir
        members = resolve(port_tags, rdir, value[1:]) | dump_tags.get((dreg, value[1:]), set()) \
            if value.startswith('#') else {value}
        if not members:
            tag_rows[(reg, op, 'empty')] += 1
            continue
        in_dump = dump_tags.get((dreg, tag), set())
        in_port = resolve(port_tags, rdir, tag)
        want = (lambda s: members <= s) if op == 'add' else (lambda s: not (members & s))
        java_ok, port_ok = want(in_dump), want(in_port)
        tag_rows[(reg, op, 'applied' if port_ok else 'not applied')] += 1
        if java_ok and not port_ok:
            tag_misses.append(f'{reg} {op} {tag} {value}')
    data_tag_files = Counter()
    for rel, full in walk(os.path.join(kubejs, 'data')):
        parts = rel.split('/')
        if len(parts) > 3 and parts[1] == 'tags':
            worldgen = parts[2] == 'worldgen'
            reg = '/'.join(parts[2:4]) if worldgen else parts[2]
            tag = f"{parts[0]}:{'/'.join(parts[4 if worldgen else 3:])[:-5]}"
            v = read_json(full) or {}
            values = [x.get('id') if isinstance(x, dict) else x for x in v.get('values', [])]
            ok = all((resolve(port_tags, reg, x[1:]) <= resolve(port_tags, reg, tag)) if x.startswith('#')
                     else x in resolve(port_tags, reg, tag) for x in values if isinstance(x, str))
            data_tag_files[(reg, 'applied' if ok else 'not applied')] += 1
    report['tags'] = {'events by registry': dict(tag_events),
                      'calls': {k: count[f'tag {k}'] for k in ('add', 'remove', 'removeAll', 'removeAllTagsFrom')},
                      'literal edits': {' '.join(k): n for k, n in sorted(tag_rows.items())},
                      'edits Java has and the port lacks': tag_misses[:60],
                      'kubejs/data tag files': {' '.join(k): n for k, n in sorted(data_tag_files.items())}}

    # --- registrations
    facts = read_json(os.path.join(a.dump, 'facts.json')) or {}
    pack = os.path.join(a.carries, 'atm10-pack')
    decl = os.path.join(pack, 'data', 'kubejs', 'horizon')
    reg_rows = {}
    for kind, key in (('item', 'items'), ('block', 'blocks'), ('fluid', 'fluids')):
        ids = sorted(i for i in facts.get(key, {}) if i.startswith('kubejs:'))
        reg_rows[kind] = {'registered (oracle)': len(ids)}
    items = sorted(i for i in facts.get('items', {}) if i.startswith('kubejs:'))
    blocks_ = sorted(i for i in facts.get('blocks', {}) if i.startswith('kubejs:'))
    fluids = sorted(i for i in facts.get('fluids', {}) if i.startswith('kubejs:'))
    block_items = {b for b in blocks_ if facts['blocks'][b].get('item') == b}
    def declared(kind, i):
        return os.path.exists(os.path.join(decl, kind, i.split(':', 1)[1] + '.json'))
    reg_rows['block']['declared'] = sum(declared('block', b) for b in blocks_)
    reg_rows['item']['declared'] = sum(declared('item', i) or (i in block_items and declared('block', i)) for i in items)
    reg_rows['fluid']['declared (sources)'] = sum(declared('fluid', f) for f in fluids)
    # Each item: a definition, its model, the textures the model names.
    asset_roots = [os.path.join(d, 'assets') for d in carry_dirs + also] + [os.path.join(a.resources, 'assets')]
    def asset(ns, rel):
        return next((os.path.join(r, ns, rel) for r in asset_roots if os.path.exists(os.path.join(r, ns, rel))), None)
    no_texture = []
    for i in items:
        ns, path = i.split(':', 1)
        d = read_json(asset(ns, f'items/{path}.json') or '') or {}
        model = (d.get('model') or {}).get('model', '')
        seen, textures, todo = set(), [], [model]
        while todo:
            m = todo.pop()
            if not m or m in seen or ':' not in m:
                continue
            seen.add(m)
            mns, mpath = m.split(':', 1)
            mv = read_json(asset(mns, f'models/{mpath}.json') or '') or {}
            textures += [t for t in (mv.get('textures') or {}).values() if isinstance(t, str) and not t.startswith('#')]
            todo.append(mv.get('parent', ''))
        found = [t for t in textures if asset(t.split(':', 1)[0] if ':' in t else 'minecraft',
                                               f"textures/{t.split(':', 1)[-1]}.png")]
        if not found:
            no_texture.append(i)
    reg_rows['item']['with no texture found'] = no_texture
    others = Counter()
    for regname, ids in registries.items():
        if regname in ('minecraft:item', 'minecraft:block', 'minecraft:fluid'):
            continue
        for i in ids:
            if i.startswith('kubejs:'):
                others[regname] += 1
    chem_decl = 0
    for d in carry_dirs + also:
        for rel, full in walk(os.path.join(d, 'data')):
            if '/horizon/chemical/' in rel and rel.startswith('kubejs/'):
                chem_decl += 1
    mi_ids = set()
    for side, rel, text in scripts(kubejs):
        if 'MIMachineEvents' in text or 'MIRegistrationEvents' in text:
            mi_ids |= {v.group(2) for v in LIT.finditer(text) if re.fullmatch(r'[a-z0-9_]+', v.group(2))}
    bes = [i for i in registries.get('minecraft:block_entity_type', []) if i.split(':', 1)[1] in mi_ids
           and i.split(':', 1)[0] in ('modern_industrialization', 'extended_industrialization')]
    mi_declared = [b for b in bes if os.path.exists(os.path.join(
        owners(a.carries).get(b.split(':')[0], pack), 'data', b.split(':')[0], 'horizon', 'block', b.split(':')[1] + '.json'))]
    report['registrations'] = {'script calls by registry': dict(registrations), 'kubejs content': reg_rows,
                               'other registries, kubejs ids (oracle)': dict(others),
                               'kubejs chemicals declared': chem_decl,
                               'MI machines and hatches from scripts (oracle block entities)': len(bes),
                               'of them declared as blocks': len(mi_declared), 'MI machines': bes,
                               'other startup events': dict(other_startup)}

    # --- the data pack and the data generators
    own = owners(a.carries)
    data_rows = Counter()
    data_missing = defaultdict(list)
    for rel, full in walk(os.path.join(kubejs, 'data')):
        parts = rel.split('/')
        if len(parts) < 3:
            continue
        ns, kind = parts[0], parts[1]
        if kind == 'neoforge' and len(parts) > 3:
            kind = 'neoforge/' + parts[2]
        if kind == 'worldgen' and len(parts) > 3:
            kind = 'worldgen/' + parts[2]
        path_in = '/'.join(parts[1:])
        if kind in DUMP_KINDS:
            rid = f"{ns}:{'/'.join(parts[2:])[:-5]}"
            if kind == 'recipe':
                ok = (rid in dump_recipes) == (rid in carried or rid in parked or rid in stands)
            elif kind == 'loot_table':
                ok = rid in dump_loot
            else:
                continue  # counted with the tags
            data_rows[(kind, 'via the dump' if ok else 'not applied')] += 1
            continue
        if kind == 'kubejs':
            data_rows[(kind, "KubeJS's own (schemas)")] += 1
            continue
        carry = own.get(ns)
        if carry is None:
            data_rows[(kind, 'namespace not carried')] += 1
            data_missing[kind].append(rel)
            continue
        mine = read_json(full)
        theirs = read_json(os.path.join(carry, 'data', ns, path_in))
        if isinstance(mine, dict) and isinstance(theirs, dict):
            if kind == 'data_maps' and isinstance(mine.get('values'), dict) and isinstance(theirs.get('values'), dict):
                # The game merges a data map; the pack's entries win.
                mine = theirs if all(theirs['values'].get(k) == v for k, v in mine['values'].items()) else mine
            elif kind.startswith('worldgen/') and isinstance(mine.get('config'), dict):
                # 26.3's form: `config` unwrapped, states rewritten; the same ids and numbers.
                flat = {**mine['config'], 'type': mine.get('type')}
                if ids_in(flat, set()) == ids_in(theirs, set()) and numbers(flat) == numbers(theirs):
                    mine = theirs
        if mine is not None and mine == theirs:
            data_rows[(kind, 'applied')] += 1
        else:
            data_rows[(kind, 'not applied: the jar\'s file stands' if theirs is not None else 'not applied: absent')] += 1
            data_missing[kind].append(rel)
    gen_rows = Counter()
    for rel, path in gen_paths:
        m = re.match(r'([a-z0-9_.-]+):([^/]+)/', path)
        kind = m.group(2) if m else ('(path not literal)' if '$' in path or path == '?' else path.split('/')[0])
        gen_rows[(rel, kind)] += 1
    report['data'] = {'kubejs/data files': {' '.join(k): n for k, n in sorted(data_rows.items())},
                      'not applied, by kind': {k: len(v) for k, v in sorted(data_missing.items())},
                      'generators': {f'{r} -> {k}': n for (r, k), n in sorted(gen_rows.items())}}

    # --- assets
    asset_rows = Counter()
    by_ns = defaultdict(Counter)
    for rel, full in walk(os.path.join(kubejs, 'assets')):
        ns = rel.split('/')[0]
        kind = rel.split('/')[1] if '/' in rel else '?'
        with open(full, 'rb') as fh:
            mine = fh.read()
        hit = False
        for d in carry_dirs:
            p = os.path.join(d, 'assets', rel)
            if os.path.exists(p):
                if kind == 'lang':
                    mv, tv = read_json(full) or {}, read_json(p) or {}
                    hit = all(tv.get(k) == v for k, v in mv.items())
                else:
                    with open(p, 'rb') as fh:
                        hit = fh.read() == mine
                if hit:
                    break
        state = 'applied' if hit else 'not applied'
        asset_rows[(kind, state)] += 1
        by_ns[ns][state] += 1
    report['assets'] = {'files': {' '.join(k): n for k, n in sorted(asset_rows.items())},
                        'namespaces not applied': sorted(ns for ns, c in by_ns.items() if c['not applied'])}

    report['handlers'] = {k: {'events': n, 'files': sorted(handler_files[k])} for k, n in sorted(handlers.items())}
    report['client'] = dict(client)
    report['files'] = {k: v for k, v in count.items() if k.startswith('files')}
    with open(a.out, 'w', encoding='utf-8') as fh:
        json.dump(report, fh, indent=1, sort_keys=False, default=list)

    # --- the table
    r = report['recipes']
    rows = [
        ('recipe removals (calls)', r['calls']['removals'],
         f"by id {r['removals by id']}, {len(r['removals by id still somewhere'])} still somewhere; jar recipes the dump "
         f"lacks {r['jar recipes the dump lacks']}, {len(r['of them still in a carry'])} in a carry, "
         f"{r['of them a vanilla or native file still holds']} held by vanilla/native files"),
        ('recipe additions (calls)', r['calls']['additions'],
         f"named {r['calls']['named additions']}, {len(r['named additions not in the dump'])} not in the dump; kubejs ids "
         f"in the dump {r['kubejs-namespace recipes in the dump']}, in carries {r['kubejs-namespace recipes in the carries']}, "
         f"parked {r['kubejs-namespace recipes parked (result not ours)']}"),
        ('recipe replacements (calls)', r['calls']['replaceInput'] + r['calls']['replaceOutput'],
         f"vanilla/native recipes the pack changed whose own file stands: {r['vanilla or native recipes the pack changed']}"),
        ('tag edits (calls)', sum(report['tags']['calls'].values()),
         ', '.join(f'{k} {n}' for k, n in report['tags']['literal edits'].items())),
        ('registered kubejs items/blocks/fluids', '/'.join(str(reg_rows[k]['registered (oracle)']) for k in ('item', 'block', 'fluid')),
         f"declared {reg_rows['item']['declared']}/{reg_rows['block']['declared']}/{reg_rows['fluid']['declared (sources)']} "
         f"(fluids: sources), items with no texture {len(no_texture)}"),
        ('other registrations', sum(others.values()) + len(bes),
         f"MI machines {len(bes)} ({len(mi_declared)} declared), {dict(others)}"),
        ('kubejs/data files', sum(data_rows.values()), ', '.join(f'{" ".join(k)} {n}' for k, n in sorted(data_rows.items()))),
        ('data generators (json calls)', len(gen_paths), f'{len(generators)} files'),
        ('assets', sum(asset_rows.values()), ', '.join(f'{" ".join(k)} {n}' for k, n in sorted(asset_rows.items()))),
        ('event handlers', sum(handlers.values()), ', '.join(f'{k} {n}' for k, n in sorted(handlers.items()))),
        ('client-only', sum(client.values()), ', '.join(f'{k} {n}' for k, n in sorted(client.items()))),
    ]
    for name, n, note in rows:
        print(f'{name:40} {str(n):>8}  {note}')
    print('report:', a.out)


if __name__ == '__main__':
    sys.exit(main())
