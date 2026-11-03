/** Mekanism's Enrichment Chamber, Crusher and Energized Smelter, as the `mekanism` native bundle and its content mod
 * make them, under ATM10's `general.toml` (`maxUpgradeMultiplier = 16`), `machine-usage.toml` (50 J a tick) and
 * `machine-storage.toml` (20 000 J). The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and the same
 * configs gave for the same steps:
 *
 * - With 20 000 J and stone, a chamber's progress counts 1 a tick and its store loses 50 J a tick; at tick 200 its
 *   output holds cracked stone bricks and the store 10 000 J. A crusher makes 6 bone meal of a bone and a smelter an
 *   iron ingot of raw iron in the same 200 ticks. It shows `active=true` while it works.
 * - Eight speed upgrades in its upgrade slot are still there after 20 ticks and installed on the 21st; with eight
 *   energy upgrades too, it holds 320 000 J, takes 800 J a tick and needs 12 ticks: 9 600 J an operation.
 * - Three redstone in its energy slot: 10 000 J a tick while they fit, the third waits.
 * - Its input taken out halfway: the progress starts over; put back, it counts from 1.
 * - Item eject on: the output goes to a chest on its right (west, facing north); a hopper above fills its input with
 *   what a recipe takes and leaves dirt; a hopper below takes its output.
 * - A creative energy cube's front fills it; AE2 reads its store as Forge Energy (20 000 J = 8 000 FE).
 * - Short of energy it waits and uses none; progress, energy, upgrades and items survive a save and restart, and with
 *   energy it goes on where it was.
 * - Its screen: the input, output and energy slots and the upgrade component's two, its side configuration (the
 *   items page changes a side) and upgrade window (uninstall puts one in the upgrade output slot); screenshots
 *   `machine-*.png` in the run folder.
 *
 * Joules, progress and items are read from the saved compound (`data get block`), the rest from `/mek probe`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-machines', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-usage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n');
  writeFileSync(join(config, 'machine-usage.toml'), 'enrichmentChamber = 50\ncrusher = 50\nenergizedSmelter = 50\nmetallurgicInfuser = 50\n');
  writeFileSync(join(config, 'machine-storage.toml'), 'enrichmentChamber = 20000\ncrusher = 20000\nenergizedSmelter = 20000\nmetallurgicInfuser = 20000\n');
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
/** `/mek probe` as a console command: it answers at once, also while the world is held. */
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
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(36, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(36, -2, 8)} minecraft:stone`);

  // A, B, C: one recipe each, 200 ticks of 50 J.
  await fixture.hold();
  const a = at(0), b = at(4), c = at(8);
  await fixture.command(`setblock ${a} mekanism:enrichment_chamber`);
  await fixture.command(`setblock ${b} mekanism:crusher`);
  await fixture.command(`setblock ${c} mekanism:energized_smelter`);
  for (const pos of [a, b, c]) await fixture.command(`data merge block ${pos} ${energy(20000)}`);
  await fixture.command(`data merge block ${a} ${items([[0, 'minecraft:stone', 1]])}`);
  await fixture.command(`data merge block ${b} ${items([[0, 'minecraft:bone', 1]])}`);
  await fixture.command(`data merge block ${c} ${items([[0, 'minecraft:raw_iron', 1]])}`);
  await fixture.stepServer(1);
  const first = await saved(a);
  assert.equal(progressIn(first), 1, `progress 1 after a tick: ${first}`);
  assert.equal(energyIn(first), 19950);
  await fixture.stepServer(198);
  const almost = await saved(a);
  log.chamber_199 = almost;
  assert.equal(progressIn(almost), 199);
  assert.equal(energyIn(almost), 10050, '199 ticks of 50 J');
  assert.equal(slotIn(almost, 1, 'minecraft:cracked_stone_bricks'), 0, 'nothing out yet');
  await fixture.stepServer(1);
  const done = await saved(a), crushed = await saved(b), smelted = await saved(c);
  log.done = { done, crushed, smelted };
  assert.equal(slotIn(done, 1, 'minecraft:cracked_stone_bricks'), 1, `the 200th tick makes cracked stone bricks: ${done}`);
  assert.equal(slotIn(done, 0, 'minecraft:stone'), 0, 'the stone is used');
  assert.equal(progressIn(done), 0);
  assert.equal(energyIn(done), 10000, '10 000 J for one operation, as on Java');
  assert.equal(slotIn(crushed, 1, 'minecraft:bone_meal'), 6, `a crusher makes 6 bone meal of a bone: ${crushed}`);
  assert.equal(slotIn(smelted, 1, 'minecraft:iron_ingot'), 1, `a smelter makes an iron ingot: ${smelted}`);
  assert.equal(energyIn(crushed), 10000);
  assert.equal(energyIn(smelted), 10000);
  const pa = await probe(a);
  log.probe_a = pa;
  assert.equal(pa.required, 200);
  assert.equal(pa.per_tick, 50);
  assert.equal(pa.capacity, 20000);
  assert.equal(pa.config.max_upgrade_multiplier, 16, 'ATM10 general.toml');
  assert(pa.recipes > 100, `the chamber read the enriching recipes: ${pa.recipes}`);
  fixture.mark('chamber, crusher and smelter: 200 ticks of 50 J each, outputs as on Java');

  // Active while it works.
  await fixture.command(`data merge block ${a} ${items([[0, 'minecraft:stone', 64]])}`);
  await fixture.command(`data merge block ${a} ${energy(20000)}`);
  await fixture.hold(false);
  await until(async () => String((await fixture.raw(`block ${a}`)).state ?? ''), s => s.includes('active=true'), 'a working chamber shows active=true');
  await fixture.hold();
  fixture.mark('a working chamber shows active=true');

  // D: upgrades from its slot, in on the 21st tick; eight speed and eight energy: 12 ticks of 800 J.
  const d = at(12);
  await fixture.command(`setblock ${d} mekanism:enrichment_chamber`);
  await fixture.command(`data merge block ${d} ${items([[3, 'mekanism:upgrade_speed', 8]])}`);
  await fixture.stepServer(20);
  assert.equal(slotIn(await saved(d), 3, 'mekanism:upgrade_speed'), 8, 'still in the slot after 20 ticks');
  await fixture.stepServer(1);
  const installed = await saved(d);
  assert.equal(slotIn(installed, 3, 'mekanism:upgrade_speed'), 0, `installed on the 21st tick: ${installed}`);
  assert(/component_upgrade: ?\{upgrades: ?\[\{(type: ?0, ?amount: ?8|amount: ?8, ?type: ?0)\}\]/.test(installed), `8 speed upgrades saved: ${installed}`);
  const speedOnly = await probe(d);
  assert.deepEqual([speedOnly.required, speedOnly.per_tick, speedOnly.capacity], [12, 12800, 20000], '8 speed: 12 ticks of 12 800 J');
  await fixture.command(`data merge block ${d} ${items([[3, 'mekanism:upgrade_energy', 8]])}`);
  await fixture.stepServer(21);
  const both = await probe(d);
  log.upgrades = { speedOnly, both };
  assert.deepEqual(both.upgrades, { speed: 8, energy: 8 });
  assert.deepEqual([both.required, both.per_tick, both.capacity], [12, 800, 320000], '8 + 8: 12 ticks of 800 J, 320 000 J');
  await fixture.command(`data merge block ${d} ${energy(320000)}`);
  await fixture.command(`data merge block ${d} ${items([[0, 'minecraft:stone', 1]])}`);
  await fixture.stepServer(11);
  assert.equal(slotIn(await saved(d), 1, 'minecraft:cracked_stone_bricks'), 0);
  await fixture.stepServer(1);
  const fast = await saved(d);
  assert.equal(slotIn(fast, 1, 'minecraft:cracked_stone_bricks'), 1, `done in 12 ticks: ${fast}`);
  assert.equal(energyIn(fast), 310400, '12 x 800 J, as on Java');
  fixture.mark('8 speed upgrades go in on the 21st tick; with 8 energy: 12 ticks, 800 J a tick, 320 000 J');

  // E: redstone in the energy slot.
  const e = at(16);
  await fixture.command(`setblock ${e} mekanism:enrichment_chamber`);
  await fixture.command(`data merge block ${e} ${items([[2, 'minecraft:redstone', 3]])}`);
  await fixture.stepServer(1);
  assert.equal(energyIn(await saved(e)), 10000);
  await fixture.stepServer(3);
  const charged = await saved(e);
  assert.equal(energyIn(charged), 20000);
  assert.equal(slotIn(charged, 2, 'minecraft:redstone'), 1, 'the third waits for room');
  fixture.mark('redstone in the energy slot: 10 000 J each while it fits');

  // F: the input taken out halfway.
  const f = at(20);
  await fixture.command(`setblock ${f} mekanism:enrichment_chamber`);
  await fixture.command(`data merge block ${f} ${energy(20000)}`);
  await fixture.command(`data merge block ${f} ${items([[0, 'minecraft:stone', 1]])}`);
  await fixture.stepServer(50);
  assert.equal(progressIn(await saved(f)), 50);
  await fixture.command(`data remove block ${f} Items`);
  await fixture.stepServer(1);
  assert.equal(progressIn(await saved(f)), 0, 'no input: from the start');
  await fixture.command(`data merge block ${f} ${items([[0, 'minecraft:stone', 1]])}`);
  await fixture.stepServer(1);
  assert.equal(progressIn(await saved(f)), 1, 'it counts from 1 again');
  fixture.mark('an input taken out halfway starts the progress over');

  // G: item eject into a chest on its right (west).
  const g = at(24), west = at(23), east = at(25);
  await fixture.command(`setblock ${g} mekanism:enrichment_chamber`);
  await fixture.command(`setblock ${west} minecraft:chest`);
  await fixture.command(`setblock ${east} minecraft:chest`);
  await fixture.command(`data merge block ${g} {component_config:{eject3:1b}}`);
  await fixture.command(`data merge block ${g} ${items([[1, 'minecraft:cobblestone', 5]])}`);
  await fixture.stepServer(2);
  const chest = await saved(west);
  log.eject = { chest, other: await saved(east), machine: await saved(g) };
  assert(/minecraft:cobblestone/.test(chest), `the output went to the west chest: ${chest}`);
  assert(!/minecraft:cobblestone/.test(await saved(east)), 'nothing in the east chest');
  assert.equal(slotIn(await saved(g), 1, 'minecraft:cobblestone'), 0, 'the whole stack moved');
  assert.equal((await probe(g)).item_config.right, 'output');
  fixture.mark('item eject puts the output in the chest on its right (west)');

  // H: hoppers: one above fills its input (stone, not dirt), one below takes its output.
  const h = at(28), above = at(28, 1), below = at(28, -1);
  await fixture.command(`setblock ${below} minecraft:hopper[facing=down]`);
  await fixture.command(`setblock ${h} mekanism:crusher`);
  await fixture.command(`setblock ${above} minecraft:hopper[facing=down]`);
  await fixture.command(`data merge block ${above} {Items:[{Slot:0b,id:"minecraft:dirt",count:1},{Slot:1b,id:"minecraft:stone",count:2}]}`);
  await fixture.command(`data merge block ${h} ${items([[1, 'minecraft:gravel', 3]])}`);
  await fixture.stepServer(40);
  const hopped = await saved(h), upper = await saved(above), lower = await saved(below);
  log.hoppers = { hopped, upper, lower };
  assert.equal(slotIn(hopped, 0, 'minecraft:stone'), 2, `the hopper above put the stone in: ${hopped}`);
  assert(upper.includes('minecraft:dirt'), `the dirt stays in the hopper: ${upper}`);
  assert(!/minecraft:dirt/.test(hopped), 'no dirt in the crusher');
  assert(lower.includes('minecraft:gravel'), `the hopper below took the output: ${lower}`);
  fixture.mark('a hopper fills the input with what a recipe takes; one below takes the output');

  // I: a creative cube's front fills a chamber; AE2 reads it as Forge Energy.
  const i = at(32), cube = at(32, 0, 1);
  await fixture.command(`setblock ${i} mekanism:enrichment_chamber`);
  await fixture.command(`setblock ${cube} mekanism:creative_energy_cube`);
  await fixture.stepServer(3);
  assert.equal(energyIn(await saved(i)), 20000, 'the cube fills it');
  const fe = await fixture.ae(`power ${i} south`);
  log.fe = fe.fe;
  assert.equal(fe.fe?.capacity, 8000, '20 000 J are 8 000 FE');
  fixture.mark('a creative cube fills a chamber; AE2 reads 8 000 FE');

  // J: paused for energy at progress 100 (30 J, less than a tick's). Save and restart: J keeps its progress, energy
  // and stone, D its upgrades; with energy J goes on where it was.
  const j = at(34);
  await fixture.command(`setblock ${j} mekanism:enrichment_chamber`);
  await fixture.command(`data merge block ${j} ${energy(30)}`);
  await fixture.command(`data merge block ${j} {progress:100}`);
  await fixture.command(`data merge block ${j} ${items([[0, 'minecraft:stone', 7]])}`);
  await fixture.stepServer(5);
  const before = await saved(j);
  assert.equal(progressIn(before), 100, `short of energy it waits: ${before}`);
  assert.equal(energyIn(before), 30, 'and uses none');
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  const after = await saved(j);
  log.restart = { before, after };
  assert.equal(progressIn(after), 100);
  assert.equal(energyIn(after), 30);
  assert.equal(slotIn(after, 0, 'minecraft:stone'), 7);
  assert.deepEqual((await probe(d)).upgrades, { speed: 8, energy: 8 }, 'upgrades survive');
  await fixture.command(`data merge block ${j} ${energy(20000)}`);
  await fixture.stepServer(1);
  assert.equal(progressIn(await saved(j)), 101, 'with energy it goes on where it was');
  fixture.mark('progress, energy, items and upgrades survive a restart; it goes on where it was');
  // Two speed upgrades in A for its upgrade window.
  await fixture.command(`data merge block ${a} {component_upgrade:{upgrades:[{type:0,amount:2}]}}`);
  await fixture.hold(false);

  // Its screen.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${a}`), v => v.block === 'mekanism:enrichment_chamber', 'the chamber reaches the client');
  await fixture.raw(`interact ${a} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the chamber screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4], 'input, output, energy and the two upgrade slots');
  assert.equal(view.title?.[0]?.text, 'Enrichment Chamber');
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const [ox, oy] = [Math.trunc((960 / scale - 176) / 2), Math.trunc((540 / scale - 166) / 2)];
  const click = (dx: number, dy: number, right = false) => fixture.raw(`click-at ${ox + dx} ${oy + dy}${right ? ' right' : ''}`);
  const shots: string[] = [];
  log.screenshots = shots;
  const shot = async (name: string) => {
    await Bun.sleep(1500);
    const path = join(fixture.run, `machine-${name}.png`);
    await fixture.raw(`screenshot ${path}`);
    shots.push(path);
  };
  await shot('main');
  // The side configuration, items page: the top side from Input to Output.
  await click(-12, 19);
  await click(-121, 63);
  await shot('items-config');
  await click(-75, 39);
  await until(() => saved(a), s => /config3: ?\[I; ?1, ?1, ?4, ?8, ?4, ?1\]/.test(s), 'the top side outputs items');
  fixture.mark("the items page changes the chamber's item side config");
  await click(-126, 10);
  // The upgrade window: one of A's two speed upgrades out, into the upgrade output slot.
  await click(189, 19);
  await shot('upgrades');
  await click(-42, 28);
  await until(() => saved(a), s => slotIn(s, 4, 'mekanism:upgrade_speed') === 1, 'one speed upgrade comes out');
  assert.deepEqual((await probe(a)).upgrades, { speed: 1 });
  await click(189, 19);
  await fixture.raw('close');
  fixture.mark('its screen shows the five slots, the side configuration and the upgrades');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK machines', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK machines', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
