/** Mekanism's Electrolytic Separator and the chemical registry, as the `mekanism` native bundle and its content mods
 * make them (docs/mekanism-chemicals.md, slice 1), under ATM10's configs. The numbers are those a NeoForge 1.21.1
 * server with Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (`/tick step`, `data get block`):
 *
 * - The registry holds the content mods' chemicals as the oracle saw them: hydrogen a gas fuel of 200 J/mB, carbon an
 *   infuse type, biomass's bar colour its own.
 * - 24 000 mB of water and 160 000 J: 2 mB of water, 2 mB of hydrogen (left), 1 mB of oxygen (right) and 400 J a
 *   tick, from the first tick; 399 J does nothing, 800 J two operations; three speed upgrades 8 a tick.
 * - A left tank with 2399 mB of hydrogen, or holding oxygen, stops it.
 * - Dumping takes 8 mB a tick; dumping excess brings 2400 down to 2040 in a tick; both tanks dumping excess and at
 *   2040 stop it.
 * - Its chemical sides (`TWO_OUTPUT`: left `OUTPUT_1`, right `OUTPUT_2`): facing north, the east face gives hydrogen
 *   and the west face oxygen to another block, neither takes any, the null side gives nothing, the top has none.
 * - A water bucket in its fluid slot empties into the tank; the tanks and dump modes survive a save and restart;
 *   its screen and a dump mode button.
 *
 * The tanks, Joules and modes are read from the saved compound (`data get block`), the handler with `/mek chemical`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-separator', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml'))) {
  for (const file of ['general.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
  writeFileSync(join(config, 'machine-storage.toml'), 'electrolyticSeparator = 160000\n');
}

const B = 'mekanism:electrolytic_separator';

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const energyIn = (nbt: string) => Number(nbt.match(/energy_containers: ?\[\{[^\]]*stored: (\d+)L/)?.[1] ?? 0);
const waterIn = (nbt: string) => Number(nbt.match(/fluid_tanks: ?\[\{[^\]]*amount: (\d+)/)?.[1] ?? 0);
/** Tank `i` of `chemical_tanks`: its chemical and mB. */
function tankIn(nbt: string, i: number): [string, number] {
  const list = nbt.match(/chemical_tanks: ?\[(.*?)\]\s*(,|\})/)?.[1] ?? '';
  for (const entry of list.match(/\{stored: ?\{[^{}]*\}, ?tank: ?\d+b\}|\{tank: ?\d+b, ?stored: ?\{[^{}]*\}\}/g) ?? []) {
    if (new RegExp(`tank: ?${i}b`).test(entry)) return [entry.match(/id: ?"([^"]+)"/)?.[1] ?? '', Number(entry.match(/amount: ?(\d+)L/)?.[1] ?? 0)];
  }
  return ['', 0];
}
const row = (nbt: string) => [waterIn(nbt), tankIn(nbt, 0)[1], tankIn(nbt, 1)[1], energyIn(nbt)];
const water = (mb: number) => `{fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:${mb}}}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const tanks = (left: [string, number] | null, right: [string, number] | null) => {
  const list = [[0, left], [1, right]].filter(([, t]) => t).map(([i, t]: any) => `{tank:${i}b,stored:{id:"${t[0]}",amount:${t[1]}L}}`);
  return `{chemical_tanks:[${list.join(',')}]}`;
};

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(30, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(30, -2, 8)} minecraft:stone`);

  // The registry: the content mods' chemicals, as the oracle saw them.
  const reg = await mek('mek chemicals mekanism:hydrogen');
  log.registry = reg;
  assert.equal(reg.count, 67, 'Mekanism\'s 64 and Generators\' 3');
  assert.equal(reg.chemical.kind, 'gas');
  assert.equal(reg.chemical.fuel.density, 200);
  assert(reg.chemical.gaseous && reg.chemical.tags.includes('mekanism:gaseous'));
  assert.equal((await mek('mek chemicals mekanism:carbon')).chemical.kind, 'infuse_type');
  const bio = (await mek('mek chemicals mekanism:bio')).chemical;
  assert.equal(bio.color & 0xFFFFFF, 0x5A4630, 'biomass\'s bar colour is its own, not its tint');
  assert.equal((await mek('mek chemicals mekanismgenerators:deuterium')).chemical.kind, 'gas');
  fixture.mark('the registry holds the content mods\' 67 chemicals with the oracle\'s facts');

  // A: water into hydrogen and oxygen, 400 J an operation.
  await fixture.hold();
  const a = at(0);
  await fixture.command(`setblock ${a} ${B}`);
  await fixture.command(`data merge block ${a} ${water(24000)}`);
  await fixture.command(`data merge block ${a} ${energy(160000)}`);
  const base: number[][] = [];
  for (let t = 1; t <= 6; t++) {
    await fixture.stepServer(1);
    base.push(row(await saved(a)));
  }
  log.base = base;
  assert.deepEqual(base, [1, 2, 3, 4, 5, 6].map(t => [24000 - 2 * t, 2 * t, t, 160000 - 400 * t]));
  const first = await saved(a);
  assert.deepEqual([tankIn(first, 0)[0], tankIn(first, 1)[0]], ['mekanism:hydrogen', 'mekanism:oxygen']);
  assert.match(first, /dump_left: ?0[,}]/, 'the dump modes saved as ints');
  fixture.mark('2 mB of water make 2 mB of hydrogen and 1 of oxygen for 400 J a tick');

  // B: 399 J nothing; 800 J two operations.
  const b = at(2);
  await fixture.command(`setblock ${b} ${B}`);
  await fixture.command(`data merge block ${b} ${water(24000)}`);
  await fixture.command(`data merge block ${b} ${energy(399)}`);
  await fixture.stepServer(5);
  assert.deepEqual(row(await saved(b)), [24000, 0, 0, 399]);
  await fixture.command(`data merge block ${b} ${energy(800)}`);
  await fixture.stepServer(4);
  assert.deepEqual(row(await saved(b)), [23996, 4, 2, 0]);
  fixture.mark('399 J does nothing, 800 J two operations');

  // C: three speed upgrades, 8 operations a tick.
  const c = at(4);
  await fixture.command(`setblock ${c} ${B}`);
  await fixture.command(`data merge block ${c} {component_upgrade:{upgrades:[{type:0,amount:3}]}}`);
  await fixture.command(`data merge block ${c} ${water(24000)}`);
  await fixture.command(`data merge block ${c} ${energy(160000)}`);
  await fixture.stepServer(1);
  assert.deepEqual(row(await saved(c)), [23984, 16, 8, 156800]);
  fixture.mark('three speed upgrades: 8 operations, 3200 J a tick');

  // D: a full or foreign output.
  const d = at(6);
  await fixture.command(`setblock ${d} ${B}`);
  await fixture.command(`data merge block ${d} ${water(24000)}`);
  await fixture.command(`data merge block ${d} ${energy(160000)}`);
  await fixture.command(`data merge block ${d} ${tanks(['mekanism:hydrogen', 2399], null)}`);
  await fixture.stepServer(3);
  assert.deepEqual(row(await saved(d)), [24000, 2399, 0, 160000], 'no room for 2 mB');
  await fixture.command(`data merge block ${d} ${tanks(['mekanism:oxygen', 10], null)}`);
  await fixture.stepServer(3);
  const foreign = await saved(d);
  assert.deepEqual([tankIn(foreign, 0), waterIn(foreign)], [['mekanism:oxygen', 10], 24000], 'oxygen on the left stops it');
  fixture.mark('a full or foreign output tank stops it');

  // E: dump modes.
  const e = at(8);
  await fixture.command(`setblock ${e} ${B}`);
  await fixture.command(`data merge block ${e} {dump_left:2}`);
  await fixture.command(`data merge block ${e} ${tanks(['mekanism:hydrogen', 100], null)}`);
  const dumped: number[] = [];
  for (let t = 0; t < 3; t++) {
    await fixture.stepServer(1);
    dumped.push(tankIn(await saved(e), 0)[1]);
  }
  assert.deepEqual(dumped, [92, 84, 76]);
  await fixture.command(`data merge block ${e} {dump_left:1}`);
  await fixture.command(`data merge block ${e} ${tanks(['mekanism:hydrogen', 2400], null)}`);
  await fixture.stepServer(1);
  assert.equal(tankIn(await saved(e), 0)[1], 2040, 'excess: down to 85 % in a tick');
  await fixture.command(`data merge block ${e} {dump_left:1,dump_right:1}`);
  await fixture.command(`data merge block ${e} ${tanks(['mekanism:hydrogen', 2040], ['mekanism:oxygen', 2040])}`);
  await fixture.command(`data merge block ${e} ${water(24000)}`);
  await fixture.command(`data merge block ${e} ${energy(160000)}`);
  await fixture.stepServer(3);
  assert.deepEqual(row(await saved(e)), [24000, 2040, 2040, 160000], 'both dumping excess and at 2040: it stops');
  fixture.mark('dumping 8 mB a tick, dumping excess to 2040, both at it stop the machine');

  // F: its chemical sides as another block meets them (facing north: left is east).
  const f = at(10);
  await fixture.command(`setblock ${f} ${B}`);
  await fixture.command(`data merge block ${f} ${tanks(['mekanism:hydrogen', 100], ['mekanism:oxygen', 50])}`);
  await fixture.stepServer(1);
  const east = await mek(`mek chemical ${f} east tanks`);
  const west = await mek(`mek chemical ${f} west tanks`);
  log.sides = { east, west };
  assert.deepEqual(east.tanks.map((t: any) => [t.id, t.amount, t.capacity]), [['mekanism:hydrogen', 100, 2400]]);
  assert.deepEqual(west.tanks.map((t: any) => [t.id, t.amount, t.capacity]), [['mekanism:oxygen', 50, 2400]]);
  assert.equal((await mek(`mek chemical ${f} up tanks`)).handler, false, 'no handler on the top');
  assert.equal((await mek(`mek chemical ${f} none tanks`)).tanks.length, 2, 'the null side reads both');
  assert.equal((await mek(`mek chemical ${f} east insert mekanism:hydrogen 10`)).rest.amount, 10, 'an output takes nothing');
  assert.deepEqual((await mek(`mek chemical ${f} east extract 30`)).got, { id: 'mekanism:hydrogen', amount: 30 });
  assert.deepEqual((await mek(`mek chemical ${f} west extract mekanism:hydrogen 5`)).got, { id: null, amount: 0 }, 'only oxygen there');
  assert.deepEqual((await mek(`mek chemical ${f} west extract 60`)).got, { id: 'mekanism:oxygen', amount: 50 });
  assert.equal((await mek(`mek chemical ${f} none extract 10`)).got.amount, 0, 'the null side is read only');
  const after = await saved(f);
  assert.deepEqual([tankIn(after, 0), tankIn(after, 1)], [['mekanism:hydrogen', 70], ['', 0]]);
  fixture.mark('its left and right tanks give hydrogen east and oxygen west, take nothing, the top has no handler');

  // G: a water bucket in the fluid slot empties into the tank.
  const g = at(12);
  await fixture.command(`setblock ${g} ${B}`);
  await fixture.command(`data merge block ${g} {Items:[{Slot:0b,id:"minecraft:water_bucket",count:1}]}`);
  await fixture.stepServer(1);
  const filled = await saved(g);
  assert.equal(waterIn(filled), 1000);
  assert.match(filled, /Slot: ?0b[^}]*"minecraft:bucket"|"minecraft:bucket"[^}]*Slot: ?0b/, `the empty bucket stays: ${filled}`);
  fixture.mark('a water bucket in the fluid slot fills the tank and leaves the bucket');

  // Save and restart: tanks and modes survive (A's energy taken away first, so it stays put).
  await fixture.command(`data merge block ${a} ${energy(0)}`);
  await fixture.command(`data merge block ${e} {dump_left:2,dump_right:0}`);
  await fixture.command(`data merge block ${e} ${tanks(null, null)}`);
  await fixture.stepServer(1);
  const before = row(await saved(a));
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  const back = await saved(a);
  log.restart = { before, back: row(back) };
  assert.deepEqual(row(back).slice(0, 3), before.slice(0, 3));
  assert.match(await saved(e), /dump_left: ?2[,}]/);
  await fixture.hold(false);
  fixture.mark('the tanks and dump modes survive a save and restart');

  // Its screen: six slots; the left dump mode button.
  await fixture.command(`data merge block ${a} {dump_left:0}`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${a}`), v => v.block === B, 'the separator reaches the client');
  await fixture.raw(`interact ${a} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the separator screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5], 'fluid, left and right outputs, energy and the two upgrade slots');
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const [ox, oy] = [Math.trunc((960 / scale - 176) / 2), Math.trunc((540 / scale - 166) / 2)];
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'separator-main.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw(`click-at ${ox + 12} ${oy + 77}`);
  await until(() => saved(a), s => /dump_left: ?1[,}]/.test(s), 'the left dump mode button: idle to dumping excess');
  await fixture.raw('close');
  fixture.mark('its screen shows six slots; a dump mode button cycles the mode');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK separator', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK separator', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
