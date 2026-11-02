/** AE2's part screens on a cable bus: each opens its style with its toolbar, filters press by place, the storage bus clears and partitions, priority and key types open their sub-screens, level emitters take typed amounts, and an Interface's stock amount opens SetStockAmountScreen. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('part-screens', mode);
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
const chat = (reply: any) => (reply.chat ?? []).join('\n');
const parts = [
  ['storage_bus', 'ae2:storage_bus'], ['import_bus', 'ae2:import_bus'], ['export_bus', 'ae2:export_bus'],
  ['formation_plane', 'ae2:formation_plane'], ['level_emitter', 'ae2:level_emitter'], ['energy_level_emitter', 'ae2:energy_level_emitter'],
] as const;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const width = parts.length * 2 + 4;
  for (const [dx, dz] of [[-2, -3], [-2, 3], [width, -3], [width, 3]] as const) {
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), state => state.loaded, 'chunk loaded', 30000);
  }
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + width} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + width} ${y - 1} ${z + 3} minecraft:stone`);
  await fixture.command('clear @s');
  const at = (i: number) => `${x + 2 * i} ${y} ${z}`;
  // Each part on the south face of its own cable bus.
  for (const [i, [id]] of parts.entries()) {
    await fixture.command(`setblock ${at(i)} ae2:cable_bus`);
    await fixture.replace('hotbar.0', `ae2:${id}`); await fixture.select(0, `ae2:${id}`);
    assert.equal((await fixture.ae(`grid part ${at(i)} south`)).attached, true, id);
  }
  await fixture.replace('hotbar.0', 'minecraft:diamond', 2);
  await fixture.replace('hotbar.8', 'minecraft:stick');
  const part = (test: (p: any) => boolean, label: string) => until(() => fixture.raw('screen'), v => v.ae2_part && test(v.ae2_part), label, 15000).then(v => v.ae2_part);
  const open = async (i: number) => {
    await fixture.select(8, 'minecraft:stick');
    await fixture.command(`tp @s ${x + 2 * i + 0.5} ${y} ${z + 2.5}`);
    await fixture.raw(`interact ${at(i)} south`);
    return part(p => p.kind === parts[i][1], `${parts[i][0]} screen`);
  };
  const press = (rect: number[], right = false) => fixture.raw(`click-at ${centre(rect)}${right ? ' right' : ''}`);
  const data = async (i: number) => chat(await fixture.command(`data get block ${at(i)} ae2_lab_config.parts`));

  // The storage bus: a chest comes in front of it, partition, clear, a filter by place, and a shut filter.
  let bus = await open(0);
  assert.equal(bus.connected, null);
  await fixture.command(`setblock ${x} ${y} ${z + 1} minecraft:chest`);
  await fixture.command(`item replace block ${x} ${y} ${z + 1} container.0 with minecraft:diamond 3`);
  await fixture.command(`item replace block ${x} ${y} ${z + 1} container.1 with minecraft:cobblestone 3`);
  bus = await part(p => p.connected === 'minecraft:chest', 'attached to the chest');
  assert.equal(bus.enabled, 18);
  assert.equal(bus.config.length, 63);
  assert.equal(bus.tools.length, 5, 'no fuzzy button without its card');
  await press(bus.tools[1].rect);
  bus = await part(p => p.config[0].item === 'minecraft:diamond' && p.config[1].item === 'minecraft:cobblestone', 'partition from the chest');
  await press(bus.tools[0].rect);
  bus = await part(p => p.config.every((c: any) => !c.item), 'cleared');
  await fixture.raw('click-slot 0');
  await press(bus.config[10].rect);
  bus = await part(p => p.config[10].item === 'minecraft:diamond', 'second page of the menu filtered by place');
  await press(bus.config[40].rect);
  assert.ok(!(await part(p => true, 'shut filter')).config[40].item, 'a shut filter takes nothing');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('inventory'), v => !v.carried && v.counts?.['minecraft:diamond'] === 2, 'diamonds back');

  // Its access setting, then priority: buttons, typed text, back, kept across restart.
  await press(bus.tools[4].rect);
  bus = await part(p => p.tools[4].value === 'gui.tooltips.ae2.Read', 'read only');
  await press(bus.priority);
  let priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority, 'priority screen', 15000)).ae2_priority;
  assert.equal(priority.text, '0');
  await press(priority.buttons.find((b: any) => b.entry === 2).rect);
  await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === '100', 'plus 100', 15000);
  await press(priority.field);
  for (let n = 0; n < 3; n++) await fixture.raw('type \\b');
  await fixture.raw('type 7');
  await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === '7', 'typed 7', 15000);
  await press(priority.back);
  await part(p => p.kind === 'ae2:storage_bus', 'back at the storage bus');
  await until(async () => await data(0), text => /priority:\s*7\b/.test(text), 'priority saved');
  await fixture.raw('close');
  await fixture.command(`setblock ${x} ${y} ${z + 1} minecraft:air`);
  fixture.mark('the storage bus shows what it faces, partitions, clears, filters by place across the menu pages, and its priority tab opens the priority screen');

  // The import bus's key types: the sub-screen, and the last type stays on.
  let imp = await open(1);
  assert.equal(imp.tools.length, 1);
  await press(imp.tools[0].rect);
  let types = (await part(p => p.key_types, 'key type screen')).key_types;
  assert.deepEqual(types.selected, [true, true]);
  await press(types.boxes[1]);
  types = (await part(p => p.key_types?.selected[1] === false, 'fluids off')).key_types;
  await press(types.boxes[0]);
  await new Promise(done => setTimeout(done, 300));
  assert.deepEqual((await part(p => p.key_types, 'still items')).key_types.selected, [true, false], 'the last type cannot go');
  await press(types.back);
  await part(p => !p.key_types, 'back at the import bus');
  await fixture.raw('close');
  fixture.mark('the import bus opens its key types and keeps at least one on');

  // The export bus and the formation plane: their settings, upgrades and player slots.
  const exp = await open(2);
  assert.equal(exp.upgrades, 5);
  assert.deepEqual(exp.tools.map((t: any) => t.place), [43]);
  await press(exp.tools[0].rect);
  await part(p => p.tools[0].value === 'gui.tooltips.ae2.SchedulingModeRoundRobin', 'round robin');
  await fixture.raw('close');
  const plane = await open(3);
  assert.deepEqual(plane.tools.map((t: any) => t.place), [53]);
  await press(plane.tools[0].rect);
  await part(p => p.tools[0].value === 'gui.tooltips.ae2.BlockPlacementNo', 'item entities');
  await fixture.raw('close');

  // The level emitter: filter by its slot, steps, typed text, no field with a crafting card.
  let emitter = await open(4);
  assert.equal(emitter.numbers.length, 8);
  await press(emitter.numbers[2].rect);
  await part(p => p.text === '100', 'plus 100');
  await press(emitter.numbers[4].rect);
  await part(p => p.text === '99', 'minus 1');
  await press(emitter.field);
  for (let n = 0; n < 2; n++) await fixture.raw('type \\b');
  await fixture.raw('type 42');
  await part(p => p.text === '42', 'typed 42');
  await until(async () => await data(4), text => /emitter_threshold:\s*42L?\b/.test(text) || /threshold:\s*42L?\b/.test(text), 'threshold saved');
  await press(emitter.tools[0].rect);
  await part(p => p.tools[0].value === 'gui.tooltips.ae2.EmitLevelsBelow' || p.tools[0].value === 'gui.tooltips.ae2.EmitLevelAbove', 'redstone toggled');
  await fixture.raw('close');

  // The energy level emitter: its field in AE.
  const energy = await open(5);
  await press(energy.numbers[1].rect);
  await part(p => p.text === '10', 'plus 10 AE');
  await fixture.raw('close');
  fixture.mark('level emitters step, take typed amounts and toggle their mode');

  // Restart keeps what the screens set.
  await fixture.restart();
  bus = await open(0);
  assert.equal(bus.config[10].item, 'minecraft:diamond');
  assert.equal(bus.tools[4].value, 'gui.tooltips.ae2.Read');
  await fixture.raw('close');
  imp = await open(1);
  await press(imp.tools[0].rect);
  assert.deepEqual((await part(p => p.key_types, 'key types again')).key_types.selected, [true, false]);
  await fixture.raw('close');
  await open(4);
  await part(p => p.text === '42', 'the threshold after restart');
  await fixture.raw('close');
  fixture.mark('filters, access, key types and thresholds survive a restart');

  // An Interface's stock amount: SetStockAmountScreen over its entries.
  const face = `${x + 2 * parts.length} ${y} ${z}`;
  await fixture.command(`setblock ${face} ae2:interface`);
  await fixture.replace('hotbar.0', 'minecraft:diamond', 2);
  await fixture.select(8, 'minecraft:stick');
  await fixture.command(`tp @s ${x + 2 * parts.length + 0.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${face} south`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'interface.json', 'interface screen', 15000);
  await fixture.raw('click-slot 0');
  await fixture.raw('click-slot 309');
  await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot === 309 && s.item === 'minecraft:diamond'), 'stock set', 15000);
  await fixture.raw('click-slot 0');
  await fixture.raw('click-slot 309 shift');
  const stock = (await until(() => fixture.raw('screen'), v => v.ae2_priority?.style === 'set_stock_amount.json', 'stock amount screen', 15000)).ae2_priority;
  assert.equal(stock.valid, true);
  await press(stock.buttons.find((b: any) => b.entry === 0).rect);
  const more = await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === String(Number(stock.text) + 1), 'plus one', 15000);
  await press(more.ae2_priority.save);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'interface.json' && !v.ae2_priority, 'saved, back at the interface', 15000);
  await fixture.raw('close');
  fixture.mark('the Interface stock amount opens SetStockAmountScreen and saves');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
