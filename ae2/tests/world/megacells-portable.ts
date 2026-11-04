/** MEGA Cells slice 4: the portable M cells and the Portable Cell Workbench. MEGA's recipes make a 1M Portable
 * Item Cell and the workbench; MEGAPortableCell holds 8 times AE2's battery (160,000 AE) and charges twice as
 * fast (160 AE/t) in a Charger, opens like AE2's portable cells, stores half its tier's bytes (4,128,768 items
 * of one type at 1M) in 18 + (index - 5) × 9 types (27 at 1M), and disassembles with MEGA's recipe, its energy
 * going into the Dense Energy Cell it gives back. The Portable Cell Workbench (PortableCellWorkbenchItem) holds a
 * cell and opens AE2's Cell Workbench screen on it: the cell's cards (two Greater Energy Cards, 2,580,000 AE before
 * MEGA's × 8, AE2's energy card refused) and its filters, which follow the cell out in clear-on-remove mode and
 * stay in keep-on-remove mode, a mode each opening of the menu starts again at clearing; the cell stays in the
 * item through a restart. Needs content/megacells (ae2/scripts/prepare-addon.ts megacells) and a client built
 * with ae2-megacells. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('megacells-portable', mode, { addons: ['megacells'] });
const CELL = 'megacells:portable_item_cell_1m', BENCH = 'megacells:portable_cell_workbench', CARD = 'megacells:greater_energy_card';
const STYLE = 'megacells/portable_cell_workbench.json';
const colours = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray', 'light_gray', 'cyan', 'purple', 'blue', 'brown', 'green'];
const kinds = colours.flatMap(c => [`minecraft:${c}_wool`, `minecraft:${c}_concrete`]).slice(0, 27);
const stored = (entries: [string, number][]) => '[' + entries.map(([id, n]) => `{"#t":"ae2:i",id:"${id}","#":${n}L}`).join(',') + ']';
const selected = async () => JSON.stringify(await fixture.command('data get entity @s SelectedItem'));
const number = (text: string, key: string) => {
  const match = text.match(new RegExp(`${key}\\\\?"?:\\s*(-?[0-9.E+-]+)d?`));
  assert(match, `no ${key} in ${text.slice(0, 600)}`); return Number(match[1]);
};
const shown = (view: any, id: string) => view.slots?.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === id);
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 20, y = Math.ceil(py) + 5, z = Math.floor(pz) + 20;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -3], [-3, 3], [6, -3], [6, 3]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(6, 4, 3)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(6, -1, 3)} minecraft:stone`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 0.5}`);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');
  const craft = async (grid: number[], items: string[], result: string) => {
    for (let i = 0; i < items.length; i++) await fixture.replace(`hotbar.${i}`, items[i]);
    for (let i = 0; i < items.length; i++) {
      await fixture.raw(`click-slot ${i}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
      await fixture.raw(`click-slot ${grid[i]}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
    }
    await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === result), `the recipe offers ${result}`);
    await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
    return until(() => fixture.raw('inventory'), s => s.counts?.[result] === 1 && !items.some(i => s.counts?.[i]), `${result} crafted from all its ingredients`);
  };

  // MEGA's shapeless cells/portable/portable_item_cell_1m in the inventory grid.
  await fixture.raw('open-inventory');
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 100), 'inventory crafting grid');
  await craft([100, 101, 102, 103], ['ae2:chest', 'megacells:cell_component_1m', 'ae2:dense_energy_cell', 'megacells:mega_item_cell_housing'], CELL);
  // MEGA's shaped network/portable_cell_workbench: a Cell Workbench over a Singularity over a MEGA Item Cell Housing.
  await fixture.command('clear @s');
  const table = at(0, 0, 2);
  await fixture.command(`setblock ${table} minecraft:crafting_table`);
  await fixture.raw(`interact ${table} north`);
  await until(() => fixture.raw('screen'), s => s.craft_size === 3, 'crafting table grid');
  await craft([101, 104, 107], ['ae2:cell_workbench', 'ae2:singularity', 'megacells:mega_item_cell_housing'], BENCH);
  await fixture.command(`setblock ${table} minecraft:air`);
  fixture.mark('MEGA\'s recipes craft a 1M Portable Item Cell and a Portable Cell Workbench');

  // A Charger on a creative energy cell's grid: getChargeRate (80 + 80 × cards) × 2 a tick, up to getAEMaxPower 20,000 × 8.
  await fixture.command('clear @s');
  const charger = at(3), source = at(4);
  await fixture.command(`setblock ${charger} ae2:charger[facing=north]`);
  await fixture.command(`setblock ${source} ae2:creative_energy_cell`);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 1.5}`);
  // ChargerBlock.useItemOn takes the held item in; useWithoutItem gives it back.
  const inCharger = async () => JSON.stringify(await fixture.command(`data get block ${charger}`));
  const charge = async (item: string, label: string) => {
    const id = item.split('[')[0];
    await fixture.replace('hotbar.1', item); await fixture.select(1, id);
    await fixture.raw(`interact ${charger} north`);
    await until(inCharger, text => text.includes(id), `${label} in the Charger`, 30000);
  };
  const energy = async () => {
    const text = await inCharger();
    return text.includes('ae2:stored_energy') ? number(text, 'ae2:stored_energy') : 0;
  };
  const takeOut = async (item: string) => {
    await fixture.replace('hotbar.8', 'minecraft:air'); await fixture.raw('hotbar 9');
    await fixture.raw(`interact ${charger} north`);
    await until(() => fixture.raw('inventory'), s => s.counts?.[item] >= 1, 'the cell back from the Charger', 30000);
  };
  await charge(CELL, 'the empty cell');
  await fixture.hold();
  const before = await energy();
  await fixture.stepServer(20);
  const gained = await energy() - before;
  await fixture.hold(false);
  assert(gained > 0 && gained <= 160 * 20 && gained % 160 === 0, `charged in steps of 160 AE a tick: ${gained}`);
  await takeOut(CELL);
  // Nearly full: it stops at 160,000, eight AE2 portable batteries; AE2's own 1K cell stops at 20,000.
  for (const [item, near, full] of [[`${CELL}[ae2:stored_energy=159900.0d]`, 159_900, 160_000], ['ae2:portable_item_cell_1k[ae2:stored_energy=19950.0d]', 19_950, 20_000]] as const) {
    await fixture.command('clear @s');
    await charge(item, item.split('[')[0]);
    assert((await until(energy, e => e === full, `${item.split('[')[0]} charges to ${full}`, 30000)) === full);
    await fixture.hold(); await fixture.stepServer(20); await fixture.hold(false);
    assert.equal(await energy(), full, `no AE past ${full} (from ${near})`);
    await takeOut(item.split('[')[0]);
  }
  fixture.mark(`a 1M Portable Item Cell charges at 160 AE a tick (${gained} AE in 20 ticks) up to 160,000 AE, AE2's 1K portable cell to 20,000`);

  // Open it (AbstractPortableCell.use): store a diamond, powered from the cell's battery.
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', `${CELL}[ae2:stored_energy=160000.0d]`); await fixture.replace('hotbar.1', 'minecraft:diamond', 3);
  await fixture.select(0, CELL); await fixture.raw('look 0 -80'); await fixture.raw('use-item');
  await until(() => fixture.raw('screen'), v => v.screen === 'container' && v.slots?.some((s: any) => s.slot >= 300), 'portable M cell menu');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:diamond', 'diamonds on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => shown(v, 'minecraft:diamond')?.count === 3 && !v.carried, 'three diamonds stored');
  let data = await until(selected, d => d.includes('minecraft:diamond'), 'the cell item holds the diamonds');
  assert(number(data, 'ae2:stored_energy') < 160_000, 'the transfer and the idle drain came out of the cell');
  await fixture.raw('close');
  fixture.mark('the cell opens like AE2\'s portable cells and stores what is put in, powered from its battery');

  // 18 + (6 - 5) × 9 = 27 types: a 28th is refused, an existing type is not.
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', `${CELL}[ae2:stored_energy=160000.0d,ae2:storage_cell_inv=${stored(kinds.map(k => [k, 1]))}]`);
  await fixture.replace('hotbar.1', 'minecraft:diamond'); await fixture.replace('hotbar.2', kinds[0]);
  await fixture.select(0, CELL); await fixture.raw('use-item');
  await until(() => fixture.raw('screen'), v => kinds.every(k => shown(v, k)), 'all 27 types shown');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:diamond', 'a 28th type on the cursor');
  await fixture.raw('click-slot 330');
  // The command follows the click on the same connection: the server has answered it.
  assert(!(await selected()).includes('minecraft:diamond'), 'a 28th type does not fit');
  let view = await fixture.raw('screen');
  assert(!shown(view, 'minecraft:diamond') && view.carried?.item === 'minecraft:diamond', 'the diamond stays on the cursor');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => !s.carried, 'diamond back');
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => s.carried?.item === kinds[0], 'a stored type on the cursor');
  await fixture.raw('click-slot 330');
  await until(() => fixture.raw('screen'), v => shown(v, kinds[0])?.count === 2 && !v.carried, 'an existing type still goes in');
  await fixture.raw('close');
  // Half the tier's bytes, a 128th of them for each type: 4,128,767 cobblestone leave room for one.
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', `${CELL}[ae2:stored_energy=160000.0d,ae2:storage_cell_inv=${stored([['minecraft:cobblestone', 4_128_767]])}]`);
  await fixture.replace('hotbar.1', 'minecraft:cobblestone', 2);
  await fixture.select(0, CELL); await fixture.raw('use-item');
  await until(() => fixture.raw('screen'), v => shown(v, 'minecraft:cobblestone'), 'the full cell open');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.count === 2, 'two cobblestone on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => v.carried?.count === 1, 'one of two accepted');
  await fixture.raw('click-slot 1'); await fixture.raw('close');
  assert.equal(number(await selected(), '#'), 4_128_768, 'the cell holds 4,128,768 cobblestone, (524,288 - 8,192) × 8');
  fixture.mark('a 1M Portable Item Cell holds 27 types and 4,128,768 items of one type');

  // Sneak-use an empty cell (AbstractPortableCell.disassembleDrive): MEGA's disassembly recipe, the energy into the Dense Energy Cell.
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', `${CELL}[ae2:stored_energy=100000.0d]`); await fixture.select(0, CELL);
  await fixture.raw('key shift down');
  try { await fixture.raw('use-item'); } finally { await fixture.raw('key shift up'); }
  const parts = await until(() => fixture.raw('inventory'), s => !s.counts?.[CELL] && s.counts?.['ae2:dense_energy_cell'] === 1, 'the cell taken apart');
  for (const part of ['megacells:cell_component_1m', 'megacells:mega_item_cell_housing', 'ae2:chest']) assert.equal(parts.counts?.[part], 1, part);
  await fixture.select(parts.hotbar.findIndex((s: any) => s?.item === 'ae2:dense_energy_cell'), 'ae2:dense_energy_cell');
  assert.equal(number(await selected(), 'ae2:stored_energy'), 100_000, 'the Dense Energy Cell holds the portable cell\'s 100,000 AE');
  fixture.mark('an empty cell disassembles by MEGA\'s recipe and its energy goes into the Dense Energy Cell');

  // The Portable Cell Workbench: AE2's Cell Workbench screen on the cell it holds.
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', BENCH); await fixture.replace('hotbar.1', CELL);
  await fixture.replace('hotbar.2', CARD, 2); await fixture.replace('hotbar.3', 'ae2:energy_card'); await fixture.replace('hotbar.4', 'minecraft:diamond');
  const open = async () => {
    await fixture.select(0, BENCH); await fixture.raw('use-item');
    return (await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === STYLE && v.ae2_workbench, 'Portable Cell Workbench screen')).ae2_workbench;
  };
  let work = await open();
  assert.equal(work.keep, false); assert(work.config.every((c: any) => !c.open), 'no filters without a cell');
  await fixture.raw('click-slot 1 shift');
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[0].open && v.ae2_workbench.upgrades === 4, 'the cell in the workbench, its four card slots')).ae2_workbench;
  // ItemMenuHostMixin: the menu's cards are the held cell's own.
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => s.carried?.count === 2, 'two Greater Energy Cards on the cursor');
  await fixture.raw('click-slot 301'); await until(() => fixture.raw('inventory'), s => s.carried?.count === 1, 'one card in');
  await fixture.raw('click-slot 302'); await until(() => fixture.raw('inventory'), s => !s.carried, 'the second card in');
  await fixture.raw('click-slot 3'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:energy_card', 'AE2\'s energy card on the cursor');
  await fixture.raw('click-slot 303');
  await fixture.raw('click-slot 3'); await until(() => fixture.raw('inventory'), s => !s.carried && s.hotbar[3]?.item === 'ae2:energy_card', 'AE2\'s energy card refused');
  data = await selected();
  assert.equal(number(data, 'ae2:energy_capacity'), 2_580_000, 'the cell inside the workbench: 20,000 × (1 + 16 × 8), before MEGA\'s × 8');
  // A filter, by its place on the screen.
  await fixture.raw('click-slot 4');
  await fixture.raw(`click-at ${centre(work.config[5].rect)}`);
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[5].item === 'minecraft:diamond', 'filter 5 set')).ae2_workbench;
  await fixture.raw('click-slot 4'); await until(() => fixture.raw('inventory'), s => !s.carried, 'diamond back');
  // Clear on remove (the default): the filters go out with the cell.
  await fixture.raw('click-slot 300'); await until(() => fixture.raw('inventory'), s => s.carried?.item === CELL, 'the cell on the cursor');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => !s.carried && s.hotbar[1]?.item === CELL, 'the cell in the hotbar');
  await until(() => fixture.raw('screen'), v => v.ae2_workbench && v.ae2_workbench.config.every((c: any) => !c.item && !c.open), 'the workbench clears its filters');
  await fixture.raw('close');
  await fixture.select(1, CELL);
  data = await selected();
  assert(data.includes(CARD) && data.includes('minecraft:diamond'), 'the cell keeps its two cards and its diamond filter');
  assert.equal(number(data, 'ae2:energy_capacity'), 2_580_000);
  fixture.mark('the Portable Cell Workbench installs two Greater Energy Cards and a filter on a held cell, refuses AE2\'s energy card, and clears its filters when the cell goes out');

  // Keep on remove: the filters stay with the workbench; a new menu starts clearing again (its IConfigManager is new).
  work = await open();
  await fixture.raw('click-slot 1 shift');
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[5].item === 'minecraft:diamond', 'the cell\'s filter copied into the workbench')).ae2_workbench;
  await fixture.raw(`click-at ${centre(work.tools[work.tools.length - 1].rect)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_workbench?.keep === true, 'copy mode: keep on remove');
  await fixture.raw('click-slot 300'); await fixture.raw('click-slot 1');
  await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[5].item === 'minecraft:diamond' && !v.carried, 'the filter stays without the cell');
  await fixture.raw('close');
  work = await open();
  assert.equal(work.keep, false, 'a new menu starts in clear-on-remove mode');
  assert((await selected()).includes('minecraft:diamond'), 'the workbench item still holds the filter');
  // The cell back in for the restart.
  await fixture.raw('click-slot 1 shift');
  await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot === 300 && s.item === CELL), 'the cell in the workbench again');
  await fixture.raw('close');
  fixture.mark('keep-on-remove keeps the filters in the workbench item; each menu opens clearing on removal');

  await fixture.restart();
  await fixture.select(0, BENCH);
  data = await selected();
  assert(data.includes(CELL) && data.includes(CARD), 'the workbench holds its cell and the cell its cards after the restart');
  work = await open();
  await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot === 300 && s.item === CELL) && v.ae2_workbench?.config[5].item === 'minecraft:diamond', 'reopened with its cell and filter');
  await fixture.raw('close');
  fixture.mark('the cell, its cards and its filter stay in the Portable Cell Workbench through a restart');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
