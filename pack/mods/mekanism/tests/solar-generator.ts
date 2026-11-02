/** Mekanism Generators' Solar Generator, as the `mekanism` native bundle and its content mods make it.
 *
 * Three generators stand in the open, each in a patch of one biome (`/fillbiome`) under ATM10's
 * `solarGeneration = 150` (`generators.toml`; written with ATM10's values when `ATM10_DIR` has none):
 * - plains (0.8, downfall 0.4): 132 J a tick at a clear noon (a peak of 0.88), max output 264;
 * - desert (2.0, no rain): 96 J; snowy plains (0.0, downfall 0.5, snow): 163 J;
 * - at midnight none; in rain the sun dims to 0.75 and where it can rain the peak drops to a fifth: plains 19, desert 72,
 *   snowy plains 24; a thunderstorm darkens the sky past 3, so none; a roof over one takes its sun away;
 * - its energy leaves through its bottom face: into a basic energy cube under it, 132 J a tick, and AE2 reads that face
 *   as Forge Energy (`/ae2 power`, 2.5 J to the FE) and no other face;
 * - its screen shows the energy slot and nothing of the hopper menu's other four.
 *
 * Joules are read from the saved compound (`data get block`, Mekanism's `energy_containers[{stored}]`), the rest from
 * `/mek probe`. Needs the staged native build (`AE2_TEST_BINARY`) and the content mods `mekanism` and
 * `mekanismgenerators` from `mekanism-content` (`MEK_CONTENT_DIR`, `MEKGEN_CONTENT_DIR`); `pack/scripts/mekanism.ts
 * world` sets them. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const generators = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-solar', 'server-thread', { devMods: [content, generators] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
const fromAtm10 = !!atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml'));
if (fromAtm10) {
  for (const file of ['general.toml', 'tiers.toml', 'generators.toml', 'generator-storage.toml']) copyFileSync(join(atm10!, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'generators.toml'), '[solar_generator]\n\tsolarGeneration = 150\n[heat_generator]\n\theatGeneration = 800\n');
  writeFileSync(join(config, 'generator-storage.toml'), 'heatGenerator = 160000\nsolarGenerator = 96000\n');
}

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
async function energy(pos: string): Promise<number> {
  const stored = (await saved(pos)).match(/stored: (\d+)L/);
  return stored ? Number(stored[1]) : 0;
}
/** `/mek probe` as the player types it. Not while the server is held: its answer needs ticks. */
async function probe(pos: string): Promise<any> {
  const needle = `"pos":[${pos.split(' ').join(',')}]`;
  const newest = async () => {
    const line = ((await fixture.raw('chat-log 100')).chat ?? []).find((t: string) => t.startsWith('MEK {'));
    return line ? JSON.parse(line.slice(4)) : { n: 0 };
  };
  const seen = (await newest()).n;
  await fixture.raw(`chat /mek probe ${pos}`);
  const answer = await until(newest, a => a.n > seen, `/mek probe ${pos}`, 60000);
  assert(JSON.stringify(answer).includes(needle) && !answer.error, `/mek probe ${pos}: ${JSON.stringify(answer)}`);
  return answer;
}
/** Each position's gain a tick over `ticks` held ticks, after `settle`. */
async function rates(positions: string[], ticks = 5, settle = 2): Promise<number[][]> {
  await fixture.hold();
  if (settle > 0) await fixture.stepServer(settle);
  const out = positions.map(() => [] as number[]);
  let last = await Promise.all(positions.map(energy));
  for (let i = 0; i < ticks; i++) {
    await fixture.stepServer(1);
    const now = await Promise.all(positions.map(energy));
    now.forEach((v, j) => out[j].push(v - last[j]));
    last = now;
  }
  await fixture.hold(false);
  return out;
}
const same = (value: number) => Array(5).fill(value);
/** The weather `kind` (`clear`, `rain`, `thunder`) at full strength: its levels move 0.01 a tick. */
async function weather(kind: string): Promise<void> {
  await fixture.hold();
  await fixture.command(`weather ${kind}`);
  await fixture.stepServer(110);
  await fixture.hold(false);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  // At most y 80: Biome.getTemperature drops above it.
  const y = Math.min(Math.floor(py) + 2, 78);
  const x = Math.floor(px) - 20, z = Math.floor(pz) + 6;
  const patches = { plains: x, desert: x + 20, snowy_plains: x + 40 } as Record<string, number>;
  const gen = (gx: number) => `${gx} ${y} ${z}`;
  await fixture.command('gamerule doDaylightCycle false');
  await fixture.command('time set 6000');
  for (const [biome, gx] of Object.entries(patches)) {
    // Open sky over a stone floor, and the biome around (Level.getBiome may pick a neighbouring cell).
    await fixture.command(`fill ${gx - 3} ${y} ${z - 3} ${gx + 3} ${y + 30} ${z + 3} minecraft:air`);
    await fixture.command(`fill ${gx - 3} ${y - 1} ${z - 3} ${gx + 3} ${y - 1} ${z + 3} minecraft:stone`);
    await fixture.command(`fillbiome ${gx - 8} ${y - 8} ${z - 8} ${gx + 8} ${y + 8} ${z + 8} minecraft:${biome}`);
  }
  await weather('clear');
  for (const gx of Object.values(patches)) await fixture.command(`setblock ${gen(gx)} mekanismgenerators:solar_generator`);
  const [plains, desert, snowy] = Object.values(patches).map(gen);

  // A clear noon.
  const noon = await rates([plains, desert, snowy]);
  log.clear_noon = noon;
  assert.deepEqual(noon, [same(132), same(96), same(163)], 'plains 132, desert 96, snowy plains 163 J a tick');
  const p = await probe(plains);
  log.probe_plains = p;
  assert.equal(p.config.solar_generation, 150);
  assert.equal(p.biome.id, 'minecraft:plains');
  assert.equal(p.biome.temperature, Math.fround(0.8), 'the float 0.8, below y 80');
  assert.equal(p.sees_sun, true);
  assert.equal(p.sky_light, 15);
  assert.equal(Math.round(p.peak * 100), 88);
  assert.equal(p.max_output, 264);
  assert.equal(p.capacity, 96000);
  assert.equal(p.energy_face, 'down');
  fixture.mark('at a clear noon: plains 132, desert 96, snowy plains 163 J a tick; peak 0.88 and max output 264 in plains');

  // Midnight.
  await fixture.command('time set 18000');
  const night = await rates([plains, desert, snowy]);
  log.midnight = night;
  assert.deepEqual(night, [same(0), same(0), same(0)]);
  assert.equal((await probe(plains)).sees_sun, false);
  fixture.mark('none at midnight');

  // Rain, then a thunderstorm, at noon.
  await fixture.command('time set 6000');
  await weather('rain');
  const rain = await rates([plains, desert, snowy]);
  log.rain = rain;
  assert.deepEqual(rain, [same(19), same(72), same(24)], 'plains 19, desert 72 (it never rains there), snowy plains 24');
  fixture.mark('in rain: plains 19, desert 72, snowy plains 24 J a tick');
  await weather('thunder');
  const storm = await rates([plains, desert, snowy]);
  log.thunder = storm;
  assert.deepEqual(storm, [same(0), same(0), same(0)], 'a thunderstorm darkens the sky past 3');
  fixture.mark('none in a thunderstorm');
  await weather('clear');

  // A roof over the plains one.
  await fixture.command(`setblock ${x} ${y + 2} ${z} minecraft:stone`);
  const roofed = await rates([plains, desert]);
  log.roofed = roofed;
  assert.deepEqual(roofed, [same(0), same(96)]);
  await fixture.command(`setblock ${x} ${y + 2} ${z} minecraft:air`);
  assert.deepEqual((await rates([plains]))[0], same(132), 'the sun again without the roof');
  fixture.mark('a roof takes its sun away');

  // Out of its bottom into a cube: 1000 J saved go out at its max output, beside what it makes, until only each
  // tick's 132 J is left to send. `data merge` gives the engine a new block entity, and a new one's first tick
  // ejects before it has read its biome: twice `solarGeneration`, 300 J; then 264.
  await fixture.hold();
  const below = `${x} ${y - 1} ${z}`;
  await fixture.command(`setblock ${below} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${plains} {energy_containers:[{container:0b,stored:1000L}]}`);
  const [toCube, kept] = await rates([below, plains], 10, 0);
  log.into_cube = { toCube, kept };
  assert.deepEqual(toCube, [300, 264, 264, 264, 264, 264, 172, 132, 132, 132], 'the cube under it: the max output, then 132 J a tick');
  assert.deepEqual(kept.slice(7), [0, 0, 0], 'it keeps only what it made this tick');
  assert.equal(await energy(plains), 132);
  const fe = await fixture.ae(`power ${plains} down`);
  log.fe_down = fe.fe;
  assert.equal(fe.fe?.capacity, 38400, '96 000 J are 38 400 FE');
  assert.equal(fe.fe?.can_extract, true);
  assert.equal((await fixture.ae(`power ${plains} north`)).fe, null, 'no energy on its sides');
  fixture.mark('132 J a tick out of its bottom into a basic cube; Forge Energy on its bottom face only');

  // Its screen: the energy slot only.
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 0.5}`);
  await until(() => fixture.raw(`block ${plains}`), v => v.block === 'mekanismgenerators:solar_generator', 'the generator reaches the client');
  await fixture.raw(`interact ${plains} up`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the solar generator screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0], 'only the energy slot');
  assert.equal(view.title?.[0]?.text, 'Solar Generator');
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'solar-screen.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('close');
  fixture.mark('its screen shows the energy slot');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK solar', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK solar', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
