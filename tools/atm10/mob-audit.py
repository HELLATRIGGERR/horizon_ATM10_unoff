#!/usr/bin/env python3
"""The full pack's entity types, as docs/mobs.md counts them: which a vanilla mob template can stand in for, which can
spawn, which can be drawn, and what the engine does with them today. Reads the jars only (class files and data),
the pack oracle's registries and the carries; runs no Java.

    python tools/atm10/mob-audit.py --pack <work>/pack.json --dump <dump> --carries <carries> \
        --species crates/horizon-client/src/mobs/species.rs --out mob-audit.json [--declare <dir>]

- registered: `minecraft:entity_type` in the oracle's registries.json (the pack on NeoForge).
- class: the entity class, from the registration's bytecode (`EntityType.Builder.of(Ghoul::new, ...)` beside the name
  it is registered under), else by name (`GhoulEntity` for `ghoul`).
- template: the first game class of its superclass chain, when it is a mob the engine has (`species.rs`): a
  `Zombie` subclass is a zombie. A chain ending in an abstract class (`Monster`, `Animal`, `PathfinderMob`...) has
  only a `base`.
- spawns: `add_spawns` biome modifiers and mod biomes' spawners in the jars (`natural`), a `<path>_spawn_egg` item.
- render: the renderer (`<Name>Renderer`, or the class registered with its field), as a vanilla renderer, a vanilla
  model, a model of the mod's own code, GeckoLib, or none found; `texture`: a PNG the renderer names, else one under
  `textures/entity/` named for the type.
- loot: `loot_table/entities/<path>.json`, in the jar and in the carry.
- today: the carries' `data/<ns>/horizon/entity_type/` declarations (what the engine registers).
- slice1: template + the template's own vanilla renderer, or a vanilla renderer or model + a texture found.

With --declare, writes each slice-1 type's declaration as `<dir>/<carry key>/data/<ns>/horizon/entity_type/<path>.json`
(`template`, `texture`, its `attributes` are left to the oracle), for a test overlay of the carries; never into them.
"""
import argparse, json, os, re, struct, sys, zipfile
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor

ENTITY_PKG = 'net/minecraft/world/entity/'
NOT_ENTITY = ('net/minecraft/world/entity/ai/', 'net/minecraft/world/entity/schedule/',
              'net/minecraft/world/entity/player/Inventory', 'net/minecraft/world/entity/EntityType')
# 1.21.1's concrete mob classes (Mojang names) and the vanilla type each is.
VANILLA = {
    'monster/Zombie': 'zombie', 'monster/Husk': 'husk', 'monster/Drowned': 'drowned',
    'monster/ZombieVillager': 'zombie_villager', 'monster/ZombifiedPiglin': 'zombified_piglin',
    'monster/Skeleton': 'skeleton', 'monster/Stray': 'stray', 'monster/WitherSkeleton': 'wither_skeleton',
    'monster/Bogged': 'bogged', 'monster/Creeper': 'creeper', 'monster/Spider': 'spider',
    'monster/CaveSpider': 'cave_spider', 'monster/EnderMan': 'enderman', 'monster/Endermite': 'endermite',
    'monster/Silverfish': 'silverfish', 'monster/Slime': 'slime', 'monster/MagmaCube': 'magma_cube',
    'monster/Blaze': 'blaze', 'monster/Ghast': 'ghast', 'monster/Witch': 'witch', 'monster/Vindicator': 'vindicator',
    'monster/Evoker': 'evoker', 'monster/Pillager': 'pillager', 'monster/Illusioner': 'illusioner',
    'monster/Ravager': 'ravager', 'monster/Vex': 'vex', 'monster/Guardian': 'guardian',
    'monster/ElderGuardian': 'elder_guardian', 'monster/Shulker': 'shulker', 'monster/Phantom': 'phantom',
    'monster/Strider': 'strider', 'monster/Zoglin': 'zoglin', 'monster/hoglin/Hoglin': 'hoglin',
    'monster/piglin/Piglin': 'piglin', 'monster/piglin/PiglinBrute': 'piglin_brute',
    'monster/warden/Warden': 'warden', 'monster/breeze/Breeze': 'breeze', 'monster/Giant': 'giant',
    'animal/Cow': 'cow', 'animal/MushroomCow': 'mooshroom', 'animal/Pig': 'pig', 'animal/Sheep': 'sheep',
    'animal/Chicken': 'chicken', 'animal/Rabbit': 'rabbit', 'animal/Wolf': 'wolf', 'animal/Cat': 'cat',
    'animal/Ocelot': 'ocelot', 'animal/Fox': 'fox', 'animal/Panda': 'panda', 'animal/PolarBear': 'polar_bear',
    'animal/Bee': 'bee', 'animal/Parrot': 'parrot', 'animal/Turtle': 'turtle', 'animal/Dolphin': 'dolphin',
    'animal/Squid': 'squid', 'GlowSquid': 'glow_squid', 'animal/Cod': 'cod', 'animal/Salmon': 'salmon',
    'animal/Pufferfish': 'pufferfish', 'animal/TropicalFish': 'tropical_fish', 'animal/IronGolem': 'iron_golem',
    'animal/SnowGolem': 'snow_golem', 'animal/horse/Horse': 'horse', 'animal/horse/Donkey': 'donkey',
    'animal/horse/Mule': 'mule', 'animal/horse/SkeletonHorse': 'skeleton_horse',
    'animal/horse/ZombieHorse': 'zombie_horse', 'animal/horse/Llama': 'llama',
    'animal/horse/TraderLlama': 'trader_llama', 'animal/goat/Goat': 'goat', 'animal/frog/Frog': 'frog',
    'animal/frog/Tadpole': 'tadpole', 'animal/axolotl/Axolotl': 'axolotl', 'animal/camel/Camel': 'camel',
    'animal/sniffer/Sniffer': 'sniffer', 'animal/allay/Allay': 'allay', 'animal/armadillo/Armadillo': 'armadillo',
    'ambient/Bat': 'bat', 'npc/Villager': 'villager', 'npc/WanderingTrader': 'wandering_trader',
    'boss/wither/WitherBoss': 'wither', 'boss/enderdragon/EnderDragon': 'ender_dragon',
}
VANILLA = {ENTITY_PKG + k: v for k, v in VANILLA.items()}
RENDER_PKG = 'net/minecraft/client/renderer/entity/'
MODEL_PKG = 'net/minecraft/client/model/'
ABSTRACT_RENDERERS = {RENDER_PKG + n for n in (
    'EntityRenderer', 'LivingEntityRenderer', 'MobRenderer', 'HumanoidMobRenderer', 'AgeableMobRenderer',
    'AbstractZombieRenderer', 'AbstractHorseRenderer', 'IllagerRenderer', 'ThrownItemRenderer')}
ABSTRACT_MODELS = {MODEL_PKG + n for n in ('EntityModel', 'HierarchicalModel', 'ListModel', 'Model', 'AgeableListModel',
                                           'AgeableHierarchicalModel', 'ColorableAgeableListModel',
                                           'ColorableHierarchicalModel')}
GECKO = 'software/bernie/geckolib/'

# --- class files -------------------------------------------------------------------------------------------------
LEN = [1] * 256
for op in (0x10, 0x12, 0xa9, 0xbc, *range(0x15, 0x1a), *range(0x36, 0x3b)):
    LEN[op] = 2
for op in (0x11, 0x13, 0x14, 0x84, *range(0x99, 0xa9), *range(0xb2, 0xb9), 0xbb, 0xbd, 0xc0, 0xc1, 0xc6, 0xc7):
    LEN[op] = 3
LEN[0xc5] = 4
for op in (0xb9, 0xba, 0xc8, 0xc9):
    LEN[op] = 5


def parse_class(b, code=False):
    """(name, super, cp strings, cp classes, events by method) or None; events only with `code`."""
    if b[:4] != b'\xca\xfe\xba\xbe':
        return None
    n = struct.unpack_from('>H', b, 8)[0]
    cp = [None] * n
    i, p = 1, 10
    while i < n:
        t = b[p]
        if t == 1:
            ln = struct.unpack_from('>H', b, p + 1)[0]
            cp[i] = (1, b[p + 3:p + 3 + ln].decode('utf-8', 'replace'))
            p += 3 + ln
        elif t in (7, 8, 16, 19, 20):
            cp[i] = (t, struct.unpack_from('>H', b, p + 1)[0]); p += 3
        elif t == 15:
            cp[i] = (15, b[p + 1], struct.unpack_from('>H', b, p + 2)[0]); p += 4
        elif t in (3, 4):
            p += 5
        elif t in (5, 6):
            p += 9; i += 1
        elif t in (9, 10, 11, 12, 17, 18):
            cp[i] = (t,) + struct.unpack_from('>HH', b, p + 1); p += 5
        else:
            return None
        i += 1
    utf = lambda k: cp[k][1] if k and cp[k] and cp[k][0] == 1 else ''
    cls = lambda k: utf(cp[k][1]) if k and cp[k] and cp[k][0] == 7 else ''
    this, sup = struct.unpack_from('>HH', b, p + 2)
    name, superclass = cls(this), cls(sup)
    strings = [utf(e[1]) for e in cp if e and e[0] == 8]
    classes = [utf(e[1]) for e in cp if e and e[0] == 7]
    if not code:
        return name, superclass, strings, classes, None
    p += 6
    ni = struct.unpack_from('>H', b, p)[0]; p += 2 + 2 * ni

    def members(p):
        out = []
        cnt = struct.unpack_from('>H', b, p)[0]; p += 2
        for _ in range(cnt):
            _acc, mn, _md, ac = struct.unpack_from('>HHHH', b, p); p += 8
            body = None
            for _ in range(ac):
                an, al = struct.unpack_from('>HI', b, p)
                if utf(an) == 'Code':
                    clen = struct.unpack_from('>I', b, p + 10)[0]
                    body = b[p + 14:p + 14 + clen]
                p += 6 + al
            out.append((utf(mn), body))
        return out, p
    _, p = members(p)
    methods, p = members(p)
    bsms = []
    ac = struct.unpack_from('>H', b, p)[0]; p += 2
    for _ in range(ac):
        an, al = struct.unpack_from('>HI', b, p)
        if utf(an) == 'BootstrapMethods':
            q = p + 6
            for _ in range(struct.unpack_from('>H', b, q)[0]):
                _ref, na = struct.unpack_from('>HH', b, q + 2)
                bsms.append(struct.unpack_from('>%dH' % na, b, q + 6))
                q += 4 + 2 * na
        p += 6 + al

    def ref_target(k):  # a Methodref's (class, method)
        e = cp[k]
        if not e or e[0] not in (9, 10, 11):
            return '', ''
        nat = cp[e[2]]
        return cls(e[1]), utf(nat[1]) if nat else ''

    events = {}
    for mname, body in methods:
        if not body:
            continue
        ev, pc = [], 0
        while pc < len(body):
            op = body[pc]
            if op in (0x12, 0x13):
                k = body[pc + 1] if op == 0x12 else struct.unpack_from('>H', body, pc + 1)[0]
                if cp[k] and cp[k][0] == 8:
                    ev.append(('s', utf(cp[k][1])))
            elif op == 0xb2:  # getstatic
                c, f = ref_target(struct.unpack_from('>H', body, pc + 1)[0])
                ev.append(('f', f))
            elif op == 0xbb:  # new
                ev.append(('n', cls(struct.unpack_from('>H', body, pc + 1)[0])))
            elif op == 0xba:
                e = cp[struct.unpack_from('>H', body, pc + 1)[0]]
                if e and e[0] == 18 and e[1] < len(bsms):
                    for a in bsms[e[1]]:
                        h = cp[a]
                        if h and h[0] == 15:
                            c, m = ref_target(h[2])
                            if h[1] == 8:
                                ev.append(('c', c))
                            elif m.startswith('lambda$') and c == name:
                                ev.append(('l', m))
            if op == 0xaa or op == 0xab:
                q = pc + 1 + (-(pc + 1) % 4)
                if op == 0xaa:
                    lo, hi = struct.unpack_from('>ii', body, q + 4)
                    pc = q + 12 + 4 * (hi - lo + 1)
                else:
                    pc = q + 8 + 8 * struct.unpack_from('>i', body, q + 4)[0]
                continue
            if op == 0xc4:
                pc += 6 if body[pc + 1] == 0x84 else 4
                continue
            pc += LEN[op]
        events.setdefault(mname, []).extend(ev)
    return name, superclass, strings, classes, events


CODE_MARKERS = (b'EntityType$Builder', b'EntityRenderers', b'RegisterRenderers', b'registerEntityRenderer')


def read_jar(path):
    """Every class's (super, strings that name textures, classes) and the code events of registration classes,
    plus the jar's data files this audit reads."""
    out = {'classes': {}, 'events': {}, 'files': {}, 'pngs': [], 'geo': []}
    try:
        z = zipfile.ZipFile(path)
    except Exception:
        return path, out
    for info in z.infolist():
        fn = info.filename
        if fn.endswith('.class') and not fn.startswith('META-INF/'):
            b = z.read(info)
            code = any(m in b for m in CODE_MARKERS)
            try:
                r = parse_class(b, code)
            except Exception:
                r = None
            if not r:
                continue
            name, sup, strings, classes, events = r
            tex = [s for s in strings if 'textures/entity' in s or s.endswith('.png')]
            out['classes'][name] = (sup, tex, [c for c in classes if c.startswith(('net/minecraft/client/model',
                                                                                   GECKO)) or '/' in c])
            if events:
                out['events'][name] = events
        elif fn.startswith('data/') and fn.endswith('.json') and (
                '/biome_modifier/' in fn or '/worldgen/biome/' in fn or '/loot_table/entities/' in fn
                or '/loot_tables/entities/' in fn):
            if '/loot_table' in fn:
                out['files'][fn] = None
            else:
                try:
                    out['files'][fn] = json.loads(z.read(info).decode('utf-8', 'replace'))
                except Exception:
                    out['files'][fn] = None
        elif fn.startswith('assets/') and '/textures/entity/' in fn and fn.endswith('.png'):
            out['pngs'].append(fn)
        elif fn.startswith('assets/') and fn.endswith('.geo.json'):
            out['geo'].append(fn)
    return path, out


# --- the audit ---------------------------------------------------------------------------------------------------
def norm(s):
    return re.sub(r'[^a-z0-9]', '', s.lower())


def simple(c):
    return c.rsplit('/', 1)[-1].split('$')[-1]


def entity_ids_in(value, out):
    if isinstance(value, dict):
        t = value.get('type')
        if isinstance(t, str) and ':' in t and ('weight' in value or 'minCount' in value or 'maxCount' in value):
            out.add(t)
        for v in value.values():
            entity_ids_in(v, out)
    elif isinstance(value, list):
        for v in value:
            entity_ids_in(v, out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--pack', required=True)
    ap.add_argument('--dump', required=True)
    ap.add_argument('--carries', required=True)
    ap.add_argument('--species', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--declare')
    ap.add_argument('--jobs', type=int, default=4)
    a = ap.parse_args()

    species = set(re.findall(r'def!\(\s*\w+,\s*"([a-z_]+)"', open(a.species, encoding='utf8').read()))
    registries = json.load(open(os.path.join(a.dump, 'registries.json'), encoding='utf8'))
    registered = sorted(t for t in registries['minecraft:entity_type'] if not t.startswith('minecraft:'))
    items = set(registries['minecraft:item'])
    paths = defaultdict(set)  # path -> namespaces
    for t in registered:
        ns, p = t.split(':', 1)
        paths[p].add(ns)
    jars = [f['jar'] for f in json.load(open(a.pack, encoding='utf8'))['files'] if f['jar'].endswith('.jar')]

    supers, tex, refs, events, files, pngs, geo = {}, {}, {}, {}, {}, set(), set()
    owner = {}  # class -> jar
    with ProcessPoolExecutor(a.jobs) as ex:
        for jar, r in ex.map(read_jar, jars, chunksize=4):
            for c, (s, t, rf) in r['classes'].items():
                supers.setdefault(c, s); tex[c] = t; refs[c] = rf; owner.setdefault(c, jar)
            events.update(r['events'])
            files.update(r['files'])
            pngs.update(r['pngs']); geo.update(r['geo'])

    def chain(c):
        out = []
        while c and c not in out and len(out) < 40:
            out.append(c)
            if c.startswith(('net/minecraft/', GECKO)) or c not in supers:
                break
            c = supers[c]
        return out

    def first_game(c):
        for x in chain(c):
            if x.startswith('net/minecraft/'):
                return x
        return chain(c)[-1] if c else ''

    def is_entity(c):
        g = first_game(c)
        return g.startswith(ENTITY_PKG) and not g.startswith(NOT_ENTITY)

    def is_renderer(c):
        return any(x.startswith(RENDER_PKG) or x.startswith(GECKO + 'renderer') for x in chain(c))

    # Registrations: a name beside an entity constructor, a field beside a renderer constructor.
    def expand(cname, ev, depth=0):
        for e in ev:
            if e[0] == 'l' and depth < 2:
                yield from expand(cname, events.get(cname, {}).get(e[1], []), depth + 1)
            else:
                yield e

    type_class, type_renderer = {}, {}
    for cname, methods in events.items():
        for mname, ev in methods.items():
            if mname.startswith('lambda$'):
                continue
            ev = list(expand(cname, ev))
            for kinds, accept, sink in ((('s',), is_entity, type_class), (('f',), is_renderer, type_renderer)):
                key = None; pending = None
                for e in ev:
                    if e[0] in kinds:
                        k = e[1].split(':')[-1].lower()
                        if k in paths:
                            if pending:
                                for ns in paths[k]:
                                    sink.setdefault(f'{ns}:{k}', pending)
                                pending = None; key = None
                            else:
                                key = k
                    elif e[0] in ('c', 'n') and e[1] and accept(e[1]):
                        if key:
                            for ns in paths[key]:
                                sink.setdefault(f'{ns}:{key}', e[1])
                            key = None
                        elif kinds == ('s',):
                            # `register(Ghoul::new, "ghoul")`; a renderer always follows its type's field.
                            pending = e[1]
    by_name = defaultdict(list)
    renderers_by_name = defaultdict(list)
    for c in supers:
        s = norm(simple(c))
        if c.startswith(('net/minecraft/', GECKO)):
            continue
        if is_entity(c):
            by_name[re.sub(r'entity$|^entity', '', s)].append(c)
        elif is_renderer(c):
            renderers_by_name[re.sub(r'renderer$|render$|entityrenderer$', '', s)].append(c)

    # Spawns and loot from the jars' data; the carries' declarations and loot.
    natural = Counter()
    modifier_kinds = Counter()
    for fn, value in files.items():
        if value is None:
            continue
        found = set()
        entity_ids_in(value, found)
        if '/biome_modifier/' in fn and isinstance(value, dict):
            modifier_kinds[str(value.get('type'))] += bool(found)
        for t in found:
            natural[t] += 1
    loot_jar = {f.split('/')[1] + ':' + f.split('/entities/', 1)[1][:-5] for f in files if '/entities/' in f}
    declared, loot_carry = set(), set()
    for key in os.listdir(a.carries):
        d = os.path.join(a.carries, key, 'data')
        if not os.path.isdir(d):
            continue
        for ns in os.listdir(d):
            et = os.path.join(d, ns, 'horizon', 'entity_type')
            if os.path.isdir(et):
                declared.update(f'{ns}:{f[:-5]}' for f in os.listdir(et) if f.endswith('.json'))
            lt = os.path.join(d, ns, 'loot_table', 'entities')
            for root, _, fs in os.walk(lt):
                rel = os.path.relpath(root, lt).replace('\\', '/')
                loot_carry.update(f'{ns}:' + (f'{rel}/' if rel != '.' else '') + f[:-5] for f in fs if f.endswith('.json'))
    carry_of = {}
    for key in os.listdir(a.carries):
        for ns in os.listdir(os.path.join(a.carries, key, 'assets')) if os.path.isdir(os.path.join(a.carries, key, 'assets')) else []:
            carry_of.setdefault(ns, key)
        d = os.path.join(a.carries, key, 'data')
        for ns in os.listdir(d) if os.path.isdir(d) else []:
            carry_of.setdefault(ns, key)

    rows = {}
    for t in registered:
        ns, p = t.split(':', 1)
        c = type_class.get(t)
        how = 'registration' if c else None
        if not c:
            cands = [x for x in by_name.get(norm(p), []) if f'/{ns}/' in x.lower() or ns in x.lower()] or by_name.get(norm(p), [])
            if len(cands) == 1:
                c, how = cands[0], 'name'
        row = {'class': c, 'found_by': how}
        if c:
            g = first_game(c)
            row['game_class'] = g
            living = g.startswith(ENTITY_PKG) and any(x in g for x in ('/monster/', '/animal/', '/ambient/', '/npc/',
                     '/boss/', 'Mob', 'LivingEntity', 'GlowSquid', 'Animal', 'Golem'))
            row['living'] = living
            v = VANILLA.get(g)
            if v and v in species:
                row['template'] = f'minecraft:{v}'
            elif living:
                row['base'] = simple(g)
        egg = f'{t}_spawn_egg'
        row['spawn'] = {'natural': natural.get(t, 0), 'egg': egg if egg in items else None}
        r = type_renderer.get(t)
        if not r:
            cands = renderers_by_name.get(norm(p), [])
            if c:
                cands = cands or renderers_by_name.get(re.sub(r'entity$', '', norm(simple(c))), [])
            r = cands[0] if len(cands) == 1 else None
        kind, texture = 'none found', None
        if r:
            ch = chain(r)
            game = next((x for x in ch if x.startswith(('net/minecraft/', GECKO))), '')
            own_models = [x for x in refs.get(r, []) if not x.startswith('net/minecraft/') and any(
                y.startswith(MODEL_PKG) for y in chain(x)) and x in supers]
            vanilla_models = [x for x in refs.get(r, []) if x.startswith(MODEL_PKG) and x not in ABSTRACT_MODELS
                              and 'geom/' not in x]
            if game.startswith(GECKO) or any(x.startswith(GECKO) for x in refs.get(r, [])):
                kind = 'geckolib'
            elif r.startswith(RENDER_PKG):
                # the vanilla renderer itself: the template's own texture when it is the template's renderer.
                own = row.get('template', '').split(':')[-1]
                kind = 'template renderer' if norm(simple(r)[:-len('Renderer')]) == norm(own) else 'other vanilla renderer'
            elif game.startswith(RENDER_PKG) and game not in ABSTRACT_RENDERERS and not own_models:
                kind = 'vanilla renderer'
            elif own_models:
                kind = 'own model (code)'
            elif vanilla_models:
                kind = 'vanilla model'
            else:
                kind = 'other'
            for x in ch:
                for s in tex.get(x, []):
                    if 'textures/entity' in s and texture is None:
                        texture = s if ':' in s else f'{ns}:{s}'
            row['renderer'] = r
        if kind == 'none found' and any(g.startswith(f'assets/{ns}/') and norm(p) in norm(g.rsplit('/', 1)[-1]) for g in geo):
            kind = 'geckolib'
        if texture is None:
            hits = sorted(f for f in pngs if f.startswith(f'assets/{ns}/textures/entity/') and
                          norm(f.rsplit('/', 1)[-1][:-4]) == norm(p))
            if hits:
                texture = f'{ns}:' + hits[0].split('/', 2)[2]
        row['render'] = kind
        row['texture'] = texture
        row['loot'] = {'jar': t in loot_jar, 'carried': t in loot_carry}
        row['today'] = 'declared' if t in declared else 'unknown to the engine'
        row['slice1'] = bool(row.get('template') and (
            kind == 'template renderer' or kind in ('vanilla renderer', 'vanilla model') and texture))
        row['carry'] = carry_of.get(ns)
        rows[t] = row

    def counts(rs):
        rs = list(rs)
        return {
            'registered': len(rs),
            'class found': sum(1 for r in rs if r['class']),
            'living': sum(1 for r in rs if r.get('living')),
            'template': sum(1 for r in rs if r.get('template')),
            'base only': sum(1 for r in rs if r.get('base')),
            'natural spawns': sum(1 for r in rs if r['spawn']['natural']),
            'spawn egg': sum(1 for r in rs if r['spawn']['egg']),
            'renderer found': sum(1 for r in rs if r['render'] != 'none found'),
            'texture found': sum(1 for r in rs if r['texture']),
            'loot carried': sum(1 for r in rs if r['loot']['carried']),
            'declared today': sum(1 for r in rs if r['today'] == 'declared'),
            'slice1': sum(1 for r in rs if r['slice1']),
        }
    by_mod = defaultdict(list)
    for t, r in rows.items():
        by_mod[t.split(':')[0]].append(r)
    report = {
        'summary': counts(rows.values()),
        'render': Counter(r['render'] for r in rows.values()),
        'templates': Counter(r['template'] for r in rows.values() if r.get('template')),
        'bases': Counter(r['base'] for r in rows.values() if r.get('base')),
        'modifier types with spawns': modifier_kinds,
        'mod classes seen': {'entity': sum(len(v) for v in by_name.values()),
                             'renderer': sum(len(v) for v in renderers_by_name.values())},
        'by_mod': {ns: counts(rs) for ns, rs in sorted(by_mod.items(), key=lambda kv: -len(kv[1]))},
        'types': rows,
    }
    with open(a.out, 'w', encoding='utf8', newline='\n') as f:
        json.dump(report, f, indent=1, sort_keys=False)
    if a.declare:
        n = 0
        for t, r in rows.items():
            if not (r['slice1'] and r['carry']):
                continue
            ns, p = t.split(':', 1)
            decl = {'template': r['template']}
            if r['render'] != 'template renderer':
                tx = r['texture'].split(':', 1)
                decl['texture'] = f"{tx[0]}:{tx[1].removeprefix('textures/').removesuffix('.png')}"
            d = os.path.join(a.declare, r['carry'], 'data', ns, 'horizon', 'entity_type')
            os.makedirs(os.path.join(d, os.path.dirname(p)), exist_ok=True)
            with open(os.path.join(d, p + '.json'), 'w', encoding='utf8', newline='\n') as f:
                json.dump(decl, f)
            n += 1
        print(f'declared {n} types under {a.declare}')
    s = report['summary']
    print(' '.join(f'{k}={v}' for k, v in s.items()))
    print('render:', dict(report['render'].most_common()))
    print('templates:', dict(report['templates'].most_common(12)))
    print('bases:', dict(report['bases'].most_common(10)))


if __name__ == '__main__':
    main()
