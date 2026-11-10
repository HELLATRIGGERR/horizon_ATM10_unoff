/** Mekanism's Thermal Evaporation Plant, as the `mekanism` native bundle makes it under ATM10's configs. The numbers
 * are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave for the same builds over RCON
 * (local `mekmb2-work/e1.py` on `mekm2-work/oracle`; the manager's saved data `mekanism_evaporation` the contents):
 *
 * - Forms: a 4x4 box of thermal evaporation blocks 3 to 18 high, open at the top, with a controller in a side; the
 *   top corners may go, and glass, a valve or the controller may sit on the top ring. 2 or 19 high, a second
 *   controller, a controller on an edge, dirt inside, a block in the top's middle or touching the ring from above
 *   don't form. Formed, the controller is active.
 * - Its tanks: `volume / 4 * 64 000` mB in (768 000 for 3 high, 4 608 000 for 18), 10 000 out; its heat capacity
 *   `600 * height` (1800, 10 800).
 * - Heat: in a desert (biome 2.0) it sits at 330 K, in plains at 300 K (300.0000003 for 0.8f).
 * - Rates: fed by an active creative tank above a valve on its top ring, at 330 K 3 high makes 9 mB of lithium a
 *   tick from 90 of brine (`(330 - 300) * 1.85 * 3 / 18` = 9.25 operations of 10 to 1), 18 high 55; water to brine
 *   the same 9; in plains none. The first lithium came a tick after the brine.
 * - The output slot: an empty bucket in slot 2 comes out of slot 3 full of lithium, 1000 mB less in the tank. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-thermal-evaporation', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const BLOCK = 'mekanism:thermal_evaporation_block', VALVE = 'mekanism:thermal_evaporation_valve';
const CONTROLLER = 'mekanism:thermal_evaporation_controller', GLASS = 'mekanism:structural_glass';
const feedOf = (fluid: string) => `{active_state:1b,fluid_tanks:[{tank:0b,stored:{id:"${fluid}",amount:2147483647}}]}`;
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const plantOf = async (pos: string) => (await mek(`mek probe ${pos}`)).multiblocks?.[0];

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  // Low enough that the tallest plant's top stays under y 80, where biomes start to cool.
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 58), z = Math.floor(pz) - 10;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (let dx = -3; dx <= 60; dx += 16) {
    const x1 = Math.min(dx + 15, 60);
    await fixture.command(`fill ${at(dx, -1, -3)} ${at(x1, 20, 14)} minecraft:air`);
    await fixture.command(`fillbiome ${at(dx, -1, -3)} ${at(x1, 20, 14)} ${x1 <= 44 ? 'minecraft:desert' : 'minecraft:plains'}`);
  }
  await fixture.hold();
  /** A 4x4 plant `h` high at (dx, 0, dz): a box of blocks with the inside air and its top open, the controller in
   * the north side at (1, 1, 0) unless `controller` says where (null: none), then `edits` [dx, dy, dz, block]. */
  const plant = async (dx: number, dz: number, h: number, edits: [number, number, number, string][] = [], controller: number[] | null = [1, 1, 0]) => {
    await fixture.command(`fill ${at(dx, 0, dz)} ${at(dx + 3, h - 1, dz + 3)} ${BLOCK}`);
    await fixture.command(`fill ${at(dx + 1, 1, dz + 1)} ${at(dx + 2, h - 1, dz + 2)} minecraft:air`);
    if (controller) await fixture.command(`setblock ${at(dx + controller[0], controller[1], dz + controller[2])} ${CONTROLLER}`);
    for (const [ex, ey, ez, b] of edits) await fixture.command(`setblock ${at(dx + ex, ey, dz + ez)} ${b}`);
    return controller ? at(dx + controller[0], controller[1], dz + controller[2]) : at(dx, 0, dz);
  };

  // Forms at 3 and 18 high: its tanks and heat by height, the controller lit.
  const low = await plant(0, 0, 3), high = await plant(6, 0, 18);
  await fixture.stepServer(2);
  const a = await plantOf(low), b = await plantOf(high);
  log.low = a;
  assert(a && b, `formed: ${JSON.stringify(await mek(`mek probe ${low}`))}`);
  assert.deepEqual(a.size, [4, 3, 4]);
  assert.deepEqual(b.size, [4, 18, 4]);
  assert.equal(a.input_capacity, 768_000, '48 / 4 * 64 000');
  assert.equal(b.input_capacity, 4_608_000);
  assert.equal(a.output_capacity, 10_000);
  assert.equal(a.heat_capacity, 1800);
  assert.equal(b.heat_capacity, 10_800);
  assert.equal(a.ambient, 330, 'a desert: 300 + 25 * (2.0 - 0.8)');
  assert.equal((await mek(`mek probe ${low}`)).active, 'true', 'the controller is active');
  fixture.mark('4x4x3 and 4x4x18 form: 768 000 and 4 608 000 mB in, 10 000 out, 1800 and 10 800 of heat capacity, 330 K air');

  // What doesn't form, and what does.
  const cases: [string, string, boolean][] = [
    ['19 high', await plant(12, 0, 19), false],
    ['2 high', await plant(18, 0, 2), false],
    ['two controllers', await plant(24, 0, 3, [[2, 1, 0, CONTROLLER]]), false],
    ['a controller on an edge', await plant(30, 0, 4, [], [0, 1, 0]), false],
    ['dirt inside', await plant(36, 0, 4, [[1, 1, 1, 'minecraft:dirt']]), false],
    ['a block in the top\'s middle', await plant(42, 8, 4, [[1, 3, 1, BLOCK]]), false],
    ['no top corners', await plant(0, 8, 4, [[0, 3, 0, 'minecraft:air'], [3, 3, 0, 'minecraft:air'], [0, 3, 3, 'minecraft:air'], [3, 3, 3, 'minecraft:air']]), true],
    ['glass and a valve on the ring, glass in a side', await plant(6, 8, 4, [[1, 3, 0, GLASS], [2, 3, 0, VALVE], [2, 2, 0, GLASS]]), true],
    ['the controller on the ring', await plant(36, 8, 4, [], [1, 3, 0]), true],
  ];
  await fixture.stepServer(2);
  const results: Record<string, boolean> = {};
  for (const [name, pos] of cases) results[name] = !!(await plantOf(pos));
  log.formation = results;
  for (const [name, , forms] of cases) assert.equal(results[name], forms, name);
  fixture.mark(`refused: ${cases.filter(c => !c[2]).map(c => c[0]).join(', ')}; formed: ${cases.filter(c => c[2]).map(c => c[0]).join(', ')}`);

  // Rates: brine and water at 330 K, 3 and 18 high; brine in plains.
  const water3 = await plant(18, 8, 3), plains3 = await plant(48, 8, 3);
  await fixture.stepServer(2);
  const feeds: [string, number, number, string][] = [[low, 0, 0, 'mekanism:brine'], [high, 6, 0, 'mekanism:brine'], [water3, 18, 8, 'minecraft:water'], [plains3, 48, 8, 'mekanism:brine']];
  // Each fed through a valve on its top ring's south edge by an active creative tank above it.
  for (const [, dx, dz, fluid] of feeds) {
    const h = dx === 6 ? 18 : 3;
    await fixture.command(`setblock ${at(dx + 1, h - 1, dz + 3)} ${VALVE}`);
    await fixture.command(`setblock ${at(dx + 1, h, dz + 3)} mekanism:creative_fluid_tank[active=true]`);
    await fixture.command(`data merge block ${at(dx + 1, h, dz + 3)} ${feedOf(fluid)}`);
  }
  await fixture.stepServer(3);
  const first = await Promise.all(feeds.map(f => plantOf(f[0])));
  await fixture.stepServer(10);
  const later = await Promise.all(feeds.map(f => plantOf(f[0])));
  const rate = (i: number) => (later[i].output.amount - first[i].output.amount) / 10;
  log.rates = feeds.map((f, i) => ({ fluid: f[3], first: first[i].output, later: later[i].output, input: later[i].input, temperature: later[i].temperature, production: later[i].production }));
  assert.equal(later[0].input.amount, 768_000, 'the input kept full');
  assert.equal(later[0].output.fluid, 'mekanism:lithium');
  assert(first[0].output.amount > 0, `lithium within 3 ticks: ${JSON.stringify(first[0])}`);
  assert.equal(rate(0), 9, '3 high: 9 mB of lithium a tick');
  assert.equal(rate(1), 55, '18 high: 55 a tick');
  assert.equal(later[2].output.fluid, 'mekanism:brine');
  assert.equal(rate(2), 9, 'water to brine: 9 a tick');
  assert.equal(later[3].output.amount, 0, 'plains: none');
  assert.equal(later[3].temperature, later[3].ambient, 'plains: at its air');
  assert.equal(later[0].temperature, 330);
  assert.equal(later[0].production, 9.25, 'the screen\'s production');
  fixture.mark(`at 330 K: 9 mB of lithium a tick 3 high, 55 18 high, 9 of brine from water; at 300 K none`);

  // The output slot: an empty bucket in slot 2 comes out of slot 3 full of lithium.
  await fixture.stepServer(10);
  assert((await plantOf(high)).output.amount >= 1000, 'a bucket of lithium');
  await fixture.command(`data merge block ${high} {Items:[{Slot:2b,id:"minecraft:bucket",count:1}]}`);
  const before = (await plantOf(high)).output.amount;
  await fixture.stepServer(2);
  const slots = await plantOf(high);
  log.slots = slots.slots;
  assert.equal(slots.slots[2], null);
  assert.equal(slots.slots[3]?.item, 'mekanism:lithium_bucket');
  assert.equal(before + 2 * 55 - slots.output.amount, 1000, '1000 mB into the bucket');
  fixture.mark('an empty bucket in the output side\'s slot fills with lithium and goes below');

  // The Configurator on the plant with two controllers says why; the controller's screen shows its four slots.
  await fixture.hold(false);
  await fixture.replace('hotbar.0', 'mekanism:configurator');
  await fixture.select(0, 'mekanism:configurator');
  await fixture.command(`tp @s ${x + 25.5} ${y + 1} ${z - 2.5}`);
  await fixture.raw(`interact ${at(25, 1, 0)} north`);
  const said = await until(async () => ((await fixture.raw('chat-log 20')).chat ?? []).join('\n'), text => /Controller conflict: found extra controller at \(/.test(text), 'the reason');
  log.configurator = said.split('\n').find((l: string) => l.includes('Controller conflict'));
  assert(String(log.configurator).includes(`(${x + 26}, ${y + 1}, ${z})`), String(log.configurator));
  await fixture.command(`tp @s ${x + 7.5} ${y + 1} ${z - 2.5}`);
  await fixture.raw(`interact ${high} north`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the controller screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3], `its four slots: ${JSON.stringify(view.slots)}`);
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'evaporation-screen.png')}`);
  await fixture.raw('close-screen').catch(() => undefined);
  await fixture.hold();
  fixture.mark(`the Configurator says "${log.configurator}"; the controller's screen shows its four slots`);

  // Broken, the cache keeps it; a restart keeps it too.
  const kept = (await plantOf(low)).output.amount;
  await fixture.command(`setblock ${at(3, 0, 0)} minecraft:air`);
  await fixture.stepServer(1);
  assert.equal(await plantOf(low), undefined, 'a corner gone: not formed');
  assert.equal((await mek(`mek probe ${low}`)).active, 'false', 'the controller goes dark');
  await fixture.command(`setblock ${at(3, 0, 0)} ${BLOCK}`);
  await fixture.stepServer(1);
  const back = await plantOf(low);
  assert(back.output.amount >= kept, `kept ${kept}: ${JSON.stringify(back.output)}`);
  await fixture.hold(false);
  await fixture.restart();
  const after = await until(() => plantOf(high), p => !!p, 'formed again after the restart');
  assert(after.output.amount > 0 && after.heat_capacity === 10_800, JSON.stringify(after));
  fixture.mark(`broken and put back it keeps its ${kept} mB; after a restart ${after.output.amount} mB of lithium`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK thermal-evaporation', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK thermal-evaporation', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
