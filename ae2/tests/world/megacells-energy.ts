/** MEGA Cells slice 3: the Superdense Energy Cell and the Greater Energy Card. MEGA's recipes make both; the
 * cell (EnergyCellBlock(12800000, 3200, 12800)) holds and takes up to 12,800,000 AE, fills after AE2's dense
 * cell and drains before it, shows its fill level as a block and as an item, and keeps its energy when broken
 * and placed again, through a restart; the card (EnergyCardItem(p, 8)) multiplies a portable cell's and a
 * matter cannon's battery as eight of AE2's energy cards would, two at most of each card. Needs content/megacells
 * (ae2/scripts/prepare-addon.ts megacells) and a client built with ae2-megacells. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { inflateSync } from 'node:zlib';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('megacells-energy', mode, { addons: ['megacells'] });
const CELL = 'megacells:mega_energy_cell', CARD = 'megacells:greater_energy_card';
const MAX = 12_800_000;
/** An 8-bit RGBA PNG screenshot, decoded with Node's zlib as dynamic-item-models does. */
function screenshot(path: string) {
  const png = readFileSync(path); assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  let width = 0, height = 0; const chunks: Buffer[] = [];
  for (let p = 8; p + 12 <= png.length;) {
    const n = png.readUInt32BE(p), type = png.toString('ascii', p + 4, p + 8), body = png.subarray(p + 8, p + 8 + n);
    if (type === 'IHDR') { width = body.readUInt32BE(0); height = body.readUInt32BE(4); assert.deepEqual([...body.subarray(8, 13)], [8, 6, 0, 0, 0]); }
    if (type === 'IDAT') chunks.push(body); p += n + 12; if (type === 'IEND') break;
  }
  const source = inflateSync(Buffer.concat(chunks)), stride = width * 4, pixels = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = source[y * (stride + 1)], start = y * stride;
    for (let x = 0; x < stride; x++) {
      const left = x >= 4 ? pixels[start + x - 4] : 0, up = y ? pixels[start + x - stride] : 0, corner = y && x >= 4 ? pixels[start + x - stride - 4] : 0;
      const paeth = () => { const p = left + up - corner, a = Math.abs(p - left), b = Math.abs(p - up), c = Math.abs(p - corner); return a <= b && a <= c ? left : b <= c ? up : corner; };
      const predictor = filter === 0 ? 0 : filter === 1 ? left : filter === 2 ? up : filter === 3 ? Math.floor((left + up) / 2) : filter === 4 ? paeth() : NaN;
      assert(Number.isFinite(predictor), 'supported PNG row filter'); pixels[start + x] = (source[y * (stride + 1) + 1 + x] + predictor) & 255;
    }
  }
  return { width, height, pixels };
}
const component = (reply: any, key: string) => {
  const match = JSON.stringify(reply).match(new RegExp(`${key}\\\\?"?:\\s*(-?[0-9.E+-]+)d?`));
  assert(match, `no ${key} in ${JSON.stringify(reply).slice(0, 400)}`); return Number(match[1]);
};
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-4, -4], [-4, 4], [16, -4], [16, 4]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-4, 0, -4)} ${at(16, 5, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-4, -1, -4)} ${at(16, -1, 4)} minecraft:stone`);
  const block = (pos: string) => fixture.raw(`block ${pos}`);
  const power = (pos: string) => fixture.ae(`power ${pos} east`);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');

  // MEGA's shaped recipe in a crafting table: eight Dense Energy Cells around an Accumulation Processor.
  const table = at(0, 0, 2);
  await fixture.command(`setblock ${table} minecraft:crafting_table`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 0.5}`);
  for (let i = 0; i < 8; i++) await fixture.replace(`hotbar.${i}`, 'ae2:dense_energy_cell');
  await fixture.replace('hotbar.8', 'megacells:accumulation_processor');
  await fixture.raw(`interact ${table} north`);
  await until(() => fixture.raw('screen'), s => s.craft_size === 3 && s.slots?.some((slot: any) => slot.slot === 108), 'crafting table grid');
  const grid = [100, 101, 102, 103, 105, 106, 107, 108, 104];
  for (let i = 0; i < 9; i++) {
    await fixture.raw(`click-slot ${i}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${grid[i]}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === CELL), 'the recipe offers a Superdense Energy Cell');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  const crafted = await until(() => fixture.raw('inventory'), s => s.counts?.[CELL] === 1, 'the crafted cell is in the inventory');
  assert(!crafted.counts?.['ae2:dense_energy_cell'] && !crafted.counts?.['megacells:accumulation_processor'], 'all nine ingredients were used');
  await fixture.command(`setblock ${table} minecraft:air`);
  fixture.mark('MEGA\'s shaped recipe crafts a Superdense Energy Cell from eight Dense Energy Cells and an Accumulation Processor');

  // MEGA's shapeless recipe in the inventory grid: an Energy Card and a Superdense Energy Cell.
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', 'ae2:energy_card'); await fixture.replace('hotbar.1', CELL);
  await fixture.raw('open-inventory');
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 100), 'inventory crafting grid');
  for (const [from, to] of [[0, 100], [1, 101]]) {
    await fixture.raw(`click-slot ${from}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${to}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === CARD), 'the recipe offers a Greater Energy Card');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  await until(() => fixture.raw('inventory'), s => s.counts?.[CARD] === 1 && !s.counts?.[CELL] && !s.counts?.['ae2:energy_card'], 'the crafted card');
  fixture.mark('MEGA\'s shapeless recipe crafts a Greater Energy Card from an Energy Card and a Superdense Energy Cell');

  // Placed from an item (EnergyCellBlockEntity.importSettings): StoredEnergyAmount.setStored on MEGA's maximum.
  await fixture.command('clear @s');
  const support = at(0, 0, -2), placed = at(0, 1, -2);
  await fixture.command(`setblock ${support} minecraft:stone`);
  // Close enough to pick up what the broken cell drops.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z - 0.5}`);
  const place = async (stored: string, label: string) => {
    await fixture.replace('hotbar.0', `${CELL}[ae2:stored_energy=${stored}]`); await fixture.select(0, CELL);
    await fixture.raw(`interact ${support} up`);
    await until(() => block(placed), b => b.block === CELL, label, 30000);
    return until(() => power(placed), p => typeof p.energy === 'number', `${label}: its block entity`, 30000);
  };
  const over = await place('20000000.0d', 'an overfilled cell placed');
  assert.equal(over.energy, MAX, 'setStored clamps to the Superdense maximum, past the dense cell\'s 1,600,000');
  assert.equal((await until(() => power(placed), p => p.properties?.fullness === '4', 'full cell state')).properties.fullness, '4');
  await fixture.command(`setblock ${placed} minecraft:air`);
  const partial = await place('12000000.0d', 'a 12M cell placed');
  assert.equal(partial.energy, 12_000_000);
  // getStorageLevelFromFillFactor: floor(4 × (12/12.8 + 0.01)) = 3, where AE2's own cells would show 4.
  await until(() => power(placed), p => p.properties?.fullness === '3', 'the block shows MEGA\'s fill level, 3 of 4');
  fixture.mark('a placed cell holds 12,000,000 AE (fullness 3) and clamps 20,000,000 to 12,800,000 (fullness 4)');

  // Broken by the player (AEBaseEntityBlock.getDrops exports the stored energy), then placed again.
  await fixture.raw(`break ${placed}`);
  await until(() => block(placed), b => b.block === 'minecraft:air', 'the cell is broken', 30000);
  const inv = await until(() => fixture.raw('inventory'), s => s.counts?.[CELL] === 1, 'the broken cell is picked up', 30000);
  const slot = inv.hotbar.findIndex((item: any) => item?.item === CELL);
  await fixture.select(slot, CELL);
  assert.equal(component(await fixture.command('data get entity @s SelectedItem'), 'ae2:stored_energy'), 12_000_000, 'the dropped cell carries its energy');
  await fixture.raw(`interact ${support} up`);
  await until(() => block(placed), b => b.block === CELL, 'the cell placed again', 30000);
  assert.equal((await until(() => power(placed), p => p.energy === 12_000_000, 'its energy restored', 30000)).energy, 12_000_000);
  fixture.mark('a broken cell drops with its 12,000,000 AE and holds them again when placed');

  // A grid: AE2's dense cell (priority 1,600) and MEGA's (12,800) behind an Energy Acceptor.
  // EnergyService injects into the lowest priority first and extracts from the highest first.
  await fixture.command('gamemode creative @s');
  const generator = at(8), acceptor = at(9), dense = at(10), mega = at(11), drive = at(12);
  await fixture.hold();
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`);
  await fixture.command(`setblock ${dense} ae2:dense_energy_cell`); await fixture.command(`setblock ${mega} ${CELL}`);
  await fixture.command(`data merge block ${dense} {ae2_lab_energy:1599990.0d}`);
  await fixture.command(`data merge block ${mega} {ae2_lab_energy:12799990.0d}`);
  await fixture.stepServer(1);
  assert.deepEqual([(await power(dense)).energy, (await power(mega)).energy], [1_599_990, 12_799_990]);
  await fixture.command(`setblock ${generator} ae2:debug_energy_gen`);
  let charged = 0, seenDenseFull = false;
  for (let tick = 0; tick < 400 && charged < MAX; tick++) {
    await fixture.stepServer(1);
    const d = (await power(dense)).energy; charged = (await power(mega)).energy;
    if (d < 1_600_000) assert.equal(charged, 12_799_990, 'MEGA\'s cell waits while the lower-priority dense cell fills');
    else seenDenseFull = true;
    assert(charged <= MAX);
  }
  assert(seenDenseFull, 'the dense cell filled first');
  assert.equal(charged, MAX, 'the Superdense cell charges up to 12,800,000 AE');
  await fixture.stepServer(5);
  assert.deepEqual([(await power(dense)).energy, (await power(mega)).energy], [1_600_000, MAX], 'no AE past either maximum');
  fixture.mark('injection fills AE2\'s dense cell before MEGA\'s, which charges to exactly 12,800,000 AE');
  await fixture.command(`setblock ${generator} minecraft:air`);
  await fixture.command(`setblock ${drive} ae2:drive`);
  // The tick the drive joins, the port's fallback grid drain may take from the cells in position order;
  // from then on the energy service's extraction is what draws.
  await fixture.stepServer(3);
  const [denseBefore, megaBefore] = [(await power(dense)).energy, (await power(mega)).energy];
  await fixture.stepServer(20);
  const megaAfter = (await power(mega)).energy;
  assert(megaAfter < megaBefore, 'the drive\'s idle drain comes out of MEGA\'s cell');
  assert.equal((await power(dense)).energy, denseBefore, 'MEGA\'s higher-priority cell gives its energy first');
  // Without the drive nothing draws any more: what is left stays through the restart.
  await fixture.command(`setblock ${drive} minecraft:air`);
  await fixture.stepServer(1);
  const drained = (await power(mega)).energy;
  fixture.mark(`extraction drains MEGA's cell first: ${megaBefore - megaAfter} AE in 20 ticks, the dense cell untouched`);
  await fixture.hold(false);

  // The Greater Energy Card in a Cell Workbench: a portable cell's battery 20,000 × (1 + 8 × multiplier).
  await fixture.command('gamemode survival @s'); await fixture.command('clear @s');
  const benches = [at(4, 0, 2), at(6, 0, 2)];
  for (const pos of benches) await fixture.command(`setblock ${pos} ae2:cell_workbench`);
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 0.5}`);
  // The cell goes into the workbench's cell slot by hand, as a player puts it.
  const load = async (bench: string, cell: string) => {
    await fixture.replace('hotbar.1', cell);
    await fixture.raw(`interact ${bench} north`);
    await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot === 300), 'Cell Workbench opens');
    await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), v => v.carried?.item === cell, 'the cell on the cursor');
    await fixture.raw('click-slot 300');
    await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot === 300 && s.item === cell), 'the cell in the workbench');
    await fixture.raw('close');
  };
  const install = async (bench: string, card: string) => {
    await fixture.replace('hotbar.0', card); await fixture.select(0, card);
    await fixture.ae(`workbench install ${bench}`);
    return component(await fixture.command(`data get block ${bench}`), 'ae2:energy_capacity');
  };
  const [bench, cannonBench] = benches;
  await load(bench, 'ae2:portable_item_cell_1k');
  assert.equal(await install(bench, CARD), 1_300_000, 'one Greater Energy Card: 20,000 × (1 + 8 × 8)');
  assert.equal(await install(bench, CARD), 2_580_000, 'two: 20,000 × (1 + 16 × 8)');
  assert.deepEqual((await fixture.ae(`workbench status ${bench}`)).upgrades.map((u: any) => u.item), [CARD, CARD]);
  await fixture.ae(`workbench remove ${bench} 1`);
  assert.equal(await install(bench, 'ae2:energy_card'), 1_460_000, 'a Greater and an AE2 Energy Card: 20,000 × (1 + 9 × 8)');
  fixture.mark('Greater Energy Cards multiply a portable cell battery by 65 and 129, with an AE2 Energy Card by 73');
  // A Matter Cannon has four card slots: two of each energy card, 2 + 16 = 18, 200,000 × (1 + 18 × 8).
  await load(cannonBench, 'ae2:matter_cannon');
  for (const card of [CARD, CARD, 'ae2:energy_card']) await install(cannonBench, card);
  assert.equal(await install(cannonBench, 'ae2:energy_card'), 29_000_000);
  fixture.mark('a Matter Cannon with two Greater and two AE2 Energy Cards holds 29,000,000 AE');

  await fixture.restart();
  assert.equal((await until(() => power(placed), p => p.energy === 12_000_000, 'the placed cell after restart', 30000)).energy, 12_000_000);
  assert.equal((await power(mega)).energy, drained);
  assert.equal((await power(dense)).energy, denseBefore);
  fixture.mark('the cells keep their energy through a restart');

  // The item shows its fill level (EnergyCellBlockItem's ae2:fill_level): empty, a 6% and a half charge,
  // and two full cells, in the hotbar of the inventory screen.
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');
  const charges = ['', '[ae2:stored_energy=800000.0d]', '[ae2:stored_energy=6400000.0d]', '[ae2:stored_energy=12800000.0d]', '[ae2:stored_energy=12800000.0d]'];
  for (const [i, charge] of charges.entries()) await fixture.replace(`hotbar.${i}`, CELL + charge);
  await fixture.raw('open-inventory');
  const ui = await until(() => fixture.raw('ui'), u => u.screen === 'inventory' && u.widgets?.some((w: any) => w.id === 'slot-4'), 'inventory widgets');
  const imagePath = join(fixture.run, 'mega-energy-items.png'); await fixture.raw(`screenshot ${imagePath}`);
  const image = screenshot(imagePath), scale = Math.round(image.width / ui.gui_size[0]);
  const crop = (index: number) => {
    const node = ui.widgets.find((w: any) => w.id === `slot-${index}`); assert(node?.bounds, 'visible hotbar slot');
    const out: number[] = [];
    for (let v = 0; v < 16; v++) for (let u = 0; u < 16; u++) {
      const sx = Math.floor((node.bounds[0] + u + 0.5) * scale), sy = Math.floor((node.bounds[1] + v + 0.5) * scale);
      const offset = (sy * image.width + sx) * 4; out.push(...image.pixels.subarray(offset, offset + 4));
    }
    return out;
  };
  const differ = (a: number[], b: number[]) => a.reduce((n, v, i) => n + (i % 4 !== 3 && v !== b[i] ? 1 : 0), 0);
  assert.deepEqual(crop(0), crop(1), 'a 6% charge draws the empty cell (fill_level below 0.2)');
  assert(differ(crop(0), crop(2)) > 0, 'a half charge draws another model');
  assert(differ(crop(2), crop(3)) > 0, 'a full cell draws another model again');
  assert.deepEqual(crop(3), crop(4), 'two full cells draw alike');
  await fixture.raw('close');
  fixture.mark('the item draws MEGA\'s fill-level models: empty below a fifth, half, full');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
