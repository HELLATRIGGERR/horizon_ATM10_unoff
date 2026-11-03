/** ExtendedAE slice 2: the Extended Export Bus moves eight times what AE2's does a tick (ATM10's
 * extended_io_bus_multiplier): 8 items with no card, 768 with four speed cards (96 x 8), of its filter only;
 * it holds eight cards and opens its own screen with them; the IO bus kit turns AE2's Import Bus into the
 * extended one with its filter and cards, which then imports 256 a tick (32 x 8); the Crystal Assembler makes
 * the bus; all through a restart, and broken the cable drops the bus and its eight cards. Needs
 * content/extendedae (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-buses', mode, { addons: ['extendedae'] });
const shots = process.env.AE2_TEST_SHOTS;
const shot = async (name: string) => { if (shots) await fixture.raw(`screenshot ${join(shots, name + '.png')}`); };
const cell = (id: string, n: number) => `ae2:item_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"${id}","#":${n}L}]]`;
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
  // One grid: power, a drive with 8,000 cobblestone and 7,000 sand, the export bus's cable with a chest east
  // of it, and the import bus's cable two blocks south with its own chest.
  const power = at(0), drive = at(1), host = at(2), sink = at(3), importHost = at(2, 0, 2), source = at(3, 0, 2);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [host, 'ae2:cable_bus'], [at(2, 0, 1), 'ae2:cable_bus'], [importHost, 'ae2:cable_bus'], [sink, 'minecraft:chest']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ${cell('minecraft:cobblestone', 8000)}`);
  await fixture.command(`item replace block ${drive} container.1 with ${cell('minecraft:sand', 7000)}`);
  const stored = async (item: string) => ((await fixture.ae(`grid status ${drive}`)).items ?? []).find((e: any) => e.item === item)?.count ?? 0;
  const inChest = async (pos: string, item: string) => ((await fixture.raw(`container ${pos}`)).slots ?? [])
    .filter((slot: any) => slot?.item === item).reduce((n: number, slot: any) => n + slot.count, 0);
  await until(() => stored('minecraft:cobblestone'), n => n === 8000, 'the drive online', 30000);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);

  // The Extended Export Bus on the cable, filtered to cobblestone.
  await fixture.replace('hotbar.0', 'extendedae:ex_export_bus_part'); await fixture.select(0, 'extendedae:ex_export_bus_part');
  assert.equal((await fixture.ae(`grid part ${host} east`)).attached, true);
  const status = () => fixture.ae(`bus status ${host} east`);
  assert.equal((await status()).id, 'ae2:export_bus', 'it behaves as AE2\'s export bus');
  await fixture.replace('hotbar.0', 'minecraft:cobblestone'); await fixture.select(0, 'minecraft:cobblestone');
  await fixture.ae(`bus filter ${host} east 0`);
  // Tick by tick: each transfer the bus makes is one tick's operations.
  await fixture.hold();
  const transfer = async (pos: string, item: string, from: number) => {
    for (let i = 0; i < 200; i++) {
      await fixture.stepServer(1);
      const n = await inChest(pos, item);
      if (n !== from) return n - from;
    }
    throw new Error(`no transfer of ${item}`);
  };
  assert.equal(await transfer(sink, 'minecraft:cobblestone', 0), 8, 'no card: 1 operation x 8');
  assert.equal(await transfer(sink, 'minecraft:cobblestone', 8), 8, 'and again');
  await fixture.hold(false);
  // Four speed cards: 96 operations x 8.
  await fixture.replace('hotbar.0', 'ae2:speed_card', 4); await fixture.select(0, 'ae2:speed_card');
  for (let i = 0; i < 4; i++) await fixture.ae(`bus install ${host} east`);
  assert.equal((await status()).state.speed_cards, 4);
  // The chest filled meanwhile (the cards went in on a running server): an empty one.
  await fixture.hold();
  await fixture.command(`setblock ${sink} minecraft:air`);
  await fixture.command(`setblock ${sink} minecraft:chest`);
  assert.equal(await transfer(sink, 'minecraft:cobblestone', 0), 768, 'four cards: 96 operations x 8');
  await fixture.hold(false);
  assert.equal(await inChest(sink, 'minecraft:sand'), 0, 'only its filter');
  fixture.mark('the Extended Export Bus moves 8 and, with four speed cards, 768 a tick: AE2\'s times 8');

  // Eight cards in all: four capacity cards fill it, a ninth has no slot.
  await fixture.replace('hotbar.0', 'ae2:capacity_card', 5); await fixture.select(0, 'ae2:capacity_card');
  for (let i = 0; i < 4; i++) await fixture.ae(`bus install ${host} east`);
  const full = await fixture.command(`ae2 bus install ${host} east`);
  assert.match(JSON.stringify(full), /slots full/, 'no ninth card');
  // Its screen: the mod's style and the eight cards, the last three past AE2's five. The chest goes, so
  // that the bus's face can be clicked.
  await fixture.command(`setblock ${sink} minecraft:air`);
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 0.5}`);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.raw(`interact ${host} east`);
  const view = await until(() => fixture.raw('screen'), v => v.ae2_part, 'Extended Export Bus screen', 15000);
  assert.equal(view.ae2_part.id, 'extendedae:ex_export_bus_part');
  const cards = [336, 337, 338, 339, 340, 345, 346, 347].map(n => view.slots?.find((s: any) => s.slot === n)?.item);
  assert.deepEqual(cards.filter(c => c === 'ae2:speed_card').length, 4, `cards ${cards}`);
  assert.deepEqual(cards.filter(c => c === 'ae2:capacity_card').length, 4, `cards ${cards}`);
  await shot('ex_export_bus');
  // A card taken out by shift-click from the last entry, and put back.
  await fixture.raw('click-slot 347 shift');
  await until(() => fixture.raw('inventory'), s => s.counts?.['ae2:capacity_card'] === 2, 'the eighth card taken');
  await fixture.raw('close');
  await fixture.select(0, 'ae2:capacity_card');
  await fixture.ae(`bus install ${host} east`);
  fixture.mark('the Extended Export Bus holds eight cards and shows them on its own screen');

  // The IO bus kit: AE2's Import Bus with a filter and two cards becomes the extended one, keeping them.
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'ae2:import_bus'); await fixture.select(0, 'ae2:import_bus');
  assert.equal((await fixture.ae(`grid part ${importHost} east`)).attached, true);
  // A redstone filter: nothing comes in until the kit's bus is set to cobblestone below.
  await fixture.replace('hotbar.0', 'minecraft:redstone'); await fixture.select(0, 'minecraft:redstone');
  await fixture.ae(`bus filter ${importHost} east 0`);
  await fixture.replace('hotbar.0', 'ae2:speed_card', 2); await fixture.select(0, 'ae2:speed_card');
  for (let i = 0; i < 2; i++) await fixture.ae(`bus install ${importHost} east`);
  await fixture.replace('hotbar.0', 'extendedae:io_bus_upgrade', 2); await fixture.select(0, 'extendedae:io_bus_upgrade');
  await fixture.sneakInteract(`${importHost} east`);
  await until(async () => (await fixture.command(`data get block ${importHost}`)).chat?.join('\n') ?? '', text => text.includes('extendedae:ex_import_bus_part'), 'the kit replaced the bus', 15000);
  const kitted = await fixture.ae(`bus status ${importHost} east`);
  assert.equal(kitted.id, 'ae2:import_bus');
  assert.equal(kitted.state.speed_cards, 2, 'its cards');
  assert.match(JSON.stringify(kitted.state.config[0]), /minecraft:redstone/, 'its filter');
  // Its chest only now: it would hide the face the kit was used on.
  await fixture.command(`setblock ${source} minecraft:chest`);
  for (let i = 0; i < 6; i++) await fixture.command(`item replace block ${source} container.${i} with minecraft:cobblestone 64`);
  await fixture.replace('hotbar.0', 'minecraft:cobblestone'); await fixture.select(0, 'minecraft:cobblestone');
  // Held first: a running server would import before the count is taken.
  await fixture.hold();
  await fixture.ae(`bus filter ${importHost} east 0`);
  const imported = -await transfer(source, 'minecraft:cobblestone', await inChest(source, 'minecraft:cobblestone'));
  await fixture.hold(false);
  assert.equal(imported, 256, 'two cards: 32 operations x 8');
  fixture.mark('the IO bus kit makes AE2\'s Import Bus extended with its filter and cards; it imports 256 a tick');

  // The bus from its Crystal Assembler recipe: an export bus, three speed cards, two pistons, a formation core.
  const assembler = at(6), assemblerPower = at(7);
  await fixture.command(`setblock ${assemblerPower} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${assembler} extendedae:crystal_assembler`);
  for (const [i, [id, n]] of ([['ae2:export_bus', 1], ['ae2:speed_card', 3], ['minecraft:piston', 2], ['ae2:formation_core', 1]] as const).entries()) {
    await fixture.command(`item replace block ${assembler} container.${i} with ${id} ${n}`);
  }
  await until(async () => (await fixture.command(`data get block ${assembler}`)).chat?.join('\n') ?? '', text => text.includes('extendedae:ex_export_bus_part'), 'an Extended Export Bus made', 60000);
  fixture.mark('the Crystal Assembler makes an Extended Export Bus');

  // Through a restart: the bus keeps its id, filter and eight cards, and its pace.
  await fixture.restart();
  await until(() => stored('minecraft:sand'), n => n === 7000, 'the drive after restart', 30000);
  const after = await fixture.ae(`bus status ${host} east`);
  assert.equal(after.state.speed_cards, 4);
  assert.equal(after.state.capacity_cards, 4);
  assert.match(JSON.stringify(after.state.config[0]), /minecraft:cobblestone/);
  assert.match((await fixture.command(`data get block ${host}`)).chat?.join('\n') ?? '', /extendedae:ex_export_bus_part/);
  // The chest filled up meanwhile: an empty one.
  await fixture.command(`setblock ${sink} minecraft:air`);
  await fixture.hold();
  await fixture.command(`setblock ${sink} minecraft:chest`);
  assert.equal(await transfer(sink, 'minecraft:cobblestone', 0), 768, 'still 768 a tick');
  await fixture.hold(false);
  fixture.mark('the extended buses keep filter, cards and pace through a restart');

  // Broken, the cable gives up the bus and its eight cards.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 4.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((entity: any) => entity.item?.item === item).reduce((sum: number, entity: any) => sum + entity.item.count, 0);
  await fixture.command(`setblock ${host} minecraft:air destroy`);
  await until(() => ground('extendedae:ex_export_bus_part'), n => n === 1, 'the bus on the ground', 30000);
  await until(() => ground('ae2:speed_card'), n => n === 4, 'its speed cards', 30000);
  await until(() => ground('ae2:capacity_card'), n => n === 4, 'its capacity cards', 30000);
  fixture.mark('broken, the cable drops the Extended Export Bus and its eight cards');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
