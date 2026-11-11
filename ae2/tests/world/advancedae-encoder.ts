/** Advanced AE slice 1b: the Advanced Pattern Encoder's screen (AdvPatternEncoderScreen over AdvPatternEncoderMenu),
 * its hotkey, the Small Advanced Pattern Provider's Filtered Import, the pattern's tooltip and its re-encoding in AE2's
 * Pattern Encoding Terminal. The encoder, used in the air, opens its screen; AE2's processing pattern of raw iron and
 * coal put in its input shows an Advanced one in its output and a row per input; the rows' direction buttons send the
 * raw iron up; taking the output takes the input. The pattern's tooltip names the raw iron's face. The hotkey opens
 * the encoder from the inventory and closes it; what is left in it stays in the item. The provider's fourth toolbar
 * button turns Filtered Import on: a hopper above it then puts in only the iron ingot its pattern makes, not the
 * cobblestone, and the player's shift-click puts in no cobblestone either; turned off, the cobblestone goes too. AE2's
 * terminal, re-encoding the Advanced pattern, keeps it Advanced with its directions. The setting survives a restart.
 * Needs content/advanced_ae (ae2/scripts/prepare-addon.ts advancedae) and a client built
 * with ae2-advancedae. */
import assert from 'node:assert/strict';
import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('advancedae-encoder', mode, { addons: ['advanced_ae'] });
// The encoder's hotkey, unbound by default (AAEHotkeysRegistry.Keys.PATTERN_ENCODER_HOTKEY).
appendFileSync(join(fixture.game, 'options.txt'), 'version:5023\nkey_key.advanced_ae.pattern_encoder_action:key.keyboard.k\n');
const PROVIDER = 'advanced_ae:small_adv_pattern_provider', PATTERN = 'advanced_ae:adv_processing_pattern', ENCODER = 'advanced_ae:adv_pattern_encoder';
const ae2Pattern = 'ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"minecraft:raw_iron","#":1L},{"#t":"ae2:i",id:"minecraft:coal","#":1L}],sparseOutputs:[{"#t":"ae2:i",id:"minecraft:iron_ingot","#":1L}]}]';
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slot = (view: any, index: number) => view.slots?.find((s: any) => s.slot === index);
const encoder = (view: any) => view.addon_screen?.advae_encoder;
const rows = (view: any) => (encoder(view)?.rows ?? []).map((r: any) => [r.id, r.direction]);
const chat = (reply: any) => (reply.chat ?? []).join('\n');
// `/ae2 advae show` of the Advanced pattern in the inventory, once the server holds it where the client shows it (a
// shift-click's slot on the client is a prediction until the server's inventory comes).
const showPattern = () => until(async () => {
  const inventory = await fixture.raw('inventory');
  const at = [...(inventory.hotbar ?? []), ...(inventory.main ?? [])].find((s: any) => s?.item === PATTERN)?.slot;
  return at === undefined ? null : fixture.ae(`advae show ${at}`).catch(() => null);
}, shown => shown !== null, 'the Advanced pattern in the server\'s inventory');
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 28, y = Math.ceil(py) + 5, z = Math.floor(pz) + 28;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -4], [-3, 4], [6, -4], [6, 4]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -4)} ${at(6, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -4)} ${at(6, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s');
  // One grid: power, a drive with an empty cell, AE2's Pattern Encoding Terminal on a cable, the Advanced provider with a
  // hopper above it, pointing down into it.
  const power = at(0), drive = at(1), terminal = at(2), provider = at(3), hopper = at(3, 1);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [provider, PROVIDER]] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k`);
  await fixture.replace('hotbar.0', 'ae2:pattern_encoding_terminal'); await fixture.select(0, 'ae2:pattern_encoding_terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  await until(() => fixture.ae(`grid status ${provider}`), view => view.online && view.powered, 'the provider online', 30000);

  // The encoder, used in the air, opens its screen: Advanced AE's style and no rows yet.
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', ENCODER); await fixture.replace('hotbar.1', ae2Pattern);
  await fixture.select(0, ENCODER);
  await fixture.raw('use-item');
  let view = await until(() => fixture.raw('screen'), v => encoder(v), 'the encoder\'s screen', 15000);
  assert.deepEqual(encoder(view).size, [176, 182], 'adv_pattern_encoder.json');
  assert.deepEqual(rows(view), []);
  // AE2's pattern into the input: the output is an Advanced one, a row per input with no direction.
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), v => v.carried?.item === 'ae2:processing_pattern', 'the pattern on the cursor');
  await fixture.raw('click-slot 300');
  view = await until(() => fixture.raw('screen'), v => slot(v, 301)?.item === PATTERN && rows(v).length === 2, 'the Advanced pattern in the output');
  assert.equal(slot(view, 300)?.item, 'ae2:processing_pattern');
  assert.deepEqual(rows(view), [['minecraft:raw_iron', null], ['minecraft:coal', null]]);
  assert.equal(encoder(view).rows[0].buttons.length, 7, 'seven direction buttons a row');
  const press = async (row: number, column: number, want: (v: any) => boolean, label: string) => {
    const [bx, by, bw, bh] = encoder(await fixture.raw('screen')).rows[row].buttons[column];
    await fixture.raw(`click-at ${bx + bw / 2} ${by + bh / 2}`);
    return until(() => fixture.raw('screen'), want, label);
  };
  // Columns: any, north, east, south, west, up, down.
  await press(0, 5, v => rows(v)[0][1] === 'up', 'the raw iron goes up');
  await press(1, 1, v => rows(v)[1][1] === 'north', 'the coal goes north');
  view = await press(1, 0, v => rows(v)[1][1] === null, 'the coal back to the provider\'s face');
  assert.deepEqual(rows(view), [['minecraft:raw_iron', 'up'], ['minecraft:coal', null]]);
  // Taking the output takes the input.
  await fixture.raw('click-slot 301 shift');
  view = await until(() => fixture.raw('screen'), v => !slot(v, 300)?.item && !slot(v, 301)?.item, 'both slots empty');
  const inventory = await fixture.raw('inventory');
  assert.equal(inventory.counts?.[PATTERN], 1); assert.equal(inventory.counts?.['ae2:processing_pattern'] ?? 0, 0, 'the input went with the output');
  fixture.mark('the encoder\'s screen turns AE2\'s pattern into an Advanced one and its buttons send the raw iron up');

  // The tooltip (MixinEncodedPatternItem) names the raw iron's face.
  const patternAt = (v: any) => v.slots.find((s: any) => s.slot < 36 && s.item === PATTERN)?.slot;
  const shown: any = await showPattern();
  assert.deepEqual(shown.inputs.map((i: any) => [i.id, i.direction]), [['minecraft:raw_iron', 'up'], ['minecraft:coal', 'none']]);
  assert.deepEqual(shown.tooltip, ['Produces: 1 x Iron Ingot', 'with: 1 x Raw Iron (U)', ' and 1 x Coal']);
  fixture.mark('the Advanced pattern\'s tooltip shows the raw iron\'s direction letter');

  // The hotkey closes the screen, and opens the encoder from the inventory; what is left in it stays in the item.
  await fixture.raw('key k tap');
  await until(() => fixture.raw('screen'), v => !encoder(v), 'the hotkey closes the encoder');
  await fixture.raw('hotbar 3');
  await fixture.raw('key k tap');
  view = await until(() => fixture.raw('screen'), v => encoder(v), 'the hotkey opens the encoder', 15000);
  await fixture.raw(`click-slot ${patternAt(view)}`); await fixture.raw('click-slot 300');
  view = await until(() => fixture.raw('screen'), v => slot(v, 301)?.item === PATTERN, 'the Advanced pattern copied to the output');
  assert.deepEqual(rows(view), [['minecraft:raw_iron', 'up'], ['minecraft:coal', null]], 'an Advanced pattern keeps its directions');
  await fixture.raw('close');
  assert.match(chat(await fixture.command('data get entity @s Inventory[{Slot:0b}]')), /inOutInventory/, 'the encoder keeps its input');
  await fixture.select(0, ENCODER);
  await fixture.raw('use-item');
  view = await until(() => fixture.raw('screen'), v => encoder(v) && slot(v, 300)?.item === PATTERN && slot(v, 301)?.item === PATTERN, 'the input again, decoded');
  await fixture.raw('click-slot 301 shift');
  await until(() => fixture.raw('screen'), v => !slot(v, 300)?.item, 'the pattern taken back');
  await fixture.raw('close');
  fixture.mark('the encoder\'s hotkey opens and closes it, and the encoder keeps what is left in it');

  // The provider: its fourth toolbar button is Filtered Import, off at first.
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Advanced Pattern Provider screen', 15000);
  assert.deepEqual(view.ae2_screen.toolbar.map((b: any) => b.entry).filter((e: any) => e !== undefined), [18, 19, 20, 21]);
  assert(fixture.settingIs(view, 21, 'NO'), 'Filtered Import off');
  await fixture.raw(`click-slot ${patternAt(view)} shift`);
  view = await until(() => fixture.raw('screen'), v => v.slots.some((s: any) => s.slot >= 300 && s.slot < 309 && s.item === PATTERN), 'the pattern installed');
  await fixture.pressSetting(21);
  view = await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 21, 'YES'), 'Filtered Import on');
  // The player's shift-click: no cobblestone goes into the return slots.
  await fixture.command('item replace entity @s hotbar.5 with minecraft:cobblestone 3');
  view = await until(() => fixture.raw('screen'), v => slot(v, 5)?.item === 'minecraft:cobblestone', 'cobblestone in the inventory');
  await fixture.raw('click-slot 5 shift');
  await fixture.step(20);
  assert.equal(slot(await fixture.raw('screen'), 5)?.count, 3, 'the cobblestone stays with the player');
  await fixture.raw('close');
  // A hopper above: the iron ingot the pattern makes goes in and on to the grid, the cobblestone stays.
  await fixture.command(`setblock ${hopper} minecraft:hopper[facing=down]`);
  await fixture.command(`item replace block ${hopper} container.0 with minecraft:cobblestone 2`);
  await fixture.command(`item replace block ${hopper} container.1 with minecraft:iron_ingot 2`);
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:iron_ingot') === 2, 'the iron ingots through the returns into the grid', 30000);
  await fixture.step(40);
  assert.equal(count(await fixture.ae(`grid status ${drive}`), 'minecraft:cobblestone'), 0, 'no cobblestone in the grid');
  assert.match(chat(await fixture.command(`data get block ${hopper} Items`)), /cobblestone/, 'the cobblestone waits in the hopper');
  fixture.mark('with Filtered Import on, the return slots take only what the provider\'s patterns make');
  // Off: the cobblestone goes too.
  await fixture.raw(`interact ${provider} south`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen, 'the provider again', 15000);
  await fixture.pressSetting(21);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 21, 'NO'), 'Filtered Import off again');
  await fixture.raw('close');
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:cobblestone') === 2, 'the cobblestone through the returns', 30000);
  fixture.mark('with Filtered Import off, the return slots take anything');

  // AE2's terminal re-encodes the Advanced pattern as an Advanced one: the provider's pattern into its encoded slot.
  await fixture.raw(`interact ${provider} south`);
  view = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot >= 300 && s.slot < 309 && s.item === PATTERN), 'the provider with its pattern', 15000);
  await fixture.raw(`click-slot ${view.slots.find((s: any) => s.slot >= 300 && s.slot < 309 && s.item === PATTERN).slot} shift`);
  await until(() => fixture.raw('inventory'), v => v.counts?.[PATTERN] === 1, 'the pattern back in the inventory');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${terminal} north`);
  view = await until(() => fixture.raw('screen'), v => String(slot(v, 346)?.components ?? '').includes('Mode:'), 'the encoding terminal', 15000);
  await fixture.raw(`click-slot ${patternAt(view)}`); await fixture.raw('click-slot 350');
  view = await until(() => fixture.raw('screen'), v => slot(v, 350)?.item === PATTERN && slot(v, 300)?.item === 'minecraft:raw_iron', 'the pattern loaded into the grid');
  await fixture.raw('click-slot 313');
  await fixture.raw('click-slot 350 shift');
  await until(() => fixture.raw('screen'), v => !slot(v, 350)?.item && patternAt(v) !== undefined, 'the re-encoded pattern in the inventory');
  await fixture.raw('close');
  const reencoded: any = await showPattern();
  assert.deepEqual(reencoded.inputs.map((i: any) => [i.id, i.direction]), [['minecraft:raw_iron', 'up'], ['minecraft:coal', 'none']]);
  fixture.mark('AE2\'s encoding terminal re-encodes the Advanced pattern as an Advanced one, its directions kept');

  // A restart keeps the setting.
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen, 'the provider', 15000);
  await fixture.pressSetting(21);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 21, 'YES'), 'Filtered Import on before the restart');
  await fixture.raw('close');
  await fixture.restart();
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'the provider after the restart', 30000);
  assert(fixture.settingIs(view, 21, 'YES'), 'Filtered Import survives the restart');
  await fixture.raw('close');
  fixture.mark('Filtered Import survives a restart');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
