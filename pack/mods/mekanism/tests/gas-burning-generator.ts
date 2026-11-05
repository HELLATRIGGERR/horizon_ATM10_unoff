/** Mekanism Generators' Gas-Burning Generator, as the `mekanism` native bundle and its content mods make it
 * (docs/mekanism-chemicals.md, slice 4), under ATM10's configs. The numbers are those a NeoForge 1.21.1 server with
 * Mekanism and Mekanism Generators 10.7.19 and ATM10's configs gave for the same steps over RCON (`/tick step`,
 * `data get block`; chem-work/m4 g1, g2):
 *
 * - 1000 mB of hydrogen (200 J a tick a mB): 15, 15, 14, 14 mB a tick (`ceil(256 * stored / 18 000)`), 3000 J each;
 *   18 000 mB: 256 then 253 mB, 51 200 and 50 600 J; 1500 J of room: 7 mB, 1400 J, then nothing; 200 000 J full.
 * - Out of its front at twice what it made last tick (400 J on its first tick): an energy cube there gets 400,
 *   102 400, 99 000, 49 800 J; one behind it nothing.
 * - Its tank takes hydrogen from another block on its sides, not oxygen, nothing on its front, gives none out.
 * - A basic chemical tank item in its fuel slot gives 1000 mB a tick.
 * - Its tank and Joules survive a save and restart; its screen shows its two slots. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-gas-generator', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'generators.toml', 'generator-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'generators.toml'), '[gas_generator]\ntankCapacity = 18000\n');
}

const G = 'mekanismgenerators:gas_burning_generator';
const H = 'mekanism:hydrogen';
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
/** The generator's energy and fuel (mB), by `/mek probe`. */
async function row(pos: string): Promise<[number, number]> {
  const p = await mek(`mek probe ${pos}`);
  return [p.energy, p.tank?.amount ?? 0];
}
const fuel = (chemical: string, mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:${mb}L}}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const allInput = '{component_config:{config0:[I;1,1,1,1,1,1],eject0:0b}}';

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(30, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(30, -2, 8)} minecraft:stone`);
  await fixture.hold();

  // A: 1000 mB of hydrogen; B: 18 000.
  const a = at(0), b = at(2);
  await fixture.command(`setblock ${a} ${G}[facing=north]`);
  await fixture.command(`data merge block ${a} ${fuel(H, 1000)}`);
  await fixture.command(`setblock ${b} ${G}[facing=north]`);
  await fixture.command(`data merge block ${b} ${fuel(H, 18000)}`);
  const rowsA: [number, number][] = [], rowsB: [number, number][] = [];
  for (let t = 1; t <= 4; t++) {
    await fixture.stepServer(1);
    rowsA.push(await row(a));
    if (t <= 2) rowsB.push(await row(b));
  }
  log.burn = { rowsA, rowsB };
  assert.deepEqual(rowsA, [[3000, 985], [6000, 970], [8800, 956], [11600, 942]]);
  assert.deepEqual(rowsB, [[51200, 17744], [101800, 17491]]);
  const probe = await mek(`mek probe ${b}`);
  log.probe = probe;
  assert.equal(probe.capacity, 200000, '1000 times hydrogen\'s 200 J/mB');
  // Two more ticks (249 and 246 mB) fill it: the last 48 400 J of room take 242 mB.
  assert.deepEqual([probe.energy, probe.tank.amount, probe.burn_rate, probe.max_output], [200000, 17000, 242, 96800], 'twice what it made last tick');
  fixture.mark('hydrogen burns by how full the tank is: 15 mB of 1000, 256 of 18 000, 200 J a mB');

  // C: 1500 J of room: 7 mB, then nothing; D: full.
  const c = at(4), d = at(6);
  await fixture.command(`setblock ${c} ${G}[facing=north]`);
  await fixture.command(`data merge block ${c} ${fuel(H, 1000)}`);
  await fixture.command(`data merge block ${c} ${energy(198500)}`);
  await fixture.command(`setblock ${d} ${G}[facing=north]`);
  await fixture.command(`data merge block ${d} ${fuel(H, 1000)}`);
  await fixture.command(`data merge block ${d} ${energy(200000)}`);
  await fixture.stepServer(3);
  assert.deepEqual([await row(c), await row(d)], [[199900, 993], [200000, 1000]]);
  fixture.mark('it burns only what its store has room for');

  // E: out of its front (facing east) into a cube; a cube behind another gets nothing.
  const e = at(8), cube = at(9), f = at(13), behind = at(12);
  await fixture.command(`setblock ${e} ${G}[facing=east]`);
  await fixture.command(`data merge block ${e} ${fuel(H, 18000)}`);
  await fixture.command(`data merge block ${e} ${energy(100000)}`);
  await fixture.command(`setblock ${cube} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${cube} ${allInput}`);
  await fixture.command(`setblock ${f} ${G}[facing=east]`);
  await fixture.command(`data merge block ${f} ${fuel(H, 1000)}`);
  await fixture.command(`data merge block ${f} ${energy(100000)}`);
  await fixture.command(`setblock ${behind} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${behind} ${allInput}`);
  const out: number[][] = [];
  for (let t = 1; t <= 4; t++) {
    await fixture.stepServer(1);
    out.push([(await row(e))[0], energyIn(await saved(cube))]);
  }
  log.output = out;
  assert.deepEqual(out, [[150800, 400], [99000, 102800], [49800, 201800], [49200, 251600]]);
  assert.equal(energyIn(await saved(behind)), 0, 'nothing out of its back');
  fixture.mark('out of its front at twice what it made last tick, 400 J on its first');

  // F: its tank as another block meets it (facing north).
  const g = at(16);
  await fixture.command(`setblock ${g} ${G}[facing=north]`);
  await fixture.stepServer(1);
  assert.equal((await mek(`mek chemical ${g} north tanks`)).handler, false, 'no handler on its front');
  assert.equal((await mek(`mek chemical ${g} west insert ${H} 100`)).rest.amount, 0, 'a side takes hydrogen');
  assert.equal((await mek(`mek chemical ${g} south insert mekanism:oxygen 100`)).rest.amount, 100, 'no oxygen: it has no fuel');
  assert.equal((await mek(`mek chemical ${g} up extract 50`)).got.amount, 0, 'nothing out to another block');
  assert.deepEqual((await mek(`mek chemical ${g} none tanks`)).tanks.map((t: any) => [t.id, t.amount, t.capacity]), [[H, 100, 18000]]);
  // A basic chemical tank west of it, its front east, ejects 1000 mB a tick into it.
  const src = at(15, 0, 2), sink = at(16, 0, 2);
  await fixture.command(`setblock ${sink} ${G}[facing=north]`);
  await fixture.command(`setblock ${src} mekanism:basic_chemical_tank[facing=east]`);
  await fixture.command(`data merge block ${src} {chemical_tanks:[{tank:0b,stored:{id:"${H}",amount:5000L}}]}`);
  const fed: number[] = [];
  for (let t = 1; t <= 2; t++) {
    await fixture.stepServer(1);
    fed.push((await row(sink))[1]);
  }
  log.fed = fed;
  // The oracle ticked the generator first ([1000, 1985]); the other order burns each 1000 at once.
  assert(JSON.stringify(fed) === '[1000,1985]' || JSON.stringify(fed) === '[985,1956]', `a tank on its side fills it 1000 a tick: ${fed}`);
  fixture.mark('its sides take fuel from other blocks and give none; its front has no handler');

  // G: a basic chemical tank item of 5000 mB in its fuel slot: 1000 a tick.
  const h = at(19);
  await fixture.command(`setblock ${h} ${G}[facing=north]`);
  await fixture.command(`data merge block ${h} {Items:[{Slot:0b,id:"mekanism:basic_chemical_tank",count:1,components:{"minecraft:custom_data":{mekanism:${fuel(H, 5000)}}}}]}`);
  const slotRows: number[][] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    const [j, mb] = await row(h);
    const item = (await saved(h)).match(/Items: ?\[.*?amount: ?(\d+)L/)?.[1];
    slotRows.push([mb, j, Number(item ?? 0)]);
  }
  log.slot = slotRows;
  assert.deepEqual(slotRows, [[985, 3000, 4000], [1956, 8800, 3000], [2913, 17400, 2000]]);
  fixture.mark('a chemical tank item in its fuel slot gives 1000 mB a tick');

  // Save and restart: its tank and Joules stay.
  const before = await row(b);
  await fixture.command(`data merge block ${b} ${energy(200000)}`);
  await fixture.stepServer(1);
  const kept = await row(b);
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  const back = await row(b);
  log.restart = { before, kept, back };
  assert.deepEqual(back, kept);
  await fixture.hold(false);
  fixture.mark('its tank and Joules survive a save and restart');

  // Its screen: the fuel and energy slots.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${a}`), v => v.block === G, 'the generator reaches the client');
  await fixture.raw(`interact ${a} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the generator screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1], 'the fuel and energy slots');
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'gas-generator.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('close');
  fixture.mark('its screen shows its fuel and energy slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK gas generator', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK gas generator', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
