/** Advanced AE slice 2: the Reaction Chamber. Its own buffer of 500,000 AE takes Forge Energy on every side. Its
 * recipe of fluix (16 each of charged certus, redstone and quartz and 500 mB of water, 200,000 AE) runs on the grid's
 * power, 2,000 AE a step for 100 steps without a card (ReactionChamberEntity.tickingRequest), with the block state's
 * `working` on meanwhile. Its screen (reaction_chamber.json): the tanks, the flush button, a bucket from the cursor,
 * four speed cards; AUTO_EXPORT shows the six output sides, and the front sends the fluix to a chest there. With four
 * cards the next craft takes 4 steps of 50,000 AE, its water from a bucket used on the block; an empty bucket takes a
 * bucket of the output tank. An AE2 Pattern Provider beside it pushes a processing pattern's items and water into it
 * and gets the fluix back through the chamber's side that faces it. The tanks, cards and settings survive a restart;
 * broken, it drops its inputs and cards. Needs content/advanced_ae (ae2/scripts/prepare-addon.ts advancedae) and a
 * client built with ae2-advancedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('advancedae-reaction-chamber', mode, { addons: ['advanced_ae'] });
const CHAMBER = 'advanced_ae:reaction_chamber', FLUIX = 'ae2:fluix_crystal';
const INPUTS: [string, number][] = [['ae2:charged_certus_quartz_crystal', 16], ['minecraft:redstone', 16], ['minecraft:quartz', 16]];
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slot = (view: any, n: number) => view.slots?.find((s: any) => s.slot === n);
const entries = (view: any) => view.ae2_screen.toolbar.map((b: any) => b.entry).filter((e: any) => e !== undefined);
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -5], [-3, 6], [14, -5], [14, 6]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -5)} ${at(14, 4, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -5)} ${at(14, -1, 6)} minecraft:stone`);
  await fixture.command('clear @s');
  const contents = async (pos: string) => (await fixture.command(`data get block ${pos}`)).chat?.join('\n') ?? '';
  const container = async (pos: string, item: string) => ((await fixture.raw(`container ${pos}`)).slots ?? []).filter((s: any) => s?.item === item).reduce((n: number, s: any) => n + s.count, 0);
  const online = (pos: string) => until(() => fixture.ae(`grid status ${pos}`), view => view.online && view.powered, `online ${pos}`, 30000);

  // A dense energy cell of 1,000,000 AE and the chamber (facing north).
  const power = at(0), chamber = at(1), chest = at(1, 0, -1);
  await fixture.command(`setblock ${power} ae2:dense_energy_cell`);
  await fixture.command(`setblock ${chamber} ${CHAMBER}`);
  await fixture.command(`data merge block ${power} {ae2_lab_energy:1000000.0d}`);
  await online(chamber);
  const gridEnergy = async () => (await fixture.ae(`grid status ${power}`)).energy;
  await until(gridEnergy, e => e === 1000000, 'the cell holds 1,000,000 AE');
  for (const face of ['up', 'north', 'east']) assert.equal((await fixture.ae(`power ${chamber} ${face}`)).fe?.capacity, 1000000, `500,000 AE on its ${face}`);
  fixture.mark('the Reaction Chamber\'s own buffer of 500,000 AE takes power on every side');

  // Fluix: the inputs and a bucket of water; 100 steps of 2 at 2,000 AE.
  for (const [i, [id, n]] of INPUTS.entries()) await fixture.command(`item replace block ${chamber} container.${i} with ${id} ${n}`);
  await fixture.command(`data merge block ${chamber} {ae2_lab_config:{tank_in:{id:"minecraft:water",amount:1000L}}}`);
  await until(async () => JSON.stringify(await fixture.raw(`block ${chamber}`)), state => /working[^a-z]+true/.test(state), 'working', 30000);
  await until(() => contents(chamber), text => text.includes(FLUIX), 'fluix made', 60000);
  const made = await contents(chamber);
  assert.match(made, /Slot: 9b[^}]*count: 64|count: 64[^}]*Slot: 9b/, '64 in the output slot');
  assert.doesNotMatch(made, /charged_certus|"minecraft:redstone"|"minecraft:quartz"/, 'the inputs used up');
  assert.match(made, /tank_in: \{[^}]*amount: 500L/, '500 mB of water left');
  assert.equal(await gridEnergy(), 800000, '200,000 AE from the grid');
  await until(async () => JSON.stringify(await fixture.raw(`block ${chamber}`)), state => /working[^a-z]+false/.test(state), 'no longer working', 30000);
  fixture.mark('the Reaction Chamber makes 64 fluix from its recipe for 200,000 AE of the grid');

  // The screen: its style, AUTO_EXPORT alone on the toolbar, the tanks; flush, a bucket from the cursor, four cards.
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'minecraft:water_bucket'); await fixture.replace('hotbar.1', 'ae2:speed_card', 4);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${chamber} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Reaction Chamber screen', 15000);
  assert.equal(view.ae2_screen.style, 'reaction_chamber.json');
  assert.deepEqual(entries(view), [24, 16, 17], 'AUTO_EXPORT, then the two flush buttons');
  assert(fixture.settingIs(view, 24, 'NO'), 'auto export off');
  assert.match(JSON.stringify(slot(view, 314) ?? ''), /minecraft:water: 500 \/ 16000 mB/, 'the input tank');
  assert(!slot(view, 315)?.item, 'the output tank is empty');
  assert.equal(slot(view, 309)?.item, FLUIX);
  await fixture.pressSetting(16);
  await until(() => fixture.raw('screen'), v => !slot(v, 314)?.item, 'the input tank flushed');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:water_bucket', 'a water bucket held');
  await fixture.raw('click-slot 314');
  await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:bucket', 'the bucket emptied into the tank');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => !s.carried, 'bucket back');
  view = await until(() => fixture.raw('screen'), v => JSON.stringify(slot(v, 314) ?? '').includes('1000 / 16000 mB'), 'a bucket in the input tank');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:speed_card', 'cards held');
  for (const entry of [310, 311, 312, 313]) await fixture.raw(`click-slot ${entry}`);
  await until(() => fixture.raw('screen'), v => [310, 311, 312, 313].every(n => slot(v, n)?.item === 'ae2:speed_card') && !v.carried, 'four speed cards in');
  // Only what a recipe uses goes into an input.
  await fixture.replace('hotbar.2', 'minecraft:dirt', 5);
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:dirt', 'dirt held');
  await fixture.raw('click-slot 300'); await fixture.step(5);
  assert(!slot(await fixture.raw('screen'), 300)?.item, 'no dirt in an input');
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), s => !s.carried, 'dirt back');
  fixture.mark('its screen shows the tanks, flushes one, fills it from a bucket and takes four speed cards and only recipe inputs');

  // AUTO_EXPORT shows the six sides; the front sends the fluix into the chest there.
  await fixture.pressSetting(24);
  view = await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 24, 'YES'), 'auto export on');
  assert.deepEqual(entries(view), [24, 18, 19, 20, 21, 22, 23, 16, 17], 'the six sides on the toolbar');
  await fixture.pressSetting(18);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 18, 'YES'), 'the front on');
  await fixture.raw('close');
  await fixture.command(`setblock ${chest} minecraft:chest`);
  await until(() => container(chest, FLUIX), n => n === 64, 'the fluix pushed into the chest at the front', 30000);
  fixture.mark('with AUTO_EXPORT the output goes out of the sides chosen');

  // Water from a bucket used on the block; with four cards, 4 steps of 50,000 AE; the fluix goes to the chest.
  await fixture.replace('hotbar.0', 'minecraft:water_bucket'); await fixture.select(0, 'minecraft:water_bucket');
  await fixture.raw(`interact ${chamber} south`);
  await until(() => fixture.raw('inventory'), s => s.counts?.['minecraft:bucket'] >= 1 && !s.counts?.['minecraft:water_bucket'], 'the bucket used on the chamber');
  await until(() => contents(chamber), text => /tank_in: \{[^}]*amount: 2000L/.test(text), 'two buckets in the input tank');
  for (const [i, [id, n]] of INPUTS.entries()) await fixture.command(`item replace block ${chamber} container.${i} with ${id} ${n}`);
  await until(() => container(chest, FLUIX), n => n === 128, 'the second 64 in the chest', 60000);
  assert.equal(await gridEnergy(), 600000, 'four cards: 4 steps of 50,000 AE');
  assert.match(await contents(chamber), /tank_in: \{[^}]*amount: 1500L/);
  // An empty bucket takes a bucket of the output tank.
  await fixture.command(`data merge block ${chamber} {ae2_lab_config:{tank_out:{id:"minecraft:lava",amount:1500L}}}`);
  await fixture.replace('hotbar.0', 'minecraft:bucket'); await fixture.select(0, 'minecraft:bucket');
  await fixture.raw(`interact ${chamber} south`);
  await until(() => fixture.raw('inventory'), s => s.counts?.['minecraft:lava_bucket'] === 1, 'a lava bucket from the output tank');
  assert.match(await contents(chamber), /tank_out: \{[^}]*amount: 500L/);
  fixture.mark('buckets fill its input tank and empty its output tank; four cards run 50 a step for the same 200,000 AE');

  // An AE2 Pattern Provider with a processing pattern of the fluix recipe, the chamber east of it sending back west (its left).
  const cell = 'ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"ae2:charged_certus_quartz_crystal","#":16L},{"#t":"ae2:i",id:"minecraft:redstone","#":16L},{"#t":"ae2:i",id:"minecraft:quartz","#":16L}]]';
  const water = 'ae2:fluid_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:f",id:"minecraft:water","#":2000L}]]';
  const pattern = 'ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"ae2:charged_certus_quartz_crystal","#":16L},{"#t":"ae2:i",id:"minecraft:redstone","#":16L},{"#t":"ae2:i",id:"minecraft:quartz","#":16L},{"#t":"ae2:f",id:"minecraft:water","#":500L}],sparseOutputs:[{"#t":"ae2:i",id:"ae2:fluix_crystal","#":64L}]}]';
  const cpuPower = at(6), drive = at(7), terminal = at(8), provider = at(9), second = at(10);
  for (const [pos, id] of [[cpuPower, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [at(8, 1), 'ae2:1k_crafting_storage'], [provider, 'ae2:pattern_provider'], [second, CHAMBER]] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ${cell}`);
  await fixture.command(`item replace block ${drive} container.1 with ${water}`);
  await fixture.command(`data merge block ${second} {ae2_lab_config:{auto_export:"YES",output_left:1b}}`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  await fixture.command('item replace entity @s hotbar.0 with minecraft:air');
  await online(provider); await online(second);
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:quartz') === 16, 'the cells mounted', 30000);
  // The pattern into the provider through its screen.
  await fixture.command(`item replace entity @s hotbar.1 with ${pattern}`);
  await fixture.command(`tp @s ${x + 9.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  view = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern'), 'the provider\'s screen', 15000);
  await fixture.raw(`click-slot ${view.slots.find((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern').slot} shift`);
  await until(() => fixture.raw('screen'), v => v.slots.some((s: any) => s.slot >= 300 && s.slot < 309 && s.item === 'ae2:processing_pattern'), 'the pattern in the provider');
  await fixture.raw('close');
  await fixture.command(`tp @s ${x + 8.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${terminal} north`);
  const entry = (v: any) => v.slots?.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === FLUIX);
  await until(() => fixture.raw('screen'), entry, 'fluix craftable', 30000);
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
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, FLUIX) === 64, 'the fluix back in the grid', 90000);
  const used = await fixture.ae(`grid status ${drive}`);
  assert.equal(count(used, 'minecraft:quartz'), 0, 'the items pushed');
  assert.doesNotMatch(await contents(second), /tank_in/, 'the water pushed and used');
  fixture.mark('an AE2 Pattern Provider pushes the items and water into it and the fluix comes back through its side');

  // Through a restart: the tanks, cards and settings stay.
  await fixture.restart();
  await online(chamber);
  const kept = await contents(chamber);
  assert.match(kept, /tank_in: \{[^}]*amount: 1500L/, 'the input tank');
  assert.match(kept, /tank_out: \{[^}]*amount: 500L/, 'the output tank');
  assert.equal((kept.match(/ae2:speed_card/g) ?? []).length, 4, 'the cards');
  assert.match(kept, /auto_export: "YES"/);
  assert.match(kept, /output_front: 1b/);
  fixture.mark('the tanks, cards and settings survive a restart');

  // Broken, it drops its inputs and cards.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 4.5}`);
  await fixture.command(`item replace block ${chamber} container.4 with minecraft:redstone 7`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((e: any) => e.item?.item === item).reduce((n: number, e: any) => n + e.item.count, 0);
  await fixture.command(`setblock ${chamber} minecraft:air destroy`);
  await until(() => ground('ae2:speed_card'), n => n === 4, 'the four cards on the ground', 30000);
  await until(() => ground('minecraft:redstone'), n => n === 7, 'the input on the ground', 30000);
  assert.equal(await ground(CHAMBER), 1, 'the block itself');
  fixture.mark('broken, the Reaction Chamber drops its inputs and cards');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
