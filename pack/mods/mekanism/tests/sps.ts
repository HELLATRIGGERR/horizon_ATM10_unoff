/** Mekanism's Supercritical Phase Shifter, as the `mekanism` native bundle makes it under ATM10's configs
 * (`general.toml` `[sps]`: 1000 mB of polonium an mB of antimatter, an output tank of 1000 mB, 100 000 J an mB of
 * polonium; `machine-storage.toml` `spsPort` 1 000 000 000 J). The numbers are those a NeoForge 1.21.1 server with
 * Mekanism 10.7.19 and ATM10's configs gave for the same builds over RCON (local `meksps-work/s1.py`; the manager's
 * saved data `mekanism_sps`):
 *
 * - Forms 7x7x7 with each face's corners left out (`ALLOWED_GRID`), or with them filled, a port or stone in a left out
 *   corner, structural or reactor glass in a face's middle, no port at all, two coils behind two ports. Not with a
 *   coil and no port right outside it, glass, a port or a hole in a face's ring, a port on an edge, a hole in a face's
 *   middle, dirt inside, or a 3x3x3.
 * - C1: polonium from a creative tank into a north port, an ultimate cube (256 000 J a tick) into a west port with a
 *   coil behind it, an east port in output mode beside a basic chemical tank: the port holds 256 000 J after the first
 *   tick, the first polonium is processed on the third (2.56 mB a tick: processed 2, 5, 7, 10, 12, 15, 17, 20, 23, 25,
 *   progress 0.56, 0.12...), 998 and 0.4 after the 392nd, the first mB of antimatter on the 393rd, in the basic tank
 *   by the 400th (processed 18).
 * - C2: a creative cube: 2000 mB a tick from the third (the input tank's whole), 2 mB of antimatter a tick into the
 *   basic tank (20 by the 12th, 196 by the 100th), the port refilled to 1 000 000 000 J each tick.
 * - C3: C2 with no output port: the output tank full (1000) on the 502nd tick; then it stops and the port keeps its
 *   Joules.
 * - Kept over a restart: C3's full output tank and polonium. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-sps', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/general.toml')), 'ATM10_DIR with its Mekanism configs');
mkdirSync(config, { recursive: true });
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const CASE = 'mekanism:sps_casing', PORT = 'mekanism:sps_port', COIL = 'mekanism:supercharged_coil';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const multiblockOf = async (pos: string) => (await mek(`mek probe ${pos}`)).multiblocks?.[0];
async function chemicalIn(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos} chemical_tanks`);
  const line = (reply.chat ?? []).find((t: string) => t.includes('has the following block data')) ?? '';
  return Number(line.match(/amount: (\d+)L/)?.[1] ?? 0);
}

type Edit = [number, number, number, string];
const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 60), z = Math.floor(pz) - 12;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (let dx = -3; dx <= 60; dx += 16) {
    const x1 = Math.min(dx + 15, 60);
    await fixture.command(`fill ${at(dx, -1, -3)} ${at(x1, 8, 42)} minecraft:air`);
    await fixture.command(`fill ${at(dx, -2, -3)} ${at(x1, -2, 42)} minecraft:stone`);
    await fixture.command(`fillbiome ${at(dx, -2, -3)} ${at(x1, 8, 42)} minecraft:plains`);
  }
  await fixture.hold();
  /** An SPS at (dx, -1, dz): a hollow 7x7x7 of casings, the 32 positions each face leaves out made air (unless
   * `full`), then `edits` [dx, dy, dz, block] from its bottom corner. That corner's position (probed at its bottom's
   * middle, the corner being air). */
  const sps = async (dx: number, dz: number, edits: Edit[], full = false) => {
    const c = (ex: number, ey: number, ez: number) => at(dx + ex, ey - 1, dz + ez);
    await fixture.command(`fill ${c(0, 0, 0)} ${c(6, 6, 6)} ${CASE} hollow`);
    if (!full) {
      for (const a of [0, 6]) for (const b of [0, 6]) {
        for (const [lo, hi] of [[0, 1], [5, 6]]) {
          await fixture.command(`fill ${c(lo, a, b)} ${c(hi, a, b)} minecraft:air`);
          await fixture.command(`fill ${c(a, lo, b)} ${c(a, hi, b)} minecraft:air`);
          await fixture.command(`fill ${c(a, b, lo)} ${c(a, b, hi)} minecraft:air`);
        }
      }
    }
    for (const [ex, ey, ez, b] of edits) await fixture.command(`setblock ${c(ex, ey, ez)} ${b}`);
    return c(0, 0, 0);
  };
  const of = (corner: string, ex: number, ey: number, ez: number) => {
    const [cx, cy, cz] = corner.split(' ').map(Number);
    return `${cx + ex} ${cy + ey} ${cz + ez}`;
  };

  // Formation.
  const base: Edit[] = [[3, 3, 0, PORT], [0, 3, 3, PORT], [1, 3, 3, `${COIL}[facing=east]`]];
  const cases: [string, Edit[], boolean, boolean][] = [
    ['plain', base, false, true],
    ['full-corners', base, true, true],
    ['no-ports', [], false, true],
    ['coil-no-port', [[1, 3, 3, COIL]], false, false],
    ['coil-beside-port', [[0, 3, 3, PORT], [1, 3, 2, COIL]], false, false],
    ['glass-side', [...base, [6, 3, 3, 'mekanism:structural_glass']], false, true],
    ['glass-ring', [...base, [6, 1, 1, 'mekanism:structural_glass']], false, false],
    ['port-ring', [...base, [6, 1, 1, PORT]], false, false],
    ['port-edge', [...base, [6, 6, 3, PORT]], false, false],
    ['hole-ring', [...base, [6, 1, 1, 'minecraft:air']], false, false],
    ['hole-side', [...base, [6, 3, 3, 'minecraft:air']], false, false],
    ['stone-corner', [...base, [6, 0, 0, 'minecraft:stone']], false, true],
    ['port-corner', [...base, [6, 6, 6, PORT]], false, true],
    ['dirt-inside', [...base, [3, 3, 3, 'minecraft:dirt']], false, false],
    ['dirt-inner-corner', [...base, [1, 1, 1, 'minecraft:dirt']], false, false],
    ['reactor-glass', [...base, [6, 3, 3, 'mekanismgenerators:reactor_glass']], false, true],
    ['two-coils', [...base, [6, 3, 3, PORT], [5, 3, 3, COIL]], false, true],
  ];
  const corners: Record<string, string> = {};
  for (const [i, [name, edits, full]] of cases.entries()) corners[name] = await sps(9 * (i % 6), 13 + 9 * Math.floor(i / 6), edits, full);
  await fixture.command(`fill ${at(54, -1, 13)} ${at(56, 1, 15)} ${CASE} hollow`);
  // C1 (ultimate cube), C2 (creative cube), C3 (creative cube, no output port).
  const conversion = (output: boolean): Edit[] => [...base, ...(output ? [[6, 3, 3, `${PORT}[active=true]`] as Edit] : [])];
  const c1 = await sps(1, 0, conversion(true));
  const c2 = await sps(14, 0, conversion(true));
  const c3 = await sps(27, 0, conversion(false));
  for (const c of [c1, c2]) await fixture.command(`data merge block ${of(c, 6, 3, 3)} {active_state:1b}`);
  await fixture.stepServer(2);

  const formed: Record<string, any> = {};
  for (const [name] of cases) formed[name] = await multiblockOf(of(corners[name], 3, 0, 3));
  log.formation = Object.fromEntries(cases.map(([name]) => [name, !!formed[name]]));
  for (const [name, , , forms] of cases) assert.equal(!!formed[name], forms, name);
  assert.equal(await multiblockOf(at(55, 0, 13)), undefined, 'a 3x3x3 of casings');
  assert.equal(formed.plain.kind, 'sps');
  assert.deepEqual([formed.plain.input_capacity, formed.plain.output_capacity, formed.plain.coils], [2000, 1000, [[...of(corners.plain, 0, 3, 3).split(' ').map(Number)]]]);
  for (const [name, c] of [['C1', c1], ['C2', c2], ['C3', c3]] as const) assert(await multiblockOf(of(c, 3, 0, 3)), `${name} formed`);
  fixture.mark(`refused: ${cases.filter(k => !k[3]).map(k => k[0]).join(', ')}, 3x3x3; formed: ${cases.filter(k => k[3]).map(k => k[0]).join(', ')}`);

  // Feeds and sinks.
  const feed = async (c: string, cube: 'ultimate' | 'creative', output: boolean) => {
    const tank = of(c, 3, 3, -1);
    await fixture.command(`setblock ${tank} mekanism:creative_chemical_tank`);
    await fixture.command(`data merge block ${tank} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:polonium",amount:1000000000000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b}}`);
    if (output) {
      await fixture.command(`setblock ${of(c, 7, 3, 3)} mekanism:basic_chemical_tank`);
      await fixture.command(`data merge block ${of(c, 7, 3, 3)} {component_config:{config2:[I;1,1,1,1,1,1],eject2:0b}}`);
    }
    const stored = cube === 'creative' ? '9223372036854775807' : '256000000';
    await fixture.command(`setblock ${of(c, -1, 3, 3)} mekanism:${cube}_energy_cube`);
    await fixture.command(`data merge block ${of(c, -1, 3, 3)} {component_config:{config0:[I;4,4,4,4,4,4],eject0:1b},energy_containers:[{container:0b,stored:"${stored}"}]}`);
  };
  await feed(c1, 'ultimate', true);
  await feed(c2, 'creative', true);
  await feed(c3, 'creative', false);
  const rows: Record<string, any[]> = { C1: [], C2: [], C3: [] };
  const sample = async (t: number) => {
    for (const [name, c] of [['C1', c1], ['C2', c2], ['C3', c3]] as const) {
      const m = await multiblockOf(of(c, 3, 0, 3));
      const port = m.ports.find((p: any) => p.pos.join(' ') === of(c, 0, 3, 3));
      const basic = name === 'C3' ? 0 : await chemicalIn(of(c, 7, 3, 3));
      rows[name].push({ t, polonium: m.polonium, antimatter: m.antimatter, processed: m.processed, progress: m.progress, last: m.last_processed, port: port?.energy, basic, could: m.could_operate });
    }
  };
  let t = 0;
  for (const target of [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 100, 392, 393, 400, 502, 510]) {
    await fixture.stepServer(target - t);
    t = target;
    await sample(t);
  }
  log.rows = rows;
  const by = (name: string, at: number) => rows[name].find(r => r.t === at);
  // C1: the oracle's progress tick by tick.
  assert.deepEqual([by('C1', 1).port, by('C1', 1).polonium, by('C1', 2).processed], [256_000, 2000, 0]);
  assert.deepEqual(rows.C1.slice(2, 12).map(r => r.processed), [2, 5, 7, 10, 12, 15, 17, 20, 23, 25]);
  assert.deepEqual(rows.C1.slice(2, 5).map(r => r.progress), [0.56, 0.1200000000000001, 0.6800000000000002]);
  assert.deepEqual([by('C1', 100).processed, by('C1', 100).progress, by('C1', 100).last], [250, 0.8800000000000052, 2.56]);
  assert.deepEqual([by('C1', 392).processed, by('C1', 392).progress, by('C1', 392).antimatter + by('C1', 392).basic], [998, 0.4000000000000208, 0]);
  assert.deepEqual([by('C1', 393).antimatter + by('C1', 393).basic, by('C1', 400).processed, by('C1', 400).basic], [1, 18, 1]);
  // C2: the whole input tank a tick, 2 mB of antimatter out.
  assert.deepEqual([by('C2', 2).basic, by('C2', 3).basic, by('C2', 3).last, by('C2', 12).basic, by('C2', 100).basic], [0, 2, 2000, 20, 196]);
  assert.equal(by('C2', 12).port, 1_000_000_000);
  // C3: full on the 502nd tick, then stopped with the port's Joules kept.
  assert.deepEqual([by('C3', 3).antimatter, by('C3', 400).antimatter, by('C3', 502).antimatter, by('C3', 510).antimatter], [2, 796, 1000, 1000]);
  assert.deepEqual([by('C3', 510).could, by('C3', 510).last, by('C3', 510).port], [false, 0, 1_000_000_000]);
  fixture.mark('C1 2.56 mB a tick (998 and 0.4 by the 392nd, its first antimatter on the 393rd), C2 2 mB of antimatter a tick, C3 full on the 502nd');

  // The screen on a casing.
  await fixture.hold(false);
  const casing = of(c2, 3, 0, 0);
  const [sx, sy, sz] = casing.split(' ').map(Number);
  await fixture.command(`tp @s ${sx + 0.5} ${sy + 1} ${sz - 1.5}`);
  await fixture.raw(`interact ${of(c2, 3, 1, 0)} north`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the SPS screen opens');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'sps-screen.png')}`);
  await fixture.raw('close');
  await fixture.hold();
  fixture.mark("the SPS's screen opens on a casing");

  // Kept over a restart: C3's full output tank.
  await fixture.restart();
  const after = await until(() => multiblockOf(of(c3, 3, 0, 3)), m => !!m, 'C3 formed again after the restart');
  log.after = after;
  assert.deepEqual([after.antimatter, after.polonium], [1000, 2000]);
  assert(await multiblockOf(of(c1, 3, 0, 3)), 'C1 formed again after the restart');
  fixture.mark(`after a restart C3 keeps ${after.antimatter} mB of antimatter and ${after.polonium} of polonium`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK sps', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK sps', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
