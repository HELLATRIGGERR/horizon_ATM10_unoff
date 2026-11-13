/** Mekanism's Laser, Laser Amplifier and Laser Tractor Beam, as the `mekanism` native bundle makes them under ATM10's
 * configs (`machine-usage.toml` `laser` 50 000 J, `[laser] energyPerHardness` 100 000 J, the stores 10^7 and
 * 5 * 10^9 J). The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave for the
 * same steps over RCON (local `meklaser-work/lab`, l1 and l2):
 *
 * - A: a laser with 10^6 J fires 50 000 J a tick at stone three blocks in front; the stone (1.5) breaks on the third.
 * - B: 120 000 J fire 50 000, 50 000, 20 000; the tick without fire drops the progress and `last_fired`, so 30 000 J
 *   more don't break it.
 * - C: a laser into an amplifier: the amplifier fires the 50 000 J on in the same tick (holds none at its end).
 * - D: an amplifier with 10^6 J, min 300 000 and a delay of 2 fires all it holds on the second tick (dirt breaks); it
 *   has no active state, so its `last_fired` stays after.
 * - E: a tractor beam fires all its 10^6 J at once; the iron ore breaks into its first slot (one raw iron).
 * - F: an amplifier's redstone output on its energy contents: 2.5 * 10^9 of 5 * 10^9 J give 8, 1 J gives 1.
 * - G: bedrock takes the Joules and stays. H: an amplifier's max of 20 000 J breaks stone on the eighth tick. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-lasers', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml')), 'ATM10_DIR with its Mekanism configs');
mkdirSync(config, { recursive: true });
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const L = 'mekanism:laser', A = 'mekanism:laser_amplifier', T = 'mekanism:laser_tractor_beam';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
async function is(pos: string, state: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${state}`);
  return (reply.chat ?? []).some((t: string) => /passed/i.test(t));
}
async function power(pos: string): Promise<number | undefined> {
  for (let p = 0; p < 16; p++) if (await is(pos, `minecraft:redstone_wire[power=${p}]`)) return p;
  return undefined;
}
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 10;
  const at = (dx: number, dz = 0, dy = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -12, -1)} ${at(26, 3, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -12, -2)} ${at(26, 3, -2)} minecraft:stone`);
  await fixture.command(`fill ${at(-2, -12, -1)} ${at(26, 3, -1)} minecraft:stone`);
  await fixture.hold();
  async function put(dx: number, block: string, ...nbt: string[]) {
    await fixture.command(`setblock ${at(dx)} ${block}[facing=north]`);
    for (const n of nbt) await fixture.command(`data merge block ${at(dx)} ${n}`);
  }
  const set = (dx: number, dz: number, block: string) => fixture.command(`setblock ${at(dx, dz)} ${block}`);
  await put(0, L, energy(1_000_000)); await set(0, -3, 'minecraft:stone');
  await put(3, L, energy(120_000)); await set(3, -3, 'minecraft:stone');
  await put(6, L, energy(10_000_000));
  await fixture.command(`setblock ${at(6, -4)} ${A}[facing=north]`); await set(6, -7, 'minecraft:obsidian');
  await put(9, A, energy(1_000_000), '{min:300000L,time:2}'); await set(9, -3, 'minecraft:dirt');
  await put(12, T, energy(1_000_000)); await set(12, -2, 'minecraft:iron_ore');
  await put(15, A, energy(2_500_000_000), '{output_mode:2,min:5000000000L}'); await set(16, 0, 'minecraft:redstone_wire');
  await put(18, A, energy(1), '{output_mode:2,min:5000000000L}'); await set(19, 0, 'minecraft:redstone_wire');
  await put(21, L, energy(1_000_000)); await set(21, -3, 'minecraft:bedrock');
  await put(24, A, energy(1_000_000), '{max:20000L}'); await set(24, -3, 'minecraft:stone');

  const probe = (dx: number, dz = 0) => mek(`mek probe ${at(dx, dz)}`);
  const rows: any[] = [];
  for (let t = 1; t <= 8; t++) {
    await fixture.stepServer(1);
    if (t === 4) await fixture.command(`data merge block ${at(3)} ${energy(30_000)}`);
    const got: any[] = [];
    for (const [dx, dz] of [[0, 0], [3, 0], [6, 0], [6, -4], [9, 0], [12, 0], [21, 0], [24, 0]]) got.push(await probe(dx, dz));
    const [a, b, c, cAmp, d, e, g, h] = got;
    rows.push({
      t,
      A: [a.energy, a.last_fired, await is(at(0, -3), 'minecraft:stone')],
      B: [b.energy, b.last_fired, await is(at(3, -3), 'minecraft:stone')],
      C: [c.energy, cAmp.energy, cAmp.last_fired, await is(at(6, -7), 'minecraft:obsidian')],
      D: [d.energy, d.last_fired, await is(at(9, -3), 'minecraft:dirt')],
      E: [e.energy, await is(at(12, -2), 'minecraft:iron_ore'), (e.slots ?? []).map((s: any) => [s.slot, s.item, s.count])],
      G: [g.energy, await is(at(21, -3), 'minecraft:bedrock')],
      H: [h.energy, h.last_fired, await is(at(24, -3), 'minecraft:stone')],
    });
    if (t === 1) log.probes = { a, cAmp, d };
  }
  log.rows = rows;
  const col = (k: string) => rows.map(r => r[k]);
  assert.deepEqual(col('A').slice(0, 4), [[950000, 50000, true], [900000, 50000, true], [850000, 50000, false], [800000, 50000, false]]);
  fixture.mark('A: 50 000 J a tick break stone on the third tick');
  assert.deepEqual(col('B').slice(0, 6), [[70000, 50000, true], [20000, 50000, true], [0, 20000, true], [30000, 0, true], [0, 30000, true], [0, 0, true]]);
  fixture.mark('B: what is left fires; a tick without fire drops the progress and last_fired');
  assert.deepEqual(col('C').slice(0, 3), [[9950000, 0, 50000, true], [9900000, 0, 50000, true], [9850000, 0, 50000, true]]);
  fixture.mark('C: the amplifier fires the laser\'s Joules on in the same tick');
  assert.deepEqual(col('D').slice(0, 4), [[1000000, 0, true], [0, 1000000, false], [0, 1000000, false], [0, 1000000, false]]);
  fixture.mark('D: min 300 000 and a delay of 2: all of it on the second tick');
  assert.deepEqual(col('E')[0], [0, false, [[0, 'minecraft:raw_iron', 1]]]);
  fixture.mark('E: the tractor beam fires all it holds and keeps the raw iron');
  assert.deepEqual(col('G').slice(0, 3), [[950000, true], [900000, true], [850000, true]]);
  assert.deepEqual([col('H')[6], col('H')[7]], [[860000, 20000, true], [840000, 20000, false]]);
  fixture.mark('G: bedrock stays; H: a max of 20 000 J breaks stone on the eighth tick');
  log.power = [await power(at(16)), await power(at(19))];
  assert.deepEqual(log.power, [8, 1]);
  fixture.mark('F: the amplifier\'s energy contents as redstone: 8 at half, 1 at 1 J');

  // The tractor beam's screen (the world running again).
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 12.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${at(12)}`), v => v.block === T, 'the tractor beam reaches the client');
  await fixture.raw(`interact ${at(12)} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).length;
  log.slots = own;
  assert.equal(own, 27, 'its 27 slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'lasers.png')}`);
  await fixture.raw('close');
  fixture.mark('the tractor beam\'s screen shows its 27 slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK lasers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK lasers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
