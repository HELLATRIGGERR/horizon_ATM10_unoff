/** All cube tiers, the creative cube as a sink, and Mekanism's configs reloading live.
 *
 * - Advanced, Elite and Ultimate cubes hold 16, 64 and 256 MJ and put out 16, 64 and 256 kJ a tick. With `ATM10_DIR`
 *   the world starts with ATM10's own `config/Mekanism/general.toml` and `tiers.toml`, which say so.
 * - An empty creative cube (`stored: 0`) takes whatever it is given and stays empty: a void.
 * - Writing `tiers.toml` (basicOutput 5000, eliteCapacity 128 MJ) and `general.toml` (feConversionRate 4) while the
 *   world runs changes the basic output to 5000 J a tick, the elite capacity and the rate within about a second.
 * Needs what `energy-cube.ts` needs; `pack/scripts/mekanism.ts world` runs every test. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-cube-tiers', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
const fromAtm10 = !!atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'));
if (fromAtm10) {
  mkdirSync(config, { recursive: true });
  for (const file of ['general.toml', 'tiers.toml']) copyFileSync(join(atm10!, 'config/Mekanism', file), join(config, file));
}

async function energy(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}`);
  const stored = line.match(/stored: (\d+)L/);
  return stored ? Number(stored[1]) : 0;
}
/** `/mek probe` as the player; `n` tells repeated probes of one place apart. */
let probes = 0;
async function probe(pos: string): Promise<any> {
  const before = probes++;
  const needle = `"pos":[${pos.split(' ').join(',')}]`;
  const count = async () => ((await fixture.raw('chat-log 100')).chat ?? []).filter((t: string) => t.startsWith('MEK {') && t.includes(needle));
  const seen = (await count()).length;
  await fixture.raw(`chat /mek probe ${pos}`);
  const lines = await until(count, l => l.length > seen, `/mek probe ${pos} (${before})`);
  return JSON.parse(lines[0].slice(4)); // chat-log gives the newest line first
}
async function gain(pos: string, ticks: number): Promise<number> {
  await fixture.hold();
  const before = await energy(pos);
  await fixture.stepServer(ticks);
  const after = await energy(pos);
  await fixture.hold(false);
  return after - before;
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;

  // The tiers.
  const sizes: Record<string, [number, number]> = { advanced: [16e6, 16e3], elite: [64e6, 64e3], ultimate: [256e6, 256e3] };
  for (const [i, tier] of Object.keys(sizes).entries()) {
    const at = `${x + 6} ${y} ${z - 2 * i}`;
    await fixture.command(`setblock ${at} mekanism:${tier}_energy_cube`);
    const p = await probe(at);
    log[tier] = [p.capacity, p.output];
    assert.deepEqual([p.capacity, p.output], sizes[tier], tier);
    assert.equal(p.config.from_files, fromAtm10, 'read from tiers.toml when the world has one');
  }
  fixture.mark(`advanced, elite and ultimate cubes hold 16, 64 and 256 MJ and put out 16, 64 and 256 kJ a tick (${fromAtm10 ? "ATM10's tiers.toml" : 'defaults'})`);

  // An empty creative cube is a void: basic cube D (4 MJ) ejects into it, and it stays empty.
  const d = `${x + 3} ${y} ${z}`, sink = `${x + 3} ${y} ${z - 1}`;
  await fixture.command(`setblock ${sink} mekanism:creative_energy_cube`);
  await fixture.command(`data merge block ${sink} {energy_containers:[{container:0b,stored:0L}]}`);
  await fixture.command(`setblock ${d} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${d} {energy_containers:[{container:0b,stored:4000000L}]}`);
  assert.equal(await gain(d, 10), -40000, 'D empties into the void at 4000 J a tick');
  assert.equal((await probe(sink)).energy, 0, 'the void stays empty');
  fixture.mark('an empty creative cube takes 4000 J a tick from a basic cube and stays empty');

  // Configs reload live: creative -> A -> B, then basicOutput 5000 and feConversionRate 4.
  const creative = `${x} ${y} ${z}`, a = `${x} ${y} ${z - 1}`, b = `${x} ${y} ${z - 2}`;
  await fixture.command(`setblock ${creative} mekanism:creative_energy_cube`);
  await fixture.command(`setblock ${a} mekanism:basic_energy_cube`);
  await fixture.command(`setblock ${b} mekanism:basic_energy_cube`);
  await gain(b, 2);
  assert.equal(await gain(b, 10), 40000);
  mkdirSync(config, { recursive: true });
  writeFileSync(join(config, 'tiers.toml'), '[energy_cubes]\nbasicOutput = 5000\neliteCapacity = 128000000\n');
  writeFileSync(join(config, 'general.toml'), '[energy_conversion]\nfeConversionRate = 4.0\n');
  const reloaded = await until(() => probe(a), p => p.output === 5000 && p.config.fe_conversion_rate === 4, 'the settings are read again', 120000);
  log.reloaded = reloaded.config;
  assert.equal(await gain(b, 10), 50000, 'B gains 5000 J a tick');
  const elite = await probe(`${x + 6} ${y} ${z - 2}`);
  log.reloaded_elite = [elite.capacity, elite.output];
  assert.deepEqual([elite.capacity, elite.output], [128e6, 64e3], 'the written capacity, the default output');
  fixture.mark('written config files are read again while the world runs: 5000 J a tick, elite 128 MJ, feConversionRate 4');
  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK tiers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK tiers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
