/** Mekanism Generators' Fission Reactor, as the `mekanism` native bundle makes it under ATM10's configs
 * (`generators.toml` `[fission_reactor]`: 250 000 heat a mB of fissile fuel, 1000 of heat capacity a casing, 8000 mB of
 * fuel an assembly, 100 000 mB of coolant and 1 000 000 of heated coolant a block, meltdowns on at 0.001). The numbers
 * are those a NeoForge 1.21.1 server with Mekanism and Mekanism Generators 10.7.19 and ATM10's configs gave for the
 * same builds over RCON (local `mekfiss-work/f1.py` to `f3.py`; the manager's saved data `mekanism_fissionreactor`):
 *
 * - Forms 3x4x3 and up: columns of fuel assemblies, each under one control rod assembly, logic adapters and ports in
 *   its sides, reactor glass (not structural glass) in them; air above the rod or below the fuel is fine. Not with
 *   no assembly, fuel with no rod, a rod with no fuel, a gap under the rod or in the fuel, two rods, a rod under the
 *   fuel, an adapter or a port on an edge, structural glass, or dirt inside. A dynamic tank takes reactor glass.
 * - The 3x4x3: 34 000 of heat capacity holding 10 200 000.01013279 (plains), 8000 mB of fuel and waste, 3 600 000 of
 *   coolant, 36 000 000 of heated coolant. W (water, the default 0.1 mB/t, started by a logic adapter in activation
 *   mode and a redstone block): 10 225 000.01 heat after the first tick, 10 449 943.795735585 after the tenth, the
 *   first waste on the eleventh, steam from the 101st (67 mB, 24 012 by the 150th) into a basic chemical tank beside
 *   an output coolant port, the waste out of an output waste port. S (sodium): superheated sodium from about the
 *   100th tick, holding at 373.88 K. R (5x5x5, two columns of two, 4 mB/t set by `/mek fission ... rate`): 30 400 000
 *   heat after the first tick, 19 991 mB of steam a tick at 393.55 K.
 * - M (dry, 1 mB/t; the test's chance is 0 while it measures): 3.034 % damage at the 150th tick, 9.637 at the 200th,
 *   129.63 at the 1000th; its temperature
 *   adapter lights its lamp past 1200 K, its damaged adapter past 100 %. With meltdowns off at a chance of 1 it turns
 *   itself off (force disabled, refusing to start); with them on again it melts down: a blast in its box, the cache at
 *   75 % damage. The redstone block taken from W's adapter turns W off.
 * - Kept over a restart: R's rate and heat, W's adapter mode. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-fission', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml')), 'ATM10_DIR with its Mekanism configs');
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml', 'generators.toml', 'generator-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}
const generators = readFileSync(join(config, 'generators.toml'), 'utf8');
/** `generators.toml` with the meltdowns' `enabled` and `chance` set. */
const meltdowns = (enabled: boolean, chance: number) => writeFileSync(join(config, 'generators.toml'), generators
  .replace(/(\[fission_reactor\.meltdowns\][\s\S]*?enabled = )\w+/, `$1${enabled}`)
  .replace(/(\[fission_reactor\.meltdowns\][\s\S]*?chance = )[\d.eE-]+/, `$1${chance}`));
// No meltdown rolls while the rates are measured (M passes 100 % at its 803rd tick).
meltdowns(true, 0);

const G = 'mekanismgenerators:';
const F = `${G}fission_reactor_casing`, P = `${G}fission_reactor_port`, LA = `${G}fission_reactor_logic_adapter`;
const FA = `${G}fission_fuel_assembly`, CR = `${G}control_rod_assembly`, RG = `${G}reactor_glass`;
const SG = 'mekanism:structural_glass', DT = 'mekanism:dynamic_tank';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const multiblockOf = async (pos: string) => (await mek(`mek probe ${pos}`)).multiblocks?.[0];
async function is(pos: string, state: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${state}`);
  return (reply.chat ?? []).some((t: string) => /passed/i.test(t));
}
async function chemicalIn(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos} chemical_tanks`);
  const line = (reply.chat ?? []).find((t: string) => t.includes('has the following block data')) ?? '';
  return Number(line.match(/amount: (\d+)L/)?.[1] ?? 0);
}
const lit = (pos: string) => is(pos, 'minecraft:redstone_lamp[lit=true]');

type Inner = Record<string, string>;
const key = (x: number, y: number, z: number) => `${x},${y},${z}`;
const min: Inner = { [key(1, 1, 1)]: FA, [key(1, 2, 1)]: CR };
const column = (x: number, z: number, fuel: number): Inner => {
  const out: Inner = {};
  for (let i = 0; i < fuel; i++) out[key(x, 1 + i, z)] = FA;
  out[key(x, 1 + fuel, z)] = CR;
  return out;
};

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 60), z = Math.floor(pz) - 12;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (let dx = -3; dx <= 92; dx += 16) {
    const x1 = Math.min(dx + 15, 92);
    await fixture.command(`fill ${at(dx, -1, -3)} ${at(x1, 12, 36)} minecraft:air`);
    await fixture.command(`fill ${at(dx, -2, -3)} ${at(x1, -2, 36)} minecraft:stone`);
    await fixture.command(`fillbiome ${at(dx, -2, -3)} ${at(x1, 12, 36)} minecraft:plains`);
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
  const of = (corner: string, ex: number, ey: number, ez: number) => {
    const [cx, cy, cz] = corner.split(' ').map(Number);
    return `${cx + ex} ${cy + ey} ${cz + ez}`;
  };

  // Formation.
  const cases: [string, number[], Inner, [number, number, number, string][], boolean][] = [
    ['min-3x4x3', [3, 4, 3], min, [], true],
    ['fuel-only', [3, 3, 3], { [key(1, 1, 1)]: FA }, [], false],
    ['rod-only', [3, 3, 3], { [key(1, 1, 1)]: CR }, [], false],
    ['empty', [3, 3, 3], {}, [], false],
    ['rod-gap', [3, 5, 3], { [key(1, 1, 1)]: FA, [key(1, 3, 1)]: CR }, [], false],
    ['fuel-gap', [3, 6, 3], { [key(1, 1, 1)]: FA, [key(1, 3, 1)]: FA, [key(1, 4, 1)]: CR }, [], false],
    ['two-rods', [3, 5, 3], { [key(1, 1, 1)]: FA, [key(1, 2, 1)]: CR, [key(1, 3, 1)]: CR }, [], false],
    ['rod-below', [3, 5, 3], { [key(1, 1, 1)]: CR, [key(1, 2, 1)]: FA }, [], false],
    ['air-above-rod', [3, 5, 3], min, [], true],
    ['fuel-off-floor', [3, 5, 3], { [key(1, 2, 1)]: FA, [key(1, 3, 1)]: CR }, [], true],
    ['reactor-glass', [3, 4, 3], min, [[0, 1, 1, RG], [1, 3, 1, RG]], true],
    ['structural-glass', [3, 4, 3], min, [[0, 1, 1, SG]], false],
    ['port-adapter', [3, 4, 3], min, [[0, 1, 1, P], [2, 2, 1, LA], [1, 3, 1, P]], true],
    ['adapter-edge', [3, 4, 3], min, [[0, 1, 0, LA]], false],
    ['port-edge', [3, 4, 3], min, [[0, 0, 1, P]], false],
    ['dirt-inside', [3, 5, 3], min, [[1, 3, 1, 'minecraft:dirt']], false],
    ['two-adapters', [3, 4, 3], min, [[0, 1, 1, LA], [2, 1, 1, LA]], true],
    ['5x5x5-two-columns', [5, 5, 5], { ...column(1, 1, 2), ...column(2, 1, 2) }, [], true],
  ];
  const corners: Record<string, string> = {};
  for (const [i, [name, size, inner, edits]] of cases.entries()) corners[name] = await box(F, 7 * (i % 12), 8 * Math.floor(i / 12), size, inner, edits);
  const tank = await box(DT, 70, 8, [3, 3, 3], {}, [[0, 1, 1, RG]]);
  // Rates. W: water; S: sodium; R: 5x5x5, two touching columns of two, 4 mB/t; M: dry, 1 mB/t, apart from the rest.
  /** A reactor `n` wide and `top + 1` high: an input port west and on top, an output coolant port east, an output
   * waste port north, a logic adapter south. */
  const ports = (n: number, top: number): [number, number, number, string][] => [[0, 1, (n - 1) / 2, P], [(n - 1) / 2, top, (n - 1) / 2, P],
    [n - 1, 1, (n - 1) / 2, `${P}[mode=output_coolant]`], [(n - 1) / 2, 1, 0, `${P}[mode=output_waste]`], [(n - 1) / 2, 2, n - 1, LA]];
  const w = await box(F, 0, 20, [3, 4, 3], min, ports(3, 3));
  const s = await box(F, 10, 20, [3, 4, 3], min, ports(3, 3));
  const r = await box(F, 20, 20, [5, 5, 5], { ...column(1, 1, 2), ...column(2, 1, 2) }, ports(5, 4));
  // M within the chunks that tick near the player, 18 blocks and more from the rest (its meltdown blasts reach 10).
  const m = await box(F, 50, 28, [3, 4, 3], min, [[1, 1, 0, P], [0, 2, 1, LA], [2, 2, 1, LA], [1, 2, 2, LA]]);
  await fixture.stepServer(2);

  const formed: Record<string, any> = {};
  for (const [name] of cases) formed[name] = await multiblockOf(corners[name]);
  log.formation = Object.fromEntries(cases.map(([name]) => [name, !!formed[name]]));
  for (const [name, , , , forms] of cases) assert.equal(!!formed[name], forms, name);
  const small = formed['min-3x4x3'];
  log.min = small;
  assert.equal(small.kind, 'fissionReactor');
  assert.deepEqual([small.heat_capacity, small.heat, small.assemblies, small.surface_area], [34_000, 10_200_000.01013279, 1, 6]);
  assert.deepEqual(small.capacities, { fuel: 8000, waste: 8000, cooled: 3_600_000, heated: 36_000_000 });
  assert.deepEqual([formed['5x5x5-two-columns'].assemblies, formed['5x5x5-two-columns'].surface_area, formed['5x5x5-two-columns'].capacities.fuel], [4, 16, 32_000]);
  assert(await multiblockOf(tank), 'a dynamic tank takes reactor glass');
  for (const [name, c] of [['W', w], ['S', s], ['R', r], ['M', m]] as const) assert(await multiblockOf(c), `${name} formed`);
  fixture.mark(`refused: ${cases.filter(k => !k[4]).map(k => k[0]).join(', ')}; formed: ${cases.filter(k => k[4]).map(k => k[0]).join(', ')}; 3x4x3: 34 000 heat capacity, 8000 mB of fuel`);

  // Feeds and sinks.
  const creative = async (pos: string, chemical: string | null) => {
    await fixture.command(`setblock ${pos} mekanism:creative_chemical_tank`);
    await fixture.command(chemical
      ? `data merge block ${pos} {chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:1000000000000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b}}`
      : `data merge block ${pos} {component_config:{config2:[I;1,1,1,1,1,1],eject2:0b}}`);
  };
  const basic = async (pos: string) => {
    await fixture.command(`setblock ${pos} mekanism:basic_chemical_tank`);
    await fixture.command(`data merge block ${pos} {component_config:{config2:[I;1,1,1,1,1,1],eject2:0b}}`);
  };
  const water = async (pos: string) => {
    await fixture.command(`setblock ${pos} mekanism:creative_fluid_tank[active=true]`);
    await fixture.command(`data merge block ${pos} {active_state:1b,fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:2147483647}}]}`);
  };
  for (const [c, n, top] of [[w, 3, 3], [s, 3, 3], [r, 5, 4]] as const) {
    const h = (n - 1) / 2;
    await creative(of(c, -1, 1, h), 'mekanism:fissile_fuel');
    await basic(of(c, n, 1, h));
    await creative(of(c, h, 1, -1), null);
    if (c === s) await creative(of(c, h, top + 1, h), 'mekanism:sodium');
    else await water(of(c, h, top + 1, h));
    await fixture.command(`data merge block ${of(c, h, 2, n - 1)} {logic_type:1,control_type:1}`);
  }
  await creative(of(m, 1, 1, -1), 'mekanism:fissile_fuel');
  await fixture.command(`data merge block ${of(m, 0, 2, 1)} {logic_type:2}`);
  await fixture.command(`data merge block ${of(m, 2, 2, 1)} {logic_type:4}`);
  await fixture.command(`data merge block ${of(m, 1, 2, 2)} {logic_type:1,control_type:1}`);
  const tempLamp = of(m, -1, 2, 1), damageLamp = of(m, 3, 2, 1);
  for (const lamp of [tempLamp, damageLamp]) await fixture.command(`setblock ${lamp} minecraft:redstone_lamp`);
  log.rate = await mek(`mek fission ${r} rate 4`);
  log.rateM = await mek(`mek fission ${m} rate 1`);
  await fixture.stepServer(3);
  const before = { w: await multiblockOf(w), r: await multiblockOf(r) };
  assert.deepEqual([before.w.fuel.amount, before.w.water.amount, before.w.active], [8000, 3_600_000, false], 'W filled, not yet on');
  assert.equal(before.r.rate_limit, 4);
  for (const [c, n] of [[w, 3], [s, 3], [r, 5], [m, 3]] as const) await fixture.command(`setblock ${of(c, (n - 1) / 2, 2, n)} minecraft:redstone_block`);

  const rows: Record<number, any> = {};
  let ticks = 0;
  const row = async () => {
    const [tw, ts, tr, tm] = [await multiblockOf(w), await multiblockOf(s), await multiblockOf(r), await multiblockOf(m)];
    return {
      w: { heat: tw.heat, waste: tw.waste.amount, steam: tw.heated_coolant.amount, out: await chemicalIn(of(w, 3, 1, 1)), active: tw.active, partial: tw.partial_waste },
      s: { heat: ts.heat, t: ts.temperature, heated: ts.heated_coolant, out: await chemicalIn(of(s, 3, 1, 1)) },
      r: { heat: tr.heat, t: tr.temperature, steam: tr.heated_coolant.amount, out: await chemicalIn(of(r, 5, 1, 2)) },
      m: { heat: tm.heat, t: tm.temperature, damage: tm.damage, waste: tm.waste.amount, temp: await lit(tempLamp), damaged: await lit(damageLamp) },
    };
  };
  for (const t of [1, 10, 11, 20, 60, 61, 100, 101, 150, 200, 400, 800, 1000]) {
    await fixture.stepServer(t - ticks);
    ticks = t;
    rows[t] = await row();
    if (t === 150) await fixture.command(`setblock ${of(w, 1, 2, 3)} minecraft:air`);
  }
  log.rows = rows;
  assert.equal(rows[1].w.heat, 10_225_000.01013279, 'W after its first tick');
  assert.deepEqual([rows[10].w.heat, rows[10].w.waste, rows[11].w.heat], [10_449_943.795735585, 0, 10_474_931.304791776], 'W: 0.1 mB/t, no waste before the eleventh tick');
  assert.equal(rows[100].w.heat, 12_693_825.686264288, 'W at its 100th tick');
  assert.deepEqual([rows[101].w.heat, rows[101].w.out], [12_715_351.057294967, 67], 'W boils from its 101st tick');
  assert.equal(rows[150].w.out, 24_012, 'W\'s steam by its 150th tick');
  assert.equal(rows[200].w.active, false, 'W off once its redstone is gone');
  assert(rows[200].s.heated.chemical === 'mekanism:superheated_sodium' && Math.abs(rows[200].s.t - 373.8817) < 0.001, `S: ${JSON.stringify(rows[200].s)}`);
  assert.equal(rows[150].s.out + rows[150].s.heated.amount, 64_000 + 181_115, 'S\'s superheated sodium by its 150th tick');
  assert.equal(rows[1].r.heat, 30_400_000.029206276, 'R after its first tick');
  assert.deepEqual([rows[10].r.out, rows[20].r.steam, rows[100].r.steam], [22_444, 152_513, 1_751_775], 'R\'s steam');
  assert.deepEqual([rows[150].m.damage, rows[200].m.damage, rows[1000].m.damage], [3.0339724558561616, 9.637275531423418, 129.6310829307864], 'M\'s damage');
  assert.deepEqual([rows[100].m.temp, rows[150].m.temp, rows[800].m.damaged, rows[1000].m.damaged], [false, true, false, true], 'M\'s adapters\' lamps');
  fixture.mark(`W: steam from tick 101 (67 mB), 24 012 by 150; S holds 373.88 K on sodium; R 4 mB/t: 19 991 mB of steam a tick; M dry at 1 mB/t: ${rows[1000].m.damage.toFixed(3)} % damage at tick 1000, lamps by temperature and damage`);

  // Meltdowns off at a chance of 1: M turns itself off and won't start; on again: it melts down.
  meltdowns(false, 1);
  await Bun.sleep(1500);
  await fixture.stepServer(1);
  const off = await multiblockOf(m);
  log.forceDisabled = off;
  assert.deepEqual([off.force_disabled, off.active], [true, false], 'M force disabled');
  await mek(`mek fission ${m} activate`);
  assert.equal((await multiblockOf(m)).active, false, 'a force disabled reactor refuses to start');
  const id = (await mek(`mek probe ${m}`)).inventory_id;
  meltdowns(true, 1);
  await Bun.sleep(1500);
  // The meltdown starts on the next tick; its first blast may come on that tick already (one draw in ten).
  let blasts = 0, seen = false;
  for (let i = 0; i < 101 && !blasts; i++) {
    await fixture.stepServer(1);
    const now = await multiblockOf(m);
    seen ||= now?.meltdowns === 1;
    if (!(await is(of(m, 0, 0, 0), F)) || !now) blasts = i + 1;
  }
  const caches = await mek('mek multiblock caches');
  const mine = (caches.fissionReactor ?? []).find((c: any) => c.id === id);
  log.meltdown = { blasts, seen, cache: mine };
  assert(blasts > 0, 'a blast came within 100 ticks');
  assert.deepEqual([mine?.reactor?.damage, mine?.reactor?.active, mine?.heat?.[0]?.stored], [75, false, 10_200_000.01013279], "the cache after the meltdown: 75 % damage, off, at the air's heat");
  meltdowns(true, 0.001);
  fixture.mark(`meltdowns off: M force disabled and refusing to start; on: it melted down ${blasts} tick(s) later, its cache at 75 % damage`);

  // The screens: R's on a casing, its statistics as a page; W's logic adapter, its high temperature mode pressed.
  await fixture.hold(false);
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const origin = (width: number, height: number) => [Math.trunc((960 / scale - width) / 2), Math.trunc((540 / scale - height) / 2)];
  const casing = of(r, 1, 1, 0);
  const [cx, cy, cz] = casing.split(' ').map(Number);
  await fixture.command(`tp @s ${cx + 0.5} ${cy - 1} ${cz - 1.5}`);
  await fixture.raw(`interact ${casing} north`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the reactor screen opens');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'fission-screen.png')}`);
  const [rx, ry] = origin(195, 255);
  await fixture.raw(`click-at ${rx - 12} ${ry + 19}`);
  await Bun.sleep(1000);
  await fixture.raw(`screenshot ${join(fixture.run, 'fission-stats.png')}`);
  await fixture.raw('close');
  const wa = of(w, 1, 2, 2);
  const [ax, ay, az] = wa.split(' ').map(Number);
  await fixture.command(`tp @s ${ax + 0.5} ${ay - 1} ${az + 1.5}`);
  await fixture.raw(`interact ${wa} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the adapter screen opens');
  await Bun.sleep(1000);
  const [lx, ly] = origin(196, 166);
  await fixture.raw(`click-at ${lx + 90} ${ly + 68}`);
  const mode = async () => (await fixture.command(`data get block ${wa} logic_type`)).chat?.find((t: string) => t.includes('has the following block data')) ?? '';
  await until(mode, v => / 2$/.test(v), 'the adapter takes the high temperature mode');
  await fixture.raw(`screenshot ${join(fixture.run, 'fission-adapter.png')}`);
  await fixture.raw('close');
  await fixture.hold();
  fixture.mark("the reactor's screen opens on a casing, its statistics as a page; an adapter's screen sets its mode");

  // Kept over a restart: R's rate and heat, S's adapter mode.
  const keep = await multiblockOf(r);
  await fixture.restart();
  const afterR = await until(() => multiblockOf(r), t => !!t, 'R formed again after the restart');
  log.after = afterR;
  assert.equal(afterR.rate_limit, 4);
  assert(Math.abs(afterR.heat - keep.heat) < 1e6, JSON.stringify({ before: keep.heat, after: afterR.heat }));
  const adapter = await fixture.command(`data get block ${of(s, 1, 2, 2)} logic_type`);
  assert((adapter.chat ?? []).some((t: string) => /has the following block data: 1$/.test(t)), JSON.stringify(adapter.chat));
  fixture.mark(`after a restart R keeps its 4 mB/t and ${Math.round(afterR.heat)} heat, S's adapter its activation mode`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK fission', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK fission', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
