/** Mekanism's recipe viewer categories (`native_recipe_categories`), as a viewer collects them
 * (`native_collect_recipe_categories`, read with `/mek recipes`): the categories Mekanism's JEI plugin registers for
 * the machines this port has, their catalysts (the machine and its factories), and known recipes of each, taken from
 * the content mod's own recipe files: an item output shows as the stack, a chemical or fluid as a note. */
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
// All the Ores' content too: ATM10's clumps, shards and dirty dusts are its items, so the purifier's and the injector's recipes need it.
const ato = resolve(process.env.MEK_ATO_CONTENT_DIR ?? join(root, '../.build/content/alltheores'));
const fixture = new WorldFixture('mekanism-recipe-viewer', 'server-thread', { devMods: [content, genContent, ato] });
type Recipe = { inputs: string[][]; outputs: { item: string; count: number; chance: number }[]; notes: string[] };

/** The content mod's recipe files by type. */
function files(dir: string, out = new Map<string, any[]>()): Map<string, any[]> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) files(path, out);
    else if (entry.name.endsWith('.json') && path.replaceAll('\\', '/').includes('/recipe/')) {
      const json = JSON.parse(readFileSync(path, 'utf8'));
      if (typeof json.type === 'string') out.set(json.type, [...(out.get(json.type) ?? []), json]);
    }
  }
  return out;
}

async function mek(command: string): Promise<any> {
  const reply = await fixture.command(`mek ${command}`);
  const line = (reply.chat ?? []).findLast((t: string) => t.startsWith('MEK {'));
  assert(line, `/mek ${command}: ${JSON.stringify(reply)}`);
  const answer = JSON.parse(line.slice(4));
  assert(!answer.error, `/mek ${command}: ${line}`);
  return answer;
}

try {
  await fixture.start();
  const byType = new Map<string, any[]>();
  for (const dir of [content, genContent, ato]) files(join(dir, 'data'), byType);
  const all = (await mek('recipes')).categories as { id: string; recipes: number; catalysts: string[] }[];
  const byId = new Map(all.map(c => [c.id, c]));
  // Category, its recipe type, a catalyst; the item machines also run in factories.
  const machines: [string, string, string, boolean][] = [
    ['mekanism:crushing', 'mekanism:crushing', 'mekanism:crusher', true],
    ['mekanism:enriching', 'mekanism:enriching', 'mekanism:enrichment_chamber', true],
    ['mekanism:smelting', 'minecraft:smelting', 'mekanism:energized_smelter', true],
    ['mekanism:combining', 'mekanism:combining', 'mekanism:combiner', true],
    ['mekanism:sawing', 'mekanism:sawing', 'mekanism:precision_sawmill', true],
    ['mekanism:compressing', 'mekanism:compressing', 'mekanism:osmium_compressor', true],
    ['mekanism:purifying', 'mekanism:purifying', 'mekanism:purification_chamber', true],
    ['mekanism:injecting', 'mekanism:injecting', 'mekanism:chemical_injection_chamber', true],
    ['mekanism:metallurgic_infusing', 'mekanism:metallurgic_infusing', 'mekanism:metallurgic_infuser', true],
    ['mekanism:crystallizing', 'mekanism:crystallizing', 'mekanism:chemical_crystallizer', false],
    ['mekanism:nucleosynthesizing', 'mekanism:nucleosynthesizing', 'mekanism:antiprotonic_nucleosynthesizer', false],
    ['mekanism:oxidizing', 'mekanism:oxidizing', 'mekanism:chemical_oxidizer', false],
    ['mekanism:chemical_infusing', 'mekanism:chemical_infusing', 'mekanism:chemical_infuser', false],
    ['mekanism:separating', 'mekanism:separating', 'mekanism:electrolytic_separator', false],
    ['mekanism:dissolution', 'mekanism:dissolution', 'mekanism:chemical_dissolution_chamber', false],
    ['mekanism:washing', 'mekanism:washing', 'mekanism:chemical_washer', false],
    ['mekanism:condensentrating', 'mekanism:rotary', 'mekanism:rotary_condensentrator', false],
    ['mekanism:decondensentrating', 'mekanism:rotary', 'mekanism:rotary_condensentrator', false],
    ['mekanism:evaporating', 'mekanism:evaporating', 'mekanism:thermal_evaporation_controller', false],
    ['mekanism:energy_conversion', 'mekanism:energy_conversion', 'mekanism:basic_energy_cube', false],
    ['mekanism:chemical_conversion', 'mekanism:chemical_conversion', 'mekanism:metallurgic_infuser', false],
  ];
  const counts: string[] = [];
  for (const [id, type, catalyst, factories] of machines) {
    const c = byId.get(id);
    assert(c, `no category ${id}: ${JSON.stringify(all.map(c => c.id))}`);
    const there = type === 'minecraft:smelting' ? Infinity : (byType.get(type) ?? []).length;
    assert(c.recipes > 0 && c.recipes <= there, `${id}: ${c.recipes} recipes of ${there} files`);
    assert(c.catalysts.includes(catalyst), `${id}: catalysts ${JSON.stringify(c.catalysts)}`);
    if (factories) assert(['basic', 'advanced', 'elite', 'ultimate'].every(t => c.catalysts.some(s => s.startsWith(`mekanism:${t}_`) && s.endsWith('_factory'))), `${id}: factories ${JSON.stringify(c.catalysts)}`);
    counts.push(`${id.slice(9)} ${c.recipes}`);
  }
  for (const id of ['mekanism:boiler', 'mekanism:sps', 'mekanismgenerators:fission']) assert(byId.get(id)?.recipes, `${id}: ${JSON.stringify(byId.get(id))}`);
  fixture.mark(`twenty-one machine categories and the boiler's, the SPS's and the fission reactor's: ${counts.join(', ')}`);

  // Known recipes: the first three files of each item-output type whose output the category shows.
  const itemTypes: [string, string, string][] = [
    ['mekanism:crushing', 'mekanism:crushing', 'output'], ['mekanism:enriching', 'mekanism:enriching', 'output'],
    ['mekanism:combining', 'mekanism:combining', 'output'], ['mekanism:sawing', 'mekanism:sawing', 'main_output'],
    ['mekanism:compressing', 'mekanism:compressing', 'output'], ['mekanism:purifying', 'mekanism:purifying', 'output'],
    ['mekanism:injecting', 'mekanism:injecting', 'output'], ['mekanism:metallurgic_infusing', 'mekanism:metallurgic_infusing', 'output'],
    ['mekanism:crystallizing', 'mekanism:crystallizing', 'output'], ['mekanism:nucleosynthesizing', 'mekanism:nucleosynthesizing', 'output'],
  ];
  let checked = 0;
  for (const [category, type, field] of itemTypes) {
    // Recipes of this world's own items: an input item of another mod (the compat recipes') is not in it and the viewer drops the recipe.
    const known = (r: any) => [...JSON.stringify(r).matchAll(/"item":"([a-z_0-9]+):/g)].every(m => ['mekanism', 'minecraft', 'alltheores'].includes(m[1]));
    const mine = (byType.get(type) ?? []).filter(r => /^(mekanism|minecraft|alltheores):/.test(r[field]?.id ?? '') && known(r)).slice(0, 3);
    for (const r of mine) {
      const item = r[field].id;
      const found = ((await mek(`recipes ${category} ${item}`)).recipes as Recipe[]).filter(x => x.outputs.some(o => o.item === item));
      assert(found.some(x => x.outputs.some(o => o.item === item && o.count === (r[field].count ?? 1))), `${category} making ${item}: ${JSON.stringify(found)}`);
      if (r.chemical_input) assert(found.some(x => x.notes.some(n => n.startsWith('In: ') && n.endsWith(' mB'))), `${category} ${item}: no chemical note`);
      checked++;
    }
  }
  const salt = ((await mek('recipes mekanism:oxidizing')).recipes as Recipe[]);
  assert(salt.every(r => r.inputs.length === 1 && r.outputs.length === 0 && r.notes.some(n => n.startsWith('Out: '))), JSON.stringify(salt));
  const hcl = ((await mek('recipes mekanism:chemical_infusing')).recipes as Recipe[]);
  assert(hcl.some(r => r.notes.includes('In: Hydrogen 1 mB') && r.notes.includes('In: Chlorine 1 mB') && r.notes.includes('Out: Hydrogen Chloride 1 mB')), JSON.stringify(hcl));
  const water = ((await mek('recipes mekanism:separating minecraft:water_bucket')).recipes as Recipe[]);
  assert(water.some(r => r.notes.includes('Out: Hydrogen 2 mB') && r.notes.includes('Out: Oxygen 1 mB')), JSON.stringify(water));
  const sawdust = ((await mek('recipes mekanism:sawing mekanism:sawdust')).recipes as Recipe[]);
  assert(sawdust.some(r => r.outputs.some(o => o.item === 'mekanism:sawdust' && o.chance > 0 && o.chance < 1)), JSON.stringify(sawdust));
  const redstone = ((await mek('recipes mekanism:energy_conversion minecraft:redstone')).recipes as Recipe[]);
  assert(redstone.some(r => r.notes.includes('Out: 10,000 J')), JSON.stringify(redstone));
  fixture.mark(`${checked} item recipes found by their output with their counts; the oxidizer's chemical outputs, hydrogen chloride, water's hydrogen and oxygen, sawdust at a chance, redstone's 10,000 J`);
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
