/** MEGA Cells slice 7: the MEGA Bulk Item Cell (`BulkCellItem`, its own `ICellHandler`, `BulkCellInventory`): one
 * filtered item type in a count of units with no limit, mounted by the Drive, the ME Chest, the Cell Dock and the IO
 * Port; its 5 AE/t, its LED (`getStatus`: empty, not empty, full on a mismatched filter) and its count through a
 * restart; one filter and one card slot in the Cell Workbench; MEGA's two recipes. Needs content/megacells
 * (ae2/scripts/prepare-addon.ts megacells) and a client built with ae2-megacells. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('megacells-bulk', mode, { addons: ['megacells'] });
const BULK = 'megacells:bulk_item_cell', DOCK = 'megacells:cell_dock';
const config = (id: string) => `ae2:storage_cell_config_inv=[{"#t":"ae2:i",id:"${id}","#":0L}]`;
const filtered = (id: string) => `${BULK}[${config(id)}]`;
/** A bulk cell holding `count` units of `stored` recorded at `factor`, partitioned for `filter`. */
const holding = (stored: string, count: string, factor: string, filter = stored) =>
  `${BULK}[${config(filter)},megacells:bulk_item={"#t":"ae2:i",id:"${stored}"},megacells:bulk_unit_count="${count}",megacells:bulk_unit_factor="${factor}"]`;
/** 4 × (2^63 - 1): past any long. */
const HUGE = '36893488147419103228';
const STACK_LIMIT = 2 ** 42;
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const units = (data: string) => [...data.matchAll(/bulk_unit_count[^0-9]*([0-9]+)/g)].map(m => m[1]);
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -4], [-3, 6], [9, -4], [9, 6]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -4)} ${at(9, 5, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -4)} ${at(9, -1, 6)} minecraft:stone`);
  await fixture.command('clear @s');
  // A Dense Energy Cell, a Drive with an AE2 1k cell, a cable bus, an ME Chest and an IO Port in a row.
  const cell = at(0, 2), drive = at(1, 2), bus = at(2, 2), chest = at(3, 2), port = at(4, 2);
  for (const [pos, id] of [[cell, 'ae2:dense_energy_cell'], [drive, 'ae2:drive'], [bus, 'ae2:cable_bus'], [chest, 'ae2:chest'], [port, 'ae2:io_port']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`data merge block ${cell} {ae2_lab_energy:1000000.0d}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, view => view.online, 'the grid is online', 30000);
  const energy = async () => (await fixture.ae(`power ${cell} east`)).energy as number;
  const drain = async () => {
    await fixture.hold(); await fixture.stepServer(3);
    const before = await energy(); await fixture.stepServer(20); const after = await energy();
    await fixture.hold(false); return before - after;
  };
  const driveData = async () => JSON.stringify(await fixture.command(`data get block ${drive}`));
  const cells = async () => (await fixture.raw(`block ${drive}`)).ae2?.cells ?? [];
  const base = await drain();

  // An unpartitioned bulk cell and one for cobblestone: 5 AE/t each (BulkCellInventory.getIdleDrain).
  await fixture.command(`item replace block ${drive} container.1 with ${BULK}`);
  await fixture.command(`item replace block ${drive} container.2 with ${filtered('minecraft:cobblestone')}`);
  await until(cells, c => c[1]?.item === BULK && c[2]?.item === BULK, 'the drive draws both bulk cells', 30000);
  const two = await drain();
  assert(Math.abs(two - base - 200) < 1e-6, `two bulk cells add 10 AE/t: ${two - base} AE in 20 ticks`);
  fixture.mark(`a Drive takes two bulk cells, which draw 5 AE/t each (${two - base} AE in 20 ticks)`);

  // The partitioned cell is the preferred storage for cobblestone (isPreferredStorageFor), before the 1k cell in
  // slot 0; dirt goes past both bulk cells into the 1k cell, the unpartitioned one taking nothing.
  await fixture.replace('hotbar.0', 'minecraft:cobblestone', 64); await fixture.select(0, 'minecraft:cobblestone');
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 64);
  await fixture.replace('hotbar.0', 'minecraft:dirt', 10); await fixture.select(0, 'minecraft:dirt');
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 10);
  let view = await until(status, v => count(v, 'minecraft:cobblestone') === 64 && count(v, 'minecraft:dirt') === 10, 'cobblestone and dirt stored', 30000);
  let data = await driveData();
  assert.deepEqual(units(data), ['64'], 'all 64 cobblestone are units of the partitioned bulk cell');
  let leds = await until(cells, c => c[1]?.state === 1 && c[2]?.state === 2, 'the LEDs: empty and not empty', 30000);
  assert.equal(leds[0].state, 2);
  fixture.mark('the partitioned cell takes 64 cobblestone before the 1k cell, the unpartitioned one nothing; LEDs empty and not empty');

  // The IO Port empties a bulk cell of 100 cobblestone into the grid (the drive's bulk cell), then moves it out.
  await fixture.command(`item replace block ${port} container.0 with ${holding('minecraft:cobblestone', '100', '1')}`);
  await until(status, v => count(v, 'minecraft:cobblestone') === 164, 'the IO Port moved 100 cobblestone', 30000);
  const portData = await until(async () => JSON.stringify(await fixture.command(`data get block ${port}`)), d => /Slot: ?6b/.test(d) && !/Slot: ?0b/.test(d), 'the emptied cell moved to the output', 30000);
  assert.match(portData, /bulk_item_cell/); assert.deepEqual(units(portData), [], 'no units left on it');
  assert.deepEqual(units(await driveData()), ['164']);
  fixture.mark('the IO Port empties a bulk cell of 100 cobblestone into the grid and moves it to its output');

  // No limit: 4 × (2^63 - 1) units; the grid lists 2^42 (CompressionChain.STACK_LIMIT), takes more and gives some.
  await fixture.command(`item replace block ${drive} container.2 with ${holding('minecraft:cobblestone', HUGE, '1')}`);
  await until(status, v => count(v, 'minecraft:cobblestone') === STACK_LIMIT, 'the grid lists 2^42 cobblestone', 30000);
  await fixture.replace('hotbar.0', 'minecraft:cobblestone', 64); await fixture.select(0, 'minecraft:cobblestone');
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 64, 'a full bulk cell still takes cobblestone');
  await until(driveData, d => units(d).includes('36893488147419103292'), '64 more units', 30000);
  await fixture.command('clear @s');
  view = await status();
  const index = view.items.findIndex((entry: any) => entry.item === 'minecraft:cobblestone');
  assert.equal((await fixture.ae(`grid withdraw ${drive} ${index} 64`)).extracted, 64);
  await until(driveData, d => units(d).includes(HUGE), 'back to 4 × (2^63 - 1) units', 30000);
  fixture.mark(`a bulk cell of ${HUGE} cobblestone lists 2^42, takes 64 more and gives 64 back, counted exactly`);

  // The Cell Dock and the ME Chest mount bulk cells too. A cell holding cobblestone but partitioned for diamonds is
  // FULL (a mismatched filter): it lists its cobblestone and takes neither item. The chest's cell takes stone.
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 0.5} 90 0`);
  await fixture.replace('hotbar.0', DOCK); await fixture.select(0, DOCK);
  assert.equal((await fixture.ae(`grid part ${bus} down`)).attached, true);
  await fixture.command(`item replace block ${chest} container.0 with ${filtered('minecraft:stone')}`);
  const dock = async () => (await fixture.raw(`block ${bus}`)).ae2?.parts?.find((p: any) => p.face === 'down');
  await until(dock, p => p?.id === DOCK && p.online, 'the dock online', 30000);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', holding('minecraft:cobblestone', '5', '1', 'minecraft:diamond')); await fixture.replace('hotbar.8', 'minecraft:blaze_rod');
  await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${bus} down`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'megacells/cell_dock.json', 'the Cell Dock screen', 15000);
  await fixture.raw('click-slot 0 shift');
  let part = await until(dock, p => p?.cell?.item === BULK, 'the dock draws the bulk cell', 30000);
  await fixture.raw('close');
  assert.equal(part.cell.state, 4, 'a mismatched filter: FULL');
  await until(status, v => count(v, 'minecraft:cobblestone') === STACK_LIMIT + 5, 'the dock\'s 5 cobblestone listed', 30000);
  for (const [item, n] of [['minecraft:diamond', 3], ['minecraft:stone', 7]] as const) {
    await fixture.replace('hotbar.1', item, n); await fixture.select(1, item);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, n);
  }
  view = await until(status, v => count(v, 'minecraft:stone') === 7 && count(v, 'minecraft:diamond') === 3, 'stone and diamonds stored', 30000);
  assert.match(JSON.stringify(await fixture.command(`data get block ${chest}`)), /bulk_unit_count[^0-9]*7\b/, 'the stone in the chest\'s bulk cell');
  assert.deepEqual(units(JSON.stringify(await fixture.command(`data get block ${bus}`))), ['5'], 'the mismatched cell kept its 5 units and took no diamonds');
  fixture.mark('the Cell Dock mounts a bulk cell with a mismatched filter (FULL, listing its 5 cobblestone, taking nothing); the ME Chest\'s takes 7 stone');

  // Through a restart: every count.
  await fixture.restart();
  view = await until(status, v => v.online && count(v, 'minecraft:stone') === 7, 'the grid after restart', 60000);
  assert.equal(count(view, 'minecraft:cobblestone'), STACK_LIMIT + 5);
  assert(units(await driveData()).includes(HUGE), 'the huge count after restart');
  part = await until(dock, p => p?.cell?.item === BULK, 'the dock\'s cell after restart', 30000);
  assert.equal(part.cell.state, 4);
  fixture.mark('the counts, the chest\'s stone and the dock\'s mismatched cell survive a restart');

  // The Cell Workbench: one filter (CellConfig.create(..., 1)) and one card slot (UpgradeInventories.forItem(is, 1)).
  const bench = at(0, 0, 3);
  await fixture.command(`setblock ${bench} ae2:cell_workbench`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', BULK); await fixture.replace('hotbar.1', 'minecraft:iron_block');
  await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 1.5}`);
  await fixture.raw(`interact ${bench} north`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'cell_workbench.json' && v.ae2_workbench, 'workbench screen', 15000);
  await fixture.raw('click-slot 0 shift');
  let work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[0].open, 'the bulk cell in the workbench', 15000)).ae2_workbench;
  assert.equal(work.upgrades, 1, 'one card slot');
  assert.equal(work.config.filter((c: any) => c.open).length, 1, 'one filter');
  await fixture.raw('click-slot 1');
  await fixture.raw(`click-at ${centre(work.config[0].rect)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[0].item === 'minecraft:iron_block', 'the filter set', 15000);
  await fixture.raw('click-slot 1');
  await fixture.raw('click-slot 300 shift');
  await until(() => fixture.raw('inventory'), s => s.counts?.[BULK] === 1, 'the cell back in the inventory');
  await fixture.raw('close');
  const held = JSON.stringify(await fixture.command('data get entity @s Inventory'));
  assert.match(held, /storage_cell_config_inv[^\]]*minecraft:iron_block/, 'the cell keeps its iron block filter');
  fixture.mark('the Cell Workbench gives a bulk cell one filter and one card slot, and the filter stays on the cell');

  // The Compression Card (MEGACells.initUpgrades: one on the bulk cell) in the workbench: the cell compresses its
  // filter's chain (CompressionService, from vanilla's iron recipes: nugget, ingot, block) and MEGA's cutoff button
  // shows last on the toolbar (CellWorkbenchScreenMixin) with the largest variant; a press goes one variant down, a
  // right press one up (mega$nextCompressionLimit).
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', filtered('minecraft:iron_block')); await fixture.replace('hotbar.2', 'megacells:compression_card');
  await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  await fixture.raw(`interact ${bench} north`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'cell_workbench.json' && v.ae2_workbench, 'workbench screen', 15000);
  await fixture.raw('click-slot 0 shift');
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[0].item === 'minecraft:iron_block', 'the iron block cell in the workbench', 15000)).ae2_workbench;
  assert.equal(work.cutoff, null, 'no card, no cutoff button');
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'megacells:compression_card', 'the card on the cursor');
  await fixture.raw('click-slot 301'); await until(() => fixture.raw('inventory'), s => !s.carried, 'the card in');
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.cutoff === 'minecraft:iron_block', 'the cutoff button at the iron block', 15000)).ae2_workbench;
  const cutoffButton = () => work.tools.at(-1);
  assert.equal(cutoffButton().title, 'gui.tooltips.megacells.CompressionCutoff', 'the cutoff button comes last');
  for (const [press, expected] of [['', 'minecraft:iron_ingot'], ['', 'minecraft:iron_nugget'], [' right', 'minecraft:iron_ingot']]) {
    await fixture.raw(`click-at ${centre(cutoffButton().rect)}${press}`);
    work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.cutoff === expected, `the cutoff at ${expected}`, 15000)).ae2_workbench;
  }
  // Into the Drive (its fourth slot) by a shift-click from the workbench, then from the inventory.
  await fixture.raw('click-slot 300'); await until(() => fixture.raw('inventory'), s => s.carried?.item === BULK, 'the compressing cell on the cursor');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !s.carried && s.hotbar[0]?.item === BULK, 'the compressing cell in the hotbar');
  await fixture.raw('close');
  assert.match(JSON.stringify(await fixture.command('data get entity @s Inventory')), /compression_cutoff[^0-9-]*1\b/, 'the cutoff recorded on the cell: 1');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z - 1.5}`);
  await fixture.raw(`interact ${drive} north`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'drive.json', 'the Drive screen', 15000);
  await fixture.raw('click-slot 0 shift');
  await until(cells, c => c[3]?.item === BULK, 'the compressing cell in the drive', 30000);
  await fixture.raw('close');
  fixture.mark('a Compression Card turns on the cutoff button, at the iron block; presses take it to the ingot, the nugget and back up to the ingot');

  // At the ingot cutoff, a block, two ingots and three nuggets (81 + 18 + 3 = 102 units) are 11 ingots and 3
  // nuggets (CompressionChain.initStacks); two ingots out leave 84 units: 9 ingots, 3 nuggets.
  for (const [item, n] of [['minecraft:iron_block', 1], ['minecraft:iron_ingot', 2], ['minecraft:iron_nugget', 3]] as const) {
    await fixture.replace('hotbar.1', item, n); await fixture.select(1, item);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, n);
  }
  view = await until(status, v => count(v, 'minecraft:iron_ingot') === 11 && count(v, 'minecraft:iron_nugget') === 3, '11 ingots and 3 nuggets listed', 30000);
  assert.equal(count(view, 'minecraft:iron_block'), 0, 'no block listed past the cutoff');
  assert(units(await driveData()).includes('102'));
  await fixture.command('clear @s');
  const ingots = view.items.findIndex((entry: any) => entry.item === 'minecraft:iron_ingot');
  assert.equal((await fixture.ae(`grid withdraw ${drive} ${ingots} 2`)).extracted, 2);
  await until(status, v => count(v, 'minecraft:iron_ingot') === 9 && count(v, 'minecraft:iron_nugget') === 3, '9 ingots and 3 nuggets', 30000);
  assert(units(await driveData()).includes('84'));
  fixture.mark('at the ingot cutoff, a block, 2 ingots and 3 nuggets list as 11 ingots and 3 nuggets; 2 ingots out leave 9 and 3 (84 units)');

  // megacells:compression_overrides: no recipe turns a snow block back into snowballs, so only the data map makes
  // the chain (snowball, snow block at 4). A compressing snowball cell in the Drive takes a snow block and two
  // snowballs and lists both at its default cutoff, the snow block.
  const card = 'ae2:upgrades=[{slot:0,item:{id:"megacells:compression_card",count:1}}]';
  await fixture.command(`item replace block ${drive} container.4 with ${BULK}[${config('minecraft:snowball')},${card}]`);
  await until(cells, c => c[4]?.item === BULK, 'the snowball cell in the drive', 30000);
  for (const [item, n] of [['minecraft:snow_block', 1], ['minecraft:snowball', 2]] as const) {
    await fixture.replace('hotbar.1', item, n); await fixture.select(1, item);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, n);
  }
  await until(status, v => count(v, 'minecraft:snow_block') === 1 && count(v, 'minecraft:snowball') === 2, 'a snow block and 2 snowballs listed', 30000);
  assert(units(await driveData()).includes('6'), 'six snowball units');
  fixture.mark('the data map\'s override chains snowballs into snow blocks: a snow block and 2 snowballs, 6 units');

  // Through a restart the server gathers its chains again: the same listings.
  await fixture.restart();
  view = await until(status, v => v.online && count(v, 'minecraft:iron_ingot') === 9, 'the iron cell after restart', 60000);
  assert.equal(count(view, 'minecraft:iron_nugget'), 3); assert.equal(count(view, 'minecraft:snow_block'), 1); assert.equal(count(view, 'minecraft:snowball'), 2);
  fixture.mark('the compressed listings come back after a restart');

  // The Portable Cell Workbench has the cutoff button too (PortableCellWorkbenchScreen, PortableCellWorkbenchMenu).
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', 'megacells:portable_cell_workbench');
  await fixture.replace('hotbar.1', `${BULK}[${config('minecraft:iron_nugget')},${card}]`);
  // Looking up, so the use is the item's (Item.use), not the Drive's.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5} 0 -90`);
  await fixture.select(0, 'megacells:portable_cell_workbench'); await fixture.raw('use-item');
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'megacells/portable_cell_workbench.json' && v.ae2_workbench, 'the Portable Cell Workbench', 15000);
  await fixture.raw('click-slot 1 shift');
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.cutoff === 'minecraft:iron_block', 'its cutoff button at the iron block', 15000)).ae2_workbench;
  await fixture.raw(`click-at ${centre(cutoffButton().rect)} right`);
  await until(() => fixture.raw('screen'), v => v.ae2_workbench?.cutoff === 'minecraft:iron_nugget', 'a right press wraps to the nugget', 15000);
  await fixture.raw('close');
  fixture.mark('the Portable Cell Workbench shows the cutoff button and a right press wraps it from the block to the nugget');

  // MEGA's recipes: crafting/bulk_cell_component, cells/standard/bulk_item_cell and crafting/compression_card.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
  await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');
  const table = at(0, 0, 5);
  await fixture.command(`setblock ${table} minecraft:crafting_table`);
  const craft = async (grid: string[], result: string) => {
    const distinct = [...new Set(grid.filter(Boolean))];
    for (const [i, id] of distinct.entries()) await fixture.replace(`hotbar.${i}`, id, grid.filter(g => g === id).length);
    await fixture.raw(`interact ${table} north`);
    await until(() => fixture.raw('screen'), s => s.craft_size === 3 && s.slots?.some((slot: any) => slot.slot === 108), 'crafting table grid');
    for (const [i, id] of distinct.entries()) {
      await fixture.raw(`click-slot ${i}`); await until(() => fixture.raw('inventory'), s => s.carried?.item === id, `${id} on the cursor`);
      for (const [cell, want] of grid.entries()) if (want === id) await fixture.raw(`click-slot ${100 + cell} right`);
      await until(() => fixture.raw('inventory'), s => !s.carried, `${id} placed`);
    }
    await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === result), `the recipe offers ${result}`);
    await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
    await until(() => fixture.raw('inventory'), s => s.counts?.[result] === 1, `the crafted ${result}`);
  };
  const [dust, glass] = ['ae2:sky_dust', 'ae2:quartz_vibrant_glass'];
  await craft([dust, 'ae2:spatial_cell_component_2', dust, 'megacells:accumulation_processor', glass, 'megacells:accumulation_processor', dust, 'megacells:cell_component_1m', dust], 'megacells:bulk_cell_component');
  await craft([glass, dust, glass, dust, 'megacells:bulk_cell_component', dust, 'minecraft:netherite_ingot', 'minecraft:netherite_ingot', 'minecraft:netherite_ingot'], BULK);
  await craft(['ae2:advanced_card', 'ae2:matter_ball', '', '', '', '', '', '', ''], 'megacells:compression_card');
  await fixture.command('gamemode creative @s');
  fixture.mark('MEGA\'s recipes make the Bulk Storage Component, the Bulk Item Cell and the Compression Card');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
