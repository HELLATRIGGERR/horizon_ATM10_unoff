/** ExtendedAE slice 2, the rest: the Extended Inscriber runs four inscriptions at once, one a thread, its
 * screen shows a thread a page and limits its inputs to its stack size, a hopper feeds only the thread whose
 * presses take the item and AUTO_EXPORT pushes every thread's output out; the Extended Charger takes four
 * items by hand and charges them all, its 3,200 AE buffer taking Forge Energy on all but its front; the
 * Extended Molecular Assembler crafts a job a provider sends it, at twice AE2's speed with its five cards;
 * an Entro Seed turns a fluix block into budding entro, which grows clusters on random ticks and from a
 * Growth Accelerator, the clusters break off with their block, and the Crystal Fixer fixes a worn one;
 * the inscriber's slots survive a restart and drop when it is broken. Needs content/extendedae
 * (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-devices', mode, { addons: ['extendedae'] });
const shots = process.env.AE2_TEST_SHOTS;
const shot = async (name: string) => { if (shots) await fixture.raw(`screenshot ${join(shots, name + '.png')}`); };
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slot = (view: any, n: number) => view.slots?.find((s: any) => s.slot === n);
// The `Items` entry of slot `n` in a `data get` text (the cards' list, before it, has slots too).
const entry = (text: string, n: number) => text.slice(text.indexOf('Items: [')).match(new RegExp(`\\{[^{}]*Slot: ${n}b[^{}]*\\}`))?.[0] ?? '';
const amountIn = (text: string, n: number, id: string) => {
  const e = entry(text, n);
  return e.includes(`"${id}"`) ? Number(/count: (\d+)/.exec(e)?.[1] ?? 0) : 0;
};
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
  await fixture.command('gamerule randomTickSpeed 0');
  const contents = async (pos: string) => (await fixture.command(`data get block ${pos}`)).chat?.join('\n') ?? '';
  const block = async (pos: string) => (await fixture.raw(`getblock ${pos}`)).block as string;
  const online = (pos: string) => until(() => fixture.ae(`grid status ${pos}`), view => view.online && view.powered, `online ${pos}`, 30000);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((e: any) => e.item?.item === item).reduce((sum: number, e: any) => sum + e.item.count, 0);

  // A grid of a creative cell, the inscriber (its front north) and the charger.
  const power = at(0), inscriber = at(1), charger = at(2);
  await fixture.command(`setblock ${power} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${inscriber} extendedae:ex_inscriber`);
  await fixture.command(`setblock ${charger} extendedae:ex_charger`);
  await online(inscriber);
  assert.equal((await fixture.ae(`power ${inscriber} east`)).fe?.capacity, 3200, '1,600 AE at 2 FE an AE');
  assert.equal((await fixture.ae(`power ${inscriber} north`)).fe, null, 'none on its front');
  assert.equal((await fixture.ae(`power ${charger} up`)).fe?.capacity, 6400, '3,200 AE');
  assert.equal((await fixture.ae(`power ${charger} north`)).fe, null);
  fixture.mark('the Extended Inscriber and Charger take power on all but their front');

  // Four threads, four presses: each its own inscription, all at once.
  const threads: [string, string, string][] = [
    ['ae2:logic_processor_press', 'minecraft:gold_ingot', 'ae2:printed_logic_processor'],
    ['ae2:calculation_processor_press', 'ae2:certus_quartz_crystal', 'ae2:printed_calculation_processor'],
    ['ae2:engineering_processor_press', 'minecraft:diamond', 'ae2:printed_engineering_processor'],
    ['ae2:silicon_press', 'ae2:silicon', 'ae2:printed_silicon'],
  ];
  // Loaded while the server waits, so that every thread starts on the same tick.
  await fixture.hold();
  for (const [n, [press, input]] of threads.entries()) {
    await fixture.command(`item replace block ${inscriber} container.${4 * n} with ${press}`);
    await fixture.command(`item replace block ${inscriber} container.${4 * n + 2} with ${input} 2`);
  }
  await fixture.hold(false);
  const first = await until(() => contents(inscriber), text => amountIn(text, 3, threads[0][2]) >= 1, 'the first thread printed one', 60000);
  for (const [n, [, , output]] of threads.entries()) assert.equal(amountIn(first, 4 * n + 3, output), 1, `thread ${n} in step with the first`);
  await until(() => contents(inscriber), text => threads.every(([, , output], n) => amountIn(text, 4 * n + 3, output) === 2), 'every thread printed two', 60000);
  fixture.mark('the Extended Inscriber\'s four threads inscribe side by side, each on its own slots');

  // Its screen: AE2's inscriber's places for the thread shown, a page at a time; the stack size.
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'minecraft:gold_ingot', 5);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${inscriber} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Extended Inscriber screen', 15000);
  assert.equal(view.ae2_screen.style, 'ex_inscriber.json');
  assert.equal(slot(view, 300)?.item, 'ae2:logic_processor_press');
  assert.equal(slot(view, 303)?.item, 'ae2:printed_logic_processor');
  for (const n of [22, 23, 24, 25, 26]) assert(view.ae2_screen.toolbar?.some((b: any) => b.entry === n), `toolbar entry ${n}`);
  await shot('ex_inscriber');
  await fixture.pressSetting(22);
  view = await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'ae2:calculation_processor_press', 'the second thread shown');
  await fixture.pressSetting(22); await fixture.pressSetting(22); await fixture.pressSetting(22);
  view = await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'ae2:silicon_press', 'the last thread shown, and no further');
  await fixture.pressSetting(23); await fixture.pressSetting(23); await fixture.pressSetting(23); await fixture.pressSetting(23);
  view = await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'ae2:logic_processor_press', 'back to the first');
  // Stack to 1: five gold placed in the empty middle leave four on the cursor.
  await fixture.pressSetting(26);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 26, 'ONE'), 'stack to one');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:gold_ingot', 'gold held');
  await fixture.raw('click-slot 302');
  await until(() => fixture.raw('inventory'), s => s.carried?.count === 4, 'one gold placed');
  await fixture.raw('click-slot 0');
  await fixture.pressSetting(26); await fixture.pressSetting(26);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 26, 'SIXTY_FOUR'), 'stack to 64 again');
  // Four speed cards, on every thread.
  await fixture.replace('hotbar.1', 'ae2:speed_card', 4);
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:speed_card', 'cards held');
  for (const n of [304, 305, 306, 307]) await fixture.raw(`click-slot ${n}`);
  await until(() => fixture.raw('screen'), v => [304, 305, 306, 307].every(n => slot(v, n)?.item === 'ae2:speed_card') && !v.carried, 'four speed cards in');
  await fixture.raw('close');
  fixture.mark('its screen shows a thread a page, limits the inputs to its stack size and takes four cards');

  // A hopper above feeds gold to the thread whose presses take it, and to no other.
  const hopper = at(1, 1);
  await fixture.command(`setblock ${hopper} minecraft:hopper[facing=down]`);
  await fixture.command(`item replace block ${hopper} container.0 with minecraft:gold_ingot 3`);
  await until(() => contents(hopper), text => !text.includes('gold_ingot'), 'the hopper empty', 30000);
  await until(() => contents(inscriber), text => amountIn(text, 3, 'ae2:printed_logic_processor') === 6, 'four gold more printed', 30000);
  const fed = await contents(inscriber);
  for (const n of [6, 10, 14]) assert.doesNotMatch(entry(fed, n), /gold_ingot/, `no gold in the middle of slot ${n}`);
  fixture.mark('a hopper fills only the thread whose presses take the item');

  // AUTO_EXPORT: every thread's output into the chest on top of it, where the hopper was.
  const chest = hopper;
  await fixture.command(`setblock ${chest} minecraft:chest`);
  await fixture.raw(`interact ${inscriber} south`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'ex_inscriber.json', 'screen again');
  await fixture.pressSetting(24);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 24, 'YES'), 'auto export on');
  await fixture.raw('close');
  await until(() => contents(chest), text => threads.every(([, , output]) => text.includes(output)), 'every output in the chest', 30000);
  fixture.mark('with AUTO_EXPORT each thread pushes its output out');

  // The Extended Charger: four crystals by hand, one a use, all charged; an empty hand takes one out.
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'ae2:certus_quartz_crystal', 5); await fixture.select(0, 'ae2:certus_quartz_crystal');
  for (let i = 0; i < 4; i++) {
    await fixture.raw(`interact ${charger} south`);
    await until(() => fixture.raw('inventory'), s => s.counts?.['ae2:certus_quartz_crystal'] === 4 - i, `crystal ${i + 1} in`);
  }
  await until(() => contents(charger), text => [0, 1, 2, 3].every(n => amountIn(text, n, 'ae2:charged_certus_quartz_crystal') === 1), 'four charged crystals', 180000);
  await fixture.replace('hotbar.0', 'minecraft:air');
  await fixture.raw(`interact ${charger} south`);
  await until(() => ground('ae2:charged_certus_quartz_crystal'), n => n === 1, 'one charged crystal dropped in front', 15000);
  assert.equal(amountIn(await contents(charger), 0, 'ae2:charged_certus_quartz_crystal'), 0, 'the first slot emptied');
  fixture.mark('the Extended Charger takes four items by hand, charges them all and gives them back');

  // The Extended Molecular Assembler: a provider's job, crafted and returned.
  const drive = at(5), terminal = at(6), cpu = at(6, 1), provider = at(7), assembler = at(7, 0, -1);
  for (const [pos, id] of [[at(4), 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [cpu, 'ae2:1k_crafting_storage'], [provider, 'ae2:pattern_provider'], [assembler, 'extendedae:ex_molecular_assembler']] as const) await fixture.command(`setblock ${pos} ${id}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  await online(drive); await online(assembler);
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  const nugget = 'minecraft:iron_nugget';
  const encoded = await fixture.ae(`crafting encode ${Array(9).fill(nugget).join(' ')}`);
  assert.deepEqual([encoded.encoded, encoded.output], [true, 'minecraft:iron_ingot']);
  await until(() => fixture.raw('inventory'), v => v.hotbar[0]?.item === 'ae2:crafting_pattern', 'the pattern');
  await fixture.command(`tp @s ${x + 7.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  const menu = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot < 36 && s.item === 'ae2:crafting_pattern'), 'provider menu', 5000);
  await fixture.raw(`click-slot ${menu.slots.find((s: any) => s.slot < 36 && s.item === 'ae2:crafting_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), v => !v.counts?.['ae2:crafting_pattern'], 'pattern installed');
  await fixture.raw('close');
  await fixture.replace('hotbar.0', nugget, 18); await fixture.select(0, nugget);
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 18);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z - 2.5}`); await fixture.raw(`interact ${terminal} north`);
  const terminalView = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot >= 300 && s.slot < 345 && s.item === 'minecraft:iron_ingot'), 'craftable ingot');
  await fixture.raw(`click-slot ${terminalView.slots.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === 'minecraft:iron_ingot').slot}`);
  await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Amount'), 'craft amount');
  await fixture.raw('ui-key right'); await fixture.raw('ui-key backspace'); await fixture.raw('type 2'); await fixture.raw('wait 5'); await fixture.raw('click-slot 302');
  await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Confirm') && !/Missing materials/.test(JSON.stringify(v)), 'confirmation');
  await fixture.raw('click-slot 302'); await fixture.raw('close');
  const crafted = await until(() => fixture.ae(`grid status ${drive}`), s => count(s, 'minecraft:iron_ingot') === 2, 'two ingots crafted in the Extended Molecular Assembler', 90000);
  assert.equal(count(crafted, nugget), 0);
  fixture.mark('the Extended Molecular Assembler crafts the jobs a provider sends it');
  await fixture.command(`tp @s ${x + 7.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${assembler} north`);
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Extended Molecular Assembler screen', 15000);
  assert.equal(view.ae2_screen.style, 'ex_molecular_assembler.json');
  await shot('ex_molecular_assembler');
  await fixture.replace('hotbar.1', 'ae2:speed_card', 6);
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'ae2:speed_card', 'cards held');
  for (const n of [311, 312, 313, 314, 315]) await fixture.raw(`click-slot ${n}`);
  await until(() => fixture.raw('screen'), v => [311, 312, 313, 314, 315].every(n => slot(v, n)?.item === 'ae2:speed_card') && v.carried?.count === 1, 'five speed cards in, the sixth refused');
  await fixture.raw('click-slot 1');
  await fixture.raw('close');
  fixture.mark('its screen opens with the mod\'s style and takes five speed cards');

  // Entro: a seed on a block of fluix; budding entro grows clusters, which break off with it.
  const fluix = at(10, 0, 0);
  await fixture.command(`setblock ${fluix} ae2:fluix_block`);
  await fixture.command(`tp @s ${x + 10.5} ${y} ${z + 2.5}`);
  // In survival, where the seed is used up (creative puts it back, as ServerPlayerGameMode.useItemOn does).
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.0', 'extendedae:entro_seed', 2); await fixture.select(0, 'extendedae:entro_seed');
  await fixture.raw(`interact ${fluix} south`);
  await until(() => block(fluix), b => b.startsWith('extendedae:entro_budding_fully'), 'a fully entroized budding block', 15000);
  await until(() => fixture.raw('inventory'), s => s.counts?.['extendedae:entro_seed'] === 1, 'one seed used');
  await fixture.command('gamemode creative @s');
  fixture.mark('an Entro Seed turns a block of fluix into budding entro');
  // A powered Growth Accelerator beside it runs its random tick (`ae2:growth_acceleratable` holds
  // `#c:budding_blocks`; the world's random ticks go the same way, with no rate the test could wait on):
  // clusters grow all the way up, and the block wears down.
  const sides = [at(10, 1, 0), at(11, 0, 0), at(10, 0, 1), at(10, 0, -1)];
  const clusters = async () => (await Promise.all(sides.map(block))).filter(b => b.startsWith('extendedae:entro_cluster'));
  const accelerator = at(9), accelPower = at(8);
  await fixture.command(`setblock ${accelPower} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${accelerator} ae2:growth_accelerator[facing=east,powered=false]`);
  await until(() => fixture.ae(`grid status ${accelerator}`), s => s.powered, 'accelerator powered', 30000);
  await until(clusters, found => found.some(b => b.startsWith('extendedae:entro_cluster[')), 'a whole entro cluster grown', 180000);
  const states = await Promise.all(sides.map(block));
  const face = sides[states.findIndex(b => b.startsWith('extendedae:entro_cluster['))];
  fixture.mark(`a Growth Accelerator grows budding entro's clusters to full size (the block now ${(await block(fluix)).split('[')[0]})`);
  // Its block gone, the cluster falls off.
  await fixture.command(`setblock ${accelerator} minecraft:air`);
  await fixture.command(`setblock ${fluix} minecraft:air`);
  await until(() => block(face), b => b === 'minecraft:air', 'the cluster broke off', 15000);
  fixture.mark('a cluster breaks off with the block it stands on');

  // The Crystal Fixer: ender dust fixes a hardly entroized block, a step at a time (70% a try).
  const fixerPower = at(10, 0, 4), fixer = at(11, 0, 4), worn = at(12, 0, 4);
  await fixture.command(`setblock ${fixerPower} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${fixer} extendedae:crystal_fixer[facing=east]`);
  await fixture.command(`setblock ${worn} extendedae:entro_budding_hardly`);
  await online(fixer);
  await fixture.command(`item replace block ${fixer} container.0 with ae2:ender_dust 16`);
  await until(() => block(worn), b => b.startsWith('extendedae:entro_budding_half') || b.startsWith('extendedae:entro_budding_mostly') || b.startsWith('extendedae:entro_budding_fully'), 'the worn block fixed', 120000);
  fixture.mark('the Crystal Fixer fixes worn budding entro with ender dust');

  // Through a restart: the inscriber's slots and settings stay.
  await fixture.command(`item replace block ${inscriber} container.14 with ae2:silicon 7`);
  await fixture.command(`setblock ${chest} minecraft:air`);
  await fixture.restart();
  await online(inscriber);
  const kept = await contents(inscriber);
  assert.equal(amountIn(kept, 12, 'ae2:silicon_press'), 1);
  assert.match(kept, /auto_export: "YES"/);
  assert.equal((kept.match(/ae2:speed_card/g) ?? []).length, 4, 'the cards');
  fixture.mark('the Extended Inscriber keeps its slots, cards and settings through a restart');

  // Broken, it drops its presses, inputs and cards.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  await fixture.command(`setblock ${inscriber} minecraft:air destroy`);
  await until(() => ground('ae2:speed_card'), n => n === 4, 'the four cards on the ground', 30000);
  for (const [press] of threads) assert.equal(await ground(press), 1, press);
  assert.equal(await ground('extendedae:ex_inscriber'), 1, 'the block itself');
  fixture.mark('broken, the Extended Inscriber drops what its threads hold');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
