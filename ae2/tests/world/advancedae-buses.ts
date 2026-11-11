/** Advanced AE slice 5: the buses and the Throughput Monitor. The ME Stock Export Bus (StockExportBusPart) keeps
 * each filter's amount in the chest it faces: the held stack's count when the filter is set, then the amount set by
 * command (the mod's SetAmountMenu); it refills what is taken. The ME Import Export Bus (ImportExportBusPart) exports
 * its filter as AE2's bus and imports what its filter does not list. The ME Advanced IO Bus (AdvancedIOBusPart, eight
 * cards, eight times the operations) imports what goes over its amount (REGULATE_STOCK, toggled on its screen) and
 * then the rest. Their screens are the mod's (no fuzzy button, the amounts on the filters). The ME Throughput Monitor
 * (ThroughputMonitorPart) shows how fast its key's amount changes, 0 for a key that stays, and its configurator turns
 * the routine. A restart keeps the amounts, REGULATE_STOCK and the routine; broken, the cable drops the parts and
 * their cards; the monitor comes from its recipe. Needs content/advanced_ae (ae2/scripts/prepare-addon.ts
 * advancedae) and a client built with ae2-advancedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('advancedae-buses', mode, { addons: ['advanced_ae'] });
const STOCK = 'advanced_ae:stock_export_bus_part', IMPORT_EXPORT = 'advanced_ae:import_export_bus_part', ADVANCED = 'advanced_ae:advanced_io_bus_part';
const MONITOR = 'advanced_ae:throughput_monitor', CONFIGURATOR = 'advanced_ae:throughput_monitor_configurator';
const cell = 'ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:cobblestone","#":2000L},{"#t":"ae2:i",id:"minecraft:sand","#":500L},{"#t":"ae2:i",id:"minecraft:gravel","#":300L}]]';
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
const chat = (reply: any) => reply.chat?.join('\n') ?? '';
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 26, y = Math.ceil(py) + 5, z = Math.floor(pz) + 26;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -4], [-3, 4], [10, -4], [10, 4]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -4)} ${at(10, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -4)} ${at(10, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s');
  // One grid: power, a drive, then four cables in a row; each bus faces north, where its chest goes once its screen
  // has been looked at, and the last cable takes two monitors.
  const power = at(0), drive = at(1), stockHost = at(2), ioHost = at(3), advHost = at(4), monitorHost = at(5);
  const stockChest = at(2, 0, -1), ioChest = at(3, 0, -1), advChest = at(4, 0, -1);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [stockHost, 'ae2:cable_bus'], [ioHost, 'ae2:cable_bus'], [advHost, 'ae2:cable_bus'], [monitorHost, 'ae2:cable_bus']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ${cell}`);
  const stored = async (item: string) => ((await fixture.ae(`grid status ${drive}`)).items ?? []).find((e: any) => e.item === item)?.count ?? 0;
  const inChest = async (pos: string, item: string) => ((await fixture.raw(`container ${pos}`)).slots ?? [])
    .filter((slot: any) => slot?.item === item).reduce((n: number, slot: any) => n + slot.count, 0);
  await until(() => stored('minecraft:cobblestone'), n => n === 2000, 'the drive online', 30000);
  const place = async (host: string, face: string, id: string) => {
    await fixture.replace('hotbar.0', id); await fixture.select(0, id);
    assert.equal((await fixture.ae(`grid part ${host} ${face}`)).attached, true, id);
  };
  const install = async (host: string, cards: [string, number][]) => {
    for (const [card, n] of cards) {
      await fixture.replace('hotbar.0', card, n); await fixture.select(0, card);
      for (let i = 0; i < n; i++) await fixture.ae(`bus install ${host} north`);
    }
  };
  const filter = async (host: string, item: string, count: number) => {
    await fixture.replace('hotbar.0', item, count); await fixture.select(0, item);
    await fixture.ae(`bus filter ${host} north 0`);
  };
  const part = (test: (p: any) => boolean, label: string) => until(() => fixture.raw('screen'), v => v.ae2_part && test(v.ae2_part), label, 15000).then(v => v.ae2_part);
  const open = async (host: string, dx: number, id: string) => {
    await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
    await fixture.command(`tp @s ${x + dx + 0.5} ${y} ${z - 2.5}`);
    await fixture.raw(`interact ${host} north`);
    return part(p => p.id === id, `${id} screen`);
  };
  const press = (rect: number[]) => fixture.raw(`click-at ${centre(rect)}`);
  const closeScreen = async () => { await fixture.raw('close'); await until(() => fixture.raw('screen'), v => !v.ae2_part, 'screen closed'); };

  // The Stock Export Bus: AE2's export bus with six cards; a filter set with 10 cobblestone keeps 10.
  await place(stockHost, 'north', STOCK);
  assert.equal((await fixture.ae(`bus status ${stockHost} north`)).id, 'ae2:export_bus', 'it behaves as AE2\'s export bus');
  await filter(stockHost, 'minecraft:cobblestone', 10);
  assert.equal((await fixture.ae(`advae bus ${stockHost} north`)).amounts[0], 10, 'the held count');
  await install(stockHost, [['ae2:speed_card', 4], ['ae2:capacity_card', 2]]);
  await fixture.replace('hotbar.0', 'ae2:redstone_card'); await fixture.select(0, 'ae2:redstone_card');
  assert.match(JSON.stringify(await fixture.command(`ae2 bus install ${stockHost} north`)), /slots full/, 'no seventh card');
  let view = await open(stockHost, 2, STOCK);
  assert.equal(view.kind, 'ae2:export_bus');
  assert.equal(view.upgrades, 6);
  assert.equal(view.config[0].item, 'minecraft:cobblestone');
  assert.equal(view.config[0].amount, '10', 'the amount on its filter');
  assert.equal(view.enabled, 36, 'two capacity cards');
  await closeScreen();
  await fixture.command(`setblock ${stockChest} minecraft:chest`);
  await until(() => inChest(stockChest, 'minecraft:cobblestone'), n => n === 10, 'the chest stocked to 10', 30000);
  await fixture.command(`item replace block ${stockChest} container.0 with minecraft:cobblestone 6`);
  await fixture.command(`item replace block ${stockChest} container.5 with minecraft:dirt 3`);
  await until(() => inChest(stockChest, 'minecraft:cobblestone'), n => n === 10, 'refilled to 10', 30000);
  await fixture.ae(`advae stock ${stockHost} north 0 100`);
  await until(() => inChest(stockChest, 'minecraft:cobblestone'), n => n === 100, 'stocked to the new amount', 30000);
  assert.equal(await inChest(stockChest, 'minecraft:dirt'), 3, 'it imports nothing');
  fixture.mark('the Stock Export Bus keeps each filter\'s amount in what it faces, refilling what is taken');

  // The Import Export Bus: sand out as AE2's export bus, the dirt and cobblestone in, as its filter does not list them.
  await place(ioHost, 'north', IMPORT_EXPORT);
  await filter(ioHost, 'minecraft:sand', 1);
  assert.equal((await fixture.ae(`advae bus ${ioHost} north`)).stocks, false, 'AE2\'s filter of keys');
  view = await open(ioHost, 3, IMPORT_EXPORT);
  assert.equal(view.upgrades, 5);
  assert.equal(view.config[0].amount, null, 'no amounts');
  await closeScreen();
  await fixture.command(`setblock ${ioChest} minecraft:chest`);
  await fixture.command(`item replace block ${ioChest} container.3 with minecraft:dirt 6`);
  await fixture.command(`item replace block ${ioChest} container.4 with minecraft:cobblestone 5`);
  await until(async () => [await inChest(ioChest, 'minecraft:dirt'), await inChest(ioChest, 'minecraft:cobblestone'), await inChest(ioChest, 'minecraft:sand')],
    ([dirt, cobble, sand]) => dirt === 0 && cobble === 0 && sand > 0, 'dirt and cobblestone in, sand out', 30000);
  assert.equal(await stored('minecraft:dirt'), 6);
  fixture.mark('the Import Export Bus exports its filter and imports what the filter does not list');

  // The Advanced IO Bus: eight cards; its screen has REGULATE_STOCK and no fuzzy button with the fuzzy card.
  await place(advHost, 'north', ADVANCED);
  await filter(advHost, 'minecraft:cobblestone', 20);
  await install(advHost, [['ae2:speed_card', 4], ['ae2:capacity_card', 3], ['ae2:fuzzy_card', 1]]);
  await fixture.replace('hotbar.0', 'ae2:redstone_card'); await fixture.select(0, 'ae2:redstone_card');
  assert.match(JSON.stringify(await fixture.command(`ae2 bus install ${advHost} north`)), /slots full/, 'no ninth card');
  view = await open(advHost, 4, ADVANCED);
  assert.equal(view.upgrades, 8);
  assert.equal(view.config[0].amount, '20');
  const places = view.tools.map((t: any) => t.place);
  assert(!places.includes(44), `no fuzzy mode button: ${places}`);
  const regulate = () => view.tools.find((t: any) => t.place === 48);
  assert.equal(regulate()?.value, 'gui.tooltips.advanced_ae.RegulateOn');
  await press(regulate().rect);
  view = await part(p => p.tools.find((t: any) => t.place === 48)?.value === 'gui.tooltips.advanced_ae.RegulateOff', 'regulation off');
  assert.equal((await fixture.ae(`advae bus ${advHost} north`)).regulate, false);
  await press(regulate().rect);
  view = await part(p => p.tools.find((t: any) => t.place === 48)?.value === 'gui.tooltips.advanced_ae.RegulateOn', 'regulation on');
  await closeScreen();
  await fixture.command(`setblock ${advChest} minecraft:chest`);
  await fixture.command(`item replace block ${advChest} container.0 with minecraft:cobblestone 50`);
  await fixture.command(`item replace block ${advChest} container.1 with minecraft:dirt 10`);
  await until(async () => [await inChest(advChest, 'minecraft:cobblestone'), await inChest(advChest, 'minecraft:dirt')],
    ([cobble, dirt]) => cobble === 20 && dirt === 0, 'the excess and the dirt imported', 30000);
  await fixture.command(`item replace block ${advChest} container.0 with minecraft:cobblestone 5`);
  await until(() => inChest(advChest, 'minecraft:cobblestone'), n => n === 20, 'and exported back up to 20', 30000);
  fixture.mark('the Advanced IO Bus keeps its amount both ways and imports the rest; REGULATE_STOCK on its screen');

  // Two Throughput Monitors: cobblestone, which the Import Export Bus brings in, and gravel, which stays.
  await place(monitorHost, 'south', MONITOR);
  await place(monitorHost, 'up', MONITOR);
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 2.5}`);
  for (const [item, face] of [['minecraft:cobblestone', 'south'], ['minecraft:gravel', 'up']] as const) {
    await fixture.replace('hotbar.0', item); await fixture.select(0, item);
    if (face === 'up') await fixture.command(`tp @s ${x + 5.5} ${y + 2} ${z + 0.5} 0 90`);
    await fixture.raw(`interact ${monitorHost} ${face}`);
  }
  const rate = (face: string) => fixture.ae(`advae throughput ${monitorHost} ${face}`);
  const gravel = await until(() => rate('up'), r => r.readings >= 2 && r.value === '0', 'gravel: no change', 30000);
  assert.equal(gravel.sign, '');
  assert.equal(gravel.key, 'gui.advanced_ae.ThroughputMonitorValue', 'per second by default');
  await until(() => rate('south'), r => r.readings >= 2, 'cobblestone read', 30000);
  for (let i = 10; i < 19; i++) await fixture.command(`item replace block ${ioChest} container.${i} with minecraft:cobblestone 64`);
  const growing = await until(() => rate('south'), r => r.sign === '+' && /^[0-9.KMG]+$/.test(r.value), 'cobblestone coming in', 30000);
  fixture.mark(`the Throughput Monitor shows ${growing.sign}${growing.value}/s of cobblestone coming in and 0 of gravel`);
  // The configurator turns the routine: per minute, then its value after a reading.
  await fixture.replace('hotbar.0', CONFIGURATOR); await fixture.select(0, CONFIGURATOR);
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${monitorHost} south`);
  const minute = await until(() => rate('south'), r => r.routine === 2, 'per minute', 15000);
  assert.equal(minute.key, 'gui.advanced_ae.SlowThroughputMonitorValue');
  await until(() => rate('south'), r => r.value !== '', 'a value per minute', 30000);
  fixture.mark('the configurator turns the Throughput Monitor to per minute');

  // Through a restart: the amounts, REGULATE_STOCK, the routine and the monitors' keys.
  await fixture.restart();
  await until(() => stored('minecraft:gravel'), n => n === 300, 'the drive after restart', 30000);
  assert.equal((await fixture.ae(`advae bus ${stockHost} north`)).amounts[0], 100);
  const adv = await fixture.ae(`advae bus ${advHost} north`);
  assert.equal(adv.amounts[0], 20); assert.equal(adv.regulate, true);
  assert.equal((await rate('south')).routine, 2);
  assert.match(chat(await fixture.command(`data get block ${monitorHost}`)), /minecraft:gravel/);
  await fixture.command(`item replace block ${stockChest} container.0 with minecraft:cobblestone 1`);
  await until(() => inChest(stockChest, 'minecraft:cobblestone'), n => n === 100, 'restocked after the restart', 30000);
  await until(() => rate('up'), r => r.readings >= 2 && r.value === '0', 'the gravel monitor reads again', 30000);
  fixture.mark('the amounts, REGULATE_STOCK and the routine survive a restart');

  // Broken, the cables drop the parts and their cards.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 3.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((e: any) => e.item?.item === item).reduce((n: number, e: any) => n + e.item.count, 0);
  await fixture.command(`setblock ${stockHost} minecraft:air destroy`);
  await until(() => ground(STOCK), n => n === 1, 'the Stock Export Bus on the ground', 30000);
  await until(() => ground('ae2:speed_card'), n => n === 4, 'its speed cards', 30000);
  await fixture.command(`setblock ${monitorHost} minecraft:air destroy`);
  await until(() => ground(MONITOR), n => n === 2, 'the two monitors on the ground', 30000);
  fixture.mark('broken, the cables drop the buses with their cards and the monitors');

  // The monitor's recipe: AE2's Storage Monitor and a Calculation Processor.
  await fixture.command('kill @e[type=minecraft:item]');
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');
  const ingredients = ['ae2:calculation_processor', 'ae2:storage_monitor'];
  for (const [i, id] of ingredients.entries()) await fixture.replace(`hotbar.${i}`, id);
  await fixture.raw('open-inventory');
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 100), 'inventory crafting grid');
  for (let i = 0; i < ingredients.length; i++) {
    await fixture.raw(`click-slot ${i}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'on the cursor');
    await fixture.raw(`click-slot ${100 + i}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'in the grid');
  }
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === MONITOR), 'the recipe offers the monitor');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  await until(() => fixture.raw('inventory'), s => s.counts?.[MONITOR] === 1, 'the monitor crafted');
  await fixture.command('gamemode creative @s');
  await until(() => fixture.raw('status'), s => s.creative === true, 'creative again');
  fixture.mark('the Throughput Monitor comes from its recipe');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
