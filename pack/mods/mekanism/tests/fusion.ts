/** Mekanism Generators' Fusion Reactor, as the `mekanism` native bundle makes it under ATM10's configs
 * (`generators.toml` `[fusion_reactor]`: 10^7 J of plasma heat a mB of D-T fuel, thermocouples at 0.04, the casing at
 * 0.333333333, water at 0.27272727272727, 500 mB of each fuel; `[hohlraum]`: 10 mB). The numbers are those a NeoForge
 * 1.21.1 server with Mekanism and Mekanism Generators 10.7.19 and ATM10's configs gave for the same build over RCON
 * (local `mekfusion-work/lab/e1.py`; the manager's saved data `mekanism_fusionreactor`):
 *
 * - Forms as a 5x5x5 with its corners and most of its edges left out: frames in the faces' rings and the edges'
 *   middles, the controller at the top's centre, ports, a logic adapter, a laser focus matrix and reactor glass in the
 *   faces' middles. Not with glass for its controller, a second controller, glass in a ring, or dirt inside.
 * - E1 (the default injection rate of 2, so no room for water): deuterium and tritium from creative tanks, a Hohlraum
 *   of 10 mB of D-T fuel through a hopper into the controller, 1.5 * 10^9 J from a laser amplifier into the focus
 *   matrix. The plasma reads 150 000 300.0000003 K the tick the amplifier fires, then it ignites (the Hohlraum used):
 *   150 900 300.0000003 K with the case at 30 000 300.000000298, then 150 858 500.0000003 / 44 175 391.05297187 and
 *   399 999 J, 150 845 133.78210625 / 50 779 754.05198853 and 989 000 J.
 * - A logic adapter in ready mode lights a lamp once the plasma passed its ignition temperature; `/mek fusion ... rate
 *   11` sets 10, and with it 10^7 mB of water and 10^9 mB of steam room. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-fusion', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml')), 'ATM10_DIR with its Mekanism configs');
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml', 'generators.toml', 'generator-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const G = 'mekanismgenerators:';
const FR = `${G}fusion_reactor_frame`, PORT = `${G}fusion_reactor_port`, CTRL = `${G}fusion_reactor_controller`;
const LA = `${G}fusion_reactor_logic_adapter`, LFM = `${G}laser_focus_matrix`, RG = `${G}reactor_glass`;
const GRID = [[0, 0, 1, 0, 0], [0, 1, 2, 1, 0], [1, 2, 2, 2, 1], [0, 1, 2, 1, 0], [0, 0, 1, 0, 0]];
const HOHLRAUM = 'id:"mekanismgenerators:hohlraum",count:1,components:{"minecraft:custom_data":{mekanism:{chemical_tanks:[{tank:0b,stored:{id:"mekanismgenerators:fusion_fuel",amount:10L}}]}}}';
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
/** The requirement at an offset of a 5x5x5: 0 ignored, 1 a frame, 2 any block, 3 inside. */
function req(d: number[]): number {
  for (let a = 0; a < 3; a++) {
    if (d[a] === 0 || d[a] === 4) {
      const [h, v] = a === 0 ? [2, 1] : a === 1 ? [0, 2] : [0, 1];
      return GRID[d[h]][d[v]];
    }
  }
  return 3;
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x0 = Math.floor(px) + 4, y0 = Math.min(Math.floor(py), 70), z0 = Math.floor(pz) + 6;
  await fixture.command(`fill ${x0 - 2} ${y0 - 2} ${z0 - 4} ${x0 + 40} ${y0 + 7} ${z0 + 6} minecraft:air`);
  await fixture.command(`fill ${x0 - 2} ${y0 - 3} ${z0 - 4} ${x0 + 40} ${y0 - 3} ${z0 + 6} minecraft:stone`);
  await fixture.hold();
  const P = (o: number[], d: number[]) => `${o[0] + d[0]} ${o[1] + d[1]} ${o[2] + d[2]}`;
  async function build(o: number[], other: Record<string, string>) {
    for (let x = 0; x < 5; x++) for (let y = 0; y < 5; y++) for (let z = 0; z < 5; z++) {
      const r = req([x, y, z]);
      const b = other[`${x},${y},${z}`] ?? (r === 1 ? FR : r === 2 ? RG : undefined);
      if (b) await fixture.command(`setblock ${P(o, [x, y, z])} ${b}`);
    }
  }
  const multiblockOf = async (pos: string) => (await mek(`mek probe ${pos}`)).multiblocks?.[0];
  const top = { '2,4,2': CTRL };
  const cases: [string, Record<string, string>][] = [
    ['plain', top],
    ['glass controller', { '2,4,2': RG }],
    ['second controller', { ...top, '2,2,0': CTRL }],
    ['glass in a ring', { ...top, '1,1,0': RG }],
    ['dirt inside', { ...top, '2,2,2': 'minecraft:dirt' }],
  ];
  for (let i = 0; i < cases.length; i++) await build([x0 + 8 + 6 * i, y0, z0], cases[i][1]);
  // R: deuterium west, tritium east, the focus matrix north, a logic adapter below with a lamp under it.
  const o = [x0, y0, z0];
  await build(o, { ...top, '0,2,2': PORT, '4,2,2': PORT, '2,2,0': LFM, '2,0,2': LA, '2,2,4': PORT });
  await fixture.command(`setblock ${P(o, [2, -1, 2])} minecraft:redstone_lamp`);
  for (const [d, chem] of [[[-1, 2, 2], 'deuterium'], [[5, 2, 2], 'tritium']] as const) {
    await fixture.command(`setblock ${P(o, d)} mekanism:creative_chemical_tank`);
    await fixture.command(`data merge block ${P(o, d)} {chemical_tanks:[{tank:0b,stored:{id:"${G}${chem}",amount:1000000000000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b}}`);
  }
  await fixture.stepServer(30);
  const formed: Record<string, boolean> = {};
  for (let i = 0; i < cases.length; i++) formed[cases[i][0]] = !!(await multiblockOf(P([x0 + 8 + 6 * i, y0, z0], [1, 1, 0])));
  log.formed = formed;
  assert.deepEqual(formed, { plain: true, 'glass controller': false, 'second controller': false, 'glass in a ring': false, 'dirt inside': false });
  const ctrl = P(o, [2, 4, 2]);
  assert(await multiblockOf(ctrl), 'R formed');
  fixture.mark('forms as the 5x5x5 with its corners out, its controller on top; refuses the four broken ones');

  await mek(`mek fusion ${P(o, [2, 0, 2])} logic ready`);
  await fixture.command(`setblock ${P(o, [2, 5, 2])} minecraft:hopper[facing=down]`);
  await fixture.command(`data merge block ${P(o, [2, 5, 2])} {Items:[{Slot:0b,${HOHLRAUM}}]}`);
  await fixture.stepServer(10);
  const probe = async () => (await mek(`mek probe ${ctrl}`)).multiblocks[0];
  const first = await probe();
  log.first = first;
  assert.equal(first.hohlraum, `${G}hohlraum`, 'the hopper put the Hohlraum in the controller');
  assert.deepEqual([first.deuterium, first.tritium, first.plasma, first.case], [500, 500, 300.0000002980232, 300.0000002980232]);
  assert.equal(await is(P(o, [2, -1, 2]), 'minecraft:redstone_lamp[lit=true]'), false);

  await fixture.command(`setblock ${P(o, [2, 2, -2])} mekanism:laser_amplifier[facing=south]`);
  await fixture.command(`data merge block ${P(o, [2, 2, -2])} {energy_containers:[{container:0b,stored:1500000000L}]}`);
  const rows: any[] = [];
  for (let t = 1; t <= 6; t++) {
    await fixture.stepServer(1);
    const k = await probe();
    rows.push([k.plasma, k.case, k.burning, k.energy, k.hohlraum]);
    if (t === 3) await fixture.command(`setblock ${P(o, [2, 2, -2])} minecraft:air`);
  }
  log.rows = rows;
  const fired = rows.findIndex(r => r[0] === 150000300.0000003);
  log.fired = fired;
  assert(fired >= 0, 'the laser\'s Joules reach the plasma');
  assert.deepEqual(rows.slice(fired, fired + 4), [
    [150000300.0000003, 300.0000002980232, false, 0, `${G}hohlraum`],
    [150900300.0000003, 30000300.000000298, true, 0, null],
    [150858500.0000003, 44175391.05297187, true, 399999, null],
    [150845133.78210625, 50779754.05198853, true, 989000, null],
  ]);
  fixture.mark(`ignition: 1.5e9 J to 150 000 300 K${fired === 0 ? ' the tick it fires' : ` ${fired} tick(s) late`}, the Hohlraum, then the oracle's heat and Joules`);
  await fixture.stepServer(1);
  assert.equal(await is(P(o, [2, -1, 2]), 'minecraft:redstone_lamp[lit=true]'), true, 'ready: the lamp lights');
  const rated = await mek(`mek fusion ${ctrl} rate 11`);
  assert.deepEqual([rated.rate, rated.max_water, rated.max_steam], [10, 10_000_000, 1_000_000_000]);
  fixture.mark('the logic adapter in ready mode lights its lamp; rate 11 sets 10 and the water and steam room');

  // Kept over a restart: the rate and the room it set, the plasma's heat, the adapter's mode.
  await fixture.stepServer(2);
  const keep = await probe();
  await fixture.restart();
  const after = await until(async () => (await mek(`mek probe ${ctrl}`)).multiblocks?.[0], t => !!t, 'the reactor forms again after the restart');
  log.after = after;
  assert.deepEqual([after.rate, after.max_water, after.max_steam], [10, 10_000_000, 1_000_000_000]);
  assert(after.plasma > keep.plasma / 2 && after.plasma <= keep.plasma + 1, JSON.stringify({ before: keep.plasma, after: after.plasma }));
  const adapter = await fixture.command(`data get block ${P(o, [2, 0, 2])} logic_type`);
  assert((adapter.chat ?? []).some((t: string) => /has the following block data: 1$/.test(t)), JSON.stringify(adapter.chat));
  fixture.mark(`after a restart the reactor keeps its rate of 10, its room for water and steam, ${Math.round(after.plasma)} K of plasma and its adapter's mode`);
  console.log('MEK fusion', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK fusion', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
