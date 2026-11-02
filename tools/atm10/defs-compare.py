"""defs-compare.py <before.json> <after.json>: per mod, block and item templates before and after the oracle's
facts (from defs-snapshot.py), and what the items gained (food, tool, armour, durability)."""
import json, sys
from collections import Counter

before, after = (json.load(open(p, encoding='utf-8')) for p in sys.argv[1:3])
def summary(m):
    blocks = Counter(d.get('template') for k, d in m['defs'].items() if k.startswith('block '))
    items = Counter(d.get('template') for k, d in m['defs'].items() if k.startswith('item '))
    gained = Counter()
    for k, d in m['defs'].items():
        c = d.get('components', {})
        if not k.startswith('item '):
            continue
        if 'minecraft:food' in c or d.get('template') == 'minecraft:cooked_beef':
            gained['food'] += 1
        if 'minecraft:tool' in c or d.get('template', '').endswith(('_sword', '_pickaxe', '_axe', '_shovel', '_hoe')):
            gained['tool or weapon'] += 1
        if 'minecraft:equippable' in c or d.get('template', '').endswith(('_helmet', '_chestplate', '_leggings', '_boots')):
            gained['armour'] += 1
        if 'minecraft:max_damage' in c:
            gained['durability'] += 1
    return blocks, items, gained
tot = Counter()
for key in after:
    b, a = before[key], after[key]
    bb, bi, _ = summary(b)
    ab, ai, gained = summary(a)
    print(f"{key}: blocks {b['blocks']} -> {a['blocks']} (left out {b['left_out']} -> {a['left_out']}, "
          f"with differences {a['differences']}), items {b['items']} -> {a['items']} (left out {b['items_unknown']} -> "
          f"{a['items_unknown']}), equipment assets {a['equipment']}, loot tables added {a['loot_added']}")
    print(f"   block templates before {dict(bb.most_common(4))}")
    print(f"   block templates after  {dict(ab.most_common(6))}")
    print(f"   item templates after   {dict(ai.most_common(6))}; gained {dict(gained)}")
    for f in ('blocks', 'left_out', 'items', 'items_unknown', 'equipment', 'loot_added'):
        tot[f + '_before'] += b[f]; tot[f + '_after'] += a[f]
    tot.update({'gained_' + k: v for k, v in gained.items()})
    tot['block_templates_changed'] += sum(1 for k, d in a['defs'].items() if k.startswith('block ')
                                          and k in b['defs'] and b['defs'][k].get('template') != d.get('template'))
print(json.dumps(tot))
