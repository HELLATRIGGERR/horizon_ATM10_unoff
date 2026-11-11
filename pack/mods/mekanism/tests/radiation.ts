/** Mekanism's radiation, as the `mekanism` native bundle makes it under ATM10's configs (`general.toml`
 * `[radiation]`: on, 5 chunks around a source, sources and doses times 0.9975 on one tick in 20, harm from a severity
 * of 0.1). The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave over RCON
 * (local `mekrad-work/r1.py`, `r1.json`; armor stands stood for the player there, as RCON has none):
 *
 * - `/mek radiation add 100` at a block: 100.0 Sv/h there and a block away, 11.111 Sv/h 3 blocks away, 1.0 at 10,
 *   15.625 mSv/h at 80 (on x or y), 99.999 nSv/h (the baseline) at 81.
 * - The source decays by 0.9975 now and then (19 times in 400 ticks there): 99.75, 99.5, 99.251, 99.003, 98.756,
 *   98.509 ... 95.594, 95.355 Sv/h.
 * - Two blocks from a fresh 100 Sv/h source a dose's first step is 0.006927183111041667 Sv (an hour's share of
 *   25.0000001 Sv/h on the baseline, then the decay); a hazmat mask lets 0.75 of it through; the whole suit none.
 *   A dose of 10.0000001 Sv away from any source decays to 9.97500009975.
 * - A basic chemical tank holding 1000 mB of nuclear waste, broken, leaves 10.0 Sv/h where it stood and drops
 *   empty; a creative one leaves nothing.
 * Here also: the Geiger Counter and the Dosimeter tell the player the level and their dose with the time to decay, a
 * dose of 100 Sv hurts, and the sources and the dose are kept over a restart. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-radiation', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/general.toml')), 'ATM10_DIR with its Mekanism configs');
for (const file of ['general.toml', 'common.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml', 'generators.toml', 'generator-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const B = 1e-7, RATE = 0.9975;
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return { ...lines.reduce((a: any, b: any) => (b.n > a.n ? b : a)), chat: reply.chat ?? [] };
}
const probe = (pos = '') => mek(`mek radiation probe ${pos}`.trim());
/** `RadiationUtil.getDecayTime` and `TextUtils.getHoursMinutes` at 20 ticks a second. */
function decayTime(magnitude: number): string {
  let m = magnitude, ticks = 0;
  while (m > 1e-5) { m *= RATE; ticks += 20; }
  const minutes = Math.ceil(ticks / 1200), hours = Math.floor(minutes / 60);
  return hours > 0 ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
}
const pow = (k: number) => { let m = 100; for (let i = 0; i < k; i++) m *= RATE; return m; };

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 60), z = Math.floor(pz) - 12;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-8, -1, -8)} ${at(24, 6, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-8, -2, -8)} ${at(24, -2, 8)} minecraft:stone`);
  await fixture.command('gamemode creative');
  await fixture.command(`tp @s ${x + 20.5} ${y} ${z + 0.5}`);
  await fixture.hold();
  await mek('mek radiation removeAll');
  await mek('mek radiation heal');

  // Levels by distance.
  const added = await mek(`mek radiation add 100 ${at(0)}`);
  assert(added.chat.some((t: string) => t === `Added 100.0 Sv/h radiation at (${x}, ${y}, ${z}) in Overworld.`), `the add line: ${added.chat}`);
  const want: [string, string][] = [
    [at(0), '100.0 Sv/h'], [at(1), '100.0 Sv/h'], [at(3), '11.111 Sv/h'], [at(10), '1.0 Sv/h'], [at(80), '15.625 mSv/h'],
    [at(81), '99.999 nSv/h'], [at(0, 80), '15.625 mSv/h'], [at(0, 81), '99.999 nSv/h'],
  ];
  for (const [pos, shown] of want) {
    const got = await mek(`mek radiation get ${pos}`);
    assert.equal(got.shown, shown, `the level at ${pos}`);
    assert(got.chat.some((t: string) => t === `Current radiation at (${pos.split(' ').join(', ')}) in Overworld: ${shown}`), `the get line: ${got.chat}`);
  }
  fixture.mark('a 100 Sv/h source reads as on the oracle at 0, 1, 3, 10, 80 and 81 blocks');

  // Its decay over 400 ticks: the oracle's values for as many decays.
  await fixture.stepServer(400);
  const decayed = await probe(at(0));
  const k = decayed.decays;
  log.decay = decayed;
  assert(k >= 5 && k <= 45, `about one decay in 20 ticks: ${k} in 400`);
  assert.equal(decayed.sources[0].radiation, pow(k), `100 times 0.9975 ${k} times`);
  const oracle: Record<number, string> = { 1: '99.75 Sv/h', 2: '99.5 Sv/h', 3: '99.251 Sv/h', 4: '99.003 Sv/h', 5: '98.756 Sv/h', 6: '98.509 Sv/h', 18: '95.594 Sv/h', 19: '95.355 Sv/h' };
  if (oracle[k]) assert.equal((await mek(`mek radiation get ${at(0)}`)).shown, oracle[k], `the oracle's level after ${k} decays`);
  fixture.mark(`the source decays ${k} times in 400 ticks, to 100 * 0.9975^${k} = ${decayed.sources[0].radiation}`);

  // Doses two blocks from a fresh source: bare, in a mask, in the whole suit.
  /** The first dose step: the dose and the source's magnitude before and after its tick (a decay may come first). */
  const firstDose = async (label: string): Promise<[number, number, number]> => {
    await mek('mek radiation heal');
    let prev = (await probe()).sources[0].radiation;
    for (let t = 0; t < 400; t++) {
      // The dose hurts now and then, and a hurt knocks the player a random way (Mekanism's too): back to the spot.
      await fixture.command(spot);
      await fixture.stepServer(1);
      const p = await probe();
      if (p.dose > B) return [p.dose, p.sources[0].radiation, prev];
      prev = p.sources[0].radiation;
    }
    throw new Error(`no dose ${label} in 400 ticks`);
  };
  const either = (dose: number, ms: number[], share: (m: number) => number) => ms.some(m => dose === (B + share(m)) * RATE);
  // The player stands on the floor (y - 1), two blocks east of the source.
  await mek('mek radiation removeAll');
  await mek(`mek radiation add 100 ${at(0, -1)}`);
  await fixture.command('gamemode survival');
  const spot = `tp @s ${x + 2.5} ${y - 1} ${z + 0.5} 0 0`;
  await fixture.command(spot);
  const [bare, m1, p1] = await firstDose('bare');
  assert(either(bare, [m1, p1], m => (m / 4 + B) / 3600), `an hour of the level, then the decay: ${bare} at ${m1}`);
  if (m1 === 100) assert.equal(bare, 0.006927183111041667, "the oracle's first dose");
  await fixture.command('item replace entity @s armor.head with mekanism:hazmat_mask');
  const [masked, m2, p2] = await firstDose('masked');
  assert(either(masked, [m2, p2], m => (m / 4 + B) / 3600 * 0.75), `a mask lets 0.75 through: ${masked} at ${m2}`);
  for (const [slot, piece] of [['armor.chest', 'gown'], ['armor.legs', 'pants'], ['armor.feet', 'boots']]) {
    await fixture.command(`item replace entity @s ${slot} with mekanism:hazmat_${piece}`);
  }
  await mek('mek radiation heal');
  await fixture.stepServer(200);
  const suited = await probe();
  assert(suited.resistance >= 1 && suited.dose === B, `the whole suit lets none through: ${JSON.stringify(suited)}`);
  log.doses = { bare, m1, masked, m2, suited: suited.dose };
  fixture.mark(`two blocks from the source a first dose of ${bare} Sv, ${masked} in a mask, none in the whole suit`);
  for (const slot of ['armor.head', 'armor.chest', 'armor.legs', 'armor.feet']) await fixture.command(`item replace entity @s ${slot} with minecraft:air`);

  // The Geiger Counter and the Dosimeter, used in the air where the player stands.
  // The source may decay while the server runs for the use: its magnitude then, or after a few decays.
  await fixture.command('gamemode creative');
  await fixture.command(spot);
  const m0 = (await probe()).sources[0].radiation;
  const since: number[] = [m0];
  for (let j = 0; j < 8; j++) since.push(since[since.length - 1] * RATE);
  await fixture.hold(false);
  await fixture.replace('hotbar.0', 'mekanism:geiger_counter');
  await fixture.replace('hotbar.1', 'mekanism:dosimeter');
  await fixture.select(0, 'mekanism:geiger_counter');
  await fixture.raw('look 0 -90');
  await fixture.raw('use-item');
  const chatLines = async () => (await fixture.raw('chat-log 20')).chat ?? [];
  const geiger = await until(chatLines, lines => lines.some((t: string) => t.startsWith('Radiation Exposure: ') && t.endsWith('Sv/h')), 'the Geiger Counter reads');
  await fixture.hold();
  const reading = geiger.find((t: string) => t.startsWith('Radiation Exposure: '));
  /** `getDisplayShort(.., SVH, 3)` for 1 to 1000 Sv/h: three decimals cut, Java's `Double.toString`. */
  const shown = (v: number) => { const c = Math.trunc(v * 1000) / 1000; return `${Number.isInteger(c) ? c.toFixed(1) : String(c)} Sv/h`; };
  const stood = await probe();
  log.geiger = { m0, stood, pos: (await fixture.command('data get entity @s Pos')).chat };
  assert(since.some(m => reading === `Radiation Exposure: ${shown(B + m / 4)}`), `the level where the player stands: ${reading} (${m0} Sv/h 2 blocks away): ${JSON.stringify(log.geiger)}`);
  assert(since.some(m => geiger.includes(`Time to Decay: ${decayTime(m)}`)), `the time the source takes to decay: ${geiger}`);
  await mek('mek radiation addEntity 2');
  const given = (await probe()).dose;
  await fixture.hold(false);
  await fixture.select(1, 'mekanism:dosimeter');
  await fixture.raw('use-item');
  const dosimeter = await until(chatLines, lines => lines.some((t: string) => /^Radiation Exposure: [\d.]+ Sv$/.test(t)), 'the Dosimeter reads');
  await fixture.hold();
  const dose = (await probe()).dose;
  log.readings = { geiger, dosimeter, given, dose };
  const doseLine = dosimeter.find((t: string) => /^Radiation Exposure: [\d.]+ Sv$/.test(t));
  assert(['2.0', '1.995'].some(v => doseLine === `Radiation Exposure: ${v} Sv`), `the dose of 2 Sv (or a decay of it): ${doseLine}`);
  assert(dosimeter.some((t: string) => t === `Time to Decay: ${decayTime(given)}` || t === `Time to Decay: ${decayTime(given * RATE)}`), `the time the dose takes to decay: ${dosimeter}`);
  fixture.mark(`the Geiger Counter reads "${reading}", the Dosimeter the player's dose`);

  // A dose of 100 Sv hurts a playing player.
  await mek('mek radiation removeAll');
  await fixture.command(`tp @s ${x + 20.5} ${y} ${z + 0.5}`);
  await fixture.command('gamemode survival');
  await mek('mek radiation addEntity 100');
  const health = async () => Number(((await fixture.command('data get entity @s Health')).chat ?? []).join(' ').match(/([\d.]+)f/)?.[1] ?? NaN);
  const full = await health();
  await fixture.stepServer(25);
  const hurt = await health();
  log.hurt = { full, hurt };
  assert(hurt < full, `100 Sv hurts: ${full} -> ${hurt}`);
  await mek('mek radiation heal');
  await fixture.command('gamemode creative');
  await fixture.command('effect give @s minecraft:instant_health 1 5');
  fixture.mark(`a dose of 100 Sv takes the player from ${full} to ${hurt} health in 25 ticks`);

  // A broken radioactive tank radiates where it stood and drops empty; a creative one doesn't.
  const T = at(12, 0, 4), C = at(16, 0, 4);
  for (const [pos, tank] of [[T, 'basic'], [C, 'creative']]) {
    await fixture.command(`setblock ${pos} mekanism:${tank}_chemical_tank`);
    await fixture.command(`data merge block ${pos} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:nuclear_waste",amount:1000L}}]}`);
  }
  await fixture.hold(false);
  await fixture.command('gamemode survival');
  await fixture.replace('hotbar.2', 'minecraft:diamond_pickaxe');
  await fixture.select(2, 'minecraft:diamond_pickaxe');
  await fixture.command(`tp @s ${x + 14.5} ${y} ${z + 1.5}`);
  await until(() => fixture.raw(`block ${T}`), v => v.block === 'mekanism:basic_chemical_tank', 'the tank reaches the client');
  await fixture.raw(`break ${T}`);
  const drop = await until(async () => (await fixture.command(`execute positioned ${T} run data get entity @e[type=minecraft:item,limit=1,sort=nearest,distance=..3] Item`)).chat?.join(' ') ?? '', v => v.includes('basic_chemical_tank'), 'the tank drops');
  assert(!drop.includes('nuclear_waste'), `the drop keeps no waste: ${drop}`);
  await fixture.command(`setblock ${C} minecraft:air destroy`);
  await fixture.command('kill @e[type=minecraft:item,distance=..32]');
  await fixture.command('gamemode creative');
  await fixture.hold();
  const tankLevel = await mek(`mek radiation get ${T}`);
  const tankSources = (await probe()).sources;
  log.tank = { drop, tankLevel, tankSources };
  // The oracle's 10.0 Sv/h, or as many decays of it as came while the server ran.
  const ten = [10];
  for (let j = 0; j < 8; j++) ten.push(ten[ten.length - 1] * RATE);
  assert(ten.some(v => tankLevel.shown === shown(B + v)), `10.0 Sv/h where the tank stood: ${tankLevel.shown}`);
  assert(tankSources.length === 1 && Math.abs(Math.log(tankSources[0].radiation / 10) / Math.log(RATE) - Math.round(Math.log(tankSources[0].radiation / 10) / Math.log(RATE))) < 1e-6, `1000 mB times 0.01: ${JSON.stringify(tankSources)}`);
  assert.deepEqual(tankSources.map((s: any) => s.pos.join(' ')), [T], 'only the basic tank radiated');
  fixture.mark('a broken tank of 1000 mB of nuclear waste leaves 10.0 Sv/h and drops empty; a creative one leaves nothing');

  // Kept over a restart: the source and the dose (each only decaying meanwhile).
  await mek('mek radiation addEntity 5');
  const before = await probe();
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  const after = await probe();
  log.restart = { before, after };
  const powerOf = (ratio: number) => Math.log(ratio) / Math.log(RATE);
  assert.deepEqual(after.sources.map((s: any) => s.pos), before.sources.map((s: any) => s.pos), 'the source is kept');
  const ks = powerOf(after.sources[0].radiation / before.sources[0].radiation), kd = powerOf(after.dose / before.dose);
  assert(Math.abs(ks - Math.round(ks)) < 1e-6 && ks > -0.5 && ks < 200, `the source only decayed: ${ks}`);
  assert(Math.abs(kd - Math.round(kd)) < 1e-6 && kd > -0.5 && kd < 200, `the dose only decayed: ${kd}`);
  fixture.mark(`after a restart the source (${after.sources[0].radiation} Sv/h) and the dose (${after.dose} Sv) are kept`);

  fixture.report('passed');
  console.log(JSON.stringify(log));
  await fixture.close();
} catch (error) {
  console.error(JSON.stringify(log));
  fixture.report('failed', error);
  await fixture.close().catch(() => undefined);
  throw error;
}
