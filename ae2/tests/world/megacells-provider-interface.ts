/** MEGA Cells slice 5: the MEGA Pattern Provider and the MEGA Interface, block and part. The provider
 * (MEGAPatternProviderBlock.createLogic: PatternProviderLogic(.., 18) with AEItemDefinitionFilter(PROCESSING_PATTERN))
 * opens MEGA's screen with 18 pattern slots, takes processing patterns into them and no crafting pattern (a
 * shift-click sends that, and a 19th processing pattern, through the return slots into the grid, as in Java), keeps AE2's PUSH_DIRECTION (a wrench on its south face makes it push north)
 * and pushes from its first and last pattern into the chest it faces; the interface (InterfaceLogic(.., 18))
 * opens MEGA's screen, stocks its first and last config slots from the grid, takes a fuzzy card and refuses a
 * speed card; the parts attach to a cable and do the same; all six of MEGA's recipes make them; everything
 * survives a restart; broken, each drops itself and what it holds. Needs content/megacells
 * (ae2/scripts/prepare-addon.ts megacells) and a client built with ae2-megacells. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('megacells-provider-interface', mode, { addons: ['megacells'] });
const PROVIDER = 'megacells:mega_pattern_provider', PROVIDER_PART = 'megacells:cable_mega_pattern_provider';
const IFACE = 'megacells:mega_interface', IFACE_PART = 'megacells:cable_mega_interface';
const PROVIDER_STYLE = 'megacells/mega_pattern_provider.json', IFACE_STYLE = 'megacells/mega_interface.json';
const shots = process.env.AE2_TEST_SHOTS;
const shot = async (name: string) => { if (shots) await fixture.raw(`screenshot ${join(shots, name + '.png')}`); };
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const colours = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray', 'light_gray', 'cyan', 'purple', 'blue', 'brown', 'green', 'red', 'black'];
// Processing patterns, a stone in and one of 19 outputs out; the 19th finds no room.
const outputs = ['minecraft:acacia_boat', ...colours.map(c => `minecraft:${c}_concrete`), 'minecraft:acacia_button', 'minecraft:oak_door'];
assert.equal(outputs.length, 19);
const pattern = (output: string) => `ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"minecraft:stone","#":1L}],sparseOutputs:[{"#t":"ae2:i",id:"${output}","#":1L}]}]`;
const CRAFTING = 'ae2:crafting_pattern';
const patterns = (view: any) => view.slots.filter((slot: any) => slot.slot >= 300 && slot.slot < 318 && slot.item === 'ae2:processing_pattern').length;
const holds = (view: any, slot: number, item: string, n?: number) => view.slots.some((s: any) => s.slot === slot && s.item === item && (n === undefined || s.count === n));
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -5], [-3, 7], [12, -5], [12, 7]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -5)} ${at(12, 5, 7)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -5)} ${at(12, -1, 7)} minecraft:stone`);
  await fixture.command('clear @s');
  // One grid of eight channels: the drive, the terminal, two CPUs (each request's job stays on its CPU,
  // waiting for a result that never comes), both blocks and both parts.
  const power = at(0), drive = at(1), terminal = at(2), cpu = at(2, 1), cpu2 = at(5, 1), provider = at(4), north = at(4, 0, -1), up = at(4, 1), iface = at(6), partHost = at(8);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [cpu, 'ae2:64k_crafting_storage'], [at(3), 'ae2:cable_bus'], [cpu2, 'ae2:64k_crafting_storage'], [provider, PROVIDER], [north, 'minecraft:chest'], [up, 'minecraft:chest'], [at(5), 'ae2:cable_bus'], [iface, IFACE], [at(7), 'ae2:cable_bus'], [partHost, 'ae2:cable_bus']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:stone","#":64L},{"#t":"ae2:i",id:"minecraft:dirt","#":64L}]]`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  const online = (pos: string) => until(() => fixture.ae(`grid status ${pos}`), view => view.online && view.powered, `online ${pos}`, 30000);
  await online(provider);
  await until(() => fixture.ae(`grid status ${drive}`), view => count(view, 'minecraft:stone') === 64 && count(view, 'minecraft:dirt') === 64, 'the cell is mounted', 30000);
  const isBlock = async (pos: string, state: string) => ((await fixture.command(`execute if block ${pos} ${state}`)).chat ?? []).includes('Test passed');
  assert(await isBlock(provider, `${PROVIDER}[push_direction=all]`), 'the provider pushes to all sides when placed');

  // MEGA Pattern Provider: MEGA's screen, processing patterns alone, 18 of them.
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', CRAFTING);
  for (const [i, output] of outputs.entries()) {
    await fixture.command(`item replace entity @s ${i < 8 ? `hotbar.${i + 1}` : `inventory.${i - 8}`} with ${pattern(output)} 1`);
  }
  await until(() => fixture.raw('inventory'), view => view.counts?.['ae2:processing_pattern'] === 19, 'patterns in the inventory');
  await fixture.raw(`interact ${provider} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'MEGA Pattern Provider screen', 15000);
  assert.equal(view.ae2_screen.style, PROVIDER_STYLE);
  assert(view.ae2_screen.priority, 'priority tab');
  assert.deepEqual(view.ae2_screen.toolbar.map((b: any) => b.entry).filter((e: any) => e !== undefined), [27, 28, 29]);
  // A crafting pattern goes into no pattern slot. Shift-clicked, it goes to the return slots and from there into
  // the grid, as in Java: AEBaseMenu.quickMoveStack moves to every slot that may take it, the return slots
  // (STORAGE, a PatternProviderReturnInventory) take anything, and the provider injects them into the network.
  // From the cursor, a pattern slot refuses it.
  await fixture.raw('click-slot 0 shift');
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, CRAFTING) === 1, 'the shift-clicked crafting pattern went through the returns into the grid', 30000);
  view = await fixture.raw('screen');
  assert(!view.slots.some((s: any) => s.slot >= 300 && s.slot < 318 && s.item), 'no pattern slot took it');
  await fixture.replace('hotbar.0', CRAFTING);
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => s.carried?.item === CRAFTING, 'crafting pattern on the cursor');
  await fixture.raw('click-slot 300');
  await fixture.step(2);
  view = await fixture.raw('screen');
  assert(!holds(view, 300, CRAFTING), 'the first pattern slot refuses the crafting pattern');
  assert.equal((await fixture.raw('inventory')).carried?.item, CRAFTING, 'it stays on the cursor');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !s.carried, 'crafting pattern back');
  // The processing patterns, in order: the 18 pattern slots fill, and the 19th goes the returns' way.
  for (let i = 0; i < 18; i++) {
    await fixture.raw(`click-slot ${i + 1} shift`);
    await until(() => fixture.raw('inventory'), v => (v.counts?.['ae2:processing_pattern'] ?? 0) === 18 - i, `pattern ${i} installed`);
  }
  view = await until(() => fixture.raw('screen'), v => patterns(v) === 18, '18 patterns shown');
  await fixture.raw('click-slot 19 shift');
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'ae2:processing_pattern') === 1, 'the 19th pattern went into the grid', 30000);
  view = await fixture.raw('screen');
  assert.equal(patterns(view), 18, 'still 18 patterns');
  await shot('mega_pattern_provider');
  assert(fixture.settingIs(view, 27, 'NO'));
  await fixture.pressSetting(27);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 27, 'YES'), 'blocking mode on');
  await fixture.pressSetting(27);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 27, 'NO'), 'blocking mode off');
  await fixture.raw('close');
  fixture.mark('the MEGA Pattern Provider opens MEGA\'s screen and takes 18 processing patterns, no crafting pattern');

  // PUSH_DIRECTION: a wrench on the south face turns an all-sides provider to push north.
  await fixture.replace('hotbar.8', 'ae2:certus_quartz_wrench'); await fixture.select(8, 'ae2:certus_quartz_wrench');
  await fixture.raw(`interact ${provider} south`);
  await until(() => isBlock(provider, `${PROVIDER}[push_direction=north]`), ok => ok, 'the wrench sets push_direction north', 15000);
  const request = async (item: string) => {
    await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
    await fixture.raw(`interact ${terminal} north`);
    const entry = (v: any) => v.slots?.find((slot: any) => slot.slot >= 300 && slot.slot < 345 && slot.item === item);
    await until(() => fixture.raw('screen'), entry, `${item} craftable`, 30000);
    await until(async () => {
      const v = await fixture.raw('screen');
      if (!JSON.stringify(v.title ?? '').includes('AE2 Craft Amount') && entry(v)) await fixture.raw(`click-slot ${entry(v).slot} middle`);
      return v;
    }, v => JSON.stringify(v.title ?? '').includes('AE2 Craft Amount'), 'craft amount');
    await fixture.raw('click-slot 302');
    const confirm = await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(v)), 'craft confirm');
    assert.doesNotMatch(JSON.stringify(confirm), /Missing materials|craft planning:/);
    await fixture.raw('click-slot 302');
    await fixture.raw('close');
  };
  const stoneIn = async (pos: string) => {
    const text = (await fixture.command(`data get block ${pos}`)).chat?.join('\n') ?? '';
    return Number(/id: "minecraft:stone"[^}]*count: (\d+)/.exec(text)?.[1] ?? /count: (\d+)[^}]*id: "minecraft:stone"/.exec(text)?.[1] ?? 0);
  };
  await request('minecraft:acacia_boat');
  await until(() => stoneIn(north), n => n === 1, 'the first pattern pushed a stone north', 60000);
  await request('minecraft:acacia_button');
  await until(() => stoneIn(north), n => n === 2, 'the 18th pattern pushed a stone north', 60000);
  assert.equal(await stoneIn(up), 0, 'nothing pushed up');
  fixture.mark('the MEGA Pattern Provider keeps AE2\'s push direction and pushes from its first and last pattern');

  // MEGA Interface: MEGA's screen; its first and last config slots stock from the grid; one card.
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 2.5}`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', 'minecraft:dirt'); await fixture.replace('hotbar.1', 'ae2:speed_card'); await fixture.replace('hotbar.2', 'ae2:fuzzy_card');
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${iface} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'MEGA Interface screen', 15000);
  assert.equal(view.ae2_screen.style, IFACE_STYLE);
  assert(view.ae2_screen.priority, 'priority tab');
  const setConfig = async (entry: number) => {
    await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !!s.carried, 'dirt on the cursor');
    await fixture.raw(`click-slot ${300 + entry}`);
    await until(() => fixture.raw('screen'), v => holds(v, 300 + entry, 'minecraft:dirt'), `config entry ${entry}`);
    await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !s.carried, 'dirt back');
  };
  // Config 0 (entry 18) stocks storage 0; config 17 (entry 35, the second row's last) stocks storage 17.
  await setConfig(18);
  await until(() => fixture.raw('screen'), v => holds(v, 300, 'minecraft:dirt', 1), 'the first slot stocks', 60000);
  await setConfig(35);
  await until(() => fixture.raw('screen'), v => holds(v, 317, 'minecraft:dirt', 1), 'the last slot stocks', 60000);
  // The cog over the last config slot opens its stock editor: one more, and Set.
  await fixture.raw(`click-slot ${300 + 35} shift`);
  await until(() => fixture.raw('screen'), v => holds(v, 310, 'minecraft:dirt') && holds(v, 308, 'minecraft:arrow'), 'stock editor of the last slot', 15000);
  await fixture.raw('click-slot 300');
  await fixture.raw('click-slot 308');
  await until(() => fixture.raw('screen'), v => holds(v, 317, 'minecraft:dirt', 2), 'the last slot stocks what its editor set', 60000);
  // The card slot (entry 36): a speed card is refused, a fuzzy card goes in and shows fuzzy mode.
  view = await fixture.raw('screen');
  assert(!view.ae2_screen.toolbar.some((b: any) => b.entry === 41), 'no fuzzy mode without the card');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:speed_card', 'speed card on the cursor');
  await fixture.raw('click-slot 336');
  await fixture.step(2);
  assert(!holds(await fixture.raw('screen'), 336, 'ae2:speed_card'), 'the speed card is refused');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => !s.carried, 'speed card back');
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:fuzzy_card', 'fuzzy card on the cursor');
  await fixture.raw('click-slot 336');
  await until(() => fixture.raw('screen'), v => holds(v, 336, 'ae2:fuzzy_card') && v.ae2_screen.toolbar.some((b: any) => b.entry === 41), 'the fuzzy card in, fuzzy mode shown');
  await shot('mega_interface');
  await fixture.raw('close');
  fixture.mark('the MEGA Interface opens MEGA\'s screen, stocks its first and last of 18 slots and takes a fuzzy card');

  // The parts on a cable: their screens, processing patterns alone, stocking.
  await fixture.command(`tp @s ${x + 8.5} ${y} ${z + 2.5}`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', PROVIDER_PART); await fixture.select(0, PROVIDER_PART);
  assert.equal((await fixture.ae(`grid part ${partHost} south`)).attached, true);
  await fixture.replace('hotbar.0', IFACE_PART); await fixture.select(0, IFACE_PART);
  assert.equal((await fixture.ae(`grid part ${partHost} north`)).attached, true);
  await fixture.replace('hotbar.0', pattern('minecraft:oak_fence')); await fixture.replace('hotbar.1', CRAFTING); await fixture.replace('hotbar.2', 'minecraft:dirt');
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${partHost} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'provider part screen', 15000);
  assert.equal(view.ae2_screen.style, PROVIDER_STYLE);
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === CRAFTING, 'crafting pattern on the cursor');
  await fixture.raw('click-slot 300');
  await fixture.step(2);
  assert(!holds(await fixture.raw('screen'), 300, CRAFTING), 'the part refuses the crafting pattern');
  assert.equal((await fixture.raw('inventory')).carried?.item, CRAFTING, 'it stays on the cursor');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => !s.carried, 'crafting pattern back');
  await fixture.raw('click-slot 0 shift');
  await until(() => fixture.raw('screen'), v => holds(v, 300, 'ae2:processing_pattern'), 'the part takes the processing pattern');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 8.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${partHost} north`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'interface part screen', 15000);
  assert.equal(view.ae2_screen.style, IFACE_STYLE);
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => !!s.carried, 'dirt on the cursor');
  await fixture.raw('click-slot 318');
  await until(() => fixture.raw('screen'), v => holds(v, 318, 'minecraft:dirt'), 'part config entry 18');
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => !s.carried, 'dirt back');
  await until(() => fixture.raw('screen'), v => holds(v, 300, 'minecraft:dirt', 1), 'the interface part stocks', 60000);
  await fixture.raw('close');
  fixture.mark('the MEGA parts attach, open MEGA\'s screens, take processing patterns alone and stock');

  // MEGA's recipes: the devices from AE2's and an Accumulation Processor, the part from the block and back.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 5.5}`);
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
  await craft(['ae2:pattern_provider', 'megacells:accumulation_processor'], PROVIDER);
  await craft([PROVIDER], PROVIDER_PART);
  await craft([PROVIDER_PART], PROVIDER);
  await craft(['ae2:interface', 'megacells:accumulation_processor'], IFACE);
  await craft([IFACE], IFACE_PART);
  await craft([IFACE_PART], IFACE);
  await fixture.command('gamemode creative @s');
  await until(() => fixture.raw('status'), s => s.creative === true, 'creative again');
  fixture.mark('MEGA\'s six recipes make the provider and the interface, block and part');

  // Through a restart.
  await fixture.restart();
  await online(provider);
  // The pushes took two stone; the interfaces stocked four dirt.
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:stone') === 62 && count(v, 'minecraft:dirt') === 60, 'the grid after restart', 30000);
  assert(await isBlock(provider, `${PROVIDER}[push_direction=north]`), 'push direction after restart');
  await fixture.command('clear @s');
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === PROVIDER_STYLE, 'provider screen after restart');
  assert.equal(patterns(view), 18, '18 patterns after restart');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${iface} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === IFACE_STYLE, 'interface screen after restart');
  assert(holds(view, 300, 'minecraft:dirt', 1) && holds(view, 317, 'minecraft:dirt', 2) && holds(view, 318, 'minecraft:dirt') && holds(view, 335, 'minecraft:dirt'), 'config and stock after restart');
  assert(holds(view, 336, 'ae2:fuzzy_card'), 'the card after restart');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 8.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${partHost} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === PROVIDER_STYLE, 'provider part after restart');
  assert(holds(view, 300, 'ae2:processing_pattern'), 'the part\'s pattern after restart');
  await fixture.raw('close');
  fixture.mark('patterns, push direction, config, stock, the card and the parts\' contents survive a restart');

  // Broken (`setblock ... destroy`: Level.destroyBlock), each drops itself and what it holds.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 6.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((entity: any) => entity.item?.item === item).reduce((sum: number, entity: any) => sum + entity.item.count, 0);
  const clearGround = () => fixture.command('kill @e[type=minecraft:item]');
  await fixture.command(`setblock ${provider} minecraft:air destroy`);
  await until(() => ground('ae2:processing_pattern'), n => n === 18, 'the 18 patterns on the ground', 30000);
  await until(() => ground(PROVIDER), n => n === 1, 'the provider on the ground', 30000);
  await clearGround();
  await fixture.command(`setblock ${iface} minecraft:air destroy`);
  await until(() => ground('minecraft:dirt'), n => n === 3, 'the interface\'s stock on the ground', 30000);
  await until(async () => [await ground('ae2:fuzzy_card'), await ground(IFACE)], ([card, block]) => card === 1 && block === 1, 'the card and the interface on the ground', 30000);
  await clearGround();
  await fixture.command(`setblock ${partHost} minecraft:air destroy`);
  await until(async () => [await ground(PROVIDER_PART), await ground(IFACE_PART), await ground('ae2:processing_pattern'), await ground('minecraft:dirt')], n => n.every(k => k === 1), 'both parts, the pattern and the stock on the ground', 30000);
  fixture.mark('broken, the blocks and the parts drop themselves with their patterns, stock and card');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
