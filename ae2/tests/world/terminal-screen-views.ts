/** MEStorageScreen's rows past the menu's places, fluid entries, the shift-wheel, the settings screen and the encoding panel's craftable mark. */
import assert from 'node:assert/strict';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('terminal-screen-views', mode);
const config = join(fixture.game, 'config', 'ae2-client.toml');
const shots = process.env.AE2_TERM_SHOTS;
const shot = async (name: string) => { if (shots) { await fixture.raw(`screenshot ${join(shots, name + '.png')}`); await Bun.sleep(700); } };
const items = ['oak_log', 'birch_log', 'spruce_log', 'jungle_log', 'acacia_log', 'dark_oak_log', 'mangrove_log', 'cherry_log', 'stone', 'granite', 'diorite', 'andesite', 'deepslate', 'calcite', 'tuff', 'dirt', 'sand', 'gravel', 'clay', 'coal', 'iron_ingot', 'gold_ingot', 'copper_ingot', 'emerald', 'diamond', 'redstone', 'lapis_lazuli', 'quartz', 'stick', 'paper', 'book', 'leather', 'string', 'feather', 'flint', 'bone', 'gunpowder', 'wheat', 'carrot', 'potato', 'beetroot', 'apple', 'melon_slice', 'sugar', 'egg', 'cactus', 'bamboo', 'kelp', 'vine', 'lily_pad', 'oak_planks', 'birch_planks', 'spruce_planks', 'jungle_planks', 'acacia_planks', 'dark_oak_planks', 'mangrove_planks', 'cherry_planks', 'glass', 'bricks', 'obsidian', 'netherrack', 'soul_sand', 'glowstone', 'ice', 'snow', 'sponge', 'wool', 'white_wool', 'orange_wool', 'magenta_wool', 'light_blue_wool', 'yellow_wool', 'lime_wool', 'pink_wool', 'gray_wool', 'cyan_wool', 'purple_wool', 'blue_wool', 'brown_wool', 'green_wool', 'red_wool', 'black_wool'].filter(id => id !== 'wool');
const cell = (ids: string[]) => `ae2:item_storage_cell_64k[ae2:storage_cell_inv=[${ids.map(id => `{"#t":"ae2:i",id:"minecraft:${id}","#":10L}`).join(',')}]]`;
const terminalOf = (view: any) => view.ae2_terminal;
const widget = (view: any, name: string) => terminalOf(view).widgets.find((w: any) => w.widget === name);
const middle = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
const carried = async () => (await fixture.raw('inventory')).carried;

try {
  mkdirSync(join(fixture.game, 'config'), { recursive: true });
  // At this window's 270 GUI pixels only without AE2's margin does FULL pass the menu's five rows.
  writeFileSync(config, '[terminals]\nterminalStyle = "FULL"\nterminalMargin = 0\n');
  const state = await fixture.start();
  const [px, py, pz] = state.player.position, x = Math.floor(px), y = Math.ceil(py) + 3, z = Math.floor(pz);
  const at = (dx: number) => `${x + dx} ${y} ${z}`, power = at(0), drive = at(1), terminal = at(2), provider = at(3);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + 6} ${y - 1} ${z + 3} minecraft:bedrock`);
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + 6} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command('clear @s');
  for (const [pos, id] of [[power, 'creative_energy_cell'], [drive, 'drive'], [terminal, 'cable_bus'], [provider, 'pattern_provider']] as const) {
    await fixture.command(`setblock ${pos} ae2:${id}`); await fixture.ae(`grid status ${pos}`);
  }
  for (let i = 0; i * 40 < items.length; i++) await fixture.command(`item replace block ${drive} container.${i} with ${cell(items.slice(i * 40, (i + 1) * 40))}`);
  await fixture.command(`item replace block ${drive} container.9 with ae2:fluid_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:f",id:"minecraft:water","#":2500L}]]`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  await fixture.replace('hotbar.0', 'ae2:pattern_encoding_terminal'); await fixture.select(0, 'ae2:pattern_encoding_terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} south`)).attached, true);
  await until(() => fixture.ae(`grid status ${terminal}`), view => view.online, 'powered terminal');
  // A pattern for planks, so the encoding panel can mark them craftable.
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  assert.equal((await fixture.ae('crafting encode minecraft:oak_log - - - - - - - -')).encoded, true);
  await until(() => fixture.raw('inventory'), view => view.hotbar?.[0]?.item === 'ae2:crafting_pattern', 'pattern encoded');
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 0.5}`); await fixture.raw(`interact ${provider} east`);
  const providerMenu = await until(() => fixture.raw('screen'), view => view.slots?.some((slot: any) => slot.slot < 36 && slot.item === 'ae2:crafting_pattern'), 'provider menu');
  await fixture.raw(`click-slot ${providerMenu.slots.find((slot: any) => slot.slot < 36 && slot.item === 'ae2:crafting_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), view => !view.counts?.['ae2:crafting_pattern'], 'pattern in the provider'); await fixture.raw('close');
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${terminal} north`);

  // The rows follow the terminal style, past the menu's five.
  const full = await until(() => fixture.raw('screen'), view => terminalOf(view)?.entries?.some((e: any) => e.extra), 'FULL style shows rows past the menu places');
  const rows = terminalOf(full).rows;
  assert(rows > 5, `FULL has more rows than the menu's five: ${rows}`);
  const filled = terminalOf(await until(() => fixture.raw('screen'), view => terminalOf(view).entries.length === rows * 9, 'every shown row has its entries'));
  assert.equal(filled.entries.filter((e: any) => e.extra).length, (rows - 5) * 9, 'the rows past the places travel in the view');
  await shot('terminal-full-rows');

  // A click on such a row reaches its entry.
  const entry = filled.entries.find((e: any) => e.extra && !e.fluid);
  await fixture.raw(`click-at ${middle(entry.rect)}`);
  await until(carried, stack => stack?.item === entry.item && stack.count === 10, 'click on an extra row picks up its entry');
  await fixture.raw(`click-at ${middle(entry.rect)}`);
  await until(carried, stack => !stack, 'click on an extra row puts the carried stack back');
  await fixture.raw(`click-at ${middle(entry.rect)} right`);
  await until(carried, stack => stack?.item === entry.item && stack.count === 5, 'right click takes half the most it can');
  await fixture.raw(`click-at ${middle(entry.rect)}`);
  await until(carried, stack => !stack, 'the half goes back');

  // Shift-wheel: down one out of the network, up one back in.
  const placed = filled.entries.find((e: any) => !e.extra && !e.fluid);
  await fixture.raw(`scroll-at ${middle(placed.rect)} -2 shift`);
  await until(carried, stack => stack?.item === placed.item && stack.count === 2, 'shift-wheel down rolls items onto the cursor');
  await fixture.raw(`scroll-at ${middle(placed.rect)} 1 shift`);
  await until(carried, stack => stack?.item === placed.item && stack.count === 1, 'shift-wheel up rolls one item back');
  await fixture.raw(`scroll-at ${middle(entry.rect)} 1 shift`);
  await until(carried, stack => !stack, 'over any entry the wheel rolls the carried item in');

  // The wheel scrolls by rows, and the view's entries follow.
  const total = filled.total;
  assert(total > rows, 'more rows than shown');
  await fixture.raw(`scroll-at ${middle(placed.rect)} -${total + 4}`);
  const end = terminalOf(await until(() => fixture.raw('screen'), view => terminalOf(view).row === total - rows && terminalOf(view).view_row === total - rows, 'the wheel reaches the last row'));
  assert.notEqual(end.entries[0].item, filled.entries[0].item, 'scrolled entries differ');

  // A fluid entry draws its fluid, in buckets.
  await fixture.raw(`scroll-at ${middle(placed.rect)} ${total + 4}`);
  await until(() => fixture.raw('screen'), view => terminalOf(view).row === 0 && terminalOf(view).view_row === 0, 'the wheel returns to the top');
  let water: any;
  for (let row = 0; !water; row += rows) {
    assert(row < total, 'the water entry is somewhere in the rows');
    const shown = terminalOf(await until(() => fixture.raw('screen'), view => terminalOf(view).view_row === Math.min(row, total - rows), 'scrolled to the next rows'));
    water = shown.entries.find((e: any) => e.fluid);
    if (!water) await fixture.raw(`scroll-at ${middle(placed.rect)} -${rows}`);
  }
  assert.equal(water.item, 'minecraft:water_bucket');
  await shot('terminal-fluid');

  // The cog: its sub screen, a setting saved, the way back.
  const cog = widget(await fixture.raw('screen'), 'Settings');
  assert(cog, 'the toolbar has the settings button');
  await fixture.raw(`click-at ${middle(cog.rect)}`);
  const settings = terminalOf(await until(() => fixture.raw('screen'), view => terminalOf(view)?.sub === 'Settings', 'the cog opens the settings'));
  const check = (from: any, name: string) => from.checks.find((c: any) => c.check === name);
  assert.equal(check(settings, 'PinAutoCrafted').selected, true);
  assert.equal(check(settings, 'UseExternalSearch').active, false, 'no external item list');
  assert.equal(check(settings, 'ClearExternal'), undefined, 'external options hide while AE search is chosen');
  await shot('terminal-settings');
  await fixture.raw(`click-at ${middle(check(settings, 'PinAutoCrafted').rect)}`);
  await until(() => fixture.raw('screen'), view => check(terminalOf(view), 'PinAutoCrafted').selected === false, 'the checkbox changes');
  assert.match(readFileSync(config, 'utf8'), /pinAutoCraftedItems = false/);
  assert.match(readFileSync(config, 'utf8'), /terminalStyle = "FULL"/, 'other settings stay');
  await fixture.raw(`click-at ${middle(settings.back)}`);
  await until(() => fixture.raw('screen'), view => terminalOf(view)?.kind === 'Terminal' && !terminalOf(view).sub, 'back returns to the terminal');
  await fixture.raw('close');

  // PatternEncodingTermScreen marks an input the grid has a pattern for.
  await fixture.replace('hotbar.0', 'minecraft:oak_planks');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${terminal} south`);
  await until(() => fixture.raw('screen'), view => terminalOf(view)?.kind === 'Encoding', 'the encoding terminal opens');
  await fixture.raw('click-slot 0');
  await until(carried, stack => stack?.item === 'minecraft:oak_planks', 'planks on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), view => terminalOf(view).craftable_inputs?.includes(0), 'the input with a known pattern is marked craftable');
  await shot('encoding-craftable');
  await fixture.command('clear @s');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.mark('rows past the menu places click and scroll; fluid entries, shift-wheel, the settings screen and the craftable mark work');
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
