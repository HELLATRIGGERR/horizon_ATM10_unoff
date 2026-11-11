/** Advanced AE slice 4: the (Extended) Advanced Pattern Provider, the part forms and the upgrade items. The large
 * provider (AdvPatternProviderEntity, AdvPatternProviderLogic(.., 36)) opens adv_pattern_provider.json with all 36
 * pattern slots and its settings after them. The large part on a cable (AdvPatternProviderPart) takes an Advanced
 * Processing Pattern and pushes it into the furnace in front of it as the block does (pushInputsDirectionally): the raw
 * iron through the furnace's top, the coal through the side the part faces. Used sneaking (useOn), the Advanced
 * Pattern Provider Upgrade turns AE2's provider, block and part, into the small Advanced one and ExtendedAE's into the
 * large one, and the Capacity Upgrade the small one into the large one, each in place with its patterns and settings;
 * a creative player keeps the item. Filtered Import on the part's screen; a restart keeps all of it; broken, the block
 * and the part drop themselves and their patterns; the part form and the upgrade come from the mod's recipes.
 * Needs content/advanced_ae (ae2/scripts/prepare-addon.ts advancedae), for ExtendedAE's provider content/extendedae,
 * and a client built with ae2-advancedae. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { addonContent } from '../../scripts/common';
import { WorldFixture, until, type Mode } from './harness';

const extendedae = (() => { try { return existsSync(addonContent('extendedae')); } catch { return false; } })();
const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('advancedae-providers', mode, { addons: extendedae ? ['advanced_ae', 'extendedae'] : ['advanced_ae'] });
const SMALL = 'advanced_ae:small_adv_pattern_provider', LARGE = 'advanced_ae:adv_pattern_provider';
const SMALL_PART = 'advanced_ae:small_adv_pattern_provider_part', LARGE_PART = 'advanced_ae:adv_pattern_provider_part';
const UPGRADE = 'advanced_ae:adv_pattern_provider_upgrade', CAPACITY = 'advanced_ae:adv_pattern_provider_capacity_upgrade';
const PATTERN = 'advanced_ae:adv_processing_pattern', AE2_PATTERN = 'ae2:processing_pattern';
const processing = (inputs: string[], output: string) => `${AE2_PATTERN}[ae2:encoded_processing_pattern={sparseInputs:[${inputs.map(id => `{"#t":"ae2:i",id:"${id}","#":1L}`).join(',')}],sparseOutputs:[{"#t":"ae2:i",id:"${output}","#":1L}]}]`;
const cell = 'ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:raw_iron","#":16L},{"#t":"ae2:i",id:"minecraft:coal","#":16L}]]';
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slotOf = (view: any, index: number) => view.slots?.find((slot: any) => slot?.slot === index) ?? view.slots?.[index];
const at300 = (view: any, index: number) => view.slots?.find((slot: any) => slot.slot === 300 + index)?.item;
const toolbar = (view: any) => view.ae2_screen.toolbar.map((b: any) => b.entry).filter((e: any) => e !== undefined);
const chat = (reply: any) => reply.chat?.join('\n') ?? '';
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 28, y = Math.ceil(py) + 5, z = Math.floor(pz) + 28;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -4], [-3, 4], [13, -4], [13, 4]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -4)} ${at(13, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -4)} ${at(13, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s');
  // Grid one: power, a drive, a terminal with a CPU above, the cable that takes the large part (a furnace north of it
  // later), the large block. Grid two, apart: power, AE2's provider, ExtendedAE's, a cable for AE2's provider part.
  const power = at(0), drive = at(1), terminal = at(2), partHost = at(3), furnace = at(3, 0, -1), large = at(4);
  const power2 = at(7), kitProvider = at(8), exProvider = at(9), kitHost = at(10);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [at(2, 1), 'ae2:1k_crafting_storage'],
    [partHost, 'ae2:cable_bus'], [large, LARGE], [power2, 'ae2:creative_energy_cell'], [kitProvider, 'ae2:pattern_provider'],
    [exProvider, extendedae ? 'extendedae:ex_pattern_provider' : 'ae2:cable_bus'], [kitHost, 'ae2:cable_bus']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ${cell}`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  const online = (pos: string) => until(() => fixture.ae(`grid status ${pos}`), view => view.online && view.powered, `online ${pos}`, 30000);
  await online(large); await online(kitProvider);
  await until(() => fixture.ae(`grid status ${drive}`), view => count(view, 'minecraft:raw_iron') === 16 && count(view, 'minecraft:coal') === 16, 'the cell is mounted', 30000);

  // The large block: 36 pattern slots on one screen, AE2's settings and Filtered Import after them, the priority tab.
  await fixture.command('clear @s');
  await fixture.command(`item replace entity @s hotbar.1 with ${processing(['minecraft:cobblestone'], 'minecraft:stone')}`);
  await fixture.command(`item replace entity @s hotbar.2 with ${processing(['minecraft:sand'], 'minecraft:glass')}`);
  await until(() => fixture.raw('inventory'), view => view.counts?.[AE2_PATTERN] === 2, 'patterns in the inventory');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${large} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'the large provider\'s screen', 15000);
  assert.equal(view.ae2_screen.style, 'adv_pattern_provider.json');
  assert.deepEqual(toolbar(view), [45, 46, 47, 48]);
  assert(view.ae2_screen.priority, 'priority tab');
  const inventoryPattern = (v: any) => v.slots.find((s: any) => s.slot < 36 && s.item === AE2_PATTERN)?.slot;
  await fixture.raw(`click-slot ${inventoryPattern(view)}`);
  await until(() => fixture.raw('inventory'), v => v.carried?.item === AE2_PATTERN, 'a pattern on the cursor');
  await fixture.raw('click-slot 335');
  view = await until(() => fixture.raw('screen'), v => at300(v, 35) === AE2_PATTERN, 'a pattern in the 36th slot');
  await fixture.raw(`click-slot ${inventoryPattern(view)} shift`);
  await until(() => fixture.raw('screen'), v => at300(v, 0) === AE2_PATTERN && at300(v, 35) === AE2_PATTERN, 'a pattern in the first slot');
  assert(fixture.settingIs(view, 48, 'NO'), 'Filtered Import off');
  await fixture.raw('close');
  fixture.mark('the large provider opens Advanced AE\'s screen of 36 patterns with its settings after them');

  // The large part on the cable's north face, its pattern put in from the north before the furnace goes there.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 2.5}`);
  await fixture.replace('hotbar.0', LARGE_PART); await fixture.select(0, LARGE_PART);
  assert.equal((await fixture.ae(`grid part ${partHost} north`)).attached, true);
  await fixture.command('clear @s');
  await fixture.command(`item replace entity @s hotbar.1 with ${processing(['minecraft:raw_iron', 'minecraft:coal'], 'minecraft:iron_ingot')}`);
  await until(() => fixture.raw('inventory'), v => v.counts?.[AE2_PATTERN] === 1, 'the iron pattern in the inventory');
  assert.equal((await fixture.ae('advae encode 1')).item, PATTERN);
  const shown = await fixture.ae('advae direction 1 minecraft:raw_iron up');
  assert.deepEqual(shown.inputs.map((i: any) => [i.id, i.direction]), [['minecraft:raw_iron', 'up'], ['minecraft:coal', 'none']]);
  await online(partHost);
  await fixture.raw(`interact ${partHost} north`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'the large part\'s screen', 15000);
  assert.equal(view.ae2_screen.style, 'adv_pattern_provider.json', 'the part opens its block\'s screen');
  assert.deepEqual(toolbar(view), [45, 46, 47, 48]);
  await fixture.raw(`click-slot ${view.slots.find((s: any) => s.slot < 36 && s.item === PATTERN)?.slot} shift`);
  await until(() => fixture.raw('screen'), v => at300(v, 0) === PATTERN, 'the Advanced pattern in the part');
  await fixture.raw('close');
  await fixture.command(`setblock ${furnace} minecraft:furnace[facing=north]`);
  fixture.mark('the large part attaches and opens the large provider\'s screen');

  // A craft of one iron ingot: the part sends the raw iron through the furnace's top and the coal through its side.
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${terminal} north`);
  const entry = (v: any) => v.slots?.find((slot: any) => slot.slot >= 300 && slot.slot < 345 && slot.item === 'minecraft:iron_ingot');
  await until(() => fixture.raw('screen'), entry, 'iron ingot craftable', 30000);
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
  const furnaceSlots = async () => {
    const c = await fixture.raw(`container ${furnace}`);
    return [0, 1, 2].map(i => { const s = slotOf(c, i); return s?.item ? `${s.item}x${s.count}` : '-'; });
  };
  const pushed = await until(furnaceSlots, slots => slots[0].startsWith('minecraft:raw_iron') || slots[2].startsWith('minecraft:iron_ingot'), 'raw iron in the furnace\'s input slot', 60000);
  assert(!pushed[1].startsWith('minecraft:raw_iron'), `no raw iron in the fuel slot: ${pushed}`);
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:raw_iron') === 15 && count(v, 'minecraft:coal') === 15, 'both inputs left the grid', 30000);
  await until(furnaceSlots, slots => slots[2].startsWith('minecraft:iron_ingot'), 'the furnace smelts with the coal it got through its side', 60000);
  assert.doesNotMatch(chat(await fixture.command(`data get block ${partHost}`)), /advanced_ae_send_faces/, 'nothing waits in the send list');
  fixture.mark('the large part pushes raw iron through the furnace\'s top and coal through the side it faces');

  // AE2's provider, a pattern in it and blocking mode on: the upgrade makes it the small Advanced one, the Capacity
  // Upgrade the large one, each keeping the pattern and the setting.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 8.5} ${y} ${z + 2.5}`);
  await fixture.command(`item replace entity @s hotbar.1 with ${processing(['minecraft:clay_ball'], 'minecraft:brick')}`);
  await until(() => fixture.raw('inventory'), v => v.counts?.[AE2_PATTERN] === 1, 'the brick pattern in the inventory');
  await fixture.raw(`interact ${kitProvider} south`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'pattern_provider.json', 'AE2\'s provider screen', 15000);
  await fixture.raw(`click-slot ${inventoryPattern(view)} shift`);
  await until(() => fixture.raw('screen'), v => at300(v, 0) === AE2_PATTERN, 'the pattern in AE2\'s provider');
  await fixture.pressSetting(18);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 18, 'YES'), 'blocking mode on');
  await fixture.raw('close');
  const kit = async (item: string, target: string, side: string, check: () => Promise<string>, id: string) => {
    await fixture.replace('hotbar.2', item, 2); await fixture.select(2, item);
    await fixture.sneakInteract(`${target} ${side}`);
    await until(check, text => text.includes(id), `${id} in place`, 15000);
    await until(() => fixture.raw('inventory'), v => v.counts?.[item] === 2, `${item} kept in creative`);
    await fixture.raw('hotbar 1');
  };
  const blockAt = (pos: string) => async () => JSON.stringify(await fixture.raw(`block ${pos}`));
  const opened = async (pos: string, side: string, style: string) => {
    await fixture.raw(`interact ${pos} ${side}`);
    return until(() => fixture.raw('screen'), v => v.ae2_screen?.style === style, style, 15000);
  };
  await kit(UPGRADE, kitProvider, 'south', blockAt(kitProvider), SMALL);
  view = await opened(kitProvider, 'south', 'small_adv_pattern_provider.json');
  assert.equal(at300(view, 0), AE2_PATTERN, 'the small one keeps the pattern');
  assert(fixture.settingIs(view, 18, 'YES'), 'and blocking mode');
  await fixture.raw('close');
  await kit(CAPACITY, kitProvider, 'south', blockAt(kitProvider), LARGE);
  view = await opened(kitProvider, 'south', 'adv_pattern_provider.json');
  assert.equal(at300(view, 0), AE2_PATTERN, 'the large one keeps the pattern');
  assert(fixture.settingIs(view, 45, 'YES'), 'and blocking mode');
  await fixture.raw('close');
  fixture.mark('the upgrade makes AE2\'s provider the small Advanced one, the Capacity Upgrade the large one, with its pattern and settings');

  if (extendedae) {
    // ExtendedAE's provider, a pattern in its 21st slot: the upgrade makes it the large one.
    await fixture.command(`tp @s ${x + 9.5} ${y} ${z + 2.5}`);
    await fixture.command(`item replace entity @s hotbar.1 with ${processing(['minecraft:kelp'], 'minecraft:dried_kelp')}`);
    view = await opened(exProvider, 'south', 'ex_pattern_provider.json');
    await fixture.raw(`click-slot ${inventoryPattern(view)}`);
    await until(() => fixture.raw('inventory'), v => v.carried?.item === AE2_PATTERN, 'a pattern on the cursor');
    await fixture.raw('click-slot 320');
    await until(() => fixture.raw('screen'), v => at300(v, 20) === AE2_PATTERN, 'the pattern in ExtendedAE\'s provider');
    await fixture.raw('close');
    await kit(UPGRADE, exProvider, 'south', blockAt(exProvider), LARGE);
    view = await opened(exProvider, 'south', 'adv_pattern_provider.json');
    assert.equal(at300(view, 20), AE2_PATTERN, 'the large one keeps it in its slot');
    await fixture.raw('close');
    fixture.mark('the upgrade makes ExtendedAE\'s provider the large Advanced one in place');
  }

  // AE2's provider part: the upgrade makes it the small part, the Capacity Upgrade the large one, with its pattern.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 10.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'ae2:cable_pattern_provider'); await fixture.select(0, 'ae2:cable_pattern_provider');
  assert.equal((await fixture.ae(`grid part ${kitHost} south`)).attached, true);
  await fixture.command('clear @s');
  await fixture.command(`item replace entity @s hotbar.1 with ${processing(['minecraft:wheat'], 'minecraft:bread')}`);
  view = await opened(kitHost, 'south', 'pattern_provider.json');
  await fixture.raw(`click-slot ${inventoryPattern(view)} shift`);
  await until(() => fixture.raw('screen'), v => at300(v, 0) === AE2_PATTERN, 'the pattern in AE2\'s provider part');
  await fixture.raw('close');
  const hostData = (pos: string) => async () => chat(await fixture.command(`data get block ${pos}`));
  await kit(UPGRADE, kitHost, 'south', hostData(kitHost), SMALL_PART);
  await kit(CAPACITY, kitHost, 'south', hostData(kitHost), LARGE_PART);
  view = await opened(kitHost, 'south', 'adv_pattern_provider.json');
  assert.equal(at300(view, 0), AE2_PATTERN, 'the large part keeps the pattern');
  await fixture.pressSetting(48);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 48, 'YES'), 'Filtered Import on the part');
  await fixture.raw('close');
  fixture.mark('the upgrades turn AE2\'s provider part into the small and then the large part, with its pattern; Filtered Import on the part');

  // A restart keeps the providers, their patterns and settings.
  await fixture.restart();
  const kept = await hostData(kitProvider)();
  assert.match(kept, /minecraft:clay_ball/); assert.match(kept, /ae2_blocking: 1b/);
  assert(JSON.stringify(await fixture.raw(`block ${kitProvider}`)).includes(LARGE), 'the large provider in place');
  const host = await hostData(kitHost)();
  assert.match(host, new RegExp(`"${LARGE_PART}"`)); assert.match(host, /minecraft:wheat/); assert.match(host, /filtered_import: "YES"/);
  const part = await hostData(partHost)();
  assert.match(part, new RegExp(`"${LARGE_PART}"`)); assert.match(part, /directionMap/);
  assert.match(await hostData(large)(), /minecraft:cobblestone/);
  fixture.mark('the upgraded providers, the parts and the patterns survive a restart');

  // Broken, the large block and the large part drop themselves and their patterns.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 2.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((e: any) => e.item?.item === item).reduce((n: number, e: any) => n + e.item.count, 0);
  await fixture.command(`setblock ${large} minecraft:air destroy`);
  await until(() => ground(AE2_PATTERN), n => n === 2, 'the large block\'s two patterns on the ground', 30000);
  assert.equal(await ground(LARGE), 1, 'the block itself');
  await fixture.command(`setblock ${kitHost} minecraft:air destroy`);
  await until(() => ground(LARGE_PART), n => n === 1, 'the large part on the ground', 30000);
  await until(() => ground(AE2_PATTERN), n => n === 3, 'and its pattern', 30000);
  fixture.mark('broken, the large block and the large part drop themselves and their patterns');

  // The recipes: the part from the block, the upgrade from an ingot, redstone, an ender pearl and a logic processor.
  await fixture.command('kill @e[type=minecraft:item]');
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
  await craft([LARGE], LARGE_PART);
  await craft([SMALL_PART], SMALL);
  await craft(['minecraft:iron_ingot', 'minecraft:redstone', 'minecraft:ender_pearl', 'ae2:logic_processor'], UPGRADE);
  await fixture.command('gamemode creative @s');
  await until(() => fixture.raw('status'), s => s.creative === true, 'creative again');
  fixture.mark('the part forms and the upgrade come from Advanced AE\'s recipes');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
