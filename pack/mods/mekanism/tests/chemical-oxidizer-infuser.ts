/** Mekanism's Chemical Oxidizer and Chemical Infuser, as the `mekanism` native bundle and its content mods make them
 * (docs/mekanism-chemicals.md, slice 4), under ATM10's configs. The numbers are those a NeoForge 1.21.1 server with
 * Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (`/tick step`, `data get block`; chem-work/m4
 * o1):
 *
 * - The oxidizer: a sulfur dust becomes 100 mB of sulfur dioxide in 100 ticks of 200 J; 8 speed and 8 energy upgrades
 *   6 ticks of 3200 J; a tank without room for the output waits with its progress; it ejects 1024 mB a tick out of its
 *   right (west, facing north); its output slot fills a basic chemical tank item 1000 mB a tick.
 * - The infuser: hydrogen and chlorine, either way round, make hydrogen chloride, an operation of 200 J a tick; three
 *   speed upgrades 8 operations of 1600 J, eight as many as the Joules allow (one of 51 200 J); a full output, an empty
 *   input or 150 J stop it; it ejects out of its front; its right takes oxygen, then its left no chlorine.
 * - Their screens show their slots. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-oxidizer-infuser', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
}

const OX = 'mekanism:chemical_oxidizer', INF = 'mekanism:chemical_infuser';
const H = 'mekanism:hydrogen', CL = 'mekanism:chlorine', HCL = 'mekanism:hydrogen_chloride', SO2 = 'mekanism:sulfur_dioxide';
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** A machine by `/mek probe`: progress, energy, each tank's mB, each slot's count. */
async function probe(pos: string): Promise<any> {
  const p = await mek(`mek probe ${pos}`);
  return { progress: p.progress, energy: p.energy, tanks: (p.tanks ?? []).map((t: any) => t.amount), slots: p.slots, tankIds: (p.tanks ?? []).map((t: any) => t.chemical) };
}
const tanks = (...list: ([string, number] | null)[]) =>
  `{chemical_tanks:[${list.map((t, i) => t && `{tank:${i}b,stored:{id:"${t[0]}",amount:${t[1]}L}}`).filter(Boolean).join(',')}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const upgrades = (speed: number, energyUp = 0) =>
  `{component_upgrade:{upgrades:[${[[0, speed], [1, energyUp]].filter(([, n]) => n > 0).map(([t, n]) => `{type:${t},amount:${n}}`).join(',')}]}}`;
const tankHolding = (chemical: string, mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:${mb}L}}]}`;
/** A basic chemical tank's mB, by `/mek probe`. */
const tankAmount = async (pos: string) => (await mek(`mek probe ${pos}`)).amount ?? 0;
const quietTank = '{component_config:{config2:[I;1,1,1,1,1,1],eject2:0b,config3:[I;0,0,0,0,0,0],eject3:0b}}';

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(30, 5, 10)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(30, -2, 10)} minecraft:stone`);
  await fixture.hold();

  // O1: two sulfur dusts, 80 000 J.
  const a = at(0);
  await fixture.command(`setblock ${a} ${OX}[facing=north]`);
  await fixture.command(`data merge block ${a} {Items:[{Slot:0b,id:"mekanism:dust_sulfur",count:2}]}`);
  await fixture.command(`data merge block ${a} ${energy(80000)}`);
  const first: number[][] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    const p = await probe(a);
    first.push([p.progress, p.energy]);
  }
  log.oxidizer = { first, probe: await mek(`mek probe ${a}`) };
  assert.deepEqual(first, [[1, 79800], [2, 79600], [3, 79400]]);
  await fixture.stepServer(96);
  const p99 = await probe(a);
  await fixture.stepServer(1);
  const p100 = await probe(a);
  log.oxidizer_end = { p99, p100 };
  assert.deepEqual([p99.progress, p99.energy, p99.tanks[0]], [99, 60200, 0]);
  assert.deepEqual([p100.progress, p100.energy, p100.tanks[0], p100.tankIds[0], p100.slots[0].count], [0, 60000, 100, SO2, 1]);
  fixture.mark('a sulfur dust becomes 100 mB of sulfur dioxide in 100 ticks of 200 J');

  // O2: 8 speed and 8 energy upgrades; O3: no room for the output.
  const b = at(2), c = at(4);
  await fixture.command(`setblock ${b} ${OX}[facing=north]`);
  await fixture.command(`data merge block ${b} {Items:[{Slot:0b,id:"mekanism:dust_sulfur",count:2}]}`);
  await fixture.command(`data merge block ${b} ${upgrades(8, 8)}`);
  await fixture.command(`data merge block ${b} ${energy(1000000)}`);
  await fixture.command(`setblock ${c} ${OX}[facing=north]`);
  await fixture.command(`data merge block ${c} {Items:[{Slot:0b,id:"mekanism:dust_sulfur",count:2}],progress:50}`);
  await fixture.command(`data merge block ${c} ${tanks([SO2, 9950])}`);
  await fixture.command(`data merge block ${c} ${energy(80000)}`);
  const fast: number[][] = [];
  for (let t = 1; t <= 6; t++) {
    await fixture.stepServer(1);
    const p = await probe(b);
    fast.push([p.progress, p.energy, p.tanks[0]]);
  }
  log.fast = fast;
  assert.deepEqual(fast, [[1, 996800, 0], [2, 993600, 0], [3, 990400, 0], [4, 987200, 0], [5, 984000, 0], [0, 980800, 100]]);
  const full = await probe(c);
  assert.deepEqual([full.progress, full.energy, full.tanks[0]], [50, 80000, 9950], 'no room for 100 mB: it waits');
  fixture.mark('8 speed and 8 energy upgrades: 6 ticks of 3200 J; a full tank waits with its progress');

  // O4: it ejects out of its right (west, facing north) at 1024 mB a tick; O5: its output slot fills a tank item.
  const d = at(8), west = at(7), east = at(9);
  await fixture.command(`setblock ${d} ${OX}[facing=north]`);
  await fixture.command(`data merge block ${d} ${tanks([SO2, 3000])}`);
  for (const t of [west, east]) {
    await fixture.command(`setblock ${t} mekanism:basic_chemical_tank[facing=north]`);
    await fixture.command(`data merge block ${t} ${quietTank}`);
  }
  const ejected: number[][] = [];
  for (let i = 1; i <= 3; i++) {
    await fixture.stepServer(1);
    ejected.push([(await probe(d)).tanks[0], await tankAmount(west), await tankAmount(east)]);
  }
  log.ejected = ejected;
  assert.deepEqual(ejected, [[1976, 1024, 0], [952, 2048, 0], [0, 3000, 0]]);
  const e = at(12);
  await fixture.command(`setblock ${e} ${OX}[facing=north]`);
  await fixture.command(`data merge block ${e} {component_config:{config2:[I;0,0,4,0,0,0],eject2:0b}}`);
  await fixture.command(`data merge block ${e} ${tanks([SO2, 3000])}`);
  await fixture.command(`data merge block ${e} {Items:[{Slot:1b,id:"mekanism:basic_chemical_tank",count:1}]}`);
  const drained: number[][] = [];
  for (let i = 1; i <= 2; i++) {
    await fixture.stepServer(1);
    const p = await probe(e);
    drained.push([p.tanks[0], p.slots[1]?.chemical?.amount ?? 0]);
  }
  log.drained = drained;
  assert.deepEqual(drained, [[2000, 1000], [1000, 2000]]);
  assert.equal((await mek(`mek chemical ${e} west extract 10`)).got.amount, 10, 'its right gives');
  assert.equal((await mek(`mek chemical ${e} west insert ${SO2} 10`)).rest.amount, 10, 'and takes nothing');
  assert.equal((await mek(`mek chemical ${e} east tanks`)).handler, false, 'its left has no chemical side');
  fixture.mark('it ejects 1024 mB a tick out of its right; its output slot fills a tank item 1000 mB a tick');

  // I1, I2: hydrogen and chlorine, either way round.
  const f = at(0, 0, 4), g = at(2, 0, 4);
  await fixture.command(`setblock ${f} ${INF}[facing=north]`);
  await fixture.command(`data merge block ${f} ${tanks([H, 1000], [CL, 1000])}`);
  await fixture.command(`data merge block ${f} ${energy(80000)}`);
  await fixture.command(`setblock ${g} ${INF}[facing=north]`);
  await fixture.command(`data merge block ${g} ${tanks([CL, 1000], [H, 1000])}`);
  await fixture.command(`data merge block ${g} ${energy(80000)}`);
  const inf: any[] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    const p = await probe(f), q = await probe(g);
    inf.push([...p.tanks, p.energy, ...q.tanks]);
  }
  log.infuser = inf;
  assert.deepEqual(inf, [1, 2, 3].map(t => [1000 - t, 1000 - t, t, 80000 - 200 * t, 1000 - t, 1000 - t, t]));
  assert.equal((await probe(f)).tankIds[2], HCL);
  fixture.mark('hydrogen and chlorine, either way round, make hydrogen chloride at 200 J an operation');

  // I3: three speed upgrades, eight; I4: a full output, an empty input, 150 J.
  const h = at(4, 0, 4), i = at(6, 0, 4), j = at(8, 0, 4), k = at(10, 0, 4), l = at(12, 0, 4);
  for (const [pos, speed] of [[h, 3], [i, 8]] as [string, number][]) {
    await fixture.command(`setblock ${pos} ${INF}[facing=north]`);
    await fixture.command(`data merge block ${pos} ${upgrades(speed)}`);
    await fixture.command(`data merge block ${pos} ${tanks([H, 1000], [CL, 1000])}`);
    await fixture.command(`data merge block ${pos} ${energy(80000)}`);
  }
  await fixture.command(`setblock ${j} ${INF}[facing=north]`);
  await fixture.command(`data merge block ${j} ${tanks([H, 1000], [CL, 1000], [HCL, 10000])}`);
  await fixture.command(`setblock ${k} ${INF}[facing=north]`);
  await fixture.command(`data merge block ${k} ${tanks([H, 1000])}`);
  await fixture.command(`setblock ${l} ${INF}[facing=north]`);
  await fixture.command(`data merge block ${l} ${tanks([H, 1000], [CL, 1000])}`);
  for (const pos of [j, k]) await fixture.command(`data merge block ${pos} ${energy(80000)}`);
  await fixture.command(`data merge block ${l} ${energy(150)}`);
  const speedRows: number[][] = [];
  for (let t = 1; t <= 2; t++) {
    await fixture.stepServer(1);
    const p = await probe(h), q = await probe(i);
    speedRows.push([p.tanks[2], p.energy, q.tanks[2], q.energy]);
  }
  log.speed = speedRows;
  assert.deepEqual(speedRows, [[8, 67200, 1, 28800], [16, 54400, 1, 28800]]);
  const stopped = [await probe(j), await probe(k), await probe(l)].map(p => [...p.tanks, p.energy]);
  log.stopped = stopped;
  assert.deepEqual(stopped, [[1000, 1000, 10000, 80000], [1000, 0, 0, 80000], [1000, 1000, 0, 150]]);
  fixture.mark('three speed upgrades: 8 operations of 1600 J; eight as far as the Joules go; a full output, no input or 150 J stop it');

  // I5: it ejects out of its front; I6: its right takes oxygen, then its left no chlorine.
  const m = at(16, 0, 4), front = at(16, 0, 3);
  await fixture.command(`setblock ${m} ${INF}[facing=north]`);
  await fixture.command(`data merge block ${m} ${tanks([H, 1000], [CL, 1000], [HCL, 500])}`);
  await fixture.command(`data merge block ${m} ${energy(80000)}`);
  await fixture.command(`setblock ${front} mekanism:basic_chemical_tank[facing=north]`);
  await fixture.command(`data merge block ${front} ${quietTank}`);
  const out: number[][] = [];
  for (let t = 1; t <= 2; t++) {
    await fixture.stepServer(1);
    out.push([(await probe(m)).tanks[2], await tankAmount(front)]);
  }
  log.infuser_eject = out;
  assert.deepEqual(out, [[1, 500], [1, 501]]);
  const n = at(20, 0, 4), westTank = at(19, 0, 4), eastTank = at(21, 0, 4);
  await fixture.command(`setblock ${n} ${INF}[facing=north]`);
  await fixture.command(`setblock ${westTank} mekanism:basic_chemical_tank[facing=east]`);
  await fixture.command(`data merge block ${westTank} ${tankHolding('mekanism:oxygen', 5000)}`);
  await fixture.stepServer(1);
  await fixture.command(`setblock ${eastTank} mekanism:basic_chemical_tank[facing=west]`);
  await fixture.command(`data merge block ${eastTank} ${tankHolding(CL, 5000)}`);
  await fixture.stepServer(2);
  const sides = await probe(n);
  log.sides = { sides, chlorine: await tankAmount(eastTank) };
  assert.deepEqual([sides.tankIds[1], sides.tanks[1], sides.tanks[0] ?? 0], ['mekanism:oxygen', 3000, 0], 'oxygen into its right (west)');
  assert.equal(await tankAmount(eastTank), 5000, 'no chlorine into its left: no recipe takes it with oxygen');
  assert.equal((await mek(`mek chemical ${n} north extract 5`)).got.amount, 0, 'its front gives only the centre, empty');
  fixture.mark('it ejects out of its front; its right takes oxygen, then its left no chlorine');

  // Their screens (the world running again).
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  for (const [pos, block, count, name, face] of [[a, OX, 5, 'oxidizer', 'south'], [f, INF, 6, 'infuser', 'north']] as [string, string, number, string, string][]) {
    await until(() => fixture.raw(`block ${pos}`), v => v.block === block, `the ${name} reaches the client`);
    await fixture.raw(`interact ${pos} ${face}`);
    const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), `the ${name} screen opens`);
    const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
    log[`${name}_slots`] = own;
    assert.deepEqual(own, [...Array(count).keys()], `the ${name}'s slots and the upgrade component's`);
    await Bun.sleep(1500);
    await fixture.raw(`screenshot ${join(fixture.run, `${name}.png`)}`);
    await fixture.raw('close');
    await Bun.sleep(300);
  }
  fixture.mark('their screens show their slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK oxidizer infuser', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK oxidizer infuser', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
