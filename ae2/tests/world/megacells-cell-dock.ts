/** MEGA Cells slice 6: the ME Cell Dock (`CellDockPart`), a cable part that holds one storage cell. It attaches to a
 * cable; its menu (`CellDockMenu`, `screens/megacells/cell_dock.json`) takes the cells AE2 handles (AE2's and MEGA's,
 * item and fluid) and nothing else; the cell is mounted on the grid at the dock's priority (`mountInventories`);
 * the dock draws 0.5 AE/t and the cell's idle drain; the client draws the cell with its LED; a hopper above puts a cell
 * in and one below takes it out (its item handler); the spin it is placed with turns with a wrench; cell, priority and
 * spin survive a restart; broken, it drops itself and its cell; MEGA's recipe makes it. Needs content/megacells
 * (ae2/scripts/prepare-addon.ts megacells) and a client built with ae2-megacells. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('megacells-cell-dock', mode, { addons: ['megacells'] });
const DOCK = 'megacells:cell_dock', STYLE = 'megacells/cell_dock.json';
const STONE_CELL = 'ae2:item_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:stone","#":64L}]]';
const DIAMOND_CELL = 'megacells:item_storage_cell_1m[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:diamond","#":5L}]]';
const GOLD_CELL = 'megacells:item_storage_cell_4m[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:gold_ingot","#":3L}]]';
const FLUID_CELL = 'megacells:fluid_storage_cell_1m';
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const holds = (view: any, slot: number, item: string) => view.slots.some((s: any) => s.slot === slot && s.item === item);
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
  // A grid two blocks up, so a player under it reaches the docks on the cables' bottom faces:
  // a Dense Energy Cell with 1,000,000 AE, a Drive with an empty 1k cell, three cables.
  const cell = at(0, 2), drive = at(1, 2), busA = at(2, 2), busC = at(3, 2), busB = at(4, 2);
  for (const [pos, id] of [[cell, 'ae2:dense_energy_cell'], [drive, 'ae2:drive'], [busA, 'ae2:cable_bus'], [busC, 'ae2:cable_bus'], [busB, 'ae2:cable_bus']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`data merge block ${cell} {ae2_lab_energy:1000000.0d}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, view => view.online, 'the grid is online', 30000);
  const energy = async () => (await fixture.ae(`power ${cell} east`)).energy as number;
  // The AE the grid draws in 20 ticks, the server held.
  const drain = async () => {
    await fixture.hold(); await fixture.stepServer(3);
    const before = await energy(); await fixture.stepServer(20); const after = await energy();
    await fixture.hold(false); return before - after;
  };
  const base = await drain();

  // CellDockPart.onPlacement: on a bottom face, the spin of the player's yaw (90 -> floor(3.5) & 3 = 3).
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 0.5} 90 0`);
  await fixture.replace('hotbar.0', DOCK); await fixture.select(0, DOCK);
  assert.equal((await fixture.ae(`grid part ${busA} down`)).attached, true);
  const dockA = async () => (await fixture.raw(`block ${busA}`)).ae2?.parts?.find((p: any) => p.face === 'down');
  let part = await until(dockA, p => p?.id === DOCK && p.online, 'dock A drawn online', 30000);
  assert.equal(part.spin, 3, 'placed with the yaw\'s spin');
  assert.equal(part.cell, null, 'no cell yet');
  const empty = await drain();
  assert(empty - base >= 10 - 1e-6, `the dock draws 0.5 AE/t: ${empty - base} AE in 20 ticks`);
  fixture.mark(`the dock attaches to a cable bottom face with spin 3 and draws ${empty - base} AE in 20 ticks (0.5 AE/t and its channel)`);

  // CellDockMenu: MEGA's screen, the cell slot (STORAGE_CELLS), the priority tab.
  const open = async () => {
    await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 0.5}`);
    await fixture.raw(`interact ${busA} down`);
    return until(() => fixture.raw('screen'), v => v.ae2_screen?.style === STYLE, 'the Cell Dock screen', 15000);
  };
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', 'minecraft:dirt'); await fixture.replace('hotbar.1', STONE_CELL);
  await fixture.replace('hotbar.2', DIAMOND_CELL); await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  let view = await open();
  assert(view.ae2_screen.priority, 'the priority tab');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:dirt', 'dirt on the cursor');
  await fixture.raw('click-slot 300'); await fixture.step(2);
  assert(!holds(await fixture.raw('screen'), 300, 'minecraft:dirt'), 'the dock refuses dirt');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !s.carried, 'dirt back');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:item_storage_cell_1k', 'the 1k cell on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => holds(v, 300, 'ae2:item_storage_cell_1k'), 'the dock takes AE2\'s cell');
  await until(status, v => count(v, 'minecraft:stone') === 64, 'the dock\'s cell is mounted on the grid', 30000);
  fixture.mark('the dock opens MEGA\'s screen, refuses dirt and mounts an AE2 1k cell\'s 64 stone on the grid');

  // The priority tab: +100 (`IPriorityHost.setPriority`).
  await fixture.raw(`click-at ${centre(view.ae2_screen.priority)}`);
  let priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority, 'priority screen', 15000)).ae2_priority;
  assert.equal(priority.text, '0');
  await fixture.raw(`click-at ${centre(priority.buttons.find((b: any) => b.entry === 2).rect)}`);
  priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === '100', 'priority 100', 15000)).ae2_priority;
  await fixture.raw(`click-at ${centre(priority.back)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === STYLE && !v.ae2_priority, 'back to the dock', 15000);
  await fixture.raw('close');
  const one = await drain();
  assert(Math.abs(one - empty - 10) < 1e-6, `the 1k cell adds its 0.5 AE/t: ${one - empty}`);
  // Mounted at priority 100 over the Drive's 0, the dock's cell takes what the grid stores.
  await fixture.replace('hotbar.0', 'minecraft:cobblestone', 10); await fixture.select(0, 'minecraft:cobblestone');
  await fixture.ae(`grid deposit ${drive}`);
  await until(status, v => count(v, 'minecraft:cobblestone') === 10, 'ten cobblestone stored', 30000);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  view = await open();
  await fixture.raw('click-slot 300 shift');
  await until(() => fixture.raw('inventory'), s => s.counts?.['ae2:item_storage_cell_1k'] === 1, 'the cell back in the inventory');
  view = await until(status, v => count(v, 'minecraft:stone') === 0 && count(v, 'minecraft:cobblestone') === 0, 'the cell left the grid with its stone and the cobblestone', 30000);
  fixture.mark('at priority 100 the dock\'s cell takes the grid\'s cobblestone before the Drive\'s; the cell adds 0.5 AE/t');

  // MEGA's 1M cell: 3 AE/t (0.5 x tier index 6).
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'megacells:item_storage_cell_1m', 'the 1M cell on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => holds(v, 300, 'megacells:item_storage_cell_1m'), 'the dock takes MEGA\'s cell');
  await fixture.raw('close');
  await until(status, v => count(v, 'minecraft:diamond') === 5, 'the 1M cell\'s diamonds on the grid', 30000);
  const mega = await drain();
  assert(Math.abs(mega - empty - 60) < 1e-6, `the 1M cell adds its 3 AE/t: ${mega - empty}`);
  part = await until(dockA, p => p?.cell?.item === 'megacells:item_storage_cell_1m', 'the client draws the 1M cell', 15000);
  assert.equal(part.cell.state, 2, 'its LED: NOT_EMPTY');
  assert.equal(part.powered, true);
  fixture.mark('MEGA\'s 1M cell mounts its 5 diamonds, adds 3 AE/t and is drawn with its NOT_EMPTY LED');

  // A wrench turns the dock (CellDockPart.onUseItemOn): 3 -> 0 -> 1.
  await fixture.replace('hotbar.7', 'ae2:certus_quartz_wrench'); await fixture.select(7, 'ae2:certus_quartz_wrench');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 0.5}`);
  await fixture.raw(`interact ${busA} down`); await until(dockA, p => p?.spin === 0, 'spin 0', 15000);
  await fixture.raw(`interact ${busA} down`); await until(dockA, p => p?.spin === 1, 'spin 1', 15000);
  assert(!(await fixture.raw('screen')).ae2_screen, 'the wrench opens no menu');
  fixture.mark('a wrench turns the dock a quarter at a time');

  // Hoppers (the part's Capabilities.ItemHandler.BLOCK): one above dock B puts a fluid cell in and keeps the
  // dirt it can't; one below dock C takes its cell out.
  await fixture.replace('hotbar.0', DOCK); await fixture.select(0, DOCK);
  assert.equal((await fixture.ae(`grid part ${busB} up`)).attached, true);
  assert.equal((await fixture.ae(`grid part ${busC} down`)).attached, true);
  await fixture.command(`setblock ${at(4, 3)} minecraft:hopper[facing=down]`);
  await fixture.command(`item replace block ${at(4, 3)} container.0 with minecraft:dirt`);
  await fixture.command(`item replace block ${at(4, 3)} container.1 with ${FLUID_CELL}`);
  const dockB = async () => (await fixture.raw(`block ${busB}`)).ae2?.parts?.find((p: any) => p.face === 'up');
  part = await until(dockB, p => p?.cell?.item === FLUID_CELL, 'the hopper puts the fluid cell into dock B', 30000);
  assert.equal(part.cell.state, 1, 'its LED: EMPTY');
  const hopper = async (pos: string) => JSON.stringify(await fixture.command(`data get block ${pos}`));
  assert((await hopper(at(4, 3))).includes('minecraft:dirt'), 'the dirt stays in the hopper');
  await fixture.replace('hotbar.0', GOLD_CELL); await fixture.select(0, 'megacells:item_storage_cell_4m');
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 0.5}`);
  await fixture.raw(`interact ${busC} down`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === STYLE, 'dock C\'s screen', 15000);
  await fixture.raw('click-slot 0 shift');
  await until(status, v => count(v, 'minecraft:gold_ingot') === 3, 'dock C\'s cell mounted', 30000);
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 3.5}`);
  await fixture.command(`setblock ${at(3, 1)} minecraft:hopper`);
  await until(() => hopper(at(3, 1)), items => items.includes('megacells:item_storage_cell_4m'), 'the hopper below takes dock C\'s cell', 30000);
  await until(status, v => count(v, 'minecraft:gold_ingot') === 0, 'its gold left the grid', 30000);
  fixture.mark('a hopper puts a MEGA fluid cell into a dock and keeps dirt; one below takes a dock\'s cell out');

  // Through a restart: the cell, the priority and the spin.
  await fixture.restart();
  await until(status, v => v.online && count(v, 'minecraft:diamond') === 5, 'dock A\'s cell mounted after restart', 30000);
  part = await until(dockA, p => p?.cell?.item === 'megacells:item_storage_cell_1m', 'dock A drawn after restart', 30000);
  assert.equal(part.spin, 1, 'the spin after restart');
  assert.equal((await until(dockB, p => !!p?.cell, 'dock B after restart', 15000)).cell.item, FLUID_CELL);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  view = await open();
  assert(holds(view, 300, 'megacells:item_storage_cell_1m'), 'the cell in the menu after restart');
  await fixture.raw(`click-at ${centre(view.ae2_screen.priority)}`);
  priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority, 'priority screen after restart', 15000)).ae2_priority;
  assert.equal(priority.text, '100');
  await fixture.raw('close');
  fixture.mark('the cells, the priority 100 and the spin survive a restart');

  // Broken (Level.destroyBlock), the cable drops the dock and its cell (out of the player's reach).
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 4.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((entity: any) => entity.item?.item === item).reduce((sum: number, entity: any) => sum + entity.item.count, 0);
  await fixture.command('kill @e[type=minecraft:item]');
  await fixture.command(`setblock ${busA} minecraft:air destroy`);
  await until(async () => [await ground(DOCK), await ground('megacells:item_storage_cell_1m')], n => n[0] === 1 && n[1] === 1, 'the dock and its cell on the ground', 30000);
  await fixture.command('kill @e[type=minecraft:item]');
  fixture.mark('broken, the cable drops the dock and its cell');

  // MEGA's recipe (network/cell_dock): iron, copper, iron over a glass cable.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
  await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');
  const table = at(0, 0, 5);
  await fixture.command(`setblock ${table} minecraft:crafting_table`);
  const ingredients: [string, number][] = [['minecraft:iron_ingot', 100], ['minecraft:copper_ingot', 101], ['minecraft:iron_ingot', 102], ['ae2:fluix_glass_cable', 104]];
  for (const [i, [id]] of ingredients.entries()) await fixture.replace(`hotbar.${i}`, id);
  await fixture.raw(`interact ${table} north`);
  await until(() => fixture.raw('screen'), s => s.craft_size === 3 && s.slots?.some((slot: any) => slot.slot === 108), 'crafting table grid');
  for (const [i, [, slot]] of ingredients.entries()) {
    await fixture.raw(`click-slot ${i}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${slot}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === DOCK), 'the recipe offers a Cell Dock');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  await until(() => fixture.raw('inventory'), s => s.counts?.[DOCK] === 1, 'the crafted dock');
  await fixture.command('gamemode creative @s');
  fixture.mark('MEGA\'s recipe makes the Cell Dock');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
