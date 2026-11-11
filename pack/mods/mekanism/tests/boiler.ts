/** Mekanism's Thermoelectric Boiler and Resistive Heater, as the `mekanism` native bundle makes them under ATM10's
 * configs. The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave for the
 * same builds over RCON (local `mekboil-work/b1.py` and `b2.py` on `mekm2-work/oracle`; the manager's saved data
 * `mekanism_thermoelectricboiler` the contents), all in plains (300.0000003 K):
 *
 * - Forms: a box of boiler casings with a full layer of pressure dispersers inside and superheating elements below
 *   it, all touching. No disperser, no element, a disperser missing from the layer or one more below it, an element
 *   above the layer, elements apart, a valve on an edge and dirt inside don't; a pocket of air the walls don't
 *   reach still does (the count leaves out the bottom wall the explore takes in). Heat capacity 50 a wall block
 *   (2100 for a 3x5x3), water `32 000` a block of the air and walls below the dispersers (544 000 for a 3x5x3, the
 *   bottom wall in), steam `320 000` a block from the dispersers' layer up (18 blocks for a 3x5x3).
 * - A resistive heater of 10 000 J/t against a 3x5x3 full of water: the boiler at 300.476 K three ticks after the
 *   heater came, 301.349 at four, 311.879 at ten, 374.390 at forty; then 19 126 mB of steam out of an output valve
 *   into a chemical tank from tick 40 to 200 (about 120 mB a tick); the heater settles at 658.71 K.
 * - Superheated sodium, 1000 mB a tick from a basic chemical tank into a 5x5x5 with nine elements: 1601 heated and
 *   399 cooled after the second tick, 1963 and 1037, 2180 and 1820.
 * - One element under a creative tank of superheated sodium: 10 005 094 mB cooled on the second tick (64 000 of it
 *   out of an output_coolant valve), then 320 000 mB of steam a tick (`1.6E7 * 1 * 0.2 / 10`), the cooled tank full
 *   (25 600 000) on the fourth; its element lit from the third.
 * - A heater beside a lone casing hands it nothing (no heat handler until a boiler forms); beside a thermal
 *   evaporation valve it warms the plant: 541 000 of heat on the third tick (1000 handed on the second), 616 604.83
 *   on the twentieth (local `mekboil-work/b3.py`). */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-boiler', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const C = 'mekanism:boiler_casing', V = 'mekanism:boiler_valve', D = 'mekanism:pressure_disperser', E = 'mekanism:superheating_element';
const G = 'mekanism:structural_glass', H = 'mekanism:resistive_heater';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const boilerOf = async (pos: string) => (await mek(`mek probe ${pos}`)).multiblocks?.[0];
async function is(pos: string, state: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${state}`);
  return (reply.chat ?? []).some((t: string) => /passed/i.test(t));
}
async function chemicalIn(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos} chemical_tanks`);
  const line = (reply.chat ?? []).find((t: string) => t.includes('has the following block data')) ?? '';
  return Number(line.match(/amount: (\d+)L/)?.[1] ?? 0);
}

type Inner = Record<string, string>;
const key = (x: number, y: number, z: number) => `${x},${y},${z}`;
/** The inside's layer `y` of a boiler `l` by `w` all `block`, but `skip` ([x, z]). */
const layer = (l: number, w: number, y: number, block: string, skip: number[][] = []): Inner => {
  const out: Inner = {};
  for (let x = 1; x < l - 1; x++) for (let z = 1; z < w - 1; z++) if (!skip.some(([sx, sz]) => sx === x && sz === z)) out[key(x, y, z)] = block;
  return out;
};

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 60), z = Math.floor(pz) - 12;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (let dx = -3; dx <= 60; dx += 16) {
    const x1 = Math.min(dx + 15, 60);
    await fixture.command(`fill ${at(dx, -1, -3)} ${at(x1, 12, 30)} minecraft:air`);
    await fixture.command(`fill ${at(dx, -2, -3)} ${at(x1, -2, 30)} minecraft:stone`);
    await fixture.command(`fillbiome ${at(dx, -2, -3)} ${at(x1, 12, 30)} minecraft:plains`);
  }
  await fixture.hold();
  /** A boiler of `size` (x, y, z) at (dx, -1, dz) on the stone: a box of casings, the inside air, then `inner` and
   * `edits` [dx, dy, dz, block] (dy from its bottom wall). Its bottom corner's position. */
  const boiler = async (dx: number, dz: number, size: number[], inner: Inner, edits: [number, number, number, string][] = []) => {
    const [l, h, w] = size;
    await fixture.command(`fill ${at(dx, -1, dz)} ${at(dx + l - 1, h - 2, dz + w - 1)} ${C}`);
    await fixture.command(`fill ${at(dx + 1, 0, dz + 1)} ${at(dx + l - 2, h - 3, dz + w - 2)} minecraft:air`);
    for (const [k, b] of Object.entries(inner)) {
      const [ix, iy, iz] = k.split(',').map(Number);
      await fixture.command(`setblock ${at(dx + ix, iy - 1, dz + iz)} ${b}`);
    }
    for (const [ex, ey, ez, b] of edits) await fixture.command(`setblock ${at(dx + ex, ey - 1, dz + ez)} ${b}`);
    return at(dx, -1, dz);
  };
  /** The block at (ex, ey, ez) of the boiler whose corner is `corner`. */
  const of = (corner: string, ex: number, ey: number, ez: number) => {
    const [cx, cy, cz] = corner.split(' ').map(Number);
    return `${cx + ex} ${cy + ey} ${cz + ez}`;
  };
  const one = { [key(1, 1, 1)]: E, [key(1, 2, 1)]: D };

  // Formation.
  const full5 = { ...layer(5, 5, 1, E), ...layer(5, 5, 2, D) };
  const cases: [string, number[], Inner, [number, number, number, string][], boolean][] = [
    ['min-3x5x3', [3, 5, 3], one, [], true],
    ['5x5x5-9el', [5, 5, 5], full5, [], true],
    ['no-disperser', [3, 5, 3], { [key(1, 1, 1)]: E }, [], false],
    ['no-element', [3, 5, 3], { [key(1, 2, 1)]: D }, [], false],
    ['missing-disperser', [5, 5, 5], { ...layer(5, 5, 1, E), ...layer(5, 5, 2, D, [[2, 2]]) }, [], false],
    ['extra-disperser', [5, 6, 5], { ...layer(5, 5, 1, E, [[3, 3]]), ...layer(5, 5, 3, D), [key(3, 1, 3)]: D }, [], false],
    ['element-above', [3, 6, 3], { ...one, [key(1, 4, 1)]: E }, [], false],
    ['split-elements', [5, 5, 5], { [key(1, 1, 1)]: E, [key(3, 1, 3)]: E, ...layer(5, 5, 2, D) }, [], false],
    ['air-pocket', [5, 6, 5], { ...layer(5, 5, 1, E), ...layer(5, 5, 2, E, [[2, 2]]), ...layer(5, 5, 3, D) }, [], true],
    ['glass-valve-side', [3, 5, 3], one, [[1, 3, 0, G], [0, 3, 1, V], [1, 4, 1, `${V}[mode=output_steam]`]], true],
    ['valve-edge', [3, 5, 3], one, [[0, 3, 0, V]], false],
    ['steam-height-0', [3, 4, 3], one, [], true],
    ['two-element-layers', [3, 6, 3], { [key(1, 1, 1)]: E, [key(1, 2, 1)]: E, [key(1, 3, 1)]: D }, [], true],
    ['dirt-inside', [3, 5, 3], { ...one, [key(1, 3, 1)]: 'minecraft:dirt' }, [], false],
  ];
  const corners: Record<string, string> = {};
  for (const [i, [name, size, inner, edits]] of cases.entries()) corners[name] = await boiler(7 * (i % 7), i < 7 ? 0 : 8, size, inner, edits);
  await fixture.stepServer(2);
  const formed: Record<string, any> = {};
  for (const [name] of cases) formed[name] = await boilerOf(corners[name]);
  log.formation = Object.fromEntries(cases.map(([name]) => [name, !!formed[name]]));
  for (const [name, , , , forms] of cases) assert.equal(!!formed[name], forms, name);
  const min = formed['min-3x5x3'];
  log.min = min;
  assert.equal(min.heat_capacity, 2100, '42 wall blocks');
  assert.deepEqual([min.water_volume, min.steam_volume, min.superheaters], [17, 18, 1]);
  assert.deepEqual([min.capacities.water, min.capacities.steam, min.capacities.heated_coolant, min.capacities.cooled_coolant], [544_000, 5_760_000, 8_704_000, 9_216_000]);
  assert(Math.abs(min.temperature - 300) < 1e-6, `at its air: ${min.temperature}`);
  assert.equal(formed['5x5x5-9el'].heat_capacity, 4900);
  assert.equal(formed['5x5x5-9el'].capacities.water, 1_312_000);
  assert.equal(formed['air-pocket'].heat_capacity, 5700);
  assert.equal(formed['steam-height-0'].heat_capacity, 1700);
  assert.equal(formed['steam-height-0'].steam_volume, 9);
  assert.equal(formed['two-element-layers'].superheaters, 2);
  assert.equal(formed['glass-valve-side'].valve_modes.length, 2);
  fixture.mark(`refused: ${cases.filter(c => !c[4]).map(c => c[0]).join(', ')}; formed: ${cases.filter(c => c[4]).map(c => c[0]).join(', ')}; 3x5x3: 2100 of heat capacity, 544 000 mB of water, 5 760 000 of steam`);

  // Rates. A: 3x5x3, water from above its top valve, a heater of 10 000 J/t against its west wall, steam out of an
  // east valve into a basic chemical tank. C: 5x5x5, nine elements, superheated sodium from a basic tank above one
  // top valve, water above another. K: 5x5x5, one element, a creative tank of superheated sodium, cooled coolant out
  // of an east valve. H0: a heater alone at 100 J/t; HL: a heater of 10 000 J/t beside a lone casing.
  const a = await boiler(0, 16, [3, 5, 3], one, [[1, 4, 1, V], [2, 3, 1, `${V}[mode=output_steam]`]]);
  const c = await boiler(6, 16, [5, 5, 5], full5, [[1, 4, 2, V], [3, 4, 2, V]]);
  const k = await boiler(14, 16, [5, 5, 5], { [key(2, 1, 2)]: E, ...layer(5, 5, 2, D) }, [[1, 4, 2, V], [3, 4, 2, V], [4, 1, 2, `${V}[mode=output_coolant]`]]);
  const lone = at(24, -1, 16), hl = at(23, -1, 16), h0 = at(28, -1, 16);
  await fixture.command(`setblock ${lone} ${C}`);
  await fixture.stepServer(2);
  for (const [name, corner] of [['A', a], ['C', c], ['K', k]] as const) assert(await boilerOf(corner), `${name} formed`);
  const aTank = of(a, 3, 3, 1), kTank = of(k, 5, 1, 2);
  for (const tank of [aTank, kTank]) {
    await fixture.command(`setblock ${tank} mekanism:basic_chemical_tank`);
    await fixture.command(`data merge block ${tank} {component_config:{config2:[I;1,1,1,1,1,1],eject2:0b}}`);
  }
  const water = async (pos: string) => {
    await fixture.command(`setblock ${pos} mekanism:creative_fluid_tank[active=true]`);
    await fixture.command(`data merge block ${pos} {active_state:1b,fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:2147483647}}]}`);
  };
  await water(of(a, 1, 5, 1));
  await water(of(c, 1, 5, 2));
  await water(of(k, 1, 5, 2));
  const sodium = of(c, 3, 5, 2);
  await fixture.command(`setblock ${sodium} mekanism:basic_chemical_tank`);
  await fixture.command(`data merge block ${sodium} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:superheated_sodium",amount:64000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b},dumping:0}`);
  const creative = of(k, 3, 5, 2);
  await fixture.command(`setblock ${creative} mekanism:creative_chemical_tank`);
  await fixture.command(`data merge block ${creative} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:superheated_sodium",amount:1000000000000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b}}`);
  const heaterAt = async (pos: string, usage: number) => {
    await fixture.command(`setblock ${pos} ${H}[facing=north]`);
    await fixture.command(`data merge block ${pos} {energy_containers:[{container:0b,stored:${usage * 400}L,energy_usage:${usage}L}]}`);
  };
  const ha = of(a, -1, 1, 1);
  await heaterAt(ha, 10_000);
  await heaterAt(hl, 10_000);
  await heaterAt(h0, 100);
  const rows: Record<number, any> = {};
  let ticks = 0;
  for (const t of [1, 2, 3, 4, 10, 40, 200]) {
    await fixture.stepServer(t - ticks);
    ticks = t;
    const [ba, bc, bk] = [await boilerOf(a), await boilerOf(c), await boilerOf(k)];
    rows[t] = {
      a: { t: ba.temperature, water: ba.water.amount, out: await chemicalIn(aTank), heater: (await mek(`mek probe ${ha}`)).heat },
      c: { heated: bc.heated_coolant.amount, cooled: bc.cooled_coolant.amount, t: bc.temperature },
      k: { steam: bk.steam.amount, cooled: bk.cooled_coolant.amount, heat: bk.heat, out: await chemicalIn(kTank), lit: await is(of(k, 2, 1, 2), `${E}[active=true]`) },
      h0: (await mek(`mek probe ${h0}`)).heat, hl: (await mek(`mek probe ${hl}`)).heat,
    };
  }
  log.rows = rows;
  const near = (got: number, want: number, what: string) => assert(Math.abs(got - want) < 1e-6, `${what}: ${got} against ${want}`);
  near(rows[1].a.heater, 36_000.000_029_802_32, 'the heater after its first tick');
  near(rows[2].a.heater, 40_996.405_421_714_46, 'after its second');
  near(rows[3].a.t, 300.476_190_774_213_7, 'A three ticks in');
  near(rows[4].a.t, 301.348_917_031_908_4, 'A four ticks in');
  near(rows[10].a.t, 311.878_984_807_374_76, 'A ten ticks in');
  near(rows[40].a.t, 374.390_125_098_067_5, 'A forty ticks in');
  assert.equal(rows[200].a.out - rows[40].a.out, 19_950 - 824, 'steam out of the output valve, ticks 40 to 200');
  assert.equal(rows[200].a.water, 544_000, 'its water kept full');
  near(rows[200].a.heater, 65_871.057_636_014_62, 'the heater settled');
  assert.deepEqual([2, 3, 4].map(t => [rows[t].c.heated, rows[t].c.cooled]), [[1601, 399], [1963, 1037], [2180, 1820]], 'superheated sodium cooling');
  near(rows[2].c.t, 300.407_143_155_166_07, 'C two ticks in');
  assert.deepEqual([rows[2].k.cooled, rows[2].k.out, rows[2].k.steam], [9_941_094, 64_000, 0], 'K: cooled on the second tick, 64 000 out');
  near(rows[2].k.heat, 51_495_470.001_460_314, 'K\'s heat on the second tick');
  assert.deepEqual([rows[3].k.steam, rows[3].k.cooled, rows[4].k.cooled], [320_000, 18_921_676, 25_600_000], 'K: one element\'s 320 000 mB a tick, the cooled tank full');
  assert.deepEqual([rows[2].k.lit, rows[3].k.lit], [false, true], 'its element lit from the third tick');
  near(rows[2].hl, 41_996.405_421_714_46, 'a lone casing takes no heat');
  near(rows[10].h0, 30_598.385_037_696_793, 'a heater alone at 100 J/t');
  fixture.mark(`a 10 000 J/t heater: ${rows[40].a.t.toFixed(3)} K at tick 40, ${(rows[200].a.out - rows[40].a.out) / 160} mB of steam a tick; sodium 1601/399, 1963/1037; one element boils 320 000 mB a tick`);

  // The heater's usage by `/mek heater`, its store 400 ticks of it; an evaporation plant's valve takes heat.
  const usage = await mek(`mek heater ${h0} 2500`);
  assert.deepEqual([usage.usage, usage.capacity], [2500, 1_000_000], JSON.stringify(usage));
  const ex = 34, ez = 16;
  await fixture.command(`fill ${at(ex, -1, ez)} ${at(ex + 3, 1, ez + 3)} mekanism:thermal_evaporation_block`);
  await fixture.command(`fill ${at(ex + 1, 0, ez + 1)} ${at(ex + 2, 1, ez + 2)} minecraft:air`);
  await fixture.command(`setblock ${at(ex + 1, 0, ez)} mekanism:thermal_evaporation_controller`);
  await fixture.command(`setblock ${at(ex, 0, ez + 1)} mekanism:thermal_evaporation_valve`);
  await fixture.stepServer(2);
  const plant = at(ex + 1, 0, ez);
  const cold = await boilerOf(plant);
  await heaterAt(at(ex - 1, 0, ez + 1), 10_000);
  await fixture.stepServer(3);
  const third = await boilerOf(plant);
  await fixture.stepServer(17);
  const warm = await boilerOf(plant);
  log.evaporation = { cold: cold?.heat, third: third?.heat, warm: warm?.heat };
  near(cold.heat, 540_000.000_536_441_8, 'the plant at its air');
  near(third.heat, 541_000.000_536_441_8, 'the heater\'s first 1000 landed on the third tick');
  near(warm.heat, 616_604.833_185_564, 'twenty ticks of the heater');
  fixture.mark(`/mek heater sets a heater's usage; one beside an evaporation plant's valve warms it to ${warm.temperature.toFixed(3)} K in 20 ticks, as on the oracle`);

  // The Configurator: a sneaking use turns a valve's mode; a plain one on a boiler that doesn't form says why.
  await fixture.hold(false);
  await fixture.replace('hotbar.0', 'mekanism:configurator');
  await fixture.select(0, 'mekanism:configurator');
  // The west valve of the boiler with glass and valves in its sides: nothing west of it.
  const sideValve = of(corners['glass-valve-side'], 0, 3, 1);
  const [vx, vy, vz] = sideValve.split(' ').map(Number);
  assert(await is(sideValve, `${V}[mode=input]`), 'an input valve');
  await fixture.command(`tp @s ${vx - 1.5} ${vy - 1} ${vz + 0.5}`);
  await fixture.sneakInteract(`${sideValve} west`);
  await until(() => is(sideValve, `${V}[mode=output_steam]`), v => v, 'the valve turned to output steam');
  const turned = (await boilerOf(sideValve))?.valve_modes?.find((m: any) => m.pos.join(' ') === sideValve);
  log.turned = turned;
  const bare = corners['no-disperser'];
  const [bx, by, bz] = bare.split(' ').map(Number);
  await fixture.command(`tp @s ${bx + 1.5} ${by + 2} ${bz - 1.5}`);
  await fixture.raw(`interact ${of(bare, 1, 2, 0)} north`);
  const said = await until(async () => ((await fixture.raw('chat-log 20')).chat ?? []).join('\n'), text => /no Pressure Disperser layer found/.test(text), 'the reason');
  log.configurator = said.split('\n').find((l: string) => l.includes('Pressure Disperser'));
  fixture.mark(`a sneaking Configurator turns a valve to output steam; on a boiler without dispersers it says "${log.configurator}"`);

  // The screens: the boiler's on a casing, the heater's with its energy slot.
  await fixture.replace('hotbar.0', 'minecraft:air');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 14.5}`);
  await fixture.raw(`interact ${of(a, 1, 2, 0)} north`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the boiler screen opens');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'boiler-screen.png')}`);
  await fixture.raw('close');
  const [hx, hy, hz] = ha.split(' ').map(Number);
  await fixture.command(`tp @s ${hx + 0.5} ${hy} ${hz - 1.5}`);
  await fixture.raw(`interact ${ha} north`);
  const heaterView = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the heater screen opens');
  log.heater_slots = (heaterView.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'heater-screen.png')}`);
  await fixture.raw('close');
  await fixture.hold();
  fixture.mark('the boiler\'s screen opens on a casing, the heater\'s with its energy slot');

  // Kept over a restart: the steam, the cooled coolant and the heat of K.
  const keptSteam = (await boilerOf(k)).steam.amount;
  await fixture.hold(false);
  await fixture.restart();
  const after = await until(() => boilerOf(k), p => !!p, 'K formed again after the restart');
  log.after = after;
  assert(after.steam.amount >= keptSteam * 0.5 && after.heat_capacity === 4900 && after.cooled_coolant.amount > 0, JSON.stringify(after));
  fixture.mark(`after a restart K holds ${after.steam.amount} mB of steam and ${after.cooled_coolant.amount} of cooled sodium`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK boiler', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK boiler', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
