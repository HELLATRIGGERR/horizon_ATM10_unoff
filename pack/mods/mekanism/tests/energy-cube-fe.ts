/** Forge Energy between Mekanism's energy cubes and AE2, through the engine's FE capability (hook points API 154).
 *
 * - AE2 powers a cube: the debug energy generator pushes 8 FE a tick into the cube beside it, which Mekanism
 *   takes as 20 J (2.5 J per FE, ATM10's feConversionRate), and the cube counts them as its input.
 * - AE2 finds the cube's FE store per side: `/ae2 power` reads it as `IEnergyStorage` (in whole FE), and a side
 *   configured NONE has none.
 * - A cube feeds AE2: a full basic cube's front against an Energy Acceptor ejects its tier output, 4000 J =
 *   1600 FE a tick, which the grid stores as 800 AE.
 * - One ejection: a second output face on another cube shares that output, 2000 J each way.
 * - A cube's output led back into itself through an FE P2P pair moves nothing, and the tick ends.
 * - `feConversionRate` and `blacklistForge` are read again from a written `general.toml`: the readback follows the
 *   rate, and with the blacklist the cube has no FE and feeds only the other cube.
 *
 * Joules are read from the cube's saved compound (`energy_containers[{stored}]`), AE from the energy cell.
 * Needs what `energy-cube.ts` needs; `pack/scripts/mekanism.ts world` runs every test. */
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-cube-fe', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
const FULL = '{energy_containers:[{container:0b,stored:4000000L}]}';
/** config0 by RelativeSide (front, left, right, back, top, bottom): front and top put out. */
const FRONT_AND_TOP = '{component_config:{config0:[I;4,1,1,1,4,1]}}';

async function energy(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  const stored = line.match(/stored: (\d+)L/);
  return stored ? Number(stored[1]) : 0;
}
/** `/mek probe` as the player types it. Not while the server is held: its answer needs ticks. */
async function probe(pos: string): Promise<any> {
  const needle = `"pos":[${pos.split(' ').join(',')}]`;
  const lines = async () => ((await fixture.raw('chat-log 100')).chat ?? []).filter((t: string) => t.startsWith('MEK {') && t.includes(needle));
  const seen = (await lines()).length;
  await fixture.raw(`chat /mek probe ${pos}`);
  const found = await until(lines, l => l.length > seen, `/mek probe ${pos}`);
  return JSON.parse(found[0].slice(4)); // chat-log gives the newest line first
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.ceil(py) + 3, z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  // Cube C with the generator on its east (an input side); cube A, its front (north) against the acceptor, with
  // an energy cell north of that. All cubes as placed: front north, ejecting, the five other sides input.
  const c = at(0), generator = at(1), a = at(0, 0, 8), acceptor = at(0, 0, 7), cell = at(0, 0, 6), top = at(0, 1, 8);
  await fixture.hold();
  await fixture.command(`fill ${at(-2, -2, -2)} ${at(3, 2, 10)} minecraft:air`);

  // AE2 → cube.
  await fixture.command(`setblock ${c} mekanism:basic_energy_cube`);
  await fixture.command(`setblock ${generator} ae2:debug_energy_gen`);
  assert.equal(await energy(c), 0);
  const fromGenerator: number[] = [];
  let last = 0;
  for (let i = 0; i < 5; i++) {
    await fixture.stepServer(1);
    const now = await energy(c);
    fromGenerator.push(now - last); last = now;
  }
  log.cube_from_generator_per_tick = fromGenerator;
  assert.deepEqual(fromGenerator, [20, 20, 20, 20, 20], '8 FE a tick are 20 J');
  fixture.mark(`the debug energy generator fills the cube by 20 J (8 FE) a tick: ${last} J after 5 ticks`);

  // AE2 reads the cube's FE store, side by side.
  const east = await fixture.ae(`power ${c} east`);
  log.cube_fe_east = east.fe;
  assert.deepEqual(east.fe, { stored: 40, capacity: 1600000, can_receive: true, can_extract: true },
    '100 J are 40 FE; 4 MJ are 1.6 M FE');
  assert.equal((await fixture.ae(`power ${c} north`)).fe?.stored, 40, 'the front (an output) exposes it too');
  await fixture.command(`data merge block ${c} {component_config:{config0:[I;4,1,1,1,1,0]}}`);
  const down = await fixture.ae(`power ${c} down`);
  log.cube_fe_down_none = down.fe;
  assert.equal(down.fe, null, 'a NONE side has no FE capability');
  fixture.mark('AE2 reads the cube as IEnergyStorage per side: 40 of 1 600 000 FE, none on a NONE side');

  // The FE it takes counts as its input.
  await fixture.hold(false);
  const counted = await until(() => probe(c), p => p.input === 20, 'the cube counts what FE brought in');
  log.cube_input = counted.input;
  await fixture.command(`setblock ${generator} minecraft:air`);
  fixture.mark('the cube counts the FE it takes as 20 J of input a tick');

  // Cube → AE2: the grid first, then the cube's energy.
  await fixture.command(`setblock ${cell} ae2:energy_cell`);
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`);
  await fixture.command(`setblock ${a} mekanism:basic_energy_cube`);
  const grid = () => fixture.ae(`power ${cell} north`);
  await until(grid, s => s.energy_service?.members > 0, 'the acceptor and cell form a grid', 30000);
  await fixture.hold();
  await fixture.command(`data merge block ${a} ${FULL}`);
  assert.equal(await energy(a), 4000000);
  const total = (s: any) => s.energy + (s.energy_service?.buffer ?? 0);
  /** Per tick over 5 ticks: what A lost, what the cube on top gained (none: no cube there), what the grid gained. */
  const measure = async (withTop: boolean) => {
    const topEnergy = async () => withTop ? await energy(top) : 0;
    const lost: number[] = [], toTop: number[] = [], toGrid: number[] = [];
    let aBefore = await energy(a), topBefore = await topEnergy(), gridBefore = total(await grid());
    for (let i = 0; i < 5; i++) {
      await fixture.stepServer(1);
      const aNow = await energy(a), topNow = await topEnergy(), gridNow = total(await grid());
      lost.push(aBefore - aNow); toTop.push(topNow - topBefore);
      toGrid.push(Math.round((gridNow - gridBefore) * 1000) / 1000);
      aBefore = aNow; topBefore = topNow; gridBefore = gridNow;
    }
    return { lost, toTop, toGrid };
  };
  const alone = await measure(false);
  log.cube_to_acceptor = alone;
  assert.deepEqual(alone.lost, [4000, 4000, 4000, 4000, 4000], 'the basic tier output, 4000 J a tick');
  for (const gain of alone.toGrid) assert(Math.abs(gain - 800) < 1, `1600 FE are 800 AE: ${alone.toGrid}`);
  fixture.mark(`a full basic cube feeds the energy acceptor 4000 J = 1600 FE = ${alone.toGrid[alone.toGrid.length - 1]} AE a tick`);

  // One ejection: another cube on A's top, which now puts out too, shares the output with the acceptor.
  await fixture.command(`setblock ${top} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${a} ${FRONT_AND_TOP}`);
  const shared = await measure(true);
  log.cube_shared = shared;
  assert.deepEqual(shared.lost, [4000, 4000, 4000, 4000, 4000], 'one output, shared');
  assert.deepEqual(shared.toTop, [2000, 2000, 2000, 2000, 2000], 'half to the other cube');
  for (const gain of shared.toGrid) assert(Math.abs(gain - 400) < 1, `2000 J = 800 FE are 400 AE: ${shared.toGrid}`);
  fixture.mark('two output faces share one output: 2000 J to the other cube, 2000 J = 800 FE = 400 AE to the acceptor');
  await fixture.command(`setblock ${a} minecraft:air`);

  // A cube's output back into itself: its front on the input tunnel of an FE P2P pair, the output tunnel on its east.
  const loop = (dx: number, dy = 0, dz = 0) => at(dx, dy, 14 + dz);
  const cube = loop(1, 0, 1), input = loop(1), output = loop(2, 0, 1);
  await fixture.hold(false);
  await fixture.command(`fill ${loop(-2, -2, -2)} ${loop(5, 3, 5)} minecraft:air`);
  await fixture.command(`fill ${loop(-2, -2, -2)} ${loop(5, -2, 5)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');
  for (const pos of [input, loop(2), output]) await fixture.command(`setblock ${pos} ae2:cable_bus`);
  await fixture.command(`setblock ${loop(1, -1)} ae2:creative_energy_cell`);
  for (const [pos, side] of [[input, 'south'], [output, 'west']]) {
    await fixture.replace('hotbar.0', 'ae2:fe_p2p_tunnel'); await fixture.select(0, 'ae2:fe_p2p_tunnel');
    assert.equal((await fixture.ae(`grid part ${pos} ${side}`)).attached, true);
  }
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 14 + 2.5}`);
  await until(() => fixture.raw('status'), s => Math.abs(s.player.position[0] - (x + 1.5)) < 0.2, 'the teleport to the tunnels');
  await fixture.replace('hotbar.0', 'ae2:memory_card'); await fixture.select(0, 'ae2:memory_card');
  // Shift has to reach the server before the click does, and a loaded machine can be slow at that.
  const onCard = async () => JSON.stringify(await fixture.command('data get entity @s SelectedItem')).includes('ae2_p2p_card');
  for (let tries = 0; tries < 5 && !(await onCard()); tries++) {
    await fixture.raw('key shift down');
    await Bun.sleep(1000);
    try { log.card_click = await fixture.raw(`interact ${input} south`); } finally { await fixture.raw('key shift up'); }
    await Bun.sleep(1000);
  }
  assert(await onCard(), 'the input frequency on the card');
  await fixture.raw(`interact ${output} west`);
  await fixture.command(`setblock ${cube} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${cube} ${FULL}`);
  // The pair reaches the cube: the input tunnel reads its FE through the output tunnel.
  const through = await until(() => fixture.ae(`power ${input} south`), s => s.fe?.capacity === 1600000, 'the tunnels reach the cube', 30000);
  log.loop_read_through_p2p = through.fe;
  await fixture.hold();
  const before = await energy(cube);
  await fixture.stepServer(5);
  assert.equal(await energy(cube) - before, 0, 'nothing leaves: the cube is out of the world in its own tick');
  assert.equal(before, 4000000);
  fixture.mark('a cube led back into itself through an FE P2P pair stays at 4 MJ and its tick ends');

  // Written settings are read again: the FE readback follows the rate.
  await fixture.hold(false);
  mkdirSync(config, { recursive: true });
  writeFileSync(join(config, 'general.toml'), '[energy_conversion]\nfeConversionRate = 4.0\n');
  const four = await until(() => fixture.ae(`power ${c} east`), s => s.fe?.capacity === 1000000, 'the FE readback at 4 J per FE', 120000);
  assert.equal(four.fe.stored, Math.floor(await energy(c) / 4));
  log.cube_fe_rate_4 = four.fe;
  fixture.mark(`with feConversionRate = 4 written, AE2 reads the cube as ${four.fe.stored} of 1 000 000 FE`);

  // With the blacklist there is no FE: A feeds only the other cube.
  writeFileSync(join(config, 'general.toml'), '[energy_conversion]\nblacklistForge = true\n');
  await until(() => fixture.ae(`power ${c} east`), s => s.fe === null, 'no FE capability with blacklistForge', 120000);
  await fixture.hold();
  await fixture.command(`setblock ${a} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${a} ${FULL}`);
  await fixture.command(`data merge block ${a} ${FRONT_AND_TOP}`);
  const blacklisted = await measure(true);
  log.cube_blacklist = blacklisted;
  assert.deepEqual(blacklisted.lost, [4000, 4000, 4000, 4000, 4000], 'all of it to the other cube');
  assert.deepEqual(blacklisted.toTop, [4000, 4000, 4000, 4000, 4000]);
  for (const gain of blacklisted.toGrid) assert(Math.abs(gain) < 1, `nothing to the acceptor: ${blacklisted.toGrid}`);
  fixture.mark('with blacklistForge written the cube has no FE: 4000 J a tick to the other cube, none to the acceptor');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK-FE numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK-FE numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
