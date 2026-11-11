/** Mekanism's Digital Miner, as the `mekanism` native bundle and its content mods make it, under ATM10's configs
 * (`[digital_miner]` silkMultiplier 12, maxRadius 32, ticksPerMine 80; 1000 J a tick and 50 000 J; maxUpgradeMultiplier
 * 16). The numbers are what a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave for the same
 * steps over RCON (local `mekminer-work`, m1 and m2), miners set as finished and running so that their first tick
 * searches and the next mines:
 *
 * - Joules a tick: 1000 plain (radius 2, three layers), 12 000 with silk touch, 2000 at radius 32, 2910 at radius 20
 *   over the whole height, 1183 over 120 layers, 4000 with two speed upgrades, 500 with two energy upgrades (and
 *   100 000 J of store), 16 000 with eight of each, 24 000 with four speed, eight energy, silk touch and radius 32;
 *   none on the first tick (its search), from the second on.
 * - Its order over a 5x5x3 area at x, z 3006..3010 (four chunks) with a stone filter at eight speed and eight energy
 *   upgrades: a block on the second tick and every five after, chunk (187, 188) first, then (188, 188), (188, 187)
 *   and (187, 187) (fastutil's hash order), in each the top layer first, z and x downwards; air and dirt left (the
 *   oracle's cobblestone went into a chest behind its back-top bounding block every 11 ticks: not checked here yet).
 *   Here a restart halfway through: it searches again and goes on in the same order.
 * - A 3x3 layer of stone, iron ore, dirt, planks and cobblestone: an iron ores tag filter takes the two ores (raw
 *   iron, with silk touch the ores), a stone filter the three stones (cobblestone, with silk touch stone), a mod id
 *   filter `mine*` all nine, an inverse dirt filter all but the dirt; a required cobblestone replace target with
 *   none in the miner takes nothing and misses it, with two cobblestone in its slots replaces the three stones (each
 *   stone's drop the next one's replacement); an inverse planks replace target, not required, leaves the planks.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mods `mekanism` and `mekanismgenerators`
 * (`MEK_CONTENT_DIR`, `MEKGEN_CONTENT_DIR`); `pack/scripts/mekanism.ts world` sets them. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-miner', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/general.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[digital_miner]\n\tsilkMultiplier = 12\n\tmaxRadius = 32\n\tticksPerMine = 80\n');
}

const MINER = 'mekanism:digital_miner';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  const last = lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
  assert(!last.error, `${command}: ${last.error}`);
  return last;
}
const probe = (pos: string) => mek(`mek probe ${pos}`);
const energy = (j: number) => `energy_containers:[{container:0b,stored:${j}L}]`;
const FAST = 'component_upgrade:{upgrades:[{type:0,amount:8},{type:1,amount:8}]}';
const STONE = '{type:"miner_itemstack_filter",target_stack:{id:"minecraft:stone"}}';
async function isBlock(pos: string, block: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${block}`);
  return (reply.chat ?? []).some((t: string) => t.includes('Test passed'));
}
/** A miner as the oracle's: set finished and running, so its first tick searches again. */
async function miner(pos: string, extra: string) {
  await fixture.command(`setblock ${pos} minecraft:air`);
  await fixture.command(`setblock ${pos} ${MINER}[facing=north]`);
  const r = await fixture.command(`data merge block ${pos} {running:1b,state:3,${extra}}`);
  assert((r.chat ?? []).some((t: string) => t.includes('Modified block data')), `data merge ${pos}: ${JSON.stringify(r)}`);
}
const count = (p: any, item: string) => p.items.filter((s: any) => s.item === item).reduce((a: number, s: any) => a + s.count, 0);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const y = Math.min(Math.floor(py) + 2, 78);
  const x = Math.floor(px) + 4, z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-4, -2, -4)} ${at(40, 24, 8)} minecraft:air`);
  await fixture.hold();

  // E: Joules a tick by settings, the store's drop on the second and third tick.
  const e = at(0, 20, 0);
  const lo = y + 1, hi = y + 3;
  await fixture.command(`fill ${at(-2, 1, -2)} ${at(2, 3, 2)} minecraft:stone`);
  const cases: [string, string, number, number][] = [
    ['plain', `radius:2,min:${lo},max:${hi}`, 50000, 1000],
    ['silk', `radius:2,min:${lo},max:${hi},silk_touch:1b`, 50000, 12000],
    ['r32', `radius:32,min:${lo},max:${hi}`, 50000, 2000],
    ['r20_full', 'radius:20,min:-64,max:319', 50000, 2910],
    ['h120', `radius:2,min:${lo},max:${lo + 119}`, 50000, 1183],
    ['speed2', `radius:2,min:${lo},max:${hi},component_upgrade:{upgrades:[{type:0,amount:2}]}`, 50000, 4000],
    ['energy2', `radius:2,min:${lo},max:${hi},component_upgrade:{upgrades:[{type:1,amount:2}]}`, 100000, 500],
    ['s8e8', `radius:2,min:${lo},max:${hi},${FAST}`, 800000, 16000],
    ['s4silk_r32', `radius:32,min:${lo},max:${hi},silk_touch:1b,component_upgrade:{upgrades:[{type:0,amount:4},{type:1,amount:8}]}`, 800000, 24000],
  ];
  const seen: Record<string, number[]> = {};
  for (const [name, extra, stored, perTick] of cases) {
    await miner(e, `${extra},filters:[${STONE}],${energy(stored)}`);
    const seq = [];
    for (let t = 0; t < 3; t++) {
      await fixture.stepServer(1);
      const p = await probe(e);
      seq.push(p.energy);
      if (t === 2) assert.equal(p.per_tick, perTick, `${name}: ${perTick} J a tick`);
    }
    seen[name] = seq;
    assert.deepEqual(seq, [stored, stored - perTick, stored - 2 * perTick], `${name}: none on the search's tick, then ${perTick} a tick`);
  }
  log.energy = seen;
  await fixture.command(`setblock ${e} minecraft:air`);
  fixture.mark('Joules a tick as the oracle: 1000, 12 000 silk, 2000 radius 32, 2910, 1183, 4000, 500, 16 000, 24 000; none on the search tick');

  // S: drops and filters on a 3x3 layer each, nine miners at once (8 speed, 8 energy: a block every 5 ticks), each
  // fed by a creative cube under it (silk touch takes 192 000 J a tick).
  const L = ['minecraft:stone', 'minecraft:iron_ore', 'minecraft:dirt', 'minecraft:oak_planks', 'minecraft:stone',
    'minecraft:iron_ore', 'minecraft:cobblestone', 'minecraft:stone', 'minecraft:dirt'];
  const COBBLE_REPLACE = '{type:"miner_itemstack_filter",target_stack:{id:"minecraft:stone"},replace_target:"minecraft:cobblestone",requires_replacement:1b}';
  const S: [string, string][] = [
    ['tag', 'filters:[{type:"miner_tag_filter",tag:"minecraft:iron_ores"}]'],
    ['tag_silk', 'silk_touch:1b,filters:[{type:"miner_tag_filter",tag:"minecraft:iron_ores"}]'],
    ['stone', `filters:[${STONE}]`],
    ['stone_silk', `silk_touch:1b,filters:[${STONE}]`],
    ['modid', 'filters:[{type:"miner_modid_filter",modid:"mine*"}]'],
    ['inverse_dirt', 'inverse:1b,filters:[{type:"miner_itemstack_filter",target_stack:{id:"minecraft:dirt"}}]'],
    ['replace_missing', `filters:[${COBBLE_REPLACE}]`],
    ['replace_two', `Items:[{Slot:0b,id:"minecraft:cobblestone",count:2}],filters:[${COBBLE_REPLACE}]`],
    ['inverse_replace', 'inverse:1b,replace_target:"minecraft:oak_planks",inverse_replace:0b,filters:[{type:"miner_itemstack_filter",target_stack:{id:"minecraft:dirt"}}]'],
  ];
  const cell = (k: number, i: number) => at(4 * k - 1 + (i % 3), 1, -1 + Math.floor(i / 3));
  const minerAt = (k: number) => at(4 * k, 6, 0);
  for (let k = 0; k < S.length; k++) {
    for (let i = 0; i < 9; i++) await fixture.command(`setblock ${cell(k, i)} ${L[i]}`);
    await fixture.command(`setblock ${at(4 * k, 5, 0)} mekanism:creative_energy_cube`);
    await fixture.command(`data merge block ${at(4 * k, 5, 0)} {facing:"up"}`);
    await miner(minerAt(k), `radius:1,min:${y + 1},max:${y + 1},${FAST},${S[k][1]}`);
  }
  const replaced: string[][] = [];
  for (let t = 1; t <= 60; t++) {
    await fixture.stepServer(1);
    if (t === 2 || t === 7 || t === 12) replaced.push(await Promise.all([0, 4, 7].map(async i => (await isBlock(cell(7, i), 'minecraft:cobblestone')) ? 'c' : 's')));
  }
  const after: Record<string, string[]> = {};
  const probes: Record<string, any> = {};
  for (let k = 0; k < S.length; k++) {
    const left = [];
    for (let i = 0; i < 9; i++) left.push((await isBlock(cell(k, i), 'minecraft:air')) ? 'air' : L[i].slice(10));
    after[S[k][0]] = left;
    probes[S[k][0]] = await probe(minerAt(k));
  }
  const pick = (p: any) => ({ energy: p.energy, state: p.state, to_mine: p.to_mine, running: p.running, per_tick: p.per_tick, delay: p.delay, missing: p.missing });
  log.cube = await probe(at(0, 5, 0));
  log.drops = { after, replaced, state: Object.fromEntries(Object.entries(probes).map(([k, p]) => [k, pick(p)])), items: Object.fromEntries(Object.entries(probes).map(([k, p]) => [k, p.items])) };
  const A = 'air';
  assert.deepEqual(after.tag, ['stone', A, 'dirt', 'oak_planks', 'stone', A, 'cobblestone', 'stone', 'dirt']);
  assert.equal(count(probes.tag, 'minecraft:raw_iron'), 2, 'the ores give raw iron');
  assert.equal(count(probes.tag_silk, 'minecraft:iron_ore'), 2, 'with silk touch the ores');
  assert.deepEqual(after.stone, [A, 'iron_ore', 'dirt', 'oak_planks', A, 'iron_ore', 'cobblestone', A, 'dirt']);
  assert.equal(count(probes.stone, 'minecraft:cobblestone'), 3, 'stone gives cobblestone');
  assert.equal(count(probes.stone_silk, 'minecraft:stone'), 3, 'with silk touch stone');
  assert.deepEqual(after.modid, Array(9).fill(A), 'mine* takes all of minecraft');
  assert.deepEqual([count(probes.modid, 'minecraft:cobblestone'), count(probes.modid, 'minecraft:oak_planks'), count(probes.modid, 'minecraft:dirt'), count(probes.modid, 'minecraft:raw_iron')], [4, 1, 2, 2]);
  assert.deepEqual(after.inverse_dirt, [A, A, 'dirt', A, A, A, A, A, 'dirt'], 'inverse: all but the dirt');
  assert.deepEqual(after.replace_missing, L.map(b => b.slice(10)), 'a required replace target missing: nothing');
  assert.equal(probes.replace_missing.missing, 'minecraft:cobblestone');
  assert.deepEqual(replaced, [['s', 's', 'c'], ['s', 'c', 'c'], ['c', 'c', 'c']], 'the stones replaced at ticks 2, 7 and 12, highest index first');
  assert.equal(count(probes.replace_two, 'minecraft:cobblestone'), 2, 'each drop the next replacement');
  assert.deepEqual(after.inverse_replace, [A, A, 'dirt', 'oak_planks', A, A, A, A, 'dirt'], 'the inverse replace target stays');
  for (let k = 0; k < S.length; k++) await fixture.command(`setblock ${minerAt(k)} minecraft:air`);
  fixture.mark('filters (tag, item, mod id, inverse), silk touch drops and the replace block as the oracle');

  // O: the oracle's order at x, z 3006..3010, fed by a creative cube under it, ejecting into a chest; a restart halfway.
  // It starts full, as the oracle's was after its first tick (its cube ticked before it there).
  const X = 3008, Z = 3008, MY = 100;
  await fixture.command(`forceload add ${X - 16} ${Z - 16} ${X + 16} ${Z + 16}`);
  await fixture.command(`fill ${X - 3} ${MY - 20} ${Z - 3} ${X + 3} ${MY + 3} ${Z + 3} minecraft:air`);
  await fixture.command(`fill ${X - 2} ${MY - 18} ${Z - 2} ${X + 2} ${MY - 16} ${Z + 2} minecraft:stone`);
  const holes = [[X - 2, MY - 16, Z - 2], [X, MY - 17, Z], [X + 1, MY - 18, Z + 2]];
  const dirt = [[X + 2, MY - 16, Z + 2], [X - 1, MY - 17, Z + 1]];
  for (const p of holes) await fixture.command(`setblock ${p.join(' ')} minecraft:air`);
  for (const p of dirt) await fixture.command(`setblock ${p.join(' ')} minecraft:dirt`);
  const M = `${X} ${MY} ${Z}`, chest = `${X} ${MY + 1} ${Z + 2}`;
  await fixture.command(`setblock ${X} ${MY - 1} ${Z} mekanism:creative_energy_cube`);
  await fixture.command(`data merge block ${X} ${MY - 1} ${Z} {facing:"up"}`);
  await fixture.command(`setblock ${chest} minecraft:chest`);
  await miner(M, `radius:2,min:${MY - 18},max:${MY - 16},eject:1b,filters:[${STONE}],${FAST},${energy(800000)}`);
  const oracle = [[-1, -56, 2], [-2, -56, 2], [-1, -56, 1], [-2, -56, 1], [-1, -56, 0], [-2, -56, 0], [-1, -57, 2], [-2, -57, 2], [-2, -57, 1], [-1, -57, 0], [-2, -57, 0], [-1, -58, 2], [-2, -58, 2], [-1, -58, 1], [-2, -58, 1], [-1, -58, 0], [-2, -58, 0], [1, -56, 2], [0, -56, 2], [2, -56, 1], [1, -56, 1], [0, -56, 1], [2, -56, 0], [1, -56, 0], [0, -56, 0], [2, -57, 2], [1, -57, 2], [0, -57, 2], [2, -57, 1], [1, -57, 1], [0, -57, 1], [2, -57, 0], [1, -57, 0], [2, -58, 2], [0, -58, 2], [2, -58, 1], [1, -58, 1], [0, -58, 1], [2, -58, 0], [1, -58, 0], [0, -58, 0], [2, -56, -1], [1, -56, -1], [0, -56, -1], [2, -56, -2], [1, -56, -2], [0, -56, -2], [2, -57, -1], [1, -57, -1], [0, -57, -1], [2, -57, -2], [1, -57, -2], [0, -57, -2], [2, -58, -1], [1, -58, -1], [0, -58, -1], [2, -58, -2], [1, -58, -2], [0, -58, -2], [-1, -56, -1], [-2, -56, -1], [-1, -56, -2], [-1, -57, -1], [-2, -57, -1], [-1, -57, -2], [-2, -57, -2], [-1, -58, -1], [-2, -58, -1], [-1, -58, -2], [-2, -58, -2]]
    .map(([dx, oy, dz]) => [X + dx, MY + 40 + oy, Z + dz].join(' '));
  const order: string[] = [];
  const ticks: number[] = [];
  let t = 0;
  const record = async () => {
    const p = await probe(M);
    if (p.last_mined && order[order.length - 1] !== p.last_mined.join(' ')) {
      order.push(p.last_mined.join(' '));
      ticks.push(t);
    }
    return p;
  };
  for (; order.length < 35 && t < 400; ) {
    await fixture.stepServer(1);
    t++;
    await record();
  }
  // The oracle took its first on the second tick; here, a forceloaded chunk far from the player, the first comes on
  // the second or third (NOTES.md), then every five as there.
  assert(ticks[0] === 2 || ticks[0] === 3, `the first block on the second or third tick: ${ticks}`);
  assert.deepEqual(ticks.slice(1, 4).map((v, i) => v - ticks[i]), [5, 5, 5], 'then a block every five ticks');
  const halfway = await probe(M);
  await fixture.hold(false);
  await fixture.restart();
  await fixture.command(`forceload add ${X - 16} ${Z - 16} ${X + 16} ${Z + 16}`);
  await fixture.hold();
  for (; t < 900; ) {
    await fixture.stepServer(1);
    t++;
    if ((await record()).to_mine === 0 && order.length > 35) break;
  }
  await fixture.stepServer(30);
  const done = await probe(M);
  const reply = await fixture.command(`data get block ${chest} Items`);
  const inChest = [...(reply.chat ?? []).join(' ').matchAll(/count: (\d+)[^}]*id: "minecraft:cobblestone"|id: "minecraft:cobblestone"[^}]*count: (\d+)/g)]
    .reduce((a, m) => a + Number(m[1] ?? m[2]), 0);
  log.order = { order, ticks, halfway: { to_mine: halfway.to_mine, delay: halfway.delay }, done: { to_mine: done.to_mine, items: done.items, state: done.state }, inChest };
  // Before the restart every block is seen; the world runs unheld while it saves and loads, so a block or two may go
  // unseen then: what is seen after it keeps the oracle's order.
  assert.deepEqual(order.slice(0, 35), oracle.slice(0, 35), 'the oracle\'s order over the four chunks up to the restart');
  const where = order.map(p => oracle.indexOf(p));
  assert(where.every((w, i) => w >= 0 && (i === 0 || w > where[i - 1])), `after the restart still the oracle's order: ${JSON.stringify(where)}`);
  assert(order.length >= 66, `nearly all seen: ${order.length}`);
  assert.equal(halfway.to_mine, 35, 'halfway, 35 to mine');
  assert.equal(done.to_mine, 0);
  for (const p of dirt) assert(await isBlock(p.join(' '), 'minecraft:dirt'), 'the dirt stays');
  // The ejection into the chest isn't checked yet (NOTES.md): the oracle's 69 of 70 were there at the end.
  fixture.mark(`the oracle's order over four chunks, a block every 5 ticks, kept over a restart (chest read ${inChest}, miner ${count(done, 'minecraft:cobblestone')})`);

  fixture.report('passed');
  console.log(JSON.stringify(log));
  await fixture.close();
} catch (error) {
  console.error(JSON.stringify(log));
  fixture.report('failed', error);
  await fixture.close().catch(() => undefined);
  throw error;
}
