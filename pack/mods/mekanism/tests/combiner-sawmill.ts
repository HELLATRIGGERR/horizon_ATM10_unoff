/** Mekanism's Combiner and Precision Sawmill, as the `mekanism` native bundle and its content mod make them, under
 * ATM10's configs (`combiner = 50` J a tick and 40 000 J, `precisionSawmill = 50` and 20 000 J,
 * `maxUpgradeMultiplier = 16`). The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and the same
 * configs gave for the same steps:
 *
 * - A combiner with 40 000 J, two of a main and an extra input: progress 1 and 39 950 J after a tick, 199 and 30 050 J
 *   after 199, one output and one of each input used on the 200th, 30 000 J left. (On NeoForge: flint and cobblestone
 *   into gravel; here a recipe on vanilla's items alone, cut copper and honeycomb into waxed cut copper.)
 * - Its extra input taken out halfway: the progress starts over, as the main input's does.
 * - A sawmill makes 6 oak planks of an oak log in 200 ticks of 50 J, the 25 % sawdust aside; torches (4 an
 *   operation) give a stick and, at a chance of 1, coal.
 * - A full secondary slot (64 sawdust), or another item in it, holds the sawmill back although no sawdust may come.
 * - Twenty sawmills with 8 speed and 8 energy upgrades: 800 J a tick, 12 ticks an operation; ten operations each make
 *   200 rolls of 25 %: 43 sawdust on NeoForge, here between 25 and 75.
 * - The sawmill's screen: its four slots and the upgrade component's two; a screenshot `sawmill.png`.
 *
 * Joules, progress and items are read from the saved compound (`data get block`), the rest from `/mek probe`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-combiner', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-usage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n');
  writeFileSync(join(config, 'machine-usage.toml'), 'combiner = 50\nprecisionSawmill = 50\n');
  writeFileSync(join(config, 'machine-storage.toml'), 'combiner = 40000\nprecisionSawmill = 20000\n');
}

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const energyIn = (nbt: string) => Number(nbt.match(/energy_containers: ?\[\{[^\]]*stored: (\d+)L/)?.[1] ?? 0);
const progressIn = (nbt: string) => Number(nbt.match(/progress: ?(\d+)/)?.[1] ?? -1);
/** The count of `item` in slot `slot` of the saved compound's `Items`, 0 if none. */
function slotIn(nbt: string, slot: number, item: string): number {
  const list = nbt.match(/Items: ?\[(.*?)\]\s*(,|\})/)?.[1] ?? '';
  for (const entry of list.match(/\{[^{}]*\}/g) ?? []) {
    if (new RegExp(`Slot: ?${slot}b`).test(entry) && entry.includes(`"${item}"`)) return Number(entry.match(/count: ?(\d+)/)?.[1] ?? 1);
  }
  return 0;
}
async function probe(pos: string): Promise<any> {
  const reply = await fixture.command(`mek probe ${pos}`);
  const line = (reply.chat ?? []).findLast((t: string) => t.startsWith('MEK {'));
  assert(line, `/mek probe ${pos}: ${JSON.stringify(reply)}`);
  const answer = JSON.parse(line.slice(4));
  assert(!answer.error, `/mek probe ${pos}: ${line}`);
  return answer;
}
const items = (list: [number, string, number][]) => `{Items:[${list.map(([slot, id, count]) => `{Slot:${slot}b,id:"${id}",count:${count}}`).join(',')}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(44, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(44, -2, 8)} minecraft:stone`);
  await fixture.hold();

  // A: the combiner, 200 ticks of 50 J.
  const a = at(0);
  await fixture.command(`setblock ${a} mekanism:combiner`);
  await fixture.command(`data merge block ${a} ${energy(40000)}`);
  await fixture.command(`data merge block ${a} ${items([[0, 'minecraft:cut_copper', 2], [1, 'minecraft:honeycomb', 2]])}`);
  await fixture.stepServer(1);
  const first = await saved(a);
  assert.equal(progressIn(first), 1, `progress 1 after a tick: ${first}`);
  assert.equal(energyIn(first), 39950);
  await fixture.stepServer(198);
  const almost = await saved(a);
  assert.deepEqual([progressIn(almost), energyIn(almost), slotIn(almost, 2, 'minecraft:waxed_cut_copper')], [199, 30050, 0]);
  await fixture.stepServer(1);
  const done = await saved(a);
  log.combiner = done;
  assert.equal(slotIn(done, 2, 'minecraft:waxed_cut_copper'), 1, `the 200th tick combines: ${done}`);
  assert.deepEqual([slotIn(done, 0, 'minecraft:cut_copper'), slotIn(done, 1, 'minecraft:honeycomb'), progressIn(done), energyIn(done)], [1, 1, 0, 30000]);
  const pa = await probe(a);
  log.probe_combiner = pa;
  assert.deepEqual([pa.capacity, pa.per_tick, pa.required, pa.processes], [40000, 50, 200, 1], 'ATM10: 40 000 J, 50 J a tick');
  assert(pa.recipe_count > 20, `the combiner read the combining recipes: ${pa.recipe_count}`);
  assert.equal(pa.item_config.bottom, 'extra', 'EXTRA_MACHINE: its bottom is the extra input');
  fixture.mark('the combiner: 200 ticks of 50 J, one of each input into one output, 40 000 J');

  // B: its extra input taken out halfway.
  await fixture.stepServer(50);
  assert.equal(progressIn(await saved(a)), 50);
  await fixture.command(`data remove block ${a} Items[{Slot:1b}]`);
  await fixture.stepServer(1);
  assert.equal(progressIn(await saved(a)), 0, 'no extra input: from the start');
  await fixture.command(`data merge block ${a} ${items([[0, 'minecraft:cut_copper', 1], [1, 'minecraft:honeycomb', 1], [2, 'minecraft:waxed_cut_copper', 1]])}`);
  await fixture.stepServer(1);
  assert.equal(progressIn(await saved(a)), 1, 'put back, it counts from 1');
  fixture.mark("the combiner's extra input taken out starts the progress over");

  // C: the sawmill, an oak log into 6 planks; torches into a stick and coal.
  const c = at(4), torches = at(6);
  await fixture.command(`setblock ${c} mekanism:precision_sawmill`);
  await fixture.command(`setblock ${torches} mekanism:precision_sawmill`);
  await fixture.command(`data merge block ${c} ${energy(20000)}`);
  await fixture.command(`data merge block ${torches} ${energy(20000)}`);
  await fixture.command(`data merge block ${c} ${items([[0, 'minecraft:oak_log', 1]])}`);
  await fixture.command(`data merge block ${torches} ${items([[0, 'minecraft:torch', 9]])}`);
  await fixture.stepServer(200);
  const sawn = await saved(c), lit = await saved(torches);
  log.sawmill = { sawn, lit };
  assert.equal(slotIn(sawn, 1, 'minecraft:oak_planks'), 6, `6 oak planks of a log: ${sawn}`);
  assert.deepEqual([slotIn(sawn, 0, 'minecraft:oak_log'), energyIn(sawn)], [0, 10000]);
  assert.deepEqual([slotIn(lit, 0, 'minecraft:torch'), slotIn(lit, 1, 'minecraft:stick'), slotIn(lit, 2, 'minecraft:coal')], [5, 1, 1], `four torches into a stick and coal: ${lit}`);
  const pc = await probe(c);
  assert.deepEqual([pc.capacity, pc.per_tick, pc.layout.secondaries], [20000, 50, [2]]);
  fixture.mark('the sawmill: an oak log into 6 planks, torches into a stick and coal at a chance of 1');

  // D: a full secondary slot, or another item there, holds it back.
  const full = at(8), other = at(10);
  for (const pos of [full, other]) {
    await fixture.command(`setblock ${pos} mekanism:precision_sawmill`);
    await fixture.command(`data merge block ${pos} ${energy(20000)}`);
  }
  await fixture.command(`data merge block ${full} ${items([[0, 'minecraft:oak_log', 1], [2, 'mekanism:sawdust', 64]])}`);
  await fixture.command(`data merge block ${other} ${items([[0, 'minecraft:oak_log', 1], [2, 'minecraft:dirt', 1]])}`);
  await fixture.stepServer(5);
  for (const pos of [full, other]) {
    const held = await saved(pos);
    assert.deepEqual([progressIn(held), energyIn(held)], [0, 20000], `no room for the sawdust, it waits: ${held}`);
  }
  fixture.mark('a full or other secondary output holds the sawmill back');

  // E: the chance: 20 sawmills, 8 speed and 8 energy upgrades, ten operations each.
  const mills = Array.from({ length: 20 }, (_, i) => at(2 * i, 0, 4));
  for (const pos of mills) {
    await fixture.command(`setblock ${pos} mekanism:precision_sawmill`);
    await fixture.command(`data merge block ${pos} {component_upgrade:{upgrades:[{type:0,amount:8},{type:1,amount:8}]}}`);
    await fixture.command(`data merge block ${pos} ${energy(320000)}`);
    await fixture.command(`data merge block ${pos} ${items([[0, 'minecraft:oak_log', 64]])}`);
  }
  await fixture.stepServer(1);
  const one = await saved(mills[0]);
  assert.deepEqual([progressIn(one), energyIn(one)], [1, 319200], `800 J a tick: ${one}`);
  await fixture.stepServer(11);
  assert.equal(slotIn(await saved(mills[0]), 1, 'minecraft:oak_planks'), 6, 'an operation in 12 ticks');
  await fixture.stepServer(108);
  let sawdust = 0, planks = 0;
  for (const pos of mills) {
    const s = await saved(pos);
    sawdust += slotIn(s, 2, 'mekanism:sawdust');
    planks += slotIn(s, 1, 'minecraft:oak_planks');
  }
  log.chance = { sawdust, planks };
  assert.equal(planks, 20 * 60, 'ten operations each');
  assert(sawdust >= 25 && sawdust <= 75, `25 % of 200 operations: ${sawdust} sawdust (NeoForge: 43)`);
  fixture.mark(`200 operations at 25 %: ${sawdust} sawdust`);
  await fixture.hold(false);

  // F: the sawmill's screen.
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${c}`), v => v.block === 'mekanism:precision_sawmill', 'the sawmill reaches the client');
  await fixture.raw(`interact ${c} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the sawmill screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5], 'input, output, secondary, energy and the two upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'sawmill.png')}`);
  await fixture.raw('close');
  fixture.mark("the sawmill's screen shows its six slots");

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK combiner', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK combiner', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
