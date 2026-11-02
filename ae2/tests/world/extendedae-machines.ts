/** ExtendedAE slice 1: an ME Extended Drive holds 20 cells; an Extended Pattern Provider takes 36
 * patterns and pushes from its first and last; an Extended Interface stocks on both of its pages; the
 * parts do the same on a cable; the kits replace AE2's own devices with what they hold; everything
 * survives a restart, and each screen opens with its own style. Needs content/extendedae
 * (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-machines', mode, { addons: ['extendedae'] });
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const shots = process.env.AE2_TEST_SHOTS;
const shot = async (name: string) => { if (shots) await fixture.raw(`screenshot ${join(shots, name + '.png')}`); };
const colours = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray', 'light_gray', 'cyan', 'purple', 'blue', 'brown', 'green', 'red', 'black'];
// Twenty kinds, one a cell, stored in the 1k cells of the drive; the pushes take the stone, the Interface the dirt.
const stored = [...colours.map(c => `minecraft:${c}_wool`), 'minecraft:dirt', 'minecraft:sand', 'minecraft:stone', 'minecraft:cobblestone'];
const amount = (i: number) => 100 + i;
const cell = (id: string, n: number) => `ae2:item_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"${id}","#":${n}L}]]`;
// Processing patterns: a stone in, one of 36 outputs out. Those of slots 0 and 35 sort first in a terminal.
const outputs = ['minecraft:acacia_boat', ...colours.map(c => `minecraft:${c}_concrete`), ...colours.map(c => `minecraft:${c}_terracotta`), 'minecraft:oak_door', 'minecraft:oak_fence', 'minecraft:acacia_button'];
assert.equal(outputs.length, 35 + 1);
// A sneaking use. The server hears of the sneak with the client's next tick (`ServerboundPlayerInputPacket`),
// and a use sent before that tick reaches it unsneaking: wait for two client ticks after the key goes down.
const sneakInteract = async (target: string, side: string) => {
  const ticks = (await fixture.raw('status')).ticks;
  await fixture.raw('key shift down');
  try {
    await until(() => fixture.raw('status'), view => view.ticks >= ticks + 2, 'the sneak sent');
    await fixture.raw(`interact ${target} ${side}`);
  } finally { await fixture.raw('key shift up'); }
};
const pattern = (output: string) => `ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"minecraft:stone","#":1L}],sparseOutputs:[{"#t":"ae2:i",id:"${output}","#":1L}]}]`;
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
  // Two grids of no more than eight channels: the working one, and one for the parts and kits.
  const power = at(0), drive = at(1), terminal = at(2), cpu1 = at(2, 1), cpu2 = at(4, 1), provider = at(5), sink = at(5, 0, -1), access = at(6), iface = at(7);
  const power2 = at(9), partHost = at(10), kitProvider = at(11), kitDrive = at(12), kitIface = at(13), kitHost = at(14), kitLast = at(15);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'extendedae:ex_drive'], [terminal, 'ae2:cable_bus'], [cpu1, 'ae2:64k_crafting_storage'], [at(3), 'ae2:cable_bus'], [at(4), 'ae2:cable_bus'], [cpu2, 'ae2:64k_crafting_storage'], [provider, 'extendedae:ex_pattern_provider'], [sink, 'minecraft:chest'], [access, 'ae2:cable_bus'], [iface, 'extendedae:ex_interface'], [power2, 'ae2:creative_energy_cell'], [partHost, 'ae2:cable_bus'], [kitProvider, 'ae2:pattern_provider'], [kitDrive, 'ae2:drive'], [kitIface, 'ae2:interface'], [kitHost, 'ae2:cable_bus']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  await fixture.replace('hotbar.0', 'ae2:pattern_access_terminal'); await fixture.select(0, 'ae2:pattern_access_terminal');
  assert.equal((await fixture.ae(`grid part ${access} north`)).attached, true);
  const online = (pos: string) => until(() => fixture.ae(`grid status ${pos}`), view => view.online && view.powered, `online ${pos}`, 30000);
  await online(drive);

  // ME Extended Drive: 20 cells, the last ten on the screen's right side.
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 2.5}`);
  for (let i = 0; i < 20; i++) await fixture.command(`item replace block ${drive} container.${i} with ${cell(stored[i], amount(i))}`);
  await until(() => fixture.ae(`grid status ${drive}`), view => stored.every((id, i) => count(view, id) === amount(i)), 'all twenty cells mounted', 30000);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${drive} south`);
  const driveView = await until(() => fixture.raw('screen'), view => view.ae2_screen, 'Extended Drive screen', 15000);
  assert.equal(driveView.ae2_screen.style, 'ex_drive.json');
  assert.equal(driveView.ae2_screen.toolbar?.[0]?.button, 'help');
  assert(driveView.ae2_screen.priority, 'priority tab');
  assert.equal(driveView.slots.filter((slot: any) => slot.slot >= 300 && slot.slot < 327 && slot.item === 'ae2:item_storage_cell_1k').length, 20);
  await shot('ex_drive');
  await fixture.raw('close');
  fixture.mark('an ME Extended Drive holds and lists 20 cells on its own screen');

  // Extended Pattern Provider: 36 patterns by shift-click from a full inventory, in order.
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 2.5}`);
  await fixture.command('clear @s');
  for (const [i, output] of outputs.entries()) {
    await fixture.command(`item replace entity @s ${i < 9 ? `hotbar.${i}` : `inventory.${i - 9}`} with ${pattern(output)} 1`);
  }
  await until(() => fixture.raw('inventory'), view => view.counts?.['ae2:processing_pattern'] === 36, 'patterns in the inventory');
  await online(provider);
  await fixture.raw(`interact ${provider} south`);
  const providerView = await until(() => fixture.raw('screen'), view => view.ae2_screen, 'Extended Pattern Provider screen', 15000);
  assert.equal(providerView.ae2_screen.style, 'ex_pattern_provider.json');
  assert(providerView.ae2_screen.priority, 'priority tab');
  assert.deepEqual(providerView.ae2_screen.toolbar.map((b: any) => b.entry).filter((e: any) => e !== undefined), [45, 46, 47]);
  // Hotbar first, in the order they lie.
  for (let i = 0; i < 36; i++) {
    await fixture.raw(`click-slot ${i} shift`);
    await until(() => fixture.raw('inventory'), view => (view.counts?.['ae2:processing_pattern'] ?? 0) === 35 - i, `pattern ${i} installed`);
  }
  const filled = await until(() => fixture.raw('screen'), view => view.slots.filter((slot: any) => slot.slot >= 300 && slot.slot < 336 && slot.item === 'ae2:processing_pattern').length === 36, '36 patterns shown');
  assert(filled.slots.every((slot: any) => slot.slot < 336 || slot.slot >= 345 || slot.item !== 'ae2:processing_pattern'), 'no pattern past the 36th slot');
  await shot('ex_pattern_provider');
  // A setting's entry is past the patterns and return slots.
  assert(fixture.settingIs(await fixture.raw('screen'), 45, 'NO'));
  await fixture.pressSetting(45);
  await until(() => fixture.raw('screen'), view => fixture.settingIs(view, 45, 'YES'), 'blocking mode on');
  await fixture.pressSetting(45);
  await until(() => fixture.raw('screen'), view => fixture.settingIs(view, 45, 'NO'), 'blocking mode off');
  await fixture.raw('close');

  const request = async (item: string) => {
    await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
    await fixture.raw(`interact ${terminal} north`);
    const entry = (view: any) => view.slots?.find((slot: any) => slot.slot >= 300 && slot.slot < 345 && slot.item === item);
    await until(() => fixture.raw('screen'), entry, `${item} craftable`, 30000);
    // A running job's item comes to the top of the list a moment after the terminal opens, and a
    // click on the older view is refused: click where the item is until the amount screen opens.
    // A middle click requests a craft (AE2's terminal gesture; shift moves stock to the player).
    await until(async () => {
      const view = await fixture.raw('screen');
      if (!JSON.stringify(view.title ?? '').includes('AE2 Craft Amount') && entry(view)) await fixture.raw(`click-slot ${entry(view).slot} middle`);
      return view;
    }, view => JSON.stringify(view.title ?? '').includes('AE2 Craft Amount'), 'craft amount');
    await fixture.raw('click-slot 302');
    const confirm = await until(() => fixture.raw('screen'), view => JSON.stringify(view.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(view)), 'craft confirm');
    assert.doesNotMatch(JSON.stringify(confirm), /Missing materials|craft planning:/);
    await fixture.raw('click-slot 302');
    await fixture.raw('close');
  };
  const inSink = async () => {
    const text = (await fixture.command(`data get block ${sink}`)).chat?.join('\n') ?? '';
    return Number(/id: "minecraft:stone"[^}]*count: (\d+)/.exec(text)?.[1] ?? /count: (\d+)[^}]*id: "minecraft:stone"/.exec(text)?.[1] ?? 0);
  };
  await request('minecraft:acacia_button');
  await until(inSink, n => n === 1, 'the 36th pattern pushed a stone', 60000);
  await request('minecraft:acacia_boat');
  await until(inSink, n => n === 2, 'the first pattern pushed a stone', 60000);
  fixture.mark('an Extended Pattern Provider takes 36 patterns and pushes from its first and last');

  // Pattern Access Terminal: the provider's header and four lines of nine.
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${access} north`);
  const lines = await until(() => fixture.raw('screen'), view => view.slots?.filter((slot: any) => slot.slot >= 300 && slot.slot < 354 && slot.item === 'ae2:processing_pattern').length === 36, 'Pattern Access lists 36 patterns', 30000);
  await shot('ex_pattern_access');
  // The fourth line of patterns, below the header line.
  assert.equal(lines.slots.filter((slot: any) => slot.slot >= 336 && slot.slot < 345 && slot.item === 'ae2:processing_pattern').length, 9, 'its last line is on the page');
  await fixture.raw('close');
  fixture.mark('the Pattern Access Terminal shows all 36 patterns');

  // Extended Interface: config on both pages stocks from the grid.
  await fixture.command(`tp @s ${x + 7.5} ${y} ${z + 2.5}`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', 'minecraft:dirt'); await fixture.replace('hotbar.8', 'minecraft:blaze_rod');
  await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${iface} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Extended Interface screen', 15000);
  assert.equal(view.ae2_screen.style, 'ex_interface.json');
  assert.equal(view.ae2_screen.data?.[2] ?? 0, 0, 'page 1 first');
  const setConfig = async (entry: number) => {
    await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !!s.carried, 'dirt on the cursor');
    await fixture.raw(`click-slot ${300 + entry}`);
    await until(() => fixture.raw('screen'), v => v.slots.some((slot: any) => slot.slot === 300 + entry && slot.item === 'minecraft:dirt'), `config entry ${entry}`);
    await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !s.carried, 'dirt back');
  };
  // Config index 0 (entry 18) on the first page, its storage slot is entry 0.
  await setConfig(18);
  await until(() => fixture.raw('screen'), v => v.slots.some((slot: any) => slot.slot === 300 && slot.item === 'minecraft:dirt'), 'first page stocks', 60000);
  await shot('ex_interface_page1');
  // The second page shows the other 18: index 35 is the last entry of its second row.
  await fixture.pressSetting(43);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.data?.[2] === 1, 'page 2', 15000);
  assert(!view.slots.some((slot: any) => slot.slot === 318 && slot.item === 'minecraft:dirt'), 'page 2 has its own config slots');
  await setConfig(35);
  await until(() => fixture.raw('screen'), v => v.slots.some((slot: any) => slot.slot === 317 && slot.item === 'minecraft:dirt'), 'second page stocks its last slot', 60000);
  await shot('ex_interface_page2');
  // The cog over the configured slot opens that slot's stock editor: one more, and Set.
  await fixture.raw(`click-slot ${300 + 35} shift`);
  await until(() => fixture.raw('screen'), v => v.slots.some((slot: any) => slot.slot === 310 && slot.item === 'minecraft:dirt') && v.slots.some((slot: any) => slot.slot === 308 && slot.item === 'minecraft:arrow'), 'stock editor of the last slot', 15000);
  await shot('ex_interface_stock');
  await fixture.raw('click-slot 300');
  await fixture.raw('click-slot 308');
  await until(() => fixture.raw('screen'), v => v.slots.some((slot: any) => slot.slot === 317 && slot.item === 'minecraft:dirt' && slot.count === 2), 'the last slot stocks what its editor set', 60000);
  await fixture.raw('close');
  fixture.mark('an Extended Interface stocks on both pages of 18');

  // Parts on a cable: 36 slots each, their screens by the part.
  await fixture.command(`tp @s ${x + 10.5} ${y} ${z + 2.5}`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', 'extendedae:ex_pattern_provider_part'); await fixture.select(0, 'extendedae:ex_pattern_provider_part');
  assert.equal((await fixture.ae(`grid part ${partHost} south`)).attached, true);
  await fixture.replace('hotbar.0', 'extendedae:ex_interface_part'); await fixture.select(0, 'extendedae:ex_interface_part');
  assert.equal((await fixture.ae(`grid part ${partHost} north`)).attached, true);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${partHost} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'provider part screen', 15000);
  assert.equal(view.ae2_screen.style, 'ex_pattern_provider.json');
  await shot('ex_pattern_provider_part');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 10.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${partHost} north`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'interface part screen', 15000);
  assert.equal(view.ae2_screen.style, 'ex_interface.json');
  await fixture.raw('close');
  fixture.mark('the extended parts attach and open their screens');

  // The kits, sneaking: AE2's device gives way to the extended one and keeps what it holds.
  await fixture.command(`tp @s ${x + 11.5} ${y} ${z + 2.5}`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', pattern('minecraft:oak_door')); await fixture.replace('hotbar.8', 'minecraft:blaze_rod');
  await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${kitProvider} south`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'pattern_provider.json', 'AE2 provider screen');
  await fixture.raw('click-slot 0 shift');
  await until(() => fixture.raw('inventory'), v => !v.counts?.['ae2:processing_pattern'], 'pattern in the provider');
  await fixture.raw('close');
  const kit = async (item: string, target: string, block: string, side: string) => {
    await fixture.replace('hotbar.1', item, 2); await fixture.select(1, item);
    await sneakInteract(target, side);
    await until(async () => (await fixture.raw(`block ${target}`)), state => JSON.stringify(state).includes(block), `${block} in place`, 15000);
    // A creative player keeps the kit (`ServerPlayerGameMode.useItemOn`).
    await until(() => fixture.raw('inventory'), v => v.counts?.[item] === 2, `${item} kept in creative`);
  };
  await kit('extendedae:pattern_provider_upgrade', kitProvider, 'extendedae:ex_pattern_provider', 'south');
  await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${kitProvider} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'ex_pattern_provider.json', 'the kit made an Extended Pattern Provider');
  assert.equal(view.slots.find((slot: any) => slot.slot === 300)?.item, 'ae2:processing_pattern', 'it keeps its pattern');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 12.5} ${y} ${z + 2.5}`);
  await fixture.command(`item replace block ${kitDrive} container.3 with ${cell('minecraft:glass', 7)}`);
  await until(() => fixture.ae(`grid status ${kitDrive}`), v => count(v, 'minecraft:glass') === 7, 'cell of the plain drive', 30000);
  await kit('extendedae:drive_upgrade', kitDrive, 'extendedae:ex_drive', 'south');
  await until(() => fixture.ae(`grid status ${kitDrive}`), v => count(v, 'minecraft:glass') === 7, 'the Extended Drive keeps its cell', 30000);
  await fixture.command(`tp @s ${x + 13.5} ${y} ${z + 2.5}`);
  await kit('extendedae:interface_upgrade', kitIface, 'extendedae:ex_interface', 'south');
  // On a cable: the part under the cursor.
  await fixture.command(`tp @s ${x + 14.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'ae2:cable_interface'); await fixture.select(0, 'ae2:cable_interface');
  assert.equal((await fixture.ae(`grid part ${kitHost} south`)).attached, true);
  await fixture.replace('hotbar.0', 'extendedae:interface_upgrade'); await fixture.select(0, 'extendedae:interface_upgrade');
  await sneakInteract(kitHost, 'south');
  await until(async () => (await fixture.command(`data get block ${kitHost}`)).chat?.join('\n') ?? '', text => text.includes('extendedae:ex_interface_part'), 'the kit replaced the part', 15000);
  fixture.mark('the kits replace AE2\'s devices and parts in place, with what they hold');

  // The recipes: a kit from its device and an ingot, the part from the block and back.
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');
  const craft = async (ingredients: string[], result: string) => {
    for (const [i, id] of ingredients.entries()) await fixture.replace(`hotbar.${i}`, id);
    await fixture.raw('open-inventory');
    await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 100), 'inventory crafting grid');
    for (let i = 0; i < ingredients.length; i++) {
      await fixture.raw(`click-slot ${i}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'on the cursor');
      await fixture.raw(`click-slot ${100 + i}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'in the grid');
    }
    await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === result), `recipe offers ${result}`);
    await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
    await until(() => fixture.raw('inventory'), s => s.counts?.[result] === 1, `${result} crafted`);
    await fixture.command('clear @s');
  };
  await craft(['extendedae:ex_pattern_provider', 'minecraft:iron_ingot'], 'extendedae:pattern_provider_upgrade');
  await craft(['extendedae:ex_interface_part', 'minecraft:copper_ingot'], 'extendedae:interface_upgrade');
  await craft(['extendedae:ex_drive', 'minecraft:gold_ingot'], 'extendedae:drive_upgrade');
  await craft(['extendedae:ex_pattern_provider'], 'extendedae:ex_pattern_provider_part');
  await craft(['extendedae:ex_interface_part'], 'extendedae:ex_interface');
  fixture.mark('the kits and the part and block forms come from ExtendedAE\'s recipes');
  // In survival a kit is used up, and without sneaking the device opens its screen instead.
  await fixture.command(`tp @s ${x + 15.5} ${y} ${z + 2.5}`);
  await fixture.command(`setblock ${kitLast} ae2:interface`);
  await fixture.replace('hotbar.0', 'extendedae:interface_upgrade'); await fixture.select(0, 'extendedae:interface_upgrade');
  await fixture.raw(`interact ${kitLast} south`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'interface.json', 'no sneak, the AE2 Interface screen');
  await fixture.raw('close');
  assert.equal((await fixture.raw('inventory')).counts?.['extendedae:interface_upgrade'], 1, 'no kit used without sneaking');
  await sneakInteract(kitLast, 'south');
  await until(async () => (await fixture.raw(`block ${kitLast}`)), state => JSON.stringify(state).includes('extendedae:ex_interface'), 'survival kit replaced the interface', 15000);
  await until(() => fixture.raw('inventory'), v => !v.counts?.['extendedae:interface_upgrade'], 'the kit is used up in survival');
  await fixture.command('gamemode creative @s');
  await until(() => fixture.raw('status'), s => s.creative === true, 'creative again');
  fixture.mark('a kit acts only while sneaking, and survival uses it up');

  // Through a restart.
  await fixture.restart();
  await online(drive);
  // The Interface stocked three dirt from its cell, and two pushes took two stone.
  const expected = (id: string, i: number) => amount(i) - (id === 'minecraft:dirt' ? 3 : id === 'minecraft:stone' ? 2 : 0);
  await until(() => fixture.ae(`grid status ${drive}`), v => stored.every((id, i) => count(v, id) === expected(id, i)), 'twenty cells after restart', 30000);
  await until(() => fixture.ae(`grid status ${kitDrive}`), v => count(v, 'minecraft:glass') === 7, 'the upgraded drive after restart', 30000);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'ex_pattern_provider.json', 'provider screen after restart');
  assert.equal(view.slots.filter((slot: any) => slot.slot >= 300 && slot.slot < 336 && slot.item === 'ae2:processing_pattern').length, 36, '36 patterns after restart');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 7.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${iface} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'ex_interface.json', 'interface screen after restart');
  const page = view.ae2_screen.data?.[2] ?? 0;
  assert.equal(view.slots.find((slot: any) => slot.slot === (page === 0 ? 300 : 317))?.item, 'minecraft:dirt', 'stock after restart');
  await fixture.raw('close');
  fixture.mark('cells, patterns, config and stock survive a restart');

  // Broken (`setblock ... destroy`: Level.destroyBlock), a device gives up all it holds; far enough that
  // nothing is picked up. A plain setblock drops nothing: AE2's block entities are Clearable, and
  // SetBlockCommand clears the old one first.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 6.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((entity: any) => entity.item?.item === item).reduce((sum: number, entity: any) => sum + entity.item.count, 0);
  await fixture.command(`setblock ${provider} minecraft:air destroy`);
  await until(() => ground('ae2:processing_pattern'), n => n === 36, 'the 36 patterns on the ground', 30000);
  await fixture.command(`setblock ${drive} minecraft:air destroy`);
  await until(() => ground('ae2:item_storage_cell_1k'), n => n === 20, 'the 20 cells on the ground', 30000);
  await fixture.command(`setblock ${iface} minecraft:air destroy`);
  await until(() => ground('minecraft:dirt'), n => n === 3, 'the Interface\'s stock on the ground', 30000);
  fixture.mark('broken, the devices drop all 36 patterns, all 20 cells and the stock');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
