/** Modern Industrialization's electric machines, as the `modern_industrialization` native bundle runs them on the
 * carry of the player's own jar (its machines on the beacon template, so a use opens their menus: the script's
 * `content` step), with ATM10's own MI recipes and `config/modern_industrialization-server.toml`.
 *
 * The numbers are MI 2.5.11's (`CrafterComponent`, `MachineTier.LV`, `EnergyApi`), worked out from its source and
 * held by the crate's unit tests:
 *
 * - Forge Energy: 16 FE to the EU (ATM10's `forgeEnergyPerEu`), whole EU only: 1,000 FE offered, 992 taken (62 EU);
 *   the LV casing holds 3,200 EU (51,200 FE), so the next offer takes the rest of that room only.
 * - The electric macerator on `materials/annealed_copper/macerator/double_ingot` (2 EU/t for 100 ticks, 200 EU):
 *   an LV machine runs it at its base 8 EU/t, so the first ingot takes 25 ticks and 200 EU, the output slot reserved
 *   from the first tick (machine lock); the second starts on the next tick, efficiency 1 of 72 still at 8 EU/t; with
 *   nothing left the efficiency falls one a tick and the recipe is forgotten at 0.
 * - `materials/aluminum/macerator/drill_head` (2 EU/t for 200 ticks): 50 ticks, 64 tiny dust in the first output
 *   slot and 3 in the second.
 * - The electric compressor on `materials/annealed_copper/compressor/double_ingot`: 25 ticks, two plates.
 * - The electric cutting machine on `vanilla_recipes/cutting_machine/glass_panes` (6 glass and 1 mB of lubricant, 100 ticks
 *   at 2 EU/t): 25 ticks, 16 glass panes, 1 mB of lubricant gone; without lubricant nothing starts.
 * - An oak log with lubricant in a second cutting machine: both the planks' recipe (the `oak_logs` tag) and the stripped
 *   log's (the item) match, MI starts neither (`overlaps`), and the screen's data says so (`matches_multiple_recipes`);
 *   an oak plank matches the slabs' recipe and the sticks' (`#minecraft:planks`) the same way.
 * - Output slots refuse a player's items; the screen draws MI's panel, bars and slots (screenshots in the run folder).
 *
 * State is read with `/mi probe` (the saved compound and the slots) after `server-step`s of a held server. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const content = resolve(process.env.MI_CONTENT ?? '');
assert(existsSync(join(content, 'horizon-mod.toml')), `MI_CONTENT must name the content mod of Modern Industrialization (the script's content step): ${content}`);
const fixture = new WorldFixture('mi-machines', 'server-thread', { devMods: [content] });
const atm10 = process.env.ATM10_DIR;
const serverToml = atm10 ? join(atm10, 'config/modern_industrialization-server.toml') : '';
if (serverToml && existsSync(serverToml)) {
  mkdirSync(join(fixture.game, 'config'), { recursive: true });
  copyFileSync(serverToml, join(fixture.game, 'config/modern_industrialization-server.toml'));
}
const FE_PER_EU = serverToml && existsSync(serverToml) ? 16 : 10;

const MI = (id: string) => `modern_industrialization:${id}`;
async function mi(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith('MI {'));
  assert(line, `no MI answer to ${command}: ${JSON.stringify(reply)}`);
  const json = JSON.parse(line.slice(3));
  assert(!json.error, `${command}: ${json.error}`);
  return json;
}
const probe = (pos: string) => mi(`mi probe ${pos}`);
const held = (p: any, i: number) => p.slots[i] ? [p.slots[i].item, p.slots[i].count] : null;
const items = (...stacks: [number, string, number][]) =>
  `{Items:[${stacks.map(([slot, id, count]) => `{Slot:${slot}b,id:"${id}",count:${count}}`).join(',')}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.min(Math.floor(py) + 1, 100), z = Math.floor(pz) - 2;
  const at = (dx: number) => `${x + dx} ${y} ${z}`;
  await fixture.command(`fill ${x - 2} ${y} ${z - 2} ${x + 10} ${y + 3} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 2} ${x + 10} ${y - 1} ${z + 3} minecraft:stone`);
  const [mac, mac2, comp, cut, cut2] = [at(0), at(2), at(4), at(6), at(8)];
  await fixture.command(`setblock ${mac} ${MI('electric_macerator')}`);
  await fixture.command(`setblock ${mac2} ${MI('electric_macerator')}`);
  await fixture.command(`setblock ${comp} ${MI('electric_compressor')}`);
  await fixture.command(`setblock ${cut} ${MI('electric_cutting_machine')}`);
  await fixture.command(`setblock ${cut2} ${MI('electric_cutting_machine')}`);
  await fixture.hold();

  // The world's recipes and Forge Energy.
  let p = await probe(mac);
  log.empty = { recipes: p.recipes, fe_per_eu: p.fe_per_eu, fe_capacity: p.fe_capacity, eu: p.eu };
  assert(p.recipes > 100, `the macerator reads ATM10's MI macerator recipes: ${p.recipes}`);
  assert.deepEqual([p.eu, p.eu_capacity, p.fe_per_eu, p.fe_capacity], [0, 3200, FE_PER_EU, 3200 * FE_PER_EU]);
  const first = await mi(`mi offer_fe ${mac} 1000`);
  const whole = Math.floor(1000 / FE_PER_EU);
  assert.equal(first.taken, whole * FE_PER_EU, `1,000 FE in whole EU: ${JSON.stringify(first)}`);
  assert.equal((await probe(mac)).eu, whole);
  const rest = await mi(`mi offer_fe ${mac} 100000`);
  assert.equal(rest.taken, (3200 - whole) * FE_PER_EU, 'then the rest of the room only');
  assert.equal((await probe(mac)).eu, 3200);
  for (const pos of [mac2, comp, cut, cut2]) await mi(`mi offer_fe ${pos} ${3200 * FE_PER_EU}`);
  log.fe = { first, rest };
  fixture.mark(`Forge Energy at ${FE_PER_EU} FE/EU in whole EU: ${first.taken} of 1000, then ${rest.taken}; 3,200 EU full`);

  // The macerator: two annealed copper double ingots, 25 ticks each at 8 EU/t.
  await fixture.command(`data merge block ${mac} ${items([0, MI('annealed_copper_double_ingot'), 2])}`);
  await fixture.stepServer(1);
  p = await probe(mac);
  log.tick1 = p;
  assert.equal(p.active_recipe, 'modern_industrialization:materials/annealed_copper/macerator/double_ingot', 'the ATM10 recipe starts');
  assert.deepEqual([p.used_energy, p.recipe_energy, p.recipe_max_eu, p.eu, held(p, 0)], [8, 200, 8, 3192, [MI('annealed_copper_double_ingot'), 1]]);
  await fixture.stepServer(23);
  p = await probe(mac);
  assert.deepEqual([p.used_energy, held(p, 1)], [192, null], 'nothing out after 24 ticks');
  await fixture.stepServer(1);
  p = await probe(mac);
  log.tick25 = p;
  assert.deepEqual(held(p, 1), [MI('annealed_copper_dust'), 2], 'two dusts on tick 25');
  assert.deepEqual([p.used_energy, p.efficiency_ticks, p.max_efficiency_ticks, p.eu], [0, 1, 72, 3000]);
  await fixture.stepServer(1);
  p = await probe(mac);
  assert.deepEqual([p.used_energy, p.recipe_max_eu, held(p, 0)], [8, 8, null], 'the second starts on the next tick, still 8 EU/t');
  await fixture.stepServer(24);
  p = await probe(mac);
  assert.deepEqual([held(p, 1), p.efficiency_ticks, p.eu], [[MI('annealed_copper_dust'), 4], 2, 2800]);
  await fixture.stepServer(1);
  p = await probe(mac);
  assert.deepEqual([p.efficiency_ticks, p.active_recipe], [1, 'modern_industrialization:materials/annealed_copper/macerator/double_ingot']);
  await fixture.stepServer(1);
  p = await probe(mac);
  assert.deepEqual([p.efficiency_ticks, p.active_recipe ?? null, p.eu], [0, null, 2800]);
  fixture.mark('the electric macerator: 25 ticks and 200 EU an ingot at 8 EU/t, efficiency 1 then 2 of 72, then back to 0');

  // A recipe with two outputs: 64 and 3 tiny dusts in two slots, after 50 ticks.
  await fixture.command(`data merge block ${mac2} ${items([0, MI('aluminum_drill_head'), 1])}`);
  await fixture.stepServer(49);
  p = await probe(mac2);
  assert.deepEqual([p.used_energy, held(p, 1)], [392, null]);
  await fixture.stepServer(1);
  p = await probe(mac2);
  log.drill = p.slots;
  assert.deepEqual([held(p, 1), held(p, 2)], [[MI('aluminum_tiny_dust'), 64], [MI('aluminum_tiny_dust'), 3]]);
  fixture.mark('a drill head: 64 and 3 tiny dusts in two output slots after 50 ticks');

  // The compressor and the cutting machine.
  await fixture.command(`data merge block ${cut2} {Items:[{Slot:0b,id:"minecraft:oak_log",count:2}],inventory_fluids:[{fluid:"${MI('lubricant')}",amount:1000L}]}`);
  await fixture.command(`data merge block ${comp} ${items([0, MI('annealed_copper_double_ingot'), 1])}`);
  await fixture.command(`data merge block ${cut} ${items([0, 'minecraft:glass', 12])}`);
  await fixture.stepServer(3);
  p = await probe(cut);
  assert.deepEqual([p.active_recipe ?? null, held(p, 0)], [null, ['minecraft:glass', 12]], 'no lubricant, no cut');
  await fixture.command(`data merge block ${cut} {inventory_fluids:[{fluid:"${MI('lubricant')}",amount:1000L}]}`);
  await fixture.stepServer(25);
  const c = await probe(comp);
  p = await probe(cut);
  log.compressor = c.slots;
  log.cutting = { slots: p.slots, fluids: p.fluids, recipe: p.active_recipe };
  log.candidates = p.candidates;
  assert.deepEqual(held(c, 1), [MI('annealed_copper_plate'), 2], 'two plates after 25 ticks');
  assert.deepEqual([held(p, 1), p.fluids[0].amount], [['minecraft:glass_pane', 16], 999], 'sixteen panes for six glass and 1 mB');
  const both = await probe(cut2);
  log.overlap = { slots: both.slots, recipe: both.active_recipe, multiple: both.matches_multiple_recipes };
  assert.deepEqual([both.active_recipe ?? null, both.matches_multiple_recipes, held(both, 0)], [null, true, ['minecraft:oak_log', 2]], 'a log matches two recipes: none starts');
  fixture.mark('the compressor: two plates in 25 ticks; the cutting machine: 16 glass panes and 1 mB of lubricant for 6 glass');

  // Leave a recipe running for the screen.
  await fixture.command(`data merge block ${mac} ${items([0, MI('annealed_copper_double_ingot'), 5])}`);
  await fixture.stepServer(30);
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'minecraft:cobblestone', 3);
  await until(() => fixture.raw(`block ${mac}`), v => v.block === MI('electric_macerator'), 'the macerator reaches the client');
  await fixture.raw(`interact ${mac} south`);
  const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);
  let view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the macerator screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0, 1, 2, 3, 4], 'the input and four outputs');
  const shots: string[] = [];
  log.screenshots = shots;
  const shot = async (name: string) => {
    await Bun.sleep(1500);
    const path = join(fixture.run, `mi-${name}.png`);
    await fixture.raw(`screenshot ${path}`);
    shots.push(path);
  };
  await shot('macerator');
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 166) / 2)];
  await fixture.raw(`click-at ${ox + 24} ${oy + 42}`);
  await shot('macerator-energy-tooltip');
  // An output slot refuses a player's item.
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:cobblestone', 'cobblestone on the cursor');
  await fixture.raw('click-slot 304');
  await Bun.sleep(1000);
  view = await fixture.raw('screen');
  assert.equal(view.carried?.item, 'minecraft:cobblestone', 'the output slot takes none');
  await fixture.raw('click-slot 0');
  await fixture.raw('close');
  fixture.mark('the macerator screen: MI panel, bars and five slots; outputs take no player item');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${cut}`), v => v.block === MI('electric_cutting_machine'), 'the cutting machine reaches the client');
  await fixture.raw(`interact ${cut} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the cutting machine screen opens');
  await fixture.raw(`click-at ${ox + 170} ${oy + 4}`);
  await shot('cutting-machine');
  await fixture.raw(`click-at ${ox + 48} ${oy + 43}`);
  await shot('cutting-machine-fluid-tooltip');
  await fixture.raw('close');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MI machines', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MI machines', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
