/** Universal Cables of all tiers: networks across save, restart and chunk unloading, with cubes and any FE block.
 *
 * - Throughput: a creative cube feeds a target cube through one cable of each tier: 8 000, 128 000, 1 024 000 and
 *   8 192 000 J a tick (`CableTier`; with `ATM10_DIR`, ATM10's own `tiers.toml`, which says the same). Three basic cables
 *   in a row carry three times one.
 * - A full basic cube feeds a line: 4000 J a tick, shared between a cube and an AE2 Energy Acceptor at its end,
 *   2000 J and 2000 J = 800 FE = 400 AE. AE2's debug generator feeds a cable like any FE block: 8 FE = 20 J a tick to the
 *   cube beyond it, and `/ae2 power` reads the cable's network as FE.
 * - Sides: pull draws up to a cable's capacity from a cube that isn't ejecting; push takes nothing in; none cuts it.
 * - Breaking the middle cable of a half-full line keeps the energy, half to each part.
 * - Chunks of a line unloading and loading again (`TransmitterNetworkTest`): their cables take their shares with them and
 *   bring them back, and no Joule is lost or made.
 * - A save and restart keep a network's buffer and a configured side, and the throughput is the same after.
 *
 * Needs what `energy-cube.ts` needs; `pack/scripts/mekanism.ts world` runs every test. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-cable', 'server-thread', { devMods: [content] });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  const config = join(fixture.game, 'config/Mekanism');
  mkdirSync(config, { recursive: true });
  for (const file of ['general.toml', 'tiers.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}
const TIERS: Record<string, number> = { basic: 8000, advanced: 128000, elite: 1024000, ultimate: 8192000 };

const stored = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
async function energy(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  const found = line.match(/stored: (\d+)L/);
  return found ? Number(found[1]) : 0;
}
/** A `/mek` command as the player types it, and its answer. Not while the server is held: its answer needs ticks. */
async function mek(command: string, pos: string): Promise<any> {
  const needle = `"pos":[${pos.split(' ').join(',')}]`;
  // chat-log gives the newest line first; each answer has its running number `n`.
  const newest = async () => {
    const line = ((await fixture.raw('chat-log 100')).chat ?? []).find((t: string) => t.startsWith('MEK {'));
    return line ? JSON.parse(line.slice(4)) : { n: 0 };
  };
  const seen = (await newest()).n;
  await fixture.raw(`chat /mek ${command}`);
  const answer = await until(newest, a => a.n > seen, `/mek ${command}`, 60000);
  assert(JSON.stringify(answer).includes(needle), `/mek ${command}: ${JSON.stringify(answer)}`);
  return answer;
}
const probe = (pos: string) => mek(`probe ${pos}`, pos);
const connect = (pos: string, side: string, type: string) => mek(`connection ${pos} ${side} ${type}`, pos);
/** Per tick over `ticks`, held: what `pos` gained. Starts after `settle` ticks, for what was just placed. */
async function rate(pos: string, ticks = 5, settle = 3): Promise<number[]> {
  await fixture.hold();
  await fixture.stepServer(settle);
  const out: number[] = [];
  let last = await energy(pos);
  for (let i = 0; i < ticks; i++) {
    await fixture.stepServer(1);
    const now = await energy(pos);
    out.push(now - last); last = now;
  }
  await fixture.hold(false);
  return out;
}
const same = (value: number) => Array(5).fill(value);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.ceil(py) + 3, z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -1, -2)} ${at(24, 3, 24)} minecraft:air`);

  // Throughput of one cable of each tier. Cubes as placed: front north; the five other sides take energy.
  const rows = Object.keys(TIERS).map((tier, i) => ({ tier, source: at(i * 3, 0, 2), cable: at(i * 3, 0, 1), target: at(i * 3, 0, 0) }));
  for (const row of rows) {
    await fixture.command(`setblock ${row.target} mekanism:ultimate_energy_cube`);
    await fixture.command(`setblock ${row.cable} mekanism:${row.tier}_universal_cable`);
    await fixture.command(`setblock ${row.source} mekanism:creative_energy_cube`);
  }
  const measureRows = async (label: string) => {
    const out: Record<string, number[]> = {};
    for (const row of rows) {
      await fixture.hold();
      await fixture.command(`data merge block ${row.target} ${stored(0)}`);
      out[row.tier] = await rate(row.target);
      assert.deepEqual(out[row.tier], same(TIERS[row.tier]), `${label}: ${row.tier} carries its capacity a tick`);
    }
    log[`throughput_${label}`] = out;
  };
  await measureRows('first');
  const shown = await until(() => probe(rows[0].cable), p => p.faces.north === 'normal' && p.faces.south === 'normal', 'the cable shows its two connections');
  log.one_cable = shown;
  assert.equal(shown.faces.east, 'none');
  assert.equal(shown.network.cables, 1);
  assert.equal(shown.network.capacity, 8000);
  fixture.mark('one cable of each tier carries 8 000, 128 000, 1 024 000 and 8 192 000 J a tick');

  // Three basic cables are a network of three times the buffer.
  const line3 = { source: at(0, 2, 4), cables: [at(0, 2, 3), at(0, 2, 2), at(0, 2, 1)], target: at(0, 2, 0) };
  await fixture.command(`setblock ${line3.target} mekanism:ultimate_energy_cube`);
  for (const cable of line3.cables) await fixture.command(`setblock ${cable} mekanism:basic_universal_cable`);
  await fixture.command(`setblock ${line3.source} mekanism:creative_energy_cube`);
  await fixture.hold();
  await fixture.command(`data merge block ${line3.target} ${stored(0)}`);
  const threeBasic = await rate(line3.target);
  log.three_basic = threeBasic;
  assert.deepEqual(threeBasic, same(24000), 'a network holds what its cables hold together');
  const middle = await probe(line3.cables[1]);
  assert.equal(middle.network.cables, 3);
  assert.equal(middle.faces.north, 'normal');
  assert.equal(middle.faces.south, 'normal');
  assert.equal(middle.faces.up, 'none');
  await fixture.command(`setblock ${line3.source} minecraft:air`);
  fixture.mark('three basic cables in a row carry 24 000 J a tick');

  // A full basic cube -> line -> a cube and an AE2 Energy Acceptor beside its end.
  const a = at(0, 0, 18), line = [at(0, 0, 17), at(0, 0, 16), at(0, 0, 15), at(0, 0, 14)];
  const acceptor = at(0, 0, 13), cell = at(0, 0, 12), b = at(1, 0, 14);
  for (const cable of line) await fixture.command(`setblock ${cable} mekanism:basic_universal_cable`);
  await fixture.command(`setblock ${cell} ae2:energy_cell`);
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`);
  await fixture.command(`setblock ${b} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${b} {component_config:{config0:[I;1,1,1,1,1,1]}}`);
  const grid = () => fixture.ae(`power ${cell} north`);
  await until(grid, s => s.energy_service?.members > 0, 'the acceptor and cell form a grid', 30000);
  const total = (s: any) => s.energy + (s.energy_service?.buffer ?? 0);
  const measureLine = async (label: string) => {
    await until(grid, s => s.energy_service?.members > 0, `${label}: the grid`, 60000);
    await fixture.hold();
    await fixture.command(`setblock ${a} mekanism:basic_energy_cube`);
    await fixture.command(`data merge block ${a} ${stored(4000000)}`);
    await fixture.stepServer(3);
    const lost: number[] = [], toB: number[] = [], toGrid: number[] = [];
    let aLast = await energy(a), bLast = await energy(b), gridLast = total(await grid());
    for (let i = 0; i < 5; i++) {
      await fixture.stepServer(1);
      const aNow = await energy(a), bNow = await energy(b), gridNow = total(await grid());
      lost.push(aLast - aNow); toB.push(bNow - bLast); toGrid.push(Math.round((gridNow - gridLast) * 1000) / 1000);
      aLast = aNow; bLast = bNow; gridLast = gridNow;
    }
    log[`line_${label}`] = { lost, toB, toGrid };
    assert.deepEqual(lost, same(4000), `${label}: the cube's output goes into the line`);
    assert.deepEqual(toB, same(2000), `${label}: half to the cube`);
    for (const gain of toGrid) assert(Math.abs(gain - 400) < 1, `${label}: 2000 J = 800 FE = 400 AE: ${toGrid}`);
    await fixture.command(`setblock ${a} minecraft:air`);
    await fixture.hold(false);
  };
  await measureLine('first');
  fixture.mark('a basic cube feeds a line of four: 2000 J a tick to a cube and 2000 J = 800 FE = 400 AE to an AE2 acceptor');

  // AE2's debug generator feeds a cable, as any FE block does; AE2 reads the network as FE.
  const generator = at(6, 2, 4), fed = at(6, 2, 3), sink = at(6, 2, 2);
  await fixture.command(`setblock ${sink} mekanism:basic_energy_cube`);
  await fixture.command(`setblock ${fed} mekanism:basic_universal_cable`);
  await fixture.command(`setblock ${generator} ae2:debug_energy_gen`);
  const fromGenerator = await rate(sink);
  log.from_generator = fromGenerator;
  assert.deepEqual(fromGenerator, same(20), '8 FE a tick are 20 J');
  const read = await fixture.ae(`power ${fed} south`);
  log.cable_as_fe = read.fe;
  assert.equal(read.fe?.capacity, 3200, '8000 J are 3200 FE');
  assert.equal(read.fe?.can_receive, true);
  await fixture.command(`setblock ${generator} minecraft:air`);
  fixture.mark('AE2 feeds a cable 8 FE = 20 J a tick and reads its network as 3200 FE of capacity');

  // Sides: a cube with its ejector off, a cable whose south side pulls.
  const p = at(12, 2, 4), pc = at(12, 2, 3), q = at(12, 2, 2);
  await fixture.command(`setblock ${q} mekanism:ultimate_energy_cube`);
  await fixture.command(`setblock ${pc} mekanism:basic_universal_cable`);
  await fixture.command(`setblock ${p} mekanism:basic_energy_cube`);
  const full = `{energy_containers:[{container:0b,stored:4000000L}],component_config:{eject0:0b}}`;
  await fixture.command(`data merge block ${p} ${full}`);
  assert.deepEqual(await rate(q), same(0), 'no ejector, no pull: nothing moves');
  const pulling = await connect(pc, 'south', 'pull');
  assert.equal(pulling.connections[3], 'pull');
  await until(() => probe(pc), s => s.faces.south === 'pull', 'the cable shows its pull side');
  const pulled = await rate(q);
  log.pulled = pulled;
  assert.deepEqual(pulled, same(8000), 'a pull side draws up to the cable a tick');
  fixture.mark('a pull side draws 8000 J a tick out of a cube that is not ejecting');
  // Push takes nothing in, even from the cube's ejector; none cuts the side.
  await fixture.command(`data merge block ${p} {component_config:{eject0:1b}}`);
  await connect(pc, 'south', 'push');
  await until(() => probe(pc), s => s.faces.south === 'push', 'the cable shows its push side');
  const pushed = await rate(q);
  log.pushed = pushed;
  assert.deepEqual(pushed, same(0), 'a push side takes nothing in');
  const cut = await connect(pc, 'south', 'none');
  assert.equal(cut.faces.south, 'none');
  assert.equal((await fixture.ae(`power ${pc} south`)).fe, null, 'no FE capability on a cut side');
  assert.deepEqual(await rate(q), same(0), 'a cut side takes nothing in');
  await connect(pc, 'south', 'normal');
  await until(() => probe(pc), s => s.faces.south === 'normal', 'the cable shows its normal side');
  assert.deepEqual(await rate(q), same(4000), 'back to the cube output');
  await fixture.command(`setblock ${p} minecraft:air`);
  fixture.mark('a push side and a cut side take nothing in; a normal side takes the cube output');

  // Breaking the middle cable of a half-full line keeps the energy.
  const half = [at(18, 2, 3), at(18, 2, 2), at(18, 2, 1)], halfSource = at(18, 2, 4);
  for (const cable of half) await fixture.command(`setblock ${cable} mekanism:basic_universal_cable`);
  await fixture.command(`setblock ${halfSource} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${halfSource} ${stored(12000)}`);
  const filled = await until(() => probe(half[0]), s => s.network?.buffer === 12000 && s.network.cables === 3, 'the line takes the cube\'s 12 000 J');
  log.half_line = filled.network;
  await fixture.command(`setblock ${half[1]} minecraft:air`);
  const parts = await until(async () => [await probe(half[0]), await probe(half[2])], ([l, r]) => l.network?.cables === 1 && r.network?.cables === 1, 'the line splits');
  log.half_parts = parts.map(s => s.network.buffer);
  assert.deepEqual(parts.map(s => s.network.buffer), [6000, 6000], 'nothing lost: half to each part');
  fixture.mark('breaking the middle of a half-full line keeps its energy: 6000 J in each part');
  await fixture.command(`setblock ${halfSource} minecraft:air`);

  // A line over six chunks with one end kept loaded: the player leaves, the far chunks unload, and it comes back
  // (`TransmitterNetworkTest`). A forced chunk keeps its neighbours too, as vanilla's tickets do, so the count is read.
  const LINE = 96, cx = (Math.floor(x / 16) + 4) * 16, lineY = y, lineZ = z + 40;
  const chunkLine = (i: number) => `${cx + i} ${lineY} ${lineZ}`;
  await fixture.command(`forceload add ${cx} ${lineZ} ${cx + LINE - 1} ${lineZ}`);
  const lineSource = `${cx} ${lineY} ${lineZ + 1}`;
  // Forced chunks load over the next ticks: a setblock beside the line in each of them says when it has (raw, as
  // `command` refuses a write into a chunk not loaded yet). The source's stone stays; the others go again.
  for (let i = 0; i < LINE; i += 16) {
    const mark = `${cx + i} ${lineY} ${lineZ + 1}`;
    await until(() => fixture.raw(`command setblock ${mark} minecraft:stone`), r => !JSON.stringify(r).includes('not loaded'), 'the line\'s chunks load', 60000);
    if (i > 0) await fixture.command(`setblock ${mark} minecraft:air`);
  }
  await fixture.command(`fill ${chunkLine(0)} ${chunkLine(LINE - 1)} mekanism:basic_universal_cable`);
  await fixture.command(`setblock ${lineSource} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${lineSource} ${stored(LINE * 1000)}`);
  const whole = await until(() => probe(chunkLine(0)), s => s.network?.buffer === LINE * 1000 && s.network.cables === LINE, 'the line takes its energy', 60000);
  log.chunk_line = whole.network;
  assert.equal(whole.network.capacity, LINE * 8000);
  await fixture.command(`setblock ${lineSource} minecraft:air`);
  await fixture.command(`forceload remove ${cx + 16} ${lineZ} ${cx + LINE - 1} ${lineZ}`);
  const loadsBefore = whole.loads, unloadsBefore = whole.unloads;
  // `teleport`: the harness's `tp @s` check expects the player to stay at that height, and here it falls.
  await fixture.command(`teleport @s ${x + 2000} ${y + 20} ${z}`);
  const split = await until(() => probe(chunkLine(0)), s => s.network?.cables < LINE, 'the far chunks unload', 180000);
  log.chunk_part = split.network;
  log.chunk_unloads = split.unloads - unloadsBefore;
  assert(split.unloads - unloadsBefore >= LINE - split.network.cables, 'each cable told it unloads');
  assert.equal(split.network.buffer, split.network.cables * 1000, 'each unloaded cable took its 1000 J to its save');
  assert.equal(split.network.capacity, split.network.cables * 8000);
  fixture.mark(`the far chunks of a ${LINE}-cable line unload: ${split.network.cables} cables are left with their ${split.network.buffer} J`);
  await fixture.command(`teleport @s ${cx + LINE / 2} ${y + 2} ${lineZ + 3}`);
  const back = await until(() => probe(chunkLine(0)), s => s.network?.cables === LINE, 'the far chunks come back', 180000);
  log.chunk_back = back.network;
  log.chunk_loads = back.loads - loadsBefore;
  assert(back.loads - loadsBefore >= LINE - split.network.cables, 'each cable told it loads');
  assert.equal(back.network.buffer, LINE * 1000, 'no Joule lost or made');
  fixture.mark(`they come back and the line is whole with its ${LINE * 1000} J`);
  await fixture.command(`forceload remove ${cx} ${lineZ}`);
  await fixture.command(`teleport @s ${px} ${py} ${pz}`);
  await until(() => fixture.raw('status'), s => Math.abs(s.player.position[0] - px) < 1 && s.chunks >= 30, 'back at the start', 60000);
  await until(() => probe(rows[0].cable), s => s.network?.cables === 1, 'the first cable back in a network', 60000);

  // A save and restart keep a line's buffer and a configured side.
  const kept = [at(18, 0, 3), at(18, 0, 2), at(18, 0, 1)], keptSource = at(18, 0, 4), keptEnd = at(18, 0, 0);
  for (const cable of kept) await fixture.command(`setblock ${cable} mekanism:basic_universal_cable`);
  await fixture.command(`setblock ${keptSource} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${keptSource} ${stored(10003)}`);
  const beforeRestart = await until(() => probe(kept[0]), s => s.network?.buffer === 10003, 'the line takes the cube\'s 10 003 J');
  await fixture.command(`setblock ${keptSource} minecraft:air`);
  await connect(kept[2], 'east', 'pull');
  log.before_restart = beforeRestart.network;
  await fixture.restart();
  const afterRestart = await until(() => probe(kept[0]), s => s.network?.cables === 3, 'the line after the restart', 120000);
  log.after_restart = afterRestart.network;
  assert.equal(afterRestart.network.buffer, 10003, 'its buffer survives');
  assert.equal(afterRestart.network.capacity, 24000);
  const endAfter = await probe(kept[2]);
  assert.equal(endAfter.connections[5], 'pull', 'its configured side too');
  assert.equal(endAfter.faces.north, 'none', 'nothing on that end');
  await fixture.command(`setblock ${keptEnd} mekanism:basic_energy_cube`);
  await until(() => energy(keptEnd), e => e === 10003, 'the saved energy reaches a cube placed at the end');
  fixture.mark('a line keeps its 10 003 J and a pull side through a save and restart, and gives the energy out');

  // The same throughput after it.
  await measureRows('after_restart');
  await measureLine('after_restart');
  fixture.mark('after the restart each tier carries the same, and the line feeds the cube and AE2 the same');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK cable', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK cable', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
