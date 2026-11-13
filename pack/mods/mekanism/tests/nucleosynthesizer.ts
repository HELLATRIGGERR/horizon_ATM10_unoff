/** Mekanism's Antiprotonic Nucleosynthesizer, as the `mekanism` native bundle makes it under ATM10's configs
 * (`machine-usage.toml` 100 000 J a run, `machine-storage.toml` 1 000 000 000 J). The numbers are those a NeoForge
 * 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (local
 * `meknucleo-work/lab`, n1 and n2; the per-tick recipe is the oracle's own datapack's, given here by a content mod):
 *
 * - Coal into a diamond (1000 ticks, 4 mB) on a full store: 100 runs of the recipe a tick (`sqrt(stored / 100 000)`),
 *   progress 100, 199, 298 ... 975, then 25 runs finish the diamond and 70 go on with the next coal (the 11th tick,
 *   893 000 000 J left, 6 mB).
 * - 1 000 000 J: 3, 2, 2, 1, 1, 1 runs, then none. 150 000 J: one run.
 * - Short of antimatter (3 mB of 4) no recipe is found: a saved progress of 50 stays. Cut to 3 mB under a cached
 *   recipe, the progress starts over.
 * - A saved progress past the 400 ticks a new machine requires starts over when the recipe is cached (500 -> 1);
 *   one short of them goes on (300 -> 301).
 * - Tin into iron (200 ticks, 1 mB), 63 iron out and a saved 150: 50 runs finish one, the other 50 find no room.
 * - A `per_tick_usage` recipe (stone into dirt, 2 mB, 50 ticks) takes 2 mB a run: two dirt and 200 mB on the first
 *   tick, 49 ticks and 98 mB of the next on the second; a saved 30 ticks with 10 used takes 4 mB for its next run.
 * - Antimatter goes in on every side and comes out on none; its screen shows its four slots and the upgrade's. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
// The oracle's per-tick recipe: none of ATM10's nucleosynthesizing recipes takes its antimatter a tick.
const lab = mkdtempSync(join(tmpdir(), 'mek-nucleo-'));
writeFileSync(join(lab, 'horizon-mod.toml'), 'id = "nlab"\nname = "Nucleosynthesizer lab"\nversion = "1"\napi = "0.2"\nside = "both"\ncapabilities = ["registries"]\n');
mkdirSync(join(lab, 'data/nlab/recipe'), { recursive: true });
writeFileSync(join(lab, 'data/nlab/recipe/stone_per_tick.json'), JSON.stringify({
  type: 'mekanism:nucleosynthesizing', chemical_input: { amount: 2, chemical: 'mekanism:antimatter' }, duration: 50,
  item_input: { count: 1, item: 'minecraft:stone' }, output: { count: 1, id: 'minecraft:dirt' }, per_tick_usage: true,
}));
const fixture = new WorldFixture('mekanism-nucleo', 'server-thread', { devMods: [content, genContent, lab] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml')), 'ATM10_DIR with its Mekanism configs');
mkdirSync(config, { recursive: true });
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const N = 'mekanism:antiprotonic_nucleosynthesizer', AM = 'mekanism:antimatter';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** progress, Joules, the tank's mB and `used_so_far`; the slots. */
async function row(pos: string): Promise<{ r: number[]; slots: any[]; p: any }> {
  const p = await mek(`mek probe ${pos}`);
  return { r: [p.progress, p.energy, p.tank?.amount ?? 0, p.used_so_far], slots: p.slots ?? [], p };
}
const count = (slots: any[], i: number) => slots[i]?.count ?? 0;
const tank = (mb: number, chemical = AM) => `{chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:${mb}L}}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const items = (list: [number, string, number][]) => `{Items:[${list.map(([s, id, n]) => `{Slot:${s}b,id:"${id}",count:${n}}`).join(',')}]}`;
async function place(pos: string, ...nbt: string[]) {
  await fixture.command(`setblock ${pos} ${N}[facing=north]`);
  for (const n of nbt) await fixture.command(`data merge block ${pos} ${n}`);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(24, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(24, -2, 8)} minecraft:stone`);
  await fixture.hold();

  // B: two coal, 10 mB, a full store; C: 10^6 J; E: 3 mB and a saved 50; H1, H2: saved 500 and 300 on 150 000 J;
  // K: 4 mB, cut to 3 after a tick; M: tin, 63 iron out and a saved 150; J1: the per-tick recipe; J4: its saved run.
  const coal = (n: number) => items([[1, 'minecraft:coal', n]]);
  const [b, c, e, h1, h2, k, m, j1, j4] = [0, 2, 4, 6, 8, 10, 12, 14, 16].map(dx => at(dx));
  await place(b, coal(2), tank(10), energy(1_000_000_000));
  await place(c, coal(1), tank(10), energy(1_000_000));
  await place(e, coal(2), tank(3), energy(1_000_000_000), '{progress:50}');
  await place(h1, coal(2), tank(10), energy(150_000), '{progress:500}');
  await place(h2, coal(2), tank(10), energy(150_000), '{progress:300}');
  await place(k, coal(2), tank(4), energy(1_000_000_000));
  await place(m, items([[1, 'mekanism:ingot_tin', 3], [2, 'minecraft:iron_ingot', 63]]), tank(10), energy(1_000_000_000), '{progress:150}');
  await place(j1, items([[1, 'minecraft:stone', 4]]), tank(500), energy(1_000_000_000));
  await place(j4, items([[1, 'minecraft:stone', 2]]), tank(500), energy(150_000), '{progress:30,used_so_far:10L}');

  const rows: Record<string, number[][]> = { b: [], c: [], j1: [] };
  const firsts: Record<string, any> = {};
  for (let t = 1; t <= 7; t++) {
    if (t === 2) await fixture.command(`data merge block ${k} ${tank(3)}`);
    await fixture.stepServer(1);
    for (const [name, pos] of [['b', b], ['c', c], ['j1', j1]] as [string, string][]) {
      const got = await row(pos);
      rows[name].push(got.r);
      if (t <= 3 && name === 'j1') rows[name][t - 1].push(count(got.slots, 1), count(got.slots, 2));
    }
    if (t === 1) {
      for (const [name, pos] of [['b', b], ['e', e], ['h1', h1], ['h2', h2], ['k', k], ['m', m], ['j4', j4]] as [string, string][]) firsts[name] = await row(pos);
    }
    if (t === 2) firsts.k2 = await row(k);
  }
  log.rows = rows;
  log.firsts = Object.fromEntries(Object.entries(firsts).map(([n, v]) => [n, { r: v.r, slots: v.slots }]));
  const bp = firsts.b.p;
  log.probe = bp;
  assert.deepEqual(rows.b.slice(0, 3), [[100, 990000000, 10, 0], [199, 980100000, 10, 0], [298, 970200000, 10, 0]]);
  assert.deepEqual([bp.recipe, bp.required, bp.per_tick, bp.capacity, bp.energy_used, bp.process_rate], ['mekanism:nucleosynthesizing/diamond', 1000, 100000, 1000000000, 10000000, '10000%']);
  fixture.mark('a full store runs the diamond recipe 100 times a tick, then 99 (sqrt of 10^9 / 100 000)');
  assert.deepEqual(rows.c.map(r => [r[0], r[1]]), [[3, 700000], [5, 500000], [7, 300000], [8, 200000], [9, 100000], [10, 0], [10, 0]]);
  fixture.mark('10^6 J run 3, 2, 2, 1, 1, 1 times, then none');
  assert.deepEqual(firsts.e.r, [50, 1000000000, 3, 0], 'short of antimatter: no recipe, the saved progress stays');
  assert.deepEqual(firsts.k.r, [100, 990000000, 4, 0]);
  assert.deepEqual(firsts.k2.r, [0, 990000000, 3, 0], 'cut to 3 mB under the cached recipe: it starts over');
  fixture.mark('no recipe short of antimatter; a cached one cut short starts over');
  assert.deepEqual([firsts.h1.r[0], firsts.h1.r[1], firsts.h2.r[0], firsts.h2.r[1]], [1, 50000, 301, 50000]);
  fixture.mark('a saved 500 starts over (past a new machine\'s 400 ticks), 300 goes on');
  const mr = firsts.m;
  assert.deepEqual([mr.r[0], mr.r[1], mr.r[2], count(mr.slots, 1), mr.slots[2]?.item, count(mr.slots, 2)], [0, 995000000, 9, 2, 'minecraft:iron_ingot', 64]);
  fixture.mark('63 iron out: 50 runs finish one, the rest find no room');
  assert.deepEqual(rows.j1.slice(0, 3), [[0, 990000000, 300, 0, 2, 2], [49, 980100000, 102, 49, 1, 3], [0, 980000000, 100, 0, 0, 4]]);
  assert.deepEqual(firsts.j4.r, [31, 50000, 496, 12], 'a saved 30 ticks with 10 used: 2 a run for the 20 left');
  fixture.mark('a per-tick recipe takes 2 mB a run, used_so_far saved');

  // B's 11th tick: 95 runs, 25 finish the diamond, 70 go on with the second coal.
  await fixture.stepServer(4);
  const b11 = await row(b);
  log.b11 = { r: b11.r, slots: b11.slots };
  assert.deepEqual([b11.r[0], b11.r[1], b11.r[2], count(b11.slots, 1), b11.slots[2]?.item, count(b11.slots, 2)], [70, 893000000, 6, 1, 'minecraft:diamond', 1]);
  fixture.mark('the diamond on the 11th tick, and 70 ticks of the next coal in the same tick');

  // Antimatter goes in on every side (`ADVANCED_MACHINE_INPUT_ONLY`), only a recipe's chemical, and never out.
  assert.equal((await mek(`mek chemical ${e} west insert ${AM} 5`)).rest.amount, 0);
  assert.equal((await mek(`mek chemical ${e} up insert ${AM} 5`)).rest.amount, 0);
  assert.equal((await mek(`mek chemical ${e} east insert mekanism:oxygen 5`)).rest.amount, 5, 'no recipe takes oxygen');
  assert.equal((await mek(`mek chemical ${e} north extract 5`)).got.amount, 0, 'nothing comes out');
  fixture.mark('antimatter goes in on every side and comes out on none');

  // Its screen (the world running again).
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${b}`), v => v.block === N, 'the nucleosynthesizer reaches the client');
  await fixture.raw(`interact ${b} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5], 'its chemical, input, output, energy and upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'nucleosynthesizer.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its six slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK nucleo', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK nucleo', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
