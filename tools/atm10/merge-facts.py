"""merge-facts.py [facts-<batch>.json...]: NeoForge oracle runs (<work>/oracle/facts-*.json, or the ones named)
merged into <work>/oracle/facts.json, the one file `horizon-port-pack carry` reads. Blocks, items and recipes are keyed by id (a later run wins on the
same id: they are the same mod and version); tag members and namespaces are joined."""
import json
import sys
from pathlib import Path

oracle = Path(__file__).resolve().parents[1] / 'oracle'
merged = None
for path in [oracle / n for n in sys.argv[1:]] or sorted(oracle.glob('facts-*.json')):
    facts = json.loads(path.read_text(encoding='utf-8'))
    if merged is None:
        merged = facts
        continue
    merged['namespaces'] = sorted(set(merged['namespaces']) | set(facts['namespaces']))
    for key in ('blocks', 'items', 'recipes'):
        merged[key].update(facts[key])
    for kind, tags in facts.get('tags', {}).items():
        mine = merged.setdefault('tags', {}).setdefault(kind, {})
        for tag, members in tags.items():
            mine[tag] = sorted(set(mine.get(tag, [])) | set(members))
    merged['vanilla_items'] = sorted(set(merged['vanilla_items']) | set(facts['vanilla_items']))
    print(f'{path.name}: {len(facts["namespaces"])} namespaces')
(oracle / 'facts.json').write_text(json.dumps(merged), encoding='utf-8')
mod_blocks = sum(1 for b in merged['blocks'] if not b.startswith('minecraft:'))
print(f'facts.json: {len(merged["namespaces"])} namespaces, {mod_blocks} mod blocks, {len(merged["items"])} items, '
      f'{len(merged["recipes"])} recipes')
