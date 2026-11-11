/** Mekanism Generators' Industrial Turbine, as the `mekanism` native bundle makes it under ATM10's configs
 * (`generators.toml` `[turbine]`: 8 blades a coil, 43 478.262 mB a vent, 1280 a disperser, 128 000 a condenser,
 * 16 000 000 J and 6400 mB a block). The numbers are those a NeoForge 1.21.1 server with Mekanism and Mekanism
 * Generators 10.7.19 and ATM10's configs gave for the same builds over RCON (local `mekturb-work/t1.py` and `t2.py`;
 * the manager's saved data `mekanism_industrialturbine` the contents):
 *
 * - Forms 5x5x5 and up, odd in length and width: rotors from the bottom wall up to the rotational complex at the
 *   middle of a full disperser layer, coils touching the block above the complex, condensers above the layer, vents
 *   at its height or higher; a rotor needs a blade. Not with no blade, an even length, the complex off the middle, no
 *   coil, no vent, a vent below the layer or on an edge, a condenser below it, a gap in the rotors, a missing or an
 *   extra disperser, a coil apart from the others, or six rotors in a 5-wide turbine (too narrow; five are fine).
 * - A: 5x5x5, two blades, one coil, one condenser, creative steam: 43 478 mB a tick flow (one vent), 31 055 J a tick
 *   into an ultimate cube against a valve, 43 478 mB of water a tick, 32 000 of it into a basic fluid tank against
 *   the vent, the vent tank full (128 000) on the fifth tick. C: 7x7x7, four blades, two coils, condensers and vents:
 *   124 223 J and 86 956 mB a tick. D: 5x9x5, ten blades on one coil (eight carried): 124 223 J a tick. B: a basic
 *   chemical tank's 1000 mB a tick into a 5x5x5: 1994 mB and 4 J on the second tick, 7213 and 561 on the eighth,
 *   11 772 and 5876 on the twentieth (the flow is the steam times how full the tank is).
 * - Filled once and left (the oracle's turbines filled through their cache): idle, 116 522 mB and 31 055 J after the
 *   first tick, 84 859 and 53 671 after the second. A basic tank into a dumping turbine: 1000 mB left a tick (the dump
 *   takes twice the last input), 4 J a tick. Dumping and dumping excess after a fill take what the oracle's rules
 *   (`max(stored / 50, last input * 2)`, the excess over 85 %) take: the dumping turbine empties, the excess one
 *   keeps 680 000 of 800 000.
 * - A boiler's steam valve into a turbine's valve: 31 055 J a tick while the boiler boils (oracle: from its third
 *   tick). */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-turbine', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml')), 'ATM10_DIR with its Mekanism configs');
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml', 'generators.toml', 'generator-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const G = 'mekanismgenerators:';
const C = `${G}turbine_casing`, V = `${G}turbine_valve`, VT = `${G}turbine_vent`, R = `${G}turbine_rotor`;
const X = `${G}rotational_complex`, K = `${G}electromagnetic_coil`, N = `${G}saturating_condenser`;
const D = 'mekanism:pressure_disperser', GL = 'mekanism:structural_glass';
const BC = 'mekanism:boiler_casing', BV = 'mekanism:boiler_valve', E = 'mekanism:superheating_element';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const turbineOf = async (pos: string) => (await mek(`mek probe ${pos}`)).multiblocks?.[0];
async function dataOf(pos: string, path: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos} ${path}`);
  return (reply.chat ?? []).find((t: string) => t.includes('has the following block data')) ?? '';
}
const chemicalIn = async (pos: string) => Number((await dataOf(pos, 'chemical_tanks')).match(/amount: (\d+)L/)?.[1] ?? 0);
const fluidIn = async (pos: string) => Number((await dataOf(pos, 'fluid_tanks')).match(/amount: (\d+)/)?.[1] ?? 0);
const energyIn = async (pos: string) => Number((await dataOf(pos, 'energy_containers')).match(/stored: (\d+)L/)?.[1] ?? 0);
const bladesIn = async (pos: string) => Number((await dataOf(pos, 'blades')).match(/data: (\d+)/)?.[1] ?? -1);

type Inner = Record<string, string>;
const key = (x: number, y: number, z: number) => `${x},${y},${z}`;
/** The disperser ring at height `y` of a turbine `l` by `w`, around its middle. */
const ring = (l: number, w: number, y: number): Inner => {
  const out: Inner = {};
  const [cx, cz] = [(l - 1) / 2, (w - 1) / 2];
  for (let x = 1; x < l - 1; x++) for (let z = 1; z < w - 1; z++) if (x !== cx || z !== cz) out[key(x, y, z)] = D;
  return out;
};
/** A 5-wide turbine's inside: `rotors` rotors from the bottom, the complex and the ring above them, a coil above it. */
const base5 = (extra: Inner = {}, rotors = 1, coil = true): Inner => {
  const cy = rotors + 1;
  const out: Inner = {};
  for (let y = 1; y < cy; y++) out[key(2, y, 2)] = R;
  out[key(2, cy, 2)] = X;
  Object.assign(out, ring(5, 5, cy));
  if (coil) out[key(2, cy + 1, 2)] = K;
  return Object.assign(out, extra);
};
const without = (inner: Inner, ...keys: string[]): Inner => Object.fromEntries(Object.entries(inner).filter(([k]) => !keys.includes(k)));

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 60), z = Math.floor(pz) - 12;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (let dx = -3; dx <= 60; dx += 16) {
    const x1 = Math.min(dx + 15, 60);
    await fixture.command(`fill ${at(dx, -1, -3)} ${at(x1, 12, 50)} minecraft:air`);
    await fixture.command(`fill ${at(dx, -2, -3)} ${at(x1, -2, 50)} minecraft:stone`);
    await fixture.command(`fillbiome ${at(dx, -2, -3)} ${at(x1, 12, 50)} minecraft:plains`);
  }
  await fixture.hold();
  /** A box of `casing` of `size` (x, y, z) at (dx, -1, dz) on the stone, the inside air, then `inner` and `edits`
   * [dx, dy, dz, block] (dy from its bottom wall). Its bottom corner's position. */
  const box = async (casing: string, dx: number, dz: number, size: number[], inner: Inner, edits: [number, number, number, string][] = []) => {
    const [l, h, w] = size;
    await fixture.command(`fill ${at(dx, -1, dz)} ${at(dx + l - 1, h - 2, dz + w - 1)} ${casing}`);
    await fixture.command(`fill ${at(dx + 1, 0, dz + 1)} ${at(dx + l - 2, h - 3, dz + w - 2)} minecraft:air`);
    for (const [k, b] of Object.entries(inner)) {
      const [ix, iy, iz] = k.split(',').map(Number);
      await fixture.command(`setblock ${at(dx + ix, iy - 1, dz + iz)} ${b}`);
    }
    for (const [ex, ey, ez, b] of edits) await fixture.command(`setblock ${at(dx + ex, ey - 1, dz + ez)} ${b}`);
    return at(dx, -1, dz);
  };
  /** The block at (ex, ey, ez) of the structure whose corner is `corner`. */
  const of = (corner: string, ex: number, ey: number, ez: number) => {
    const [cx, cy, cz] = corner.split(' ').map(Number);
    return `${cx + ex} ${cy + ey} ${cz + ez}`;
  };
  /** A turbine, then its rotors' blades {key: n}. */
  const turbine = async (dx: number, dz: number, size: number[], inner: Inner, edits: [number, number, number, string][] = [], blades: Record<string, number> = { [key(2, 1, 2)]: 1 }) => {
    const corner = await box(C, dx, dz, size, inner, edits);
    for (const [k, n] of Object.entries(blades)) {
      const [bx, by, bz] = k.split(',').map(Number);
      await fixture.command(`data merge block ${of(corner, bx, by, bz)} {blades:${n}}`);
    }
    return corner;
  };

  // Formation.
  const vent: [number, number, number, string][] = [[2, 3, 0, VT]];
  const cases: [string, number[], Inner, [number, number, number, string][], boolean][] = [
    ['min-5x5x5', [5, 5, 5], base5(), vent, true],
    ['no-blades', [5, 5, 5], base5(), vent, false],
    ['even-6x5x5', [6, 5, 5], base5(), vent, false],
    ['complex-off-center', [5, 5, 5], base5({ [key(2, 2, 2)]: D, [key(1, 2, 1)]: X }), vent, false],
    ['no-coil', [5, 5, 5], base5({}, 1, false), vent, false],
    ['no-vent', [5, 5, 5], base5(), [], false],
    ['vent-below', [5, 5, 5], base5(), [[2, 1, 0, VT]], false],
    ['vent-at-complex', [5, 5, 5], base5(), [[2, 2, 0, VT]], true],
    ['condenser-below', [5, 5, 5], base5({ [key(1, 1, 1)]: N }), vent, false],
    ['rotor-gap', [5, 6, 5], { [key(2, 1, 2)]: R, [key(2, 3, 2)]: X, [key(2, 4, 2)]: K, ...ring(5, 5, 3) }, [[2, 4, 0, VT]], false],
    ['missing-disperser', [5, 5, 5], without(base5(), key(1, 2, 1)), vent, false],
    ['extra-disperser', [5, 5, 5], base5({ [key(1, 3, 1)]: D }), vent, false],
    ['coil-apart', [5, 5, 5], base5({ [key(1, 3, 1)]: K }), vent, false],
    ['5-rotors-5x9x5', [5, 9, 5], base5({}, 5), [[2, 7, 0, VT]], true],
    ['6-rotors-5x10x5', [5, 10, 5], base5({}, 6), [[2, 8, 0, VT]], false],
    ['glass-valve', [5, 5, 5], base5(), [...vent, [0, 2, 2, GL], [4, 1, 2, V], [2, 4, 2, V]], true],
    ['vent-edge', [5, 5, 5], base5(), [...vent, [0, 3, 0, VT]], false],
    ['condensers-mixed', [5, 5, 5], base5({ [key(1, 3, 1)]: N, [key(3, 3, 3)]: N, [key(1, 3, 3)]: N }), vent, true],
    ['top-vent', [5, 5, 5], base5(), [[2, 4, 1, VT]], true],
  ];
  const corners: Record<string, string> = {};
  for (const [i, [name, size, inner, edits]] of cases.entries()) {
    corners[name] = await turbine(7 * (i % 8), 12 * Math.floor(i / 8), size, inner, edits, name === 'no-blades' ? {} : undefined);
  }
  // Rates. A: 5x5x5, two blades, a coil, a condenser, a vent north; creative steam west, an ultimate cube east, a
  // basic fluid tank north of the vent. B: the same with no condenser, a basic tank of steam. C: 7x7x7, two rotors of
  // two blades, two coils, two condensers, two vents. D: 5x9x5, five rotors of two blades, one coil. E: a turbine fed
  // by a 5x5x5 boiler of one element (water and sodium from creative tanks). I, P and Q: filled once and left, idle,
  // dumping and dumping excess (P and Q set from their screens). P2: dumping, fed by a basic tank.
  const two = { [key(2, 1, 2)]: 2 };
  const a = await turbine(0, 32, [5, 5, 5], base5({ [key(1, 3, 1)]: N }), [[2, 3, 0, VT], [0, 1, 2, V], [4, 1, 2, V]], two);
  const b = await turbine(8, 32, [5, 5, 5], base5(), [[2, 3, 0, VT], [0, 1, 2, V]], two);
  const c = await turbine(15, 32, [7, 7, 7], { [key(3, 1, 3)]: R, [key(3, 2, 3)]: R, [key(3, 3, 3)]: X, [key(3, 4, 3)]: K, [key(3, 5, 3)]: K, [key(1, 4, 1)]: N, [key(5, 4, 5)]: N, ...ring(7, 7, 3) },
    [[3, 4, 0, VT], [3, 5, 0, VT], [0, 1, 3, V]], { [key(3, 1, 3)]: 2, [key(3, 2, 3)]: 2 });
  const five = Object.fromEntries([1, 2, 3, 4, 5].map(h => [key(2, h, 2), 2]));
  const d = await turbine(24, 32, [5, 9, 5], base5({}, 5), [[2, 7, 0, VT], [0, 1, 2, V]], five);
  const bo = await box(BC, 31, 32, [5, 5, 5], { [key(2, 1, 2)]: E, ...Object.fromEntries([1, 2, 3].flatMap(i => [1, 2, 3].map(k => [key(i, 2, k), D]))) },
    [[1, 4, 2, BV], [3, 4, 2, BV], [4, 3, 2, `${BV}[mode=output_steam]`]]);
  const e = await turbine(36, 32, [5, 5, 5], base5(), [[2, 3, 0, VT], [0, 3, 2, V]], two);
  const i5 = await turbine(43, 32, [5, 5, 5], base5(), [[2, 3, 0, VT], [0, 1, 2, V]], two);
  const p = await turbine(0, 42, [5, 5, 5], base5(), [[2, 3, 0, VT], [0, 1, 2, V]], two);
  const q = await turbine(8, 42, [5, 9, 5], base5({}, 5), [[2, 7, 0, VT], [0, 1, 2, V]], five);
  const p2 = await turbine(16, 42, [5, 5, 5], base5(), [[2, 3, 0, VT], [0, 1, 2, V]], two);
  // Two rotors on their own, for the blades.
  const r0 = at(26, -1, 43), r1 = at(26, 0, 43);
  await fixture.command(`setblock ${r0} ${R}`);
  await fixture.command(`setblock ${r1} ${R}`);
  await fixture.stepServer(2);

  const formed: Record<string, any> = {};
  for (const [name] of cases) formed[name] = await turbineOf(corners[name]);
  log.formation = Object.fromEntries(cases.map(([name]) => [name, !!formed[name]]));
  for (const [name, , , , forms] of cases) assert.equal(!!formed[name], forms, name);
  const min = formed['min-5x5x5'];
  log.min = min;
  assert.deepEqual([min.lower_volume, min.rotors, min.blades, min.coils, min.condensers, min.dispersers], [25, 1, 1, 1, 0, 8]);
  assert.deepEqual([min.capacities.steam, min.capacities.energy, min.capacities.water], [160_000, 2_000_000_000, 0]);
  assert.deepEqual([min.max_flow, min.max_production], [43_478, 15_527], 'one vent, one blade');
  assert.deepEqual([formed['5-rotors-5x9x5'].lower_volume, formed['5-rotors-5x9x5'].capacities.steam], [125, 800_000]);
  assert.deepEqual([formed['condensers-mixed'].condensers, formed['condensers-mixed'].capacities.water], [3, 384_000]);
  assert.equal(formed['top-vent'].vents[0].side, 'up');
  assert.equal(formed['glass-valve'].valves.length, 2);
  for (const [name, corner] of [['A', a], ['B', b], ['C', c], ['D', d], ['E', e], ['I', i5], ['P', p], ['Q', q], ['P2', p2]] as const) assert(await turbineOf(corner), `${name} formed`);
  assert(await turbineOf(bo), 'the boiler formed');
  fixture.mark(`refused: ${cases.filter(k => !k[4]).map(k => k[0]).join(', ')}; formed: ${cases.filter(k => k[4]).map(k => k[0]).join(', ')}; 5x5x5: 160 000 mB of steam, 2 000 000 000 J`);

  // The blades: used on the upper of two rotors they fill the lower first; a sneaking empty hand takes the top one.
  await fixture.hold(false);
  const [rx, ry, rz] = r0.split(' ').map(Number);
  await fixture.command(`tp @s ${rx + 0.5} ${ry} ${rz - 1.5}`);
  await fixture.replace('hotbar.0', `${G}turbine_blade`, 3);
  await fixture.select(0, `${G}turbine_blade`);
  for (let n = 0; n < 3; n++) {
    await fixture.raw(`interact ${r1} north`);
    await until(async () => (await bladesIn(r0)) + (await bladesIn(r1)), v => v === n + 1, `blade ${n + 1} on the rotors`);
  }
  const added = [await bladesIn(r0), await bladesIn(r1)];
  await fixture.replace('hotbar.0', 'minecraft:air');
  await fixture.sneakInteract(`${r0} north`);
  await until(() => bladesIn(r1), v => v === 0, 'the top blade comes off');
  const taken = [await bladesIn(r0), await bladesIn(r1)];
  log.blades = { added, taken };
  assert.deepEqual(added, [2, 1], 'the lower rotor fills first');
  assert.deepEqual(taken, [2, 0], 'the highest blade comes off first');
  fixture.mark('blades used on a rotor fill the column from the bottom; a sneaking empty hand takes the top one off');

  // The dump mode button of the screen: P to dumping (two presses), Q and P2 likewise.
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const [ox, oy] = [Math.trunc((960 / scale - 190) / 2), Math.trunc((540 / scale - 166) / 2)];
  const press = async (corner: string, times: number, want: string) => {
    const wall = of(corner, 2, 1, 0);
    const [wx, wy, wz] = wall.split(' ').map(Number);
    await fixture.command(`tp @s ${wx + 0.5} ${wy - 1} ${wz - 1.5}`);
    await fixture.raw(`interact ${wall} north`);
    await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the turbine screen opens');
    for (let n = 0; n < times; n++) await fixture.raw(`click-at ${ox + 178} ${oy + 77}`);
    await until(async () => (await turbineOf(corner))?.dumping, v => v === want, `dump mode ${want}`);
    await fixture.raw('close');
  };
  await press(p, 2, 'dumping');
  await press(q, 1, 'dumping_excess');
  await press(p2, 2, 'dumping');
  fixture.mark('the screen\'s dump mode button: idle, dumping excess, dumping');
  await fixture.hold();

  const creative = async (pos: string) => {
    await fixture.command(`setblock ${pos} mekanism:creative_chemical_tank`);
    await fixture.command(`data merge block ${pos} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:steam",amount:1000000000000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b}}`);
  };
  const basic = async (pos: string) => {
    await fixture.command(`setblock ${pos} mekanism:basic_chemical_tank`);
    await fixture.command(`data merge block ${pos} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:steam",amount:64000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b},dumping:0}`);
  };
  for (const t of [a, c, d, i5, p, q]) await creative(of(t, -1, 1, t === c ? 3 : 2));
  await basic(of(b, -1, 1, 2));
  await basic(of(p2, -1, 1, 2));
  const cube = of(a, 5, 1, 2), tank = of(a, 2, 3, -1);
  await fixture.command(`setblock ${cube} mekanism:ultimate_energy_cube`);
  await fixture.command(`data merge block ${cube} {component_config:{config0:[I;1,1,1,1,1,1],eject0:0b}}`);
  await fixture.command(`setblock ${tank} mekanism:basic_fluid_tank`);
  const water = of(bo, 1, 5, 2), sodium = of(bo, 3, 5, 2);
  await fixture.command(`setblock ${water} mekanism:creative_fluid_tank[active=true]`);
  await fixture.command(`data merge block ${water} {active_state:1b,fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:2147483647}}]}`);
  await fixture.command(`setblock ${sodium} mekanism:creative_chemical_tank`);
  await fixture.command(`data merge block ${sodium} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:superheated_sodium",amount:1000000000000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b}}`);
  const rows: Record<number, any> = {};
  let ticks = 0;
  const row = async () => {
    const probes = [];
    for (const t of [a, b, c, d, e, i5, p, q, p2]) probes.push(await turbineOf(t));
    const [ta, tb, tc, td, te, ti, tp, tq, tp2] = probes;
    return {
      a: { steam: ta.steam.amount, water: ta.water.amount, energy: ta.energy, cube: await energyIn(cube), tank: await fluidIn(tank) },
      b: { steam: tb.steam.amount, energy: tb.energy }, c: { water: tc.water.amount, energy: tc.energy }, d: { energy: td.energy },
      e: { steam: te.steam.amount, energy: te.energy }, i: { steam: ti.steam.amount, energy: ti.energy },
      p: { steam: tp.steam.amount, energy: tp.energy }, q: { steam: tq.steam.amount, energy: tq.energy }, p2: { steam: tp2.steam.amount, energy: tp2.energy },
    };
  };
  for (const t of [1, 2, 3, 4, 5, 8, 10, 20]) {
    await fixture.stepServer(t - ticks);
    ticks = t;
    if (t === 1) for (const tt of [i5, p, q]) await fixture.command(`setblock ${of(tt, -1, 1, 2)} minecraft:air`);
    rows[t] = await row();
  }
  log.rows = rows;
  assert.deepEqual([rows[1].a.steam, rows[2].a.water, rows[2].a.cube, rows[2].a.tank], [160_000, 11_478, 31_055, 32_000], 'A on its first two ticks');
  assert.deepEqual([rows[5].a.water, rows[5].a.cube, rows[5].a.energy], [128_000, 124_220, 0], 'A: the vent tank full, the cube 4 ticks of 31 055');
  assert.deepEqual([rows[2].c.water, rows[2].c.energy, rows[4].c.water], [86_956, 124_223, 256_000], 'C');
  assert.equal(rows[2].d.energy, 124_223, 'D: eight of ten blades carried');
  assert.deepEqual([2, 3, 8, 20].map(t => [rows[t].b.steam, rows[t].b.energy]), [[1994, 4], [2970, 21], [7213, 561], [11_772, 5876]], 'B fed 1000 mB a tick');
  assert.deepEqual([2, 3].map(t => [rows[t].i.steam, rows[t].i.energy]), [[116_522, 31_055], [84_859, 53_671]], 'I filled once, idle');
  assert.deepEqual([rows[2].p2.steam, rows[2].p2.energy, rows[3].p2.steam, rows[3].p2.energy], [1000, 4, 1000, 8], 'P2 dumping its input');
  assert.deepEqual([rows[2].p.steam, rows[2].p.energy, rows[3].p.energy], [0, 31_055, 31_055], 'P dumps what it holds');
  assert.deepEqual([rows[2].q.steam, rows[2].q.energy, rows[3].q.steam, rows[3].q.energy], [680_000, 124_223, 643_044, 229_813], 'Q dumps the excess');
  assert.equal(rows[10].e.energy - rows[5].e.energy, 5 * 31_055, 'E on the boiler\'s steam');
  assert(rows[4].e.energy >= 31_055, `E turning by the fourth tick: ${rows[4].e.energy}`);
  fixture.mark(`A: 31 055 J and 43 478 mB of water a tick; C 124 223 J and 86 956 mB; B ${rows[20].b.steam} mB, ${rows[20].b.energy} J at tick 20; dumping and excess as the oracle's rules; a boiler's steam turns E at 31 055 J a tick`);

  // The screen on a casing of A, its statistics page too.
  await fixture.hold(false);
  const aw = of(a, 2, 1, 0);
  const [awx, awy, awz] = aw.split(' ').map(Number);
  await fixture.command(`tp @s ${awx + 0.5} ${awy - 1} ${awz - 1.5}`);
  await fixture.raw(`interact ${aw} north`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the turbine screen opens');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'turbine-screen.png')}`);
  await fixture.raw(`click-at ${ox - 12} ${oy + 19}`);
  await Bun.sleep(1000);
  await fixture.raw(`screenshot ${join(fixture.run, 'turbine-stats.png')}`);
  await fixture.raw('close');
  fixture.mark('the turbine\'s screen opens on a casing, its statistics as a page');

  // Kept over a restart: C's water and Joules, Q's dump mode.
  const before = { c: await turbineOf(c), q: await turbineOf(q) };
  await fixture.restart();
  const afterC = await until(() => turbineOf(c), t => !!t, 'C formed again after the restart');
  const afterQ = await until(() => turbineOf(q), t => !!t, 'Q formed again after the restart');
  log.after = { c: afterC, q: afterQ };
  assert(afterC.energy >= before.c.energy && afterC.water.amount > 0, JSON.stringify({ before: before.c, after: afterC }));
  assert.equal(afterQ.dumping, 'dumping_excess');
  assert.equal(afterQ.blades, 10);
  fixture.mark(`after a restart C holds ${afterC.energy} J and ${afterC.water.amount} mB of water, Q still dumps the excess`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK turbine', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK turbine', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
