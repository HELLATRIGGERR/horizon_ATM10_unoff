/** AE2's and its addons' recipe viewer categories (`native_recipe_categories`), as a viewer collects them
 * (`native_collect_recipe_categories`, read with `/ae2 recipes`): the categories ae2jeiintegration, ExtendedAE and
 * Advanced AE register for JEI, their catalysts, and a few known recipes in each. */
import assert from 'node:assert/strict';
import { WorldFixture, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('recipe-viewer', mode, { addons: ['extendedae', 'advanced_ae'] });
type Recipe = { inputs: string[][]; outputs: { item: string; count: number; chance: number }[]; notes: string[] };
try {
  await fixture.start();
  const all = (await fixture.ae('recipes')).categories as { id: string; recipes: number; catalysts: string[] }[];
  const byId = new Map(all.map(c => [c.id, c]));
  const expected: [string, string, number][] = [
    ['ae2:inscriber', 'ae2:inscriber', 10], ['ae2:charger', 'ae2:charger', 1], ['ae2:condenser', 'ae2:condenser', 2],
    ['ae2:item_transformation', 'minecraft:water_bucket', 3], ['ae2:entropy', 'ae2:entropy_manipulator', 5],
    ['ae2:certus_growth', 'ae2:flawless_budding_quartz', 2], ['ae2:attunement', 'ae2:me_p2p_tunnel', 3],
    ['extendedae:circuit_cutter', 'extendedae:circuit_cutter', 1], ['extendedae:crystal_assembler', 'extendedae:crystal_assembler', 1],
    ['extendedae:crystal_fixer', 'extendedae:crystal_fixer', 1], ['advanced_ae:reaction', 'advanced_ae:reaction_chamber', 1],
  ];
  for (const [id, catalyst, least] of expected) {
    const c = byId.get(id);
    assert(c, `no category ${id}: ${JSON.stringify(all.map(c => c.id))}`);
    assert(c.recipes >= least, `${id}: ${c.recipes} recipes, at least ${least}`);
    assert(c.catalysts.includes(catalyst), `${id}: catalysts ${JSON.stringify(c.catalysts)}`);
  }
  assert(byId.get('ae2:charger')!.catalysts.includes('ae2:crank'));
  fixture.mark(`AE2's seven categories and the addons' four, with their catalysts: ${expected.map(([id]) => `${id} ${byId.get(id)!.recipes}`).join(', ')}`);

  const recipes = async (category: string, item: string): Promise<Recipe[]> => {
    const answer = await fixture.ae(`recipes ${category} ${item}`);
    assert(!answer.error, JSON.stringify(answer));
    return answer.recipes as Recipe[];
  };
  const making = async (category: string, item: string, check: (r: Recipe) => boolean, what: string) => {
    const found = (await recipes(category, item)).filter(r => r.outputs.some(o => o.item === item));
    assert(found.some(check), `${category} making ${item} (${what}): ${JSON.stringify(found)}`);
  };
  const has = (r: Recipe, slot: number, item: string) => (r.inputs[slot] ?? []).some(s => s.endsWith(`x${item}`));
  await making('ae2:inscriber', 'ae2:printed_silicon', r => has(r, 0, 'ae2:silicon_press') && r.inputs[2].length === 0
    && r.notes.includes('Keeps the presses'), 'the silicon press on top, silicon in the middle');
  await making('ae2:inscriber', 'ae2:logic_processor', r => has(r, 0, 'ae2:printed_logic_processor') && has(r, 2, 'ae2:printed_silicon')
    && r.notes.includes('Uses up the top and bottom items'), 'the two printed circuits pressed around redstone');
  await making('ae2:charger', 'ae2:charged_certus_quartz_crystal', r => has(r, 0, 'ae2:certus_quartz_crystal'), 'a certus crystal');
  await making('ae2:item_transformation', 'ae2:fluix_crystal', r => r.notes.some(n => n.startsWith('Thrown into Water'))
    && r.inputs.some(s => s.some(i => i.endsWith('xminecraft:water_bucket'))), 'charged certus, redstone and quartz in water');
  await making('ae2:condenser', 'ae2:singularity', r => r.notes[0].startsWith('256,000'), '256 000 per singularity');
  await making('ae2:entropy', 'minecraft:obsidian', r => r.notes.some(n => n.startsWith('Cool')), 'lava cooled');
  await making('ae2:attunement', 'ae2:fluid_p2p_tunnel', r => has(r, 0, 'ae2:me_p2p_tunnel') && r.inputs[1].length > 0, 'its tag');
  fixture.mark('AE2: the silicon press, the logic processor, charged certus, fluix in water, the singularity, obsidian, fluid attunement');

  const cut = await fixture.ae('recipes extendedae:circuit_cutter');
  assert((cut.recipes as Recipe[]).every(r => r.inputs.length === 1 && r.outputs.length === 1), JSON.stringify(cut.recipes));
  const fixer = (await fixture.ae('recipes extendedae:crystal_fixer')).recipes as Recipe[];
  assert(fixer.some(r => r.outputs.some(o => o.item === 'ae2:flawless_budding_quartz' && o.chance > 0 && o.chance < 1)), JSON.stringify(fixer));
  const reaction = (await fixture.ae('recipes advanced_ae:reaction')).recipes as Recipe[];
  assert(reaction.every(r => r.notes.some(n => / AE$/.test(n)) && r.notes.some(n => n.startsWith('In: '))), JSON.stringify(reaction));
  fixture.mark('the addons: one input per cut, the fixer\'s chance under one, each reaction\'s fluid and AE');
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
