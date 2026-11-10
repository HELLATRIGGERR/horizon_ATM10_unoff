/** Mekanism's tier installers (`ItemTierInstaller`), as the `mekanism` native bundle makes them: a basic installer
 * turns an Enrichment Chamber, a Precision Sawmill or a Combiner into the basic factory of its type, an advanced one a
 * basic factory into an advanced one, and so on, keeping what the block held (`getUpgradeData`, `parseUpgradeData`):
 *
 * - the chamber's input, output and energy slot go to process 0 and the factory's energy slot, its upgrades and the
 *   upgrade slots with them, its side configuration and ejecting, its facing, its Joules and its progress, which the
 *   factory's first recipe takes on (`loadSavedOperatingTicks`);
 * - its Joules up to the factory's own store before its upgrades come (`setEnergy` clamps first): 300 000 J of a
 *   chamber with eight energy upgrades become 30 000;
 * - a sawmill's secondary output goes to process 0's (slot 2), a combiner's extra input to the factory's extra slot;
 * - a basic factory's processes go to the same processes of the advanced one, its sorting too;
 * - an installer of the wrong tier changes nothing.
 *
 * The installer is used as a sneaking player's right click uses it (`native_use_item_on`, hook points API 424); the
 * world test runs the same conversion with `/mek install`, as the harness has no sneaking click. Everything is read
 * from the saved compound (`data get block`) and `/mek probe`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-installer', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-usage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n');
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
async function mek(line: string): Promise<any> {
  const reply = await fixture.command(`mek ${line}`);
  const answer = (reply.chat ?? []).findLast((t: string) => t.startsWith('MEK {'));
  assert(answer, `/mek ${line}: ${JSON.stringify(reply)}`);
  return JSON.parse(answer.slice(4));
}
async function probe(pos: string): Promise<any> {
  const answer = await mek(`probe ${pos}`);
  assert(!answer.error, `/mek probe ${pos}: ${JSON.stringify(answer)}`);
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
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(20, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(20, -2, 8)} minecraft:stone`);
  await fixture.hold();

  // A: a chamber, facing east, with energy, progress, items, upgrades and its ejector on, into a basic factory.
  const a = at(0);
  await fixture.command(`setblock ${a} mekanism:enrichment_chamber[facing=east]`);
  await fixture.command(`data merge block ${a} {component_upgrade:{upgrades:[{type:0,amount:2}]}}`);
  await fixture.command(`data merge block ${a} {component_config:{eject3:1b}}`);
  await fixture.command(`data merge block ${a} ${energy(15000)}`);
  await fixture.command(`data merge block ${a} {progress:40}`);
  await fixture.command(`data merge block ${a} ${items([[0, 'minecraft:stone', 5], [1, 'minecraft:cracked_stone_bricks', 3], [2, 'minecraft:redstone', 2], [4, 'mekanism:upgrade_energy', 1]])}`);
  const wrong = await mek(`install ${a} advanced_tier_installer`);
  assert(wrong.error, `an advanced installer doesn't take a chamber: ${JSON.stringify(wrong)}`);
  assert.match(await saved(a), /id: ?"mekanism:enrichment_chamber"/, 'unchanged');
  const made = await mek(`install ${a} basic_tier_installer`);
  assert.equal(made.block, 'mekanism:basic_enriching_factory', JSON.stringify(made));
  const fa = await saved(a);
  log.chamber = fa;
  const facing = await fixture.command(`execute if block ${a} mekanism:basic_enriching_factory[facing=east]`);
  assert((facing.chat ?? []).some((t: string) => t.includes('passed')), `it keeps its facing: ${JSON.stringify(facing.chat)}`);
  assert.deepEqual(progressIn(fa), [40, 0, 0], `progress to process 0: ${fa}`);
  assert.equal(energyIn(fa), 15000);
  assert.deepEqual([slotIn(fa, 0, 'minecraft:stone'), slotIn(fa, 1, 'minecraft:cracked_stone_bricks'), slotIn(fa, 6, 'minecraft:redstone'), slotIn(fa, 8, 'mekanism:upgrade_energy')], [5, 3, 2, 1], `slots moved: ${fa}`);
  const pa = await probe(a);
  log.probe_chamber = pa;
  assert.deepEqual([pa.processes, pa.upgrades, pa.item_eject], [3, { speed: 2 }, true], 'upgrades and ejecting come along');
  await fixture.stepServer(1);
  assert.equal(progressIn(await saved(a))[0], 41, 'the factory goes on where the chamber was (two speed upgrades: 100 ticks)');
  fixture.mark('a basic installer turns a chamber into a basic enriching factory with everything it held');

  // B: the energy clamps to the factory's own store.
  const b = at(2);
  await fixture.command(`setblock ${b} mekanism:enrichment_chamber`);
  await fixture.command(`data merge block ${b} {component_upgrade:{upgrades:[{type:1,amount:8}]}}`);
  await fixture.command(`data merge block ${b} ${energy(300000)}`);
  await fixture.stepServer(1);
  assert.equal(energyIn(await saved(b)), 300000, 'eight energy upgrades: 320 000 J');
  await mek(`install ${b} basic_tier_installer`);
  const fb = await saved(b);
  assert.equal(energyIn(fb), 30000, `setEnergy before the upgrades: 30 000 J: ${fb}`);
  assert.equal((await probe(b)).capacity, 480000, 'then the upgrades: 480 000 J of store');
  fixture.mark('the energy goes up to the new store before its upgrades');

  // C: a sawmill's secondary output and a combiner's extra input.
  const c = at(4), d = at(6);
  await fixture.command(`setblock ${c} mekanism:precision_sawmill`);
  await fixture.command(`setblock ${d} mekanism:combiner`);
  await fixture.command(`data merge block ${c} ${items([[0, 'minecraft:oak_log', 4], [1, 'minecraft:oak_planks', 12], [2, 'mekanism:sawdust', 3]])}`);
  await fixture.command(`data merge block ${d} ${items([[0, 'minecraft:cut_copper', 2], [1, 'minecraft:honeycomb', 7], [2, 'minecraft:waxed_cut_copper', 1]])}`);
  assert.equal((await mek(`install ${c} basic_tier_installer`)).block, 'mekanism:basic_sawing_factory');
  assert.equal((await mek(`install ${d} basic_tier_installer`)).block, 'mekanism:basic_combining_factory');
  const fc = await saved(c), fd = await saved(d);
  log.others = { fc, fd };
  assert.deepEqual([slotIn(fc, 0, 'minecraft:oak_log'), slotIn(fc, 1, 'minecraft:oak_planks'), slotIn(fc, 2, 'mekanism:sawdust')], [4, 12, 3], `sawmill slots: ${fc}`);
  assert.deepEqual([slotIn(fd, 0, 'minecraft:cut_copper'), slotIn(fd, 1, 'minecraft:waxed_cut_copper'), slotIn(fd, 6, 'minecraft:honeycomb')], [2, 1, 7], `combiner slots: ${fd}`);
  fixture.mark("a sawmill's secondary output and a combiner's extra input go to the factory's");

  // D: a basic factory into an advanced one, then on to the ultimate.
  const e = at(8);
  await fixture.command(`setblock ${e} mekanism:basic_smelting_factory`);
  await fixture.command(`data merge block ${e} {sorting:1b,progress:[I;10,20,30]}`);
  await fixture.command(`data merge block ${e} ${items([[0, 'minecraft:raw_iron', 1], [3, 'minecraft:iron_ingot', 4], [4, 'minecraft:raw_gold', 1]])}`);
  const elite = await mek(`install ${e} elite_tier_installer`);
  assert(elite.error, 'an elite installer takes an advanced factory, not a basic one');
  assert.equal((await mek(`install ${e} advanced_tier_installer`)).block, 'mekanism:advanced_smelting_factory');
  const fe = await saved(e);
  log.advanced = fe;
  assert.deepEqual(progressIn(fe), [10, 20, 30, 0, 0], `the processes keep their progress: ${fe}`);
  assert.deepEqual([slotIn(fe, 0, 'minecraft:raw_iron'), slotIn(fe, 3, 'minecraft:iron_ingot'), slotIn(fe, 4, 'minecraft:raw_gold')], [1, 4, 1]);
  assert.match(fe, /sorting: ?1b/, 'and the sorting');
  assert.equal((await mek(`install ${e} elite_tier_installer`)).block, 'mekanism:elite_smelting_factory');
  assert.equal((await mek(`install ${e} ultimate_tier_installer`)).block, 'mekanism:ultimate_smelting_factory');
  assert((await mek(`install ${e} ultimate_tier_installer`)).error, 'nothing above the ultimate');
  const pu = await probe(e);
  assert.deepEqual([pu.processes, pu.progress.slice(0, 3)], [9, [10, 20, 30]]);
  fixture.mark('advanced, elite and ultimate installers take a factory one tier up each');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK installer', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK installer', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
