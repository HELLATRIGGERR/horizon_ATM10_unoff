/** AE2 Import Export Card's cards on wireless terminals: the Import Card puts the checked inventory slots its filter
 * passes into the network (a bucket's fluid too), in range of an access point only; the Export Card fills its slots
 * to a full stack from the network, leaves what the server config blocks, fills an empty bucket, and with the
 * Crafting Card starts a job for what is missing; the Import Card's screen sets a filter from the cursor and checks a
 * slot. Needs the content of ae2importexportcard (ae2/scripts/prepare-addon.ts) and a client built with
 * ae2-importexportcard. */
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('ae2importexportcard-cards', 'server-thread', { addons: ['ae2importexportcard'] });
// ServerConfig: diamonds may not leave the network.
mkdirSync(join(fixture.game, 'config'), { recursive: true });
writeFileSync(join(fixture.game, 'config/ae2importexportcard-server.toml'), 'allowEnergyExport = true\nblockedItems = ["minecraft:diamond"]\nblockedFluids = []\n');
const centre = ([cx, cy, w, h]: number[]) => `${cx + w / 2} ${cy + h / 2}`;
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 32, y = Math.ceil(py) + 5, z = Math.floor(pz) + 32;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const access = at(1), drive = at(-1), provider = at(0, 1);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  for (const [dx, dz] of [[-3, -3], [-3, 6], [8, -3], [8, 6], [48, 3]] as const)
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), block => block.loaded, 'fixture chunks loaded', 30000);
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(8, 4, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(8, -1, 6)} minecraft:stone`);
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [access, 'ae2:wireless_access_point[facing=east]'], [drive, 'ae2:drive'],
    [provider, 'ae2:pattern_provider'], [at(-1, 1), 'ae2:1k_crafting_storage']] as const)
    await fixture.command(`setblock ${pos} ${id}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k`);
  await fixture.command(`item replace block ${drive} container.1 with ae2:fluid_storage_cell_4k`);
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'powered wireless access point', 30000);
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  await until(() => fixture.raw('status'), v => Math.abs(v.player.position[1] - y) < 0.01, 'standing on the floor');
  const stored = async (id: string) => (await fixture.ae(`grid status ${drive}`)).items?.find((e: any) => e.item === id)?.count ?? 0;
  const water = async () => (await fixture.ae(`fluid grid status ${drive}`)).entries?.find((e: any) => e.fluid === 'minecraft:water')?.amount_mb ?? 0;
  const deposit = async (item: string, amount: number) => {
    await fixture.replace('hotbar.0', item, amount); await fixture.select(0, item);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, amount);
  };
  // Sticks are craftable: two planks in a column, an assembler on the provider.
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  assert.equal((await fixture.ae('crafting encode minecraft:oak_planks - - minecraft:oak_planks - - - - -')).encoded, true);
  await until(() => fixture.raw('inventory'), s => s.hotbar[0]?.item === 'ae2:crafting_pattern', 'the stick pattern');
  await fixture.raw(`interact ${provider}`);
  const menu = await until(() => fixture.raw('screen'), s => s.slots?.some((v: any) => v.slot < 36 && v.item === 'ae2:crafting_pattern'), 'provider menu');
  await fixture.raw(`click-slot ${menu.slots.find((v: any) => v.slot < 36 && v.item === 'ae2:crafting_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), s => !s.counts?.['ae2:crafting_pattern'], 'pattern installed'); await fixture.raw('close');
  await fixture.command(`setblock ${at(0, 2)} ae2:molecular_assembler`);
  for (let i = 0; i < 2; i++) await deposit('minecraft:cobblestone', 64);
  await deposit('minecraft:oak_planks', 32); await deposit('minecraft:diamond', 5);
  for (let i = 0; i < 2; i++) {
    await fixture.replace('hotbar.0', 'minecraft:water_bucket'); await fixture.select(0, 'minecraft:water_bucket');
    assert.equal((await fixture.ae(`fluid grid insert ${drive}`)).inserted_mb, 1000);
  }
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  const link = `ae2:wireless_link_target={dimension:"minecraft:overworld",pos:[I;${x + 1},${y},${z}]},ae2:stored_energy=1000000.0d`;
  const card = (id: string, slots: number[], extra = '') => {
    const selected = Array.from({ length: 40 }, (_, i) => slots[i] ?? 0).join(',');
    return `{id:"ae2importexportcard:${id}",count:1,components:{"ae2importexportcard:selected_inventory_slots":[I;${selected}]${extra}}}`;
  };
  const inv = () => fixture.raw('inventory');
  const hotbar = async (i: number) => (await inv()).hotbar[i];

  // The Import Card in AE2's wireless terminal (inventory slot 9): hotbar 1, 2 and 3 checked.
  await fixture.replace('hotbar.1', 'minecraft:cobblestone', 10); await fixture.replace('hotbar.2', 'minecraft:water_bucket');
  await fixture.replace('hotbar.3', 'minecraft:dirt', 5); await fixture.replace('hotbar.4', 'minecraft:cobblestone', 7);
  await fixture.command(`item replace entity @s inventory.0 with ae2:wireless_terminal[${link},ae2:upgrades=[{slot:0,item:${card('import_card', [0, 1, 1, 1])}}]]`);
  await Bun.sleep(1000);
  assert.equal((await hotbar(1)).count, 10, 'an empty filter passes nothing');
  await fixture.ae('iecard filter 9 import 0 minecraft:cobblestone');
  await fixture.ae('iecard filter 9 import 1 minecraft:water fluid');
  await until(() => hotbar(1), s => !s?.item, 'the checked cobblestone goes into the network');
  await until(() => hotbar(2), s => s?.item === 'minecraft:bucket', 'the bucket is emptied into the network');
  assert.equal(await stored('minecraft:cobblestone'), 138);
  assert.equal(await water(), 3000);
  assert.equal((await hotbar(3)).count, 5, 'dirt is not in the filter');
  assert.equal((await hotbar(4)).count, 7, 'an unchecked slot stays');
  assert.equal((await hotbar(2)).item, 'minecraft:bucket', 'the emptied bucket stays');
  fixture.mark('the Import Card puts the checked slots its filter passes into the network, a bucket\'s water too');

  // Out of the access point's range (16 blocks) nothing moves; back in range it does.
  await fixture.command(`tp @s ${x + 40.5} ${y} ${z + 3.5}`);
  await fixture.replace('hotbar.1', 'minecraft:cobblestone', 6);
  await Bun.sleep(1500);
  assert.equal((await hotbar(1)).count, 6, 'out of range the slot keeps its stack');
  assert.equal(await stored('minecraft:cobblestone'), 138);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  await until(() => hotbar(1), s => !s?.item, 'in range again it goes in');
  assert.equal(await stored('minecraft:cobblestone'), 144);
  fixture.mark('the Import Card works in the access point\'s range only');

  // The Export Card in AE2's wireless crafting terminal (inventory slot 10), with a Crafting Card: hotbar 5 to filter
  // 1 (cobblestone), 6 to 2 (diamond, blocked), 7 to 3 (stick, none stored), 8 to 4 (water).
  await fixture.replace('hotbar.5', 'minecraft:cobblestone', 60); await fixture.replace('hotbar.8', 'minecraft:bucket');
  const exportCard = card('export_card', [0, 0, 0, 0, 0, 1, 2, 3, 4], ',"ae2:upgrades":[{slot:0,item:{id:"ae2:crafting_card",count:1}}]');
  await fixture.command(`item replace entity @s inventory.1 with ae2:wireless_crafting_terminal[${link},ae2:upgrades=[{slot:0,item:${exportCard}}]]`);
  for (const [index, id] of [[0, 'minecraft:cobblestone'], [1, 'minecraft:diamond'], [2, 'minecraft:stick']] as const)
    await fixture.ae(`iecard filter 10 export ${index} ${id}`);
  await fixture.ae('iecard filter 10 export 3 minecraft:water fluid');
  await until(() => hotbar(5), s => s?.count === 64, 'the slot is filled to a full stack');
  await until(() => hotbar(8), s => s?.item === 'minecraft:water_bucket', 'the empty bucket is filled');
  await Bun.sleep(1000);
  assert.equal((await hotbar(5)).count, 64, 'and no more');
  assert.equal(await stored('minecraft:cobblestone'), 140);
  assert.equal(await water(), 2000);
  fixture.mark('the Export Card fills its slots to a full stack from the network and an empty bucket with water');
  assert(!(await hotbar(6))?.item, 'the config blocks diamonds');
  assert.equal(await stored('minecraft:diamond'), 5);
  fixture.mark('blocked items stay in the network');
  // None stored, a pattern makes them: the Crafting Card starts a job, whose sticks it exports.
  await until(() => hotbar(7), s => s?.item === 'minecraft:stick' && s.count >= 2, 'sticks crafted for the export', 60000);
  assert((await stored('minecraft:oak_planks')) < 32, 'the job used planks');
  fixture.mark('the Crafting Card starts a job for what the network lacks');
  await fixture.command('item replace entity @s inventory.1 with minecraft:air');

  // The Import Card's screen on a wireless terminal in the hotbar: dirt on the cursor sets its first filter slot,
  // then hotbar 3's dirt is checked, and goes in.
  await fixture.replace('hotbar.8', `ae2:wireless_terminal[${link},ae2:upgrades=[{slot:0,item:{id:"ae2importexportcard:import_card",count:1}}]]`);
  await fixture.select(8, 'ae2:wireless_terminal'); await fixture.raw('use-item');
  const term = await until(() => fixture.raw('screen'), v => v.ae2_terminal?.widgets?.some((w: any) => w.widget === 'IeCard(0)'), 'the terminal with the Import Card\'s button');
  await fixture.raw('click-slot 3');
  await until(inv, s => s.carried?.item === 'minecraft:dirt', 'dirt on the cursor');
  await fixture.raw(`click-at ${centre(term.ae2_terminal.widgets.find((w: any) => w.widget === 'IeCard(0)').rect)}`);
  const screen = await until(() => fixture.raw('screen'), v => v.ae2_terminal?.iecard?.which === 0, 'the Import Card\'s screen');
  const part = (name: string) => screen.ae2_terminal.iecard.parts.find((p: any) => p.part === name).rect;
  assert.equal(screen.ae2_terminal.iecard.parts.filter((p: any) => p.part.startsWith('Filter(')).length, 18, 'eighteen filter slots without Capacity Cards');
  await fixture.raw(`click-at ${centre(part('Filter(0)'))}`);
  await until(() => fixture.raw('screen'), v => v.ae2_terminal?.iecard?.filters?.[0] === 'minecraft:dirt', 'the filter takes the cursor\'s dirt');
  await fixture.raw(`screenshot ${join(fixture.run, 'iecard-import-screen.png')}`);
  await fixture.raw(`click-at ${centre(part('Player(3)'))}`);
  await until(inv, s => !s.carried && s.hotbar[3]?.item === 'minecraft:dirt', 'with an item on the cursor the slot takes it back');
  await fixture.raw(`click-at ${centre(part('Player(3)'))}`);
  await until(() => fixture.raw('screen'), v => v.ae2_terminal?.iecard?.selected?.[3] === 1, 'an empty cursor checks the slot');
  await until(() => hotbar(3), s => !s?.item, 'the checked dirt goes into the network');
  assert.equal(await stored('minecraft:dirt'), 5);
  await fixture.raw(`click-at ${centre(part('Back'))}`);
  await until(() => fixture.raw('screen'), v => v.ae2_terminal?.widgets?.some((w: any) => w.widget === 'IeCard(0)'), 'back to the terminal');
  await fixture.raw('close');
  fixture.mark('the Import Card\'s screen sets a filter from the cursor and checks a slot');

  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
