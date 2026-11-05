/** Mekanism's factories, as the `mekanism` native bundle and its content mod make them, under ATM10's configs. The
 * numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and the same configs gave for the same steps:
 *
 * - A basic smelting factory stores 30 000 J (`max(storage / 2, usage) * processes`), an advanced one 50 000, an elite
 *   enriching one 70 000, an ultimate smelting one 90 000, a basic combining one 60 000; 3, 5, 7 and 9 processes, the
 *   ultimate's ninth input in slot 16.
 * - Three inputs in a basic smelting factory: progress `[1, 1, 1]` and 150 J used after a tick, three ingots on the
 *   200th, its store empty.
 * - Sorting on, ten raw iron in process 0 become 4, 3 and 3 before the first tick's processes run; sorting off, they
 *   stay and only process 0 works. Seven hanging roots (three an operation) in a combining factory become 4 and 3
 *   (on NeoForge: ten obsidian dust, four an operation, became 6 and 4).
 * - 120 J for three processes of 50: processes 0 and 1 run, 2 waits, 20 J are left.
 * - Eight speed upgrades: 12 800 J a process a tick, so only two of three run on 30 000 J; with eight energy upgrades
 *   too, 480 000 J and 800 J a tick, all three done in 12 ticks.
 * - A combining factory's processes share its extra slot; a sawing factory's give their secondary outputs in slots
 *   2, 5 and 8 (coal of torches).
 * - A basic energy cube's item with 1 000 000 J in a factory's or a chamber's energy slot gives 4 000 J a tick (its
 *   tier's output): 30 000 J and 970 000 J left after 8 ticks, a chamber's 20 000 and 980 000.
 * - The progress array, the sorting and the inputs survive a restart.
 * - The factory's screen: its slots, the sorting tab (a click turns sorting on), the ultimate sawing factory's 30
 *   slots on its wider and taller panel; screenshots `factory-*.png`.
 *
 * Joules, progress and items are read from the saved compound (`data get block`), the rest from `/mek probe`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-factories', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-usage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n');
  writeFileSync(join(config, 'machine-usage.toml'), 'enrichmentChamber = 50\ncrusher = 50\nenergizedSmelter = 50\ncombiner = 50\nprecisionSawmill = 50\n');
  writeFileSync(join(config, 'machine-storage.toml'), 'enrichmentChamber = 20000\ncrusher = 20000\nenergizedSmelter = 20000\ncombiner = 40000\nprecisionSawmill = 20000\n');
}

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const energyIn = (nbt: string) => Number(nbt.match(/energy_containers: ?\[\{[^\]]*stored: (\d+)L/)?.[1] ?? 0);
const progressIn = (nbt: string) => (nbt.match(/progress: ?\[I; ?([\d, ]*)\]/)?.[1] ?? '').split(',').filter(s => s.trim()).map(Number);
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
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(40, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(40, -2, 8)} minecraft:stone`);
  await fixture.hold();

  // A: each tier's store and processes.
  const tiers: [string, number, number][] = [
    ['basic_smelting_factory', 3, 30000], ['advanced_smelting_factory', 5, 50000], ['elite_enriching_factory', 7, 70000],
    ['ultimate_smelting_factory', 9, 90000], ['basic_combining_factory', 3, 60000], ['ultimate_sawing_factory', 9, 90000],
  ];
  const stores: Record<string, unknown> = {};
  for (const [i, [block, processes, capacity]] of tiers.entries()) {
    const pos = at(2 * i, 0, 6);
    await fixture.command(`setblock ${pos} mekanism:${block}`);
    await fixture.command(`data merge block ${pos} ${energy(capacity)}`);
    await fixture.stepServer(1);
    const p = await probe(pos);
    stores[block] = [p.processes, p.capacity, p.progress.length];
    assert.deepEqual([p.processes, p.capacity, p.progress.length, p.per_tick], [processes, capacity, processes, 50], `${block}: ${JSON.stringify(p)}`);
  }
  log.stores = stores;
  const ultimate = at(6, 0, 6);
  await fixture.command(`data merge block ${ultimate} ${items([[16, 'minecraft:raw_iron', 1]])}`);
  await fixture.stepServer(1);
  const u = await saved(ultimate);
  assert.deepEqual(progressIn(u), [0, 0, 0, 0, 0, 0, 0, 0, 1], `the ultimate's ninth process reads slot 16: ${u}`);
  assert.equal(energyIn(u), 89950);
  fixture.mark('3, 5, 7 and 9 processes; 30 000, 50 000, 70 000 and 90 000 J, a combining factory 60 000');

  // B: three processes on one store.
  const b = at(0);
  await fixture.command(`setblock ${b} mekanism:basic_smelting_factory`);
  await fixture.command(`data merge block ${b} ${energy(30000)}`);
  await fixture.command(`data merge block ${b} ${items([[0, 'minecraft:raw_iron', 1], [2, 'minecraft:raw_iron', 1], [4, 'minecraft:raw_gold', 1]])}`);
  await fixture.stepServer(1);
  const first = await saved(b);
  assert.deepEqual(progressIn(first), [1, 1, 1], `each process counts: ${first}`);
  assert.equal(energyIn(first), 29850, 'three processes of 50 J');
  await fixture.stepServer(199);
  const done = await saved(b);
  log.three = done;
  assert.deepEqual([slotIn(done, 1, 'minecraft:iron_ingot'), slotIn(done, 3, 'minecraft:iron_ingot'), slotIn(done, 5, 'minecraft:gold_ingot')], [1, 1, 1], `three ingots: ${done}`);
  assert.deepEqual([progressIn(done), energyIn(done)], [[0, 0, 0], 0]);
  fixture.mark('a basic factory runs three processes on one store');

  // C: sorting.
  const sorted = at(4), unsorted = at(6), roots = at(8);
  await fixture.command(`setblock ${sorted} mekanism:basic_smelting_factory`);
  await fixture.command(`setblock ${unsorted} mekanism:basic_smelting_factory`);
  await fixture.command(`setblock ${roots} mekanism:basic_combining_factory`);
  await fixture.command(`data merge block ${sorted} {sorting:1b}`);
  await fixture.command(`data merge block ${roots} {sorting:1b}`);
  for (const pos of [sorted, unsorted, roots]) await fixture.command(`data merge block ${pos} ${energy(30000)}`);
  await fixture.command(`data merge block ${sorted} ${items([[0, 'minecraft:raw_iron', 10]])}`);
  await fixture.command(`data merge block ${unsorted} ${items([[0, 'minecraft:raw_iron', 10]])}`);
  await fixture.command(`data merge block ${roots} ${items([[0, 'minecraft:hanging_roots', 7], [6, 'minecraft:dirt', 5]])}`);
  await fixture.stepServer(1);
  const s = await saved(sorted), n = await saved(unsorted), r = await saved(roots);
  log.sorting = { s, n, r };
  assert.deepEqual([0, 2, 4].map(i => slotIn(s, i, 'minecraft:raw_iron')), [4, 3, 3], `sorted 4, 3, 3: ${s}`);
  assert.deepEqual([progressIn(s), energyIn(s)], [[1, 1, 1], 29850], 'sorted before the processes ran');
  assert.deepEqual([slotIn(n, 0, 'minecraft:raw_iron'), progressIn(n).join()], [10, '1,0,0'], `not sorting: ${n}`);
  assert.deepEqual([0, 2, 4].map(i => slotIn(r, i, 'minecraft:hanging_roots')), [4, 3, 0], `three an operation: 4 and 3: ${r}`);
  assert.equal((await probe(sorted)).sorting, true);
  fixture.mark('sorting spreads the inputs over the processes, in multiples of what an operation needs');

  // D: short of energy: 120 J for three processes.
  const d = at(10);
  await fixture.command(`setblock ${d} mekanism:basic_smelting_factory`);
  await fixture.command(`data merge block ${d} ${energy(120)}`);
  await fixture.command(`data merge block ${d} ${items([[0, 'minecraft:raw_iron', 1], [2, 'minecraft:raw_iron', 1], [4, 'minecraft:raw_iron', 1]])}`);
  await fixture.stepServer(1);
  const short = await saved(d);
  assert.deepEqual([progressIn(short), energyIn(short)], [[1, 1, 0], 20], `two processes ran, the third waits: ${short}`);
  await fixture.stepServer(1);
  assert.deepEqual(progressIn(await saved(d)), [1, 1, 0], '20 J: none');
  fixture.mark('the processes take the store in turn; short of energy the last ones wait');

  // E: upgrades: eight speed, then eight energy too.
  const e = at(12), f = at(14);
  await fixture.command(`setblock ${e} mekanism:basic_smelting_factory`);
  await fixture.command(`setblock ${f} mekanism:basic_smelting_factory`);
  await fixture.command(`data merge block ${e} {component_upgrade:{upgrades:[{type:0,amount:8}]}}`);
  await fixture.command(`data merge block ${f} {component_upgrade:{upgrades:[{type:0,amount:8},{type:1,amount:8}]}}`);
  await fixture.command(`data merge block ${e} ${energy(30000)}`);
  await fixture.command(`data merge block ${f} ${energy(480000)}`);
  const three = items([[0, 'minecraft:raw_iron', 1], [2, 'minecraft:raw_iron', 1], [4, 'minecraft:raw_iron', 1]]);
  await fixture.command(`data merge block ${e} ${three}`);
  await fixture.command(`data merge block ${f} ${three}`);
  await fixture.stepServer(1);
  const speed = await saved(e);
  assert.deepEqual([progressIn(speed), energyIn(speed)], [[1, 1, 0], 4400], `12 800 J a process: two run on 30 000 J: ${speed}`);
  const pf = await probe(f);
  assert.deepEqual([pf.capacity, pf.per_tick, pf.required], [480000, 800, 12], '8 + 8: 480 000 J, 800 J a tick, 12 ticks');
  await fixture.stepServer(11);
  const fast = await saved(f);
  log.upgrades = { speed, fast };
  assert.deepEqual([1, 3, 5].map(i => slotIn(fast, i, 'minecraft:iron_ingot')), [1, 1, 1], `done in 12 ticks: ${fast}`);
  assert.equal(energyIn(fast), 480000 - 36 * 800);
  fixture.mark('upgrades: 8 speed, 12 800 J a process; 8 + 8, 480 000 J, 12 ticks of 800 J');

  // F: a combining factory's shared extra slot; a sawing factory's secondary outputs.
  const g = at(16), h = at(18);
  await fixture.command(`setblock ${g} mekanism:basic_combining_factory`);
  await fixture.command(`setblock ${h} mekanism:basic_sawing_factory`);
  await fixture.command(`data merge block ${g} ${energy(60000)}`);
  await fixture.command(`data merge block ${h} ${energy(30000)}`);
  await fixture.command(`data merge block ${g} ${items([[0, 'minecraft:cut_copper', 1], [2, 'minecraft:cut_copper', 1], [6, 'minecraft:honeycomb', 3]])}`);
  await fixture.command(`data merge block ${h} ${items([[0, 'minecraft:torch', 4], [3, 'minecraft:torch', 4], [6, 'minecraft:torch', 4]])}`);
  await fixture.stepServer(200);
  const combined = await saved(g), sawn = await saved(h);
  log.shared = { combined, sawn };
  assert.deepEqual([slotIn(combined, 1, 'minecraft:waxed_cut_copper'), slotIn(combined, 3, 'minecraft:waxed_cut_copper'), slotIn(combined, 6, 'minecraft:honeycomb')], [1, 1, 1], `two processes, one extra slot: ${combined}`);
  assert.equal(energyIn(combined), 40000);
  assert.deepEqual([1, 4, 7].map(i => slotIn(sawn, i, 'minecraft:stick')), [1, 1, 1], `sticks: ${sawn}`);
  assert.deepEqual([2, 5, 8].map(i => slotIn(sawn, i, 'minecraft:coal')), [1, 1, 1], 'coal in the secondary slots');
  fixture.mark("a combining factory's processes share its extra slot; a sawing factory's give their secondaries");

  // F2: an energy cube's item in the energy slot gives its tier's output a tick: 4 000 J of a basic cube.
  const fe = at(22), ce = at(24);
  await fixture.command(`setblock ${fe} mekanism:basic_smelting_factory`);
  await fixture.command(`setblock ${ce} mekanism:enrichment_chamber`);
  const cube = (slot: number) => `{Items:[{Slot:${slot}b,id:"mekanism:basic_energy_cube",count:1,components:{"minecraft:custom_data":{mekanism:{energy_containers:[{container:0b,stored:1000000L}]}}}}]}`;
  await fixture.command(`data merge block ${fe} ${cube(6)}`);
  await fixture.command(`data merge block ${ce} ${cube(2)}`);
  await fixture.stepServer(1);
  const charging = await saved(fe);
  assert.equal(energyIn(charging), 4000, `4 000 J a tick of a basic cube: ${charging}`);
  assert(/stored: ?996000L/.test(charging), `the cube item keeps 996 000 J: ${charging}`);
  await fixture.stepServer(7);
  const charged = await saved(fe), chamber = await saved(ce);
  log.charging = { charged, chamber };
  assert.equal(energyIn(charged), 30000, 'the factory full after 8 ticks');
  assert(/stored: ?970000L/.test(charged), `970 000 J left in the cube: ${charged}`);
  assert.equal(energyIn(chamber), 20000, 'a chamber too');
  assert(/stored: ?980000L/.test(chamber), `980 000 J left: ${chamber}`);
  fixture.mark("an energy cube's item charges a factory and a chamber at its output, 4 000 J a tick");

  // G: a restart keeps the progress array, the sorting and the inputs. 30 ticks of five processes and 30 J more,
  // less than a process's tick: it waits there while the game restarts.
  const k = at(20);
  await fixture.command(`setblock ${k} mekanism:advanced_smelting_factory`);
  await fixture.command(`data merge block ${k} {sorting:1b}`);
  await fixture.command(`data merge block ${k} ${energy(30 * 250 + 30)}`);
  await fixture.command(`data merge block ${k} ${items([[0, 'minecraft:raw_iron', 5]])}`);
  await fixture.stepServer(30);
  const before = await saved(k);
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  const after = await saved(k);
  log.restart = { before, after };
  assert.deepEqual(progressIn(after), progressIn(before));
  assert.deepEqual(progressIn(before), [30, 30, 30, 30, 30], `five processes of one raw iron each: ${before}`);
  assert.match(after, /sorting: ?1b/);
  assert.equal(energyIn(after), 30);
  await fixture.command(`data merge block ${k} ${energy(50000)}`);
  await fixture.stepServer(1);
  assert.deepEqual(progressIn(await saved(k)), [31, 31, 31, 31, 31], 'it goes on where it was');
  fixture.mark('progress, sorting and inputs survive a restart');
  await fixture.hold(false);

  // H: the screen: a basic smelting factory's slots and its sorting tab.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${b}`), v => v.block === 'mekanism:basic_smelting_factory', 'the factory reaches the client');
  await fixture.raw(`interact ${b} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the factory screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5, 6, 7, 8], 'three inputs and outputs, the energy slot and the two upgrade slots');
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const [ox, oy] = [Math.trunc((960 / scale - 176) / 2), Math.trunc((540 / scale - 166) / 2)];
  const shot = async (name: string) => {
    await Bun.sleep(1500);
    await fixture.raw(`screenshot ${join(fixture.run, `factory-${name}.png`)}`);
  };
  await shot('basic');
  await fixture.raw(`click-at ${ox - 12} ${oy + 75}`);
  await until(() => saved(b), t => /sorting: ?1b/.test(t), 'the sorting tab turns sorting on');
  await shot('sorting');
  await fixture.raw('close');
  // The ultimate sawing factory: 34 pixels wider, 21 taller, nine processes of three slots.
  const us = at(10, 0, 6);
  await fixture.command(`tp @s ${x + 10.5} ${y} ${z + 8.5}`);
  await until(() => fixture.raw(`block ${us}`), v => v.block === 'mekanism:ultimate_sawing_factory', 'the ultimate factory reaches the client');
  await fixture.raw(`interact ${us} south`);
  const wide = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 329), 'its screen opens');
  assert.equal((wide.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).length, 30, '27 process slots, the energy slot and the two upgrade slots');
  await shot('ultimate-sawing');
  await fixture.raw('close');
  fixture.mark("the factory's screen shows its slots; its sorting tab turns sorting on");

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK factories', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK factories', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
