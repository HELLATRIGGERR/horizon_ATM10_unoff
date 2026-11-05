/** Mekanism's ore chain past the enrichment chamber (the Chemical Dissolution Chamber, the Chemical Washer and the
 * Chemical Crystallizer), as the `mekanism` native bundle and its content mods make them (docs/mekanism-chemicals.md,
 * slice 5), under ATM10's configs, with ATM10's recipes: All the Ores' (`MEK_ATO_CONTENT_DIR`, the content step's
 * content mod of its carry, which declares its slurries) dissolve its ores into its dirty slurries, wash them 5 mB and
 * 25 of water into 3 clean, and crystallize 200 mB into its crystals. The numbers are those a NeoForge 1.21.1 server
 * with Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON with Mekanism's own iron recipes
 * (chem-work/m5 c1):
 *
 * - The dissolution chamber: 100 ticks of 400 J an ore, 1000 mB of dirty slurry into its output tank on the last; its
 *   sulfuric acid drawn every tick at random (`inversePoisson(1)`: 0 to 3 mB, 91 in 101 ticks there), `used_so_far`
 *   counting it and back to 0 with the ore; 8 speed and 8 energy upgrades 6 ticks of 6400 J and about 256 mB a tick;
 *   8 chemical upgrades a sixteenth of it; no acid waits with its progress.
 * - The washer: an operation a tick of 200 J; 3 speed upgrades 8 of 1600 J; 8 speed one of 51 200 J as far as
 *   80 000 J go; a water bucket in its fluid slot empties into its tank and the bucket goes to the slot below.
 * - The crystallizer: 200 ticks of 400 J, the crystal and 200 mB taken on the last; 8 speed and 8 energy upgrades 12
 *   ticks of 6400 J; 150 mB does nothing.
 * - In a row (each ejecting out of its right into the next): an ore becomes crystals.
 * - Their screens show their slots. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const ato = resolve(process.env.MEK_ATO_CONTENT_DIR ?? join(root, '../.build/content/alltheores'));
if (!existsSync(join(ato, 'data/alltheores/mekanism/chemical'))) {
  throw new Error(`All the Ores' content with its slurries declared is needed: none at ${ato} (MEK_ATO_CONTENT_DIR; the content step makes it with MEK_CHEMICALS)`);
}
const fixture = new WorldFixture('mekanism-ore-chain', 'server-thread', { devMods: [content, genContent, ato] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
}

const DI = 'mekanism:chemical_dissolution_chamber', WA = 'mekanism:chemical_washer', CR = 'mekanism:chemical_crystallizer';
const SA = 'mekanism:sulfuric_acid', DIRTY = 'alltheores:dirty_aluminum', CLEAN = 'alltheores:clean_aluminum';
const ORE = 'alltheores:aluminum_ore', CRYSTAL = 'alltheores:aluminum_crystal';
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** A machine by `/mek probe`: progress, energy, each tank's mB and chemical, the fluid, the slots, `used_so_far`. */
async function probe(pos: string): Promise<any> {
  const p = await mek(`mek probe ${pos}`);
  return {
    progress: p.progress, energy: p.energy, tanks: (p.tanks ?? []).map((t: any) => t.amount), ids: (p.tanks ?? []).map((t: any) => t.chemical),
    fluid: p.fluid?.amount ?? 0, slots: p.slots ?? [], used: p.used_so_far, recipes: p.recipes,
  };
}
const count = (slots: any[], i: number) => slots[i]?.count ?? 0;
const tanks = (...list: ([string, number] | null)[]) =>
  `{chemical_tanks:[${list.map((t, i) => t && `{tank:${i}b,stored:{id:"${t[0]}",amount:${t[1]}L}}`).filter(Boolean).join(',')}]}`;
const water = (mb: number) => `{fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:${mb}}}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const upgrades = (list: [number, number][]) => `{component_upgrade:{upgrades:[${list.map(([t, n]) => `{type:${t},amount:${n}}`).join(',')}]}}`;
const items = (list: [number, string, number][]) => `{Items:[${list.map(([s, id, n]) => `{Slot:${s}b,id:"${id}",count:${n}}`).join(',')}]}`;
async function place(pos: string, block: string, ...nbt: string[]) {
  await fixture.command(`setblock ${pos} ${block}[facing=north]`);
  for (const n of nbt) await fixture.command(`data merge block ${pos} ${n}`);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(30, 5, 10)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(30, -2, 10)} minecraft:stone`);
  await fixture.hold();

  // D1: two ores, 10 000 mB of acid, 160 000 J; D2: 8 speed and 8 energy; D3: 8 chemical upgrades; D4: no acid.
  const a = at(0), b = at(2), c = at(4), d = at(6);
  const ores = items([[1, ORE, 2]]);
  await place(a, DI, ores, tanks([SA, 10000]), energy(160000));
  await place(b, DI, ores, tanks([SA, 10000]), upgrades([[0, 8], [1, 8]]), energy(2000000));
  await place(c, DI, ores, tanks([SA, 10000]), upgrades([[3, 8]]), energy(160000));
  await place(d, DI, ores, '{progress:50}', energy(160000));
  const first: number[][] = [], fast: number[][] = [];
  for (let t = 1; t <= 6; t++) {
    await fixture.stepServer(1);
    const p = await probe(a), q = await probe(b);
    if (t <= 3) first.push([p.progress, p.energy, 10000 - p.tanks[0], p.used]);
    fast.push([q.progress, q.energy, q.tanks[1] ?? 0]);
  }
  log.dissolution = { first, fast, probe: await mek(`mek probe ${a}`) };
  assert((await probe(a)).recipes >= 13, 'All the Ores\' dissolution recipes');
  assert.deepEqual(first.map(r => r.slice(0, 2)), [[1, 159600], [2, 159200], [3, 158800]]);
  assert(first.every(([, , used, soFar]) => used === soFar && used <= 9), `acid drawn 0 to 3 a tick, counted in used_so_far: ${JSON.stringify(first)}`);
  assert.deepEqual(fast.map(r => r.slice(0, 2)), [[1, 1993600], [2, 1987200], [3, 1980800], [4, 1974400], [5, 1968000], [0, 1961600]]);
  assert.deepEqual(fast.map(r => r[2]), [0, 0, 0, 0, 0, 1000]);
  const acidFast = 10000 - (await probe(b)).tanks[0];
  assert(acidFast > 6 * 180 && acidFast < 6 * 340, `8 speed upgrades draw about 256 a tick: ${acidFast} in 6`);
  fixture.mark('8 speed and 8 energy upgrades: 6 ticks of 6400 J, about 256 mB of acid a tick, 1000 mB of dirty slurry');
  await fixture.stepServer(93);
  const p99 = await probe(a);
  await fixture.stepServer(1);
  const p100 = await probe(a);
  log.dissolution_end = { p99, p100 };
  assert.deepEqual([p99.progress, p99.energy, p99.tanks[1] ?? 0], [99, 120400, 0]);
  assert.deepEqual([p100.progress, p100.energy, p100.tanks[1], p100.ids[1], count(p100.slots, 1), p100.used], [0, 120000, 1000, DIRTY, 1, 0]);
  const acid = 10000 - p100.tanks[0];
  assert(acid >= 55 && acid <= 130, `0.91 mB a tick on average: ${acid} in 100 ticks`);
  const chem = await probe(c), none = await probe(d);
  log.chemical8 = chem;
  assert(10000 - chem.tanks[0] <= 30, `8 chemical upgrades: a sixteenth: ${10000 - chem.tanks[0]}`);
  assert.deepEqual([chem.tanks[1], chem.ids[1]], [1000, DIRTY]);
  assert.deepEqual([none.progress, none.energy], [50, 160000], 'no acid: it waits with its progress');
  fixture.mark('an ore becomes 1000 mB of dirty slurry in 100 ticks of 400 J, its acid drawn at random; no acid waits');

  // W1: 10 000 mB of water, 1000 of dirty slurry, 80 000 J; W2: 3 speed; W3: a water bucket; W4: 8 speed.
  const e = at(0, 0, 3), f = at(2, 0, 3), g = at(4, 0, 3), h = at(6, 0, 3);
  await place(e, WA, water(10000), tanks([DIRTY, 1000]), energy(80000));
  await place(f, WA, water(10000), tanks([DIRTY, 1000]), upgrades([[0, 3]]), energy(80000));
  await place(g, WA, water(12), tanks([DIRTY, 1000]), items([[0, 'minecraft:water_bucket', 1]]), energy(80000));
  await place(h, WA, water(10000), tanks([DIRTY, 1000]), upgrades([[0, 8]]), energy(80000));
  const washed: number[][] = [];
  for (let t = 1; t <= 2; t++) {
    await fixture.stepServer(1);
    const [p, q, r, s] = [await probe(e), await probe(f), await probe(g), await probe(h)];
    washed.push([p.tanks[0], p.tanks[1], p.fluid, p.energy, q.tanks[0], q.tanks[1], q.fluid, q.energy, r.fluid, s.tanks[1], s.energy]);
    if (t === 1) log.bucket = r.slots;
  }
  log.washer = washed;
  assert.deepEqual(washed, [
    [995, 3, 9975, 79800, 960, 24, 9800, 67200, 987, 3, 28800],
    [990, 6, 9950, 79600, 920, 48, 9600, 54400, 962, 3, 28800],
  ]);
  assert.equal((log.bucket as any[])[1]?.item, 'minecraft:bucket', 'the empty bucket in the slot below');
  assert.equal((await probe(e)).ids[1], CLEAN);
  fixture.mark('the washer: an operation a tick of 200 J (5 mB and 25 of water into 3), 3 speed 8 of 1600 J, 8 speed as far as the Joules go; a water bucket empties');

  // C1: 1000 mB of clean slurry, 160 000 J; C2: 8 speed and 8 energy; C3: 150 mB.
  const i = at(0, 0, 6), j = at(2, 0, 6), k = at(4, 0, 6);
  await place(i, CR, tanks([CLEAN, 1000]), energy(160000));
  await place(j, CR, tanks([CLEAN, 1000]), upgrades([[0, 8], [1, 8]]), energy(2000000));
  await place(k, CR, tanks([CLEAN, 150]), energy(160000));
  const crystals: number[][] = [];
  for (let t = 1; t <= 12; t++) {
    await fixture.stepServer(1);
    const q = await probe(j);
    crystals.push([q.progress, q.energy, q.tanks[0], count(q.slots, 1)]);
  }
  log.crystallizer_fast = crystals;
  assert.deepEqual(crystals.at(-1), [0, 1923200, 800, 1]);
  assert.deepEqual(crystals[10], [11, 1929600, 1000, 0]);
  await fixture.stepServer(187);
  const c199 = await probe(i);
  await fixture.stepServer(1);
  const c200 = await probe(i), short = await probe(k);
  log.crystallizer = { c199, c200, short };
  assert.deepEqual([c199.progress, c199.energy, c199.tanks[0]], [199, 80400, 1000]);
  assert.deepEqual([c200.progress, c200.energy, c200.tanks[0], c200.slots[1]?.item, count(c200.slots, 1)], [0, 80000, 800, CRYSTAL, 1]);
  assert.deepEqual([short.progress, short.energy, short.tanks[0]], [0, 160000, 150], '150 mB: nothing');
  fixture.mark('the crystallizer: 200 ticks of 400 J, a crystal and 200 mB on the last; 8 speed and 8 energy 12 ticks of 6400 J; 150 mB nothing');

  // A row, west to east: crystallizer, washer, dissolution chamber; each ejects out of its right (west) into the next.
  const cr = at(10, 0, 0), wa = at(11, 0, 0), di = at(12, 0, 0);
  await place(cr, CR, upgrades([[0, 8], [1, 8]]), energy(2000000));
  await place(wa, WA, water(10000), upgrades([[0, 3], [1, 8]]), energy(1000000));
  await place(di, DI, items([[1, ORE, 1]]), tanks([SA, 10000]), upgrades([[0, 8], [1, 8]]), energy(2000000));
  let row: any = null;
  for (let t = 1; t <= 60; t++) {
    await fixture.stepServer(1);
    row = await probe(cr);
    if (count(row.slots, 1) > 0) {
      log.row = { t, crystallizer: row, washer: await probe(wa), dissolution: await probe(di) };
      break;
    }
  }
  assert.equal(row.slots[1]?.item, CRYSTAL, `the row makes a crystal: ${JSON.stringify(log.row ?? row)}`);
  fixture.mark('in a row, an ore becomes All the Ores\' crystals');

  // Their screens (the world running again).
  await fixture.hold(false);
  for (const [pos, dz, block, slotCount, name] of [[a, 0, DI, 6, 'dissolution'], [e, 3, WA, 6, 'washer'], [i, 6, CR, 5, 'crystallizer']] as [string, number, string, number, string][]) {
    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + dz - 1.5}`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === block, `the ${name} reaches the client`);
    await fixture.raw(`interact ${pos} north`);
    const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), `the ${name} screen opens`);
    const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
    log[`${name}_slots`] = own;
    assert.deepEqual(own, [...Array(slotCount).keys()], `the ${name}'s slots and the upgrade component's`);
    await Bun.sleep(1500);
    await fixture.raw(`screenshot ${join(fixture.run, `${name}.png`)}`);
    await fixture.raw('close');
    await Bun.sleep(300);
  }
  fixture.mark('their screens show their slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK ore chain', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK ore chain', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
