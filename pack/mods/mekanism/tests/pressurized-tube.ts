/** Mekanism's Pressurized Tubes, as the `mekanism` native bundle and its content mods make them
 * (docs/mekanism-chemicals.md, slice 3), under ATM10's configs. The numbers are those a NeoForge 1.21.1 server with
 * Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (`/tick step`, `data get block`):
 *
 * - The Electrolytic Separator (facing north: hydrogen out of its left, east) into three basic tubes and a basic tank:
 *   the tank has 4, 6, 8, 10 mB three to six ticks after they are placed, the separator keeps 2 mB.
 * - Throughput: an ultimate tank into tubes and an ultimate tank, the tank receiving each tick what the tubes hold:
 *   one basic tube 4000, three 12 000, advanced 16 000, elite 256 000; an ultimate tube passes all of the tank's
 *   512 000.
 * - Sides: a pull side draws 750 a tick (basic), 2000 (advanced) from a tank that isn't ejecting; a push side takes
 *   nothing from a tank ejecting into it; a tank whose touching side is none doesn't connect.
 * - A basic tank into three basic tubes with nothing at the end: after six ticks they save 1667, 1667 and 1666 mB;
 *   full, 12 000, which a save and restart keep; that hydrogen network takes no oxygen from a tank beside it.
 *
 * Tanks are read from the saved compound (`data get block`), tubes with `/mek probe`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-tube', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
}

const H = 'mekanism:hydrogen';
const tank = (tier: string) => `mekanism:${tier}_chemical_tank`;
const tube = (tier: string) => `mekanism:${tier}_pressurized_tube`;
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
function tankIn(nbt: string, i = 0): [string, number] {
  const list = nbt.match(/chemical_tanks: ?\[(.*?)\]\s*(,|\})/)?.[1] ?? '';
  for (const entry of list.match(/\{stored: ?\{[^{}]*\}, ?tank: ?\d+b\}|\{tank: ?\d+b, ?stored: ?\{[^{}]*\}\}/g) ?? []) {
    if (new RegExp(`tank: ?${i}b`).test(entry)) return [entry.match(/id: ?"([^"]+)"/)?.[1] ?? '', Number(entry.match(/amount: ?(\d+)L/)?.[1] ?? 0)];
  }
  return ['', 0];
}
const amount = async (pos: string) => (await mek(`mek probe ${pos}`)).amount ?? 0;
const held = (chemical: string, mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:${mb}L}}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -1, -2)} ${at(12, 4, 26)} minecraft:air`);
  await fixture.hold();

  // The separator into three basic tubes and a basic tank (facing north: its west side takes).
  const sep = at(0, 0, 0), end = at(4, 0, 0);
  await fixture.command(`setblock ${sep} mekanism:electrolytic_separator`);
  await fixture.command(`data merge block ${sep} {fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:24000}}],energy_containers:[{container:0b,stored:160000L}]}`);
  for (const dx of [1, 2, 3]) await fixture.command(`setblock ${at(dx)} ${tube('basic')}`);
  await fixture.command(`setblock ${end} ${tank('basic')}`);
  await fixture.stepServer(2);
  const line: number[][] = [];
  for (let t = 0; t < 4; t++) {
    await fixture.stepServer(1);
    line.push([tankIn(await saved(sep), 0)[1], await amount(end)]);
  }
  log.separator = line;
  assert.deepEqual(line, [[2, 4], [2, 6], [2, 8], [2, 10]]);
  const p = await mek(`mek probe ${at(2)}`);
  log.line_tube = p;
  assert.equal(p.network.tubes, 3);
  assert.deepEqual([p.faces.west, p.faces.east, p.faces.up], ['normal', 'normal', 'none']);
  fixture.mark('the separator\'s hydrogen reaches a tank through three tubes at 2 mB a tick, as Mekanism\'s');

  // Throughput: an ultimate tank (front east) into tubes and an ultimate tank.
  const rates: Record<string, number[]> = {};
  let row = 2;
  for (const [tier, n] of [['basic', 1], ['basic', 3], ['advanced', 1], ['elite', 1], ['ultimate', 1]] as const) {
    const a = at(0, 0, row), b = at(n + 1, 0, row);
    await fixture.command(`setblock ${a} ${tank('ultimate')}[facing=east]`);
    await fixture.command(`data merge block ${a} ${held(H, 1000000)}`);
    for (let i = 1; i <= n; i++) await fixture.command(`setblock ${at(i, 0, row)} ${tube(tier)}`);
    await fixture.command(`setblock ${b} ${tank('ultimate')}`);
    await fixture.stepServer(2);
    const seen: number[] = [];
    for (let t = 0; t < 2; t++) { await fixture.stepServer(1); seen.push(await amount(b)); }
    rates[`${tier}x${n}`] = seen;
    row += 2;
  }
  log.rates = rates;
  assert.deepEqual(rates, { basicx1: [8000, 12000], basicx3: [24000, 36000], advancedx1: [32000, 48000], elitex1: [512000, 768000], ultimatex1: [1000000, 1000000] });
  fixture.mark('a tube network passes what it holds a tick: 4000 basic, 12 000 for three, 16 000, 256 000, 512 000+');

  // Pull sides: from a tank that isn't ejecting.
  const pulls: Record<string, number[]> = {};
  for (const tier of ['basic', 'advanced']) {
    const a = at(0, 0, row), b = at(2, 0, row);
    await fixture.command(`setblock ${a} ${tank('ultimate')}[facing=east]`);
    await fixture.command(`data merge block ${a} {component_config:{eject2:0b}}`);
    await fixture.command(`data merge block ${a} ${held(H, 1000000)}`);
    await fixture.command(`setblock ${at(1, 0, row)} ${tube(tier)}`);
    await fixture.command(`setblock ${b} ${tank('ultimate')}`);
    await mek(`mek connection ${at(1, 0, row)} west pull`);
    await fixture.stepServer(2);
    const seen: number[] = [];
    for (let t = 0; t < 2; t++) { await fixture.stepServer(1); seen.push(await amount(b)); }
    pulls[tier] = seen;
    row += 2;
  }
  log.pulls = pulls;
  assert.deepEqual(pulls, { basic: [1500, 2250], advanced: [4000, 6000] });
  // A push side takes nothing from a tank ejecting into it.
  const pa = at(0, 0, row), pb = at(2, 0, row);
  await fixture.command(`setblock ${pa} ${tank('ultimate')}[facing=east]`);
  await fixture.command(`data merge block ${pa} ${held(H, 1000000)}`);
  await fixture.command(`setblock ${at(1, 0, row)} ${tube('basic')}`);
  await fixture.command(`setblock ${pb} ${tank('ultimate')}`);
  await mek(`mek connection ${at(1, 0, row)} west push`);
  await fixture.stepServer(3);
  assert.deepEqual([await amount(pa), await amount(pb)], [1000000, 0], 'a push side takes nothing in');
  row += 2;
  // A tank whose touching side is none doesn't connect.
  const na = at(0, 0, row), nb = at(2, 0, row);
  await fixture.command(`setblock ${na} ${tank('ultimate')}[facing=east]`);
  await fixture.command(`data merge block ${na} {component_config:{config2:[I;0,1,1,1,1,1]}}`);
  await fixture.command(`data merge block ${na} ${held(H, 1000000)}`);
  await fixture.command(`setblock ${at(1, 0, row)} ${tube('basic')}`);
  await fixture.command(`setblock ${nb} ${tank('ultimate')}`);
  await fixture.stepServer(3);
  const cut = await mek(`mek probe ${at(1, 0, row)}`);
  log.cut = cut;
  assert.deepEqual([await amount(na), await amount(nb), cut.faces.west, cut.faces.east], [1000000, 0, 'none', 'normal']);
  row += 2;
  fixture.mark('a pull side draws 750 a tick (2000 advanced); a push side takes nothing; a none side doesn\'t connect');

  // Saved shares of a line with nothing at its end, a restart, and one chemical a network.
  const sa = at(0, 0, row);
  await fixture.command(`setblock ${sa} ${tank('basic')}[facing=east]`);
  await fixture.command(`data merge block ${sa} ${held(H, 30000)}`);
  for (const dx of [1, 2, 3]) await fixture.command(`setblock ${at(dx, 0, row)} ${tube('basic')}`);
  await fixture.stepServer(6);
  const shares: number[] = [];
  for (const dx of [1, 2, 3]) shares.push((await mek(`mek probe ${at(dx, 0, row)}`)).saved_share);
  const net = (await mek(`mek probe ${at(1, 0, row)}`)).network;
  log.shares = { shares, net, tank: await amount(sa) };
  assert.deepEqual(shares, [1667, 1667, 1666]);
  assert.deepEqual([net.buffer, net.chemical, await amount(sa)], [5000, H, 25000]);
  await fixture.stepServer(11);
  assert.equal(await amount(sa), 18000, 'full at 12 000');
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  await fixture.stepServer(1);
  const back = (await mek(`mek probe ${at(2, 0, row)}`)).network;
  log.restart = back;
  assert.deepEqual([back.buffer, back.chemical, back.tubes], [12000, H, 3]);
  const ox = at(2, 0, row + 1);
  await fixture.command(`setblock ${ox} ${tank('basic')}`);
  await fixture.command(`data merge block ${ox} ${held('mekanism:oxygen', 5000)}`);
  await fixture.stepServer(3);
  assert.equal(await amount(ox), 5000, 'the hydrogen network takes no oxygen');
  await fixture.hold(false);
  fixture.mark('three tubes save 1667, 1667, 1666; a full line keeps its 12 000 over a restart and takes no oxygen');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK tube', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK tube', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
