/** Mekanism's Electric Pump and Fluidic Plenisher, as the `mekanism` native bundle and its content mods make them,
 * under ATM10's configs (`general.toml` `[pump]`: range 80, heavyWaterAmount 50, maxPlenisherNodes 4000; 100 J a
 * tick and 40 000 J each). The numbers are what a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs
 * gave for the same steps over RCON (local `chem-work/g2`, w2):
 *
 * - A pump over a lava pool, a basic mechanical pipe and a basic fluid tank on top: nothing and no Joules for 18
 *   ticks, then on the 19th the source under it goes into its tank (1000 mB) and up the pipe at 256 mB a tick;
 *   from then 100 J a tick and a source every 19 ticks, the block under it first, then the sources beside those it
 *   took; after 122 ticks six sources are gone and the tank on top holds their 6000 mB.
 * - Over water it takes 1000 mB an operation and the water stays (`waterSourceConversion`); with a filter upgrade
 *   50 mB of heavy water.
 * - A plenisher holding 10 000 mB of lava over a closed 3x3 hole: 100 J every tick while it walks, a source every
 *   20 ticks (the middle, then north, south, west, east, then the corners), finished after the tenth operation
 *   with 1000 mB left; its tank takes a fluid from an active fluid tank above it.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mods `mekanism` and `mekanismgenerators`
 * (`MEK_CONTENT_DIR`, `MEKGEN_CONTENT_DIR`); `pack/scripts/mekanism.ts world` sets them. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-pump', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/general.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), '[pump]\n\trange = 80\n\theavyWaterAmount = 50\n\tmaxPlenisherNodes = 4000\n');
}

const PUMP = 'mekanism:electric_pump', PLENISHER = 'mekanism:fluidic_plenisher';
const L = 'minecraft:lava', W = 'minecraft:water';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const probe = (pos: string) => mek(`mek probe ${pos}`);
async function source(pos: string, fluid: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${fluid}[level=0]`);
  return (reply.chat ?? []).some((t: string) => t.includes('Test passed'));
}
const energy = (j: number) => `energy_containers:[{container:0b,stored:${j}L}]`;
const held = (fluid: string, mb: number) => `fluid_tanks:[{tank:0b,stored:{id:"${fluid}",amount:${mb}}}]`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const y = Math.min(Math.floor(py) + 2, 78);
  const x = Math.floor(px) + 4, z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, 0, -2)} ${at(20, 6, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -5, -2)} ${at(20, -1, 8)} minecraft:stone`);

  // A lava pool of 3x3x2 under the floor, the pump in the floor over its middle, a pipe and a tank on top.
  await fixture.command(`fill ${at(0, -3, 0)} ${at(2, -2, 2)} ${L}`);
  const pump = at(1, -1, 1), pipe = at(1, 0, 1), tank = at(1, 1, 1);
  await fixture.command(`setblock ${pump} ${PUMP}[facing=north]`);
  await fixture.command(`setblock ${pipe} mekanism:basic_mechanical_pipe`);
  await fixture.command(`setblock ${tank} mekanism:basic_fluid_tank`);
  const pool: string[] = [];
  for (let dx = 0; dx < 3; dx++) for (let dy = -3; dy <= -2; dy++) for (let dz = 0; dz < 3; dz++) pool.push(at(dx, dy, dz));
  const lavaLeft = async () => (await Promise.all(pool.map(p => source(p, L)))).filter(Boolean).length;
  // Water: a pool of 3x3 under another, and one with a filter upgrade.
  await fixture.command(`fill ${at(6, -2, 0)} ${at(8, -2, 2)} ${W}`);
  await fixture.command(`fill ${at(11, -2, 0)} ${at(13, -2, 2)} ${W}`);
  const water = at(7, -1, 1), heavy = at(12, -1, 1);
  await fixture.command(`setblock ${water} ${PUMP}[facing=north]`);
  await fixture.command(`setblock ${heavy} ${PUMP}[facing=north]`);
  await fixture.hold();
  await fixture.command(`data merge block ${pump} {${energy(40000)}}`);
  await fixture.command(`data merge block ${water} {${energy(40000)}}`);
  await fixture.command(`data merge block ${heavy} {${energy(40000)},component_upgrade:{upgrades:[{type:2,amount:1}]}}`);
  assert.equal(await lavaLeft(), 18);

  await fixture.stepServer(18);
  const idle = await probe(pump);
  await fixture.stepServer(1);
  const first = await probe(pump);
  log.first = { idle, first };
  assert.deepEqual([idle.energy, idle.tank.amount, idle.progress], [40000, 0, 18], 'nothing and no Joules for 18 ticks');
  assert.deepEqual([first.energy, first.tank.fluid, first.tank.amount, first.active_type], [39900, L, 744, L], 'the 19th: 1000 mB, 256 up the pipe');
  assert.deepEqual(first.recurring_nodes, [[x + 1, y - 2, z + 1]], 'the block under it');
  assert.equal(await source(at(1, -2, 1), L), false);
  await fixture.stepServer(122 - 19);
  const after = await probe(pump);
  const top = await probe(tank);
  const left = await lavaLeft();
  log.lava = { after, top, left };
  assert.equal(after.energy, 40000 - 100 * (122 - 18), '100 J a tick from its first source on');
  assert.equal(left, 12, 'six sources gone');
  assert.equal(top.amount, 6000, 'their 6000 mB in the tank on top');
  assert.equal(after.tank.amount, 0);
  const [w, h] = await Promise.all([water, heavy].map(probe));
  log.water = { w, h };
  assert.deepEqual([w.tank.fluid, w.tank.amount], [W, 6000], 'water: 1000 mB an operation');
  assert.equal(await source(at(7, -2, 1), W), true, 'the water stays');
  assert.deepEqual([h.tank.fluid, h.tank.amount], ['mekanism:heavy_water', 300], 'a filter: 50 mB of heavy water an operation');
  await fixture.hold(false);
  fixture.mark('pump: a lava source every 19 ticks up a pipe into a tank (6000 mB in 122 ticks), 100 J a tick; water stays; a filter makes 50 mB of heavy water');

  // The plenisher over a closed 3x3 hole.
  await fixture.command(`fill ${at(16, -2, 0)} ${at(18, -2, 2)} minecraft:air`);
  const plenisher = at(17, -1, 1);
  await fixture.command(`setblock ${plenisher} ${PLENISHER}[facing=north]`);
  await fixture.hold();
  await fixture.command(`data merge block ${plenisher} {${energy(40000)},${held(L, 10000)}}`);
  const hole: string[] = [];
  for (let dx = 0; dx < 3; dx++) for (let dz = 0; dz < 3; dz++) hole.push(at(16 + dx, -2, dz));
  const filled = async () => (await Promise.all(hole.map(p => source(p, L)))).map((s, i) => (s ? i : -1)).filter(i => i >= 0);
  const order: number[][] = [];
  for (let op = 1; op <= 10; op++) {
    await fixture.stepServer(20);
    order.push(await filled());
  }
  const pl = await probe(plenisher);
  log.plenisher = { order, probe: pl };
  // Hole cells by index: dx * 3 + dz, the middle 4; north is dz 0.
  const firsts = order.map((now, i) => now.find(c => !(order[i - 1] ?? []).includes(c)));
  assert.deepEqual(firsts.slice(0, 5), [4, 3, 5, 1, 7], 'the middle, then north, south, west, east');
  assert.equal(order[8].length, 9, 'all nine after nine operations');
  assert.deepEqual([pl.finished, pl.tank.amount, pl.energy], [true, 1000, 40000 - 100 * 200], 'finished after the tenth, 100 J a tick while it walked');
  await fixture.stepServer(40);
  assert.equal((await probe(plenisher)).energy, 20000, 'no Joules once finished');
  await fixture.hold(false);
  fixture.mark('plenisher: a source every 20 ticks, the middle first, then out; finished with 1000 mB left');

  // Its tank from an active fluid tank above it.
  const fed = at(4, -1, 6), feeder = at(4, 0, 6);
  await fixture.command(`setblock ${fed} ${PLENISHER}[facing=north]`);
  await fixture.command(`setblock ${feeder} mekanism:basic_fluid_tank`);
  await fixture.command(`data merge block ${feeder} {${held(W, 5000)},active_state:1b}`);
  const got = await until(() => probe(fed), p => p.tank.amount >= 5000, 'the plenisher\'s tank fills from above', 30000);
  log.fed = got.tank;
  assert.equal(got.tank.fluid, W);
  fixture.mark('its tank takes water from a fluid tank above it');

  // The pump's screen.
  const shown = at(10, 0, 6);
  await fixture.command(`setblock ${shown} ${PUMP}[facing=north]`);
  await fixture.command(`tp @s ${x + 10.5} ${y} ${z + 4.5}`);
  await until(() => fixture.raw(`block ${shown}`), v => v.block === PUMP, 'the pump reaches the client');
  await fixture.raw(`interact ${shown} north`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the pump screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4]);
  assert.equal(view.title?.[0]?.text, 'Electric Pump');
  await Bun.sleep(1000);
  await fixture.raw(`screenshot ${join(fixture.run, 'electric-pump.png')}`);
  await fixture.raw('close');
  fixture.mark('the pump\'s screen: its fluid, output and energy slots, its upgrade slots beside');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK pump', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK pump', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
