/** Mekanism Generators' Bio-Generator, Wind Generator and Advanced Solar Generator, as the `mekanism` native bundle
 * and its content mods make them, under ATM10's configs (`generators.toml`: bioGeneration 350, wind 150 to 900 J
 * from y 24, advancedSolarGeneration 1200; `generator-storage.toml`: 160 000, 200 000 and 2 500 000 J). The numbers
 * are what a NeoForge 1.21.1 server with Mekanism and Mekanism Generators 10.7.19 and ATM10's configs gave for the
 * same steps over RCON (`/tick step`, `data get block`; local `chem-work/g2`, w1 and w1b):
 *
 * - Bio: 350 J and one mB of bioethanol a tick, a store without room for a whole 350 J burns nothing; two bio fuel
 *   in its slot give 200 mB a tick before it burns (199, then 398); 700 J a tick out of its front into a cube. Its
 *   tank takes bioethanol from an active fluid tank above it, and no lava.
 * - Wind: 150 J a tick at the ground and below y 24 (its top four blocks up), then a straight line to 900 at the
 *   world's logical top: 353 at y 100, 607 at 200, 900 at 315; stone over its top stops it at its next look, every
 *   20 ticks; 1800 J a tick out of its front and bottom.
 * - Advanced solar: 1056 J a tick in plains at a clear noon (nine checks of 0.88), max output 2112; five of its
 *   eight outer checks shaded make it stop, three leave 704.
 * - Bounding blocks: a player placing the wind generator puts four `mekanism:bounding_block`s over it, the advanced
 *   solar generator one and nine over that; a placement whose bounding blocks don't fit doesn't happen; a generator
 *   taken away takes them with it.
 * - Their screens: the bio-generator's fuel and energy slots, the wind generator's energy slot.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mods `mekanism` and `mekanismgenerators`
 * (`MEK_CONTENT_DIR`, `MEKGEN_CONTENT_DIR`); `pack/scripts/mekanism.ts world` sets them. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const generators = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-generators-2', 'server-thread', { devMods: [content, generators] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'generators.toml', 'generator-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'generators.toml'), '[solar_generator]\n\tsolarGeneration = 150\n\tadvancedSolarGeneration = 1200\n'
    + '[bio_generator]\n\tbioGeneration = 350\n[wind_generator]\n\tgenerationMin = 150\n\tgenerationMax = 900\n');
  writeFileSync(join(config, 'generator-storage.toml'), 'bioGenerator = 160000\nadvancedSolarGenerator = 2500000\nwindGenerator = 200000\n');
}

const BIO = 'mekanismgenerators:bio_generator', WIND = 'mekanismgenerators:wind_generator', SOLAR = 'mekanismgenerators:advanced_solar_generator';
const ETHANOL = 'mekanismgenerators:bioethanol', BOUNDING = 'mekanism:bounding_block';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const probe = (pos: string) => mek(`mek probe ${pos}`);
async function energy(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((t: string) => t.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  const stored = line.match(/energy_containers: \[\{[^\]]*stored: (\d+)L/);
  return stored ? Number(stored[1]) : 0;
}
async function blockIs(pos: string, block: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${block}`);
  return (reply.chat ?? []).some((t: string) => t.includes('Test passed'));
}
/** Each position's gain a tick over `ticks` held ticks. */
async function gains(positions: string[], ticks = 5): Promise<number[][]> {
  const out = positions.map(() => [] as number[]);
  let last = await Promise.all(positions.map(energy));
  for (let i = 0; i < ticks; i++) {
    await fixture.stepServer(1);
    const now = await Promise.all(positions.map(energy));
    now.forEach((v, j) => out[j].push(v - last[j]));
    last = now;
  }
  return out;
}
const same = (value: number, n = 5) => Array(n).fill(value);
/** The wind generator's Joules a tick with its top at `top` (Mekanism's line, in doubles). */
function wind(top: number, minBuild = -64, maxBuild = 320): number {
  const minY = Math.max(24, minBuild), maxY = Math.min(2031, maxBuild - 1);
  const clamped = Math.min(maxY, Math.max(minY, top));
  const toGen = 150 + (900 - 150) / (maxY - minY) * (clamped - minY);
  return Math.trunc(150 * (toGen / 150));
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const y = Math.min(Math.floor(py) + 2, 78);
  const x = Math.floor(px) + 4, z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command('gamerule doDaylightCycle false');
  await fixture.command('time set 6000');
  await fixture.command('weather clear');
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(24, 12, 24)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(24, -1, 24)} minecraft:stone`);
  await fixture.command(`fillbiome ${at(-6, -6, -6)} ${at(28, 14, 28)} minecraft:plains`);

  // The bio-generator: 1000 mB, then a nearly full store, then bio fuel in its slot, then into a cube in front.
  const held = (fluid: string, mb: number) => `fluid_tanks:[{tank:0b,stored:{id:"${fluid}",amount:${mb}}}]`;
  const bio = at(0), full = at(2), fuelled = at(4), feeding = at(6), cube = at(7);
  for (const p of [bio, full, fuelled]) await fixture.command(`setblock ${p} ${BIO}[facing=north]`);
  await fixture.command(`setblock ${feeding} ${BIO}[facing=east]`);
  await fixture.command(`setblock ${cube} mekanism:basic_energy_cube`);
  await fixture.hold();
  await fixture.command(`data merge block ${bio} {${held(ETHANOL, 1000)}}`);
  await fixture.command(`data merge block ${full} {${held(ETHANOL, 1000)},energy_containers:[{container:0b,stored:159700L}]}`);
  await fixture.command(`item replace block ${fuelled} container.0 with mekanism:bio_fuel 2`);
  await fixture.command(`data merge block ${feeding} {${held(ETHANOL, 1000)},energy_containers:[{container:0b,stored:10000L}]}`);
  const burn: number[][] = [];
  for (let t = 0; t < 3; t++) {
    const [b, f, fu] = await Promise.all([bio, full, fuelled].map(probe));
    burn.push([b.energy, b.tank.amount, f.energy, f.tank.amount, fu.tank.amount]);
    await fixture.stepServer(1);
  }
  const [b, f, fu, fe] = await Promise.all([bio, full, fuelled, feeding].map(probe));
  burn.push([b.energy, b.tank.amount, f.energy, f.tank.amount, fu.tank.amount]);
  log.bio = { burn, probe: b };
  assert.deepEqual(burn.slice(1).map(r => [r[0], r[1]]), [[350, 999], [700, 998], [1050, 997]], '350 J and one mB a tick');
  assert.deepEqual(burn.slice(1).map(r => [r[2], r[3]]), same([159700, 1000], 3), 'no room for 350 J: no burn');
  assert.deepEqual(burn.slice(1).map(r => r[4]), [199, 398, 397], 'two bio fuel, 200 mB a tick, then it burns');
  assert.deepEqual([b.max_output, b.capacity, b.tank.capacity, b.energy_faces], [700, 160000, 24000, ['north']]);
  assert.equal(b.active, true);
  const toCube = await gains([cube], 3);
  log.bio_cube = toCube;
  assert.deepEqual(toCube[0], same(700, 3), '700 J a tick out of its front');
  assert.equal(fe.energy_faces[0], 'east');
  await fixture.hold(false);
  fixture.mark('bio: 350 J and one mB a tick; 200 mB of bio fuel a tick into its tank; 700 J a tick out of its front');

  // Its tank from an active fluid tank above it: bioethanol in, lava not.
  const tankAbove = at(0, 1), lavaBio = at(2, 0, 3), lavaAbove = at(2, 1, 3);
  await fixture.command(`setblock ${lavaBio} ${BIO}[facing=north]`);
  await fixture.command(`setblock ${tankAbove} mekanism:basic_fluid_tank`);
  await fixture.command(`setblock ${lavaAbove} mekanism:basic_fluid_tank`);
  await fixture.command(`data merge block ${tankAbove} {${held(ETHANOL, 5000)},active_state:1b}`);
  await fixture.command(`data merge block ${lavaAbove} {${held('minecraft:lava', 5000)},active_state:1b}`);
  const filled = await until(() => probe(bio), p => p.tank.amount > 3000, 'bioethanol comes down into the bio-generator', 30000);
  const lavaFed = await probe(lavaBio);
  log.bio_fill = { filled: filled.tank, lava: lavaFed.tank };
  assert.equal(filled.tank.fluid, ETHANOL);
  assert.equal(lavaFed.tank.amount, 0, 'no lava');
  fixture.mark('its tank takes bioethanol from a fluid tank above it, and no lava');

  // The wind at four heights, and one blocked.
  const columns = [y, 100, 200, 315].map((wy, i) => ({ pos: `${x + 10 + 2 * i} ${wy} ${z}`, top: wy + 4, wy }));
  for (const c of columns) {
    await fixture.command(`fill ${x + 10 + 2 * columns.indexOf(c)} ${c.wy - 1} ${z} ${x + 10 + 2 * columns.indexOf(c)} ${Math.min(c.wy + 6, 319)} ${z} minecraft:air`);
    await fixture.command(`setblock ${c.pos} ${WIND}[facing=north]`);
  }
  const blocked = at(18, 0, 0);
  await fixture.command(`setblock ${blocked} ${WIND}[facing=north]`);
  await fixture.command(`setblock ${at(18, 5, 0)} minecraft:stone`);
  // The light under the stone settles before its second look, 20 ticks after its first.
  await fixture.hold();
  await fixture.stepServer(22);
  const winds = await gains([...columns.map(c => c.pos), blocked]);
  log.wind = winds;
  const expected = columns.map(c => wind(c.top));
  assert.deepEqual(expected.slice(1), [353, 607, 900], 'the oracle\'s 353, 607 and 900');
  assert.deepEqual(winds, [...expected.map(j => same(j)), same(0)]);
  const wp = await probe(columns[1].pos);
  log.wind_probe = wp;
  assert.deepEqual([wp.generation, wp.max_output, wp.capacity, wp.energy_faces], [353, 1800, 200000, ['north', 'down']]);
  // Stone over its top: it makes until it looks again, at most 20 ticks on.
  await fixture.command(`setblock ${x + 12} 105 ${z} minecraft:stone`);
  await fixture.stepServer(21);
  assert.deepEqual((await gains([columns[1].pos], 3))[0], same(0, 3), 'blocked at its next look');
  await fixture.hold(false);
  fixture.mark(`wind: ${expected.join(', ')} J a tick at y ${columns.map(c => c.wy).join(', ')}; none with its top blocked`);

  // The advanced solar generator: open, and with its outer checks shaded.
  const solar = at(1, 0, 10), shadedFive = at(6, 0, 10), shadedThree = at(11, 0, 10);
  for (const p of [solar, shadedFive, shadedThree]) await fixture.command(`setblock ${p} ${SOLAR}[facing=north]`);
  const cells = [[-1, -1], [-1, 0], [-1, 1], [0, -1], [0, 1], [1, -1], [1, 0], [1, 1]];
  for (const [p, n] of [[6, 5], [11, 3]] as const) {
    for (const [dx, dz] of cells.slice(0, n)) await fixture.command(`setblock ${at(p + dx, 3, 10 + dz)} minecraft:stone`);
  }
  // Each check looks again 10 to 30 ticks after its first look, by when the light under the stone has settled.
  await fixture.hold();
  await fixture.stepServer(32);
  const suns = await gains([solar, shadedFive, shadedThree]);
  const sp = await probe(solar);
  log.solar = { suns, probe: sp };
  assert.deepEqual(suns, [same(1056), same(0), same(704)], '1056 J a tick in plains; five shaded none, three 704');
  assert.deepEqual([sp.max_output, sp.capacity, sp.energy_faces], [2112, 2500000, ['north', 'down']]);
  assert.equal(sp.checks.length, 9);
  assert(sp.checks.every((c: any) => c.every >= 10 && c.every <= 30), 'each check looks every 10 to 30 ticks');
  await fixture.hold(false);
  fixture.mark('advanced solar: 1056 J a tick at a clear noon, max output 2112; five of eight shaded stop it, three leave 704');

  // Bounding blocks: placed by a player, refused where they don't fit, gone with the generator.
  await fixture.command('gamemode creative');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 16.5}`);
  await fixture.raw('look 0 60');
  const place = async (item: string, target: string) => {
    await fixture.replace('hotbar.0', item);
    await fixture.select(0, item);
    await fixture.raw(`interact ${target} up`);
  };
  const windAt = at(2, 0, 19), solarAt = at(5, 0, 20), lowAt = at(8, 0, 17);
  await place(WIND, at(2, -1, 19));
  await until(() => blockIs(windAt, WIND), v => v, 'the wind generator is placed');
  await place(SOLAR, at(5, -1, 20));
  await until(() => blockIs(solarAt, SOLAR), v => v, 'the advanced solar generator is placed');
  await fixture.command(`fill ${at(7, 2, 16)} ${at(9, 2, 18)} minecraft:stone`);
  await place(SOLAR, at(8, -1, 17));
  await fixture.hold();
  await fixture.stepServer(5);
  await fixture.hold(false);
  const windBounding = (await probe(windAt)).bounding.map((b: any) => b.block);
  const solarBounding = (await probe(solarAt)).bounding.map((b: any) => b.block);
  const low = await blockIs(lowAt, 'minecraft:air');
  log.bounding = { windBounding, solarBounding, low };
  assert.deepEqual(windBounding, Array(4).fill(BOUNDING), 'four over the wind generator');
  assert.deepEqual(solarBounding, Array(10).fill(BOUNDING), 'one and nine over the advanced solar generator');
  assert.equal(low, true, 'a ceiling where its panel would go: not placed');
  await fixture.hold();
  await fixture.stepServer(2);
  assert.deepEqual((await gains([solarAt], 3))[0], same(1056, 3), 'a placed one makes as much under its own panel');
  await fixture.hold(false);
  await fixture.command(`setblock ${windAt} minecraft:air`);
  await fixture.command(`setblock ${solarAt} minecraft:air`);
  await until(async () => (await blockIs(at(2, 4, 19), 'minecraft:air')) && (await blockIs(at(5, 2, 20), 'minecraft:air')), v => v, 'the bounding blocks go');
  assert.equal(await blockIs(at(6, 2, 21), 'minecraft:air'), true);
  fixture.mark('bounding blocks: four over a placed wind generator, ten over an advanced solar one, none where they don\'t fit, gone with it');

  // Their screens.
  await fixture.replace('hotbar.0', 'minecraft:air');
  for (const [pos, slots, title] of [[bio, [0, 1], 'Bio-Generator'], [columns[0].pos, [0], 'Wind Generator']] as const) {
    const [sx, sy, sz] = pos.split(' ').map(Number);
    await fixture.command(`tp @s ${sx + 0.5} ${sy} ${sz - 1.5}`);
    await until(() => fixture.raw(`block ${pos}`), v => typeof v.block === 'string' && v.block.startsWith('mekanismgenerators:'), 'the generator reaches the client');
    await fixture.raw(`interact ${pos} north`);
    const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), `${title} screen opens`);
    const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
    log[`screen_${title}`] = own;
    assert.deepEqual(own, slots);
    assert.equal(view.title?.[0]?.text, title);
    await Bun.sleep(1000);
    await fixture.raw(`screenshot ${join(fixture.run, `${title.toLowerCase().replace(/[^a-z]+/g, '-')}.png`)}`);
    await fixture.raw('close');
  }
  fixture.mark('the bio-generator\'s screen shows its fuel and energy slots, the wind generator\'s its energy slot');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK generators-2', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK generators-2', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
