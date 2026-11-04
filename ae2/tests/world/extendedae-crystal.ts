/** ExtendedAE slice 2: the Crystal Assembler makes an Extended Drive from its Crystal Assembler recipe for
 * 2,000 AE of the grid (ten AE a progress step, 200 steps), with a fluid from its tank (fluix: four each of
 * charged certus, redstone and quartz and 100 mB of water) and four speed cards; it pushes its output out with
 * AUTO_EXPORT; the Circuit Slicer cuts a silicon block into nine printed silicon; the Crystal Fixer turns
 * the chipped budding quartz in front of it into a flawed one, burning its fuel; the Extended IO Port empties
 * a cell 2,048 items a tick (AE2's moves 256); everything through a restart, and broken the assembler drops
 * what it holds. Needs content/extendedae (ae2/scripts/prepare-addon.ts extendedae) and a client built with
 * ae2-extendedae. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-crystal', mode, { addons: ['extendedae'] });
const shots = process.env.AE2_TEST_SHOTS;
const shot = async (name: string) => { if (shots) await fixture.raw(`screenshot ${join(shots, name + '.png')}`); };
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slot = (view: any, n: number) => view.slots?.find((s: any) => s.slot === n);
const cell = (id: string, n: number) => `ae2:item_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"${id}","#":${n}L}]]`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -5], [-3, 7], [17, -5], [17, 7]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -5)} ${at(17, 5, 7)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -5)} ${at(17, -1, 7)} minecraft:stone`);
  await fixture.command('clear @s');
  // A grid of an energy cell, the assembler and the slicer; the fixer's own; the IO port's with a drive.
  const power = at(0), assembler = at(1), cutter = at(2), chest = at(1, 0, -1);
  const fixerPower = at(5), fixer = at(6), budding = at(7);
  const portPower = at(10), port = at(11), drive = at(12);
  await fixture.command(`setblock ${power} ae2:energy_cell`);
  await fixture.command(`setblock ${assembler} extendedae:crystal_assembler`);
  await fixture.command(`setblock ${cutter} extendedae:circuit_cutter`);
  await fixture.command(`data merge block ${power} {ae2_lab_energy:100000.0d}`);
  const online = (pos: string) => until(() => fixture.ae(`grid status ${pos}`), view => view.online && view.powered, `online ${pos}`, 30000);
  await online(assembler);
  const gridEnergy = async () => (await fixture.ae(`grid status ${power}`)).energy;
  await until(gridEnergy, e => e === 100000, 'the cell holds 100,000 AE');

  // Its own buffer of 8,000 AE takes Forge Energy on every side but the top (facing north, spin 0).
  const east = await fixture.ae(`power ${assembler} east`), up = await fixture.ae(`power ${assembler} up`);
  assert.equal(east.fe?.capacity, 16000, '8,000 AE at 2 FE an AE (AE2 default)');
  assert.equal(up.fe, null, 'no power on its top');
  assert.equal((await fixture.ae(`power ${cutter} north`)).fe, null, 'none on the slicer front');
  assert.equal((await fixture.ae(`power ${cutter} up`)).fe?.capacity, 16000);
  fixture.mark('the Crystal Assembler and Circuit Slicer take power on the sides the mod connects');

  // The Extended Drive's recipe: a drive, two glass cables, a capacity card and a concurrent processor.
  const inputs: [string, number][] = [['ae2:drive', 1], ['ae2:fluix_glass_cable', 3], ['ae2:capacity_card', 1], ['extendedae:concurrent_processor', 1]];
  for (const [i, [id, n]] of inputs.entries()) await fixture.command(`item replace block ${assembler} container.${i} with ${id} ${n}`);
  const contents = async (pos: string) => (await fixture.command(`data get block ${pos}`)).chat?.join('\n') ?? '';
  await until(() => contents(assembler), text => text.includes('extendedae:ex_drive'), 'an Extended Drive made', 60000);
  const made = await contents(assembler);
  // The `Items` entry of slot `n` in a `data get` text.
  const entry = (text: string, n: number) => text.match(new RegExp(`\\{[^{}]*Slot: ${n}b[^{}]*\\}`))?.[0] ?? '';
  assert.match(entry(made, 9), /id: "extendedae:ex_drive"/, 'in the output slot');
  assert.match(entry(made, 1), /count: 1,.*fluix_glass_cable/, 'one cable of three left');
  assert.doesNotMatch(made, /ae2:capacity_card|concurrent_processor|"ae2:drive"/, 'the rest used up');
  assert.equal(await gridEnergy(), 98000, '2,000 AE: 100 steps of 2 progress at 10 AE each');
  fixture.mark('the Crystal Assembler crafts an Extended Drive from its recipe for 2,000 AE');

  // The screen: its style, the auto-export toggle, four card slots; the tank, filled from buckets.
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'minecraft:water_bucket'); await fixture.replace('hotbar.1', 'ae2:speed_card', 4);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${assembler} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Crystal Assembler screen', 15000);
  assert.equal(view.ae2_screen.style, 'crystal_assembler.json');
  assert(fixture.settingIs(view, 24, 'NO'), 'auto export off');
  assert.equal(slot(view, 309)?.item, 'extendedae:ex_drive');
  await shot('crystal_assembler');
  for (let i = 0; i < 2; i++) {
    await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:water_bucket', 'a water bucket held');
    await fixture.raw('click-slot 320');
    await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:bucket', 'the bucket emptied into the tank');
    await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !s.carried, 'bucket back');
    await fixture.replace('hotbar.0', 'minecraft:water_bucket');
  }
  view = await until(() => fixture.raw('screen'), v => JSON.stringify(slot(v, 320) ?? '').includes('2000 / 16000 mB'), 'two buckets in the tank');
  // Speed cards one to a slot, four at most.
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:speed_card', 'cards held');
  for (const entry of [310, 311, 312, 313]) await fixture.raw(`click-slot ${entry}`);
  await until(() => fixture.raw('screen'), v => [310, 311, 312, 313].every(n => slot(v, n)?.item === 'ae2:speed_card') && !v.carried, 'four speed cards in');
  // The output comes out by shift-click; nothing goes in its slot.
  await fixture.raw('click-slot 309 shift');
  await until(() => fixture.raw('inventory'), s => s.counts?.['extendedae:ex_drive'] === 1, 'the Extended Drive taken');
  await fixture.raw('close');
  fixture.mark('its screen fills the tank from buckets and takes four speed cards');

  // Fluix: four each of charged certus, redstone and quartz and 100 mB of water make eight.
  // Its fluid is NeoForge's FluidIngredient `{"fluid": "minecraft:water"}`: two buckets of lava in the tank make nothing.
  await fixture.command(`data merge block ${assembler} {ae2_lab_config:{tank_in:{id:"minecraft:lava",amount:2000L}}}`);
  assert.match(await contents(assembler), /tank_in: \{[^}]*minecraft:lava/, 'lava in the tank');
  for (const [i, id] of ['ae2:charged_certus_quartz_crystal', 'minecraft:redstone', 'minecraft:quartz'].entries()) {
    await fixture.command(`item replace block ${assembler} container.${i} with ${id} 4`);
  }
  const before = await gridEnergy();
  await fixture.step(60);
  const idle = await contents(assembler);
  assert.doesNotMatch(idle, /ae2:fluix_crystal/, 'no fluix from lava');
  assert.match(idle, /amount: 2000L/, 'the lava untouched');
  assert.equal(await gridEnergy(), before, 'no power spent on it');
  fixture.mark('the Crystal Assembler takes no recipe whose fluid ingredient its tank does not match');
  await fixture.command(`data merge block ${assembler} {ae2_lab_config:{tank_in:{id:"minecraft:water",amount:2000L}}}`);
  await until(() => contents(assembler), text => /ae2:fluix_crystal/.test(text) && /count: 8/.test(text), 'eight fluix made', 60000);
  const fluix = await contents(assembler);
  assert.match(fluix, /amount: 1900L/, '100 mB of water used');
  assert.doesNotMatch(fluix, /charged_certus|"minecraft:redstone"|"minecraft:quartz"/);
  assert.equal(before - await gridEnergy(), 2000, 'four cards: 4 steps of 50 progress at 500 AE');
  fixture.mark('the Crystal Assembler takes the fluid of its tank, and four cards run 50 a step for the same 2,000 AE');

  // AUTO_EXPORT: the output goes to the chest beside it.
  await fixture.command(`setblock ${chest} minecraft:chest`);
  await fixture.raw(`interact ${assembler} south`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'crystal_assembler.json', 'screen again');
  await fixture.pressSetting(24);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 24, 'YES'), 'auto export on');
  await fixture.raw('close');
  await until(() => contents(chest), text => /ae2:fluix_crystal/.test(text) && /count: 8/.test(text), 'fluix pushed into the chest', 30000);
  fixture.mark('with AUTO_EXPORT the output goes out to its neighbours');

  // The Circuit Slicer: a silicon block, nine printed silicon.
  await fixture.command(`item replace block ${cutter} container.0 with extendedae:silicon_block 2`);
  await until(() => contents(cutter), text => /ae2:printed_silicon/.test(text) && /count: 9/.test(text), 'nine printed silicon', 60000);
  assert.match(await contents(cutter), /extendedae:silicon_block/, 'one block still to cut');
  await fixture.raw(`interact ${cutter} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Circuit Slicer screen', 15000);
  assert.equal(view.ae2_screen.style, 'circuit_cutter.json');
  await shot('circuit_cutter');
  await fixture.raw('close');
  fixture.mark('the Circuit Slicer cuts a silicon block into nine printed silicon');

  // The Crystal Fixer: fuel by hand, then the budding quartz in front of it grows one step (80% a try).
  await fixture.command(`setblock ${fixerPower} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${fixer} extendedae:crystal_fixer[facing=east]`);
  await fixture.command(`setblock ${budding} ae2:chipped_budding_quartz`);
  await online(fixer);
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'ae2:charged_certus_quartz_crystal', 16); await fixture.select(0, 'ae2:charged_certus_quartz_crystal');
  await fixture.raw(`interact ${fixer} south`);
  await until(() => fixture.raw('inventory'), s => !s.counts?.['ae2:charged_certus_quartz_crystal'], 'fuel in the fixer');
  await until(async () => JSON.stringify(await fixture.raw(`block ${budding}`)), state => state.includes('ae2:flawed_budding_quartz'), 'the budding quartz fixed', 120000);
  const fuel = Number(/count: (\d+)/.exec(await contents(fixer))?.[1] ?? 16);
  assert(fuel < 16, `fuel burned (${fuel} left)`);
  fixture.mark('the Crystal Fixer burns fuel to fix the budding quartz in front of it');

  // The Extended IO Port: a cell of 8,000 cobblestone into the drive's empty cell, 2,048 a tick.
  await fixture.command(`setblock ${portPower} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${port} extendedae:ex_io_port`);
  await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k`);
  await online(port);
  await fixture.hold();
  await fixture.command(`item replace block ${port} container.0 with ${cell('minecraft:cobblestone', 8000)}`);
  let moved = 0;
  for (let i = 0; i < 40 && moved === 0; i++) { await fixture.stepServer(1); moved = count(await fixture.ae(`grid status ${drive}`), 'minecraft:cobblestone'); }
  assert.equal(moved, 2048, 'the first tick moves 2,048');
  for (let i = 0; i < 40 && moved < 8000; i++) { await fixture.stepServer(1); moved = count(await fixture.ae(`grid status ${drive}`), 'minecraft:cobblestone'); }
  assert.equal(moved, 8000);
  await fixture.hold(false);
  await until(() => contents(port), text => /Slot: 6b/.test(text), 'the empty cell moved out', 30000);
  await fixture.command(`tp @s ${x + 11.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${port} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Extended IO Port screen', 15000);
  assert.equal(view.ae2_screen.style, 'ex_io_port.json');
  await shot('ex_io_port');
  await fixture.raw('close');
  fixture.mark('the Extended IO Port moves 2,048 items a tick');

  // Through a restart: the tank, the cards and the setting stay.
  await fixture.restart();
  await online(assembler);
  const kept = await contents(assembler);
  assert.match(kept, /amount: 1900L/, 'the tank');
  assert.equal((kept.match(/ae2:speed_card/g) ?? []).length, 4, 'the cards');
  assert.match(kept, /auto_export: "YES"/, 'AUTO_EXPORT');
  fixture.mark('the tank, cards and settings survive a restart');

  // Broken, the assembler drops its slots and cards; the tank's water is gone.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 6.5}`);
  await fixture.command(`item replace block ${assembler} container.4 with minecraft:redstone 7`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((entity: any) => entity.item?.item === item).reduce((sum: number, entity: any) => sum + entity.item.count, 0);
  await fixture.command(`setblock ${assembler} minecraft:air destroy`);
  await until(() => ground('ae2:speed_card'), n => n === 4, 'the four cards on the ground', 30000);
  await until(() => ground('minecraft:redstone'), n => n === 7, 'the input on the ground', 30000);
  assert.equal(await ground('extendedae:crystal_assembler'), 1, 'the block itself');
  fixture.mark('broken, the Crystal Assembler drops its inputs and cards');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
