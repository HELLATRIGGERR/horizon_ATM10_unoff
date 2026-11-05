/** AE2WTLib's Magnet Card on a wireless crafting terminal: its hotkey turns it through its modes; the magnet pulls what the
 * pickup filter takes from 11 blocks away, and in the pickup-to-ME modes what the insert filter takes goes into the
 * network the terminal reaches, the rest into the inventory. */
import assert from 'node:assert/strict';
import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('ae2wtlib-magnet', 'server-thread', { addons: ['ae2wtlib'] });
appendFileSync(join(fixture.game, 'options.txt'), 'version:5023\nkey_key.ae2.ae2wtlib_magnet:key.keyboard.m\n');
const chat = (reply: any) => JSON.stringify(reply.chat ?? '');
const wait = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 32, y = Math.ceil(py) + 5, z = Math.floor(pz) + 32;
  const at = (dx: number) => `${x + dx} ${y} ${z}`;
  const access = at(1), drive = at(-1);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 2.5}`);
  for (const [dx, dz] of [[-2, -3], [-2, 3], [14, -3], [14, 3]] as const)
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), block => block.loaded, 'fixture chunks loaded', 30000);
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + 14} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + 14} ${y - 1} ${z + 3} minecraft:stone`);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${access} ae2:wireless_access_point[facing=east]`);
  await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'powered wireless access point', 30000);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');

  // A crafting terminal with the card.
  const link = `ae2:wireless_link_target={dimension:"minecraft:overworld",pos:[I;${x + 1},${y},${z}]},ae2:stored_energy=1000000.0d`;
  await fixture.replace('hotbar.0', `ae2:wireless_crafting_terminal[${link},ae2:upgrades=[{slot:0,item:{id:"ae2wtlib:magnet_card",count:1}}]]`);
  await fixture.replace('hotbar.1', 'minecraft:cobblestone'); await fixture.replace('hotbar.2', 'minecraft:dirt');
  const centre = ([cx, cy, w, h]: number[]) => `${cx + w / 2} ${cy + h / 2}`;
  const carry = async (slot: number, item: string | null) => {
    await fixture.raw(`click-slot ${slot}`);
    await until(() => fixture.raw('inventory'), s => (s.carried?.item ?? null) === item, `${item ?? 'nothing'} on the cursor`);
  };

  // MagnetScreen over the crafting terminal: the pickup filter allows cobblestone and dirt, the insert filter denies dirt
  // (26.3's SNBT has no empty keys, so `ConfigInventory`'s "" tag can't be typed in a command).
  await fixture.select(0, 'ae2:wireless_crafting_terminal');
  await fixture.raw('use-item');
  const opened = await until(() => fixture.raw('screen'), v => v.ae2_terminal?.widgets?.some((w: any) => w.widget === 'MagnetFilter'), 'the crafting terminal with its Magnet Filter button');
  await fixture.raw(`click-at ${centre(opened.ae2_terminal.widgets.find((w: any) => w.widget === 'MagnetFilter').rect)}`);
  const magnet = () => until(() => fixture.raw('screen'), v => v.ae2_terminal?.sub === 'Magnet' && v.ae2_terminal.magnet?.parts?.length === 59, 'the magnet filter screen');
  const part = async (find: (p: any) => boolean) => (await magnet()).ae2_terminal.magnet.parts.find(find);
  const filter = (which: number, index: number) => part(p => p.filter === which && p.index === index);
  for (const [slot, item, places] of [[1, 'minecraft:cobblestone', [[0, 0]]], [2, 'minecraft:dirt', [[0, 1], [1, 0]]]] as const) {
    await carry(slot, item);
    for (const [which, index] of places) {
      await fixture.raw(`click-at ${centre((await filter(which, index)).rect)}`);
      await until(magnet, v => v.ae2_terminal.magnet.parts.find((p: any) => p.filter === which && p.index === index)?.item === item, `${item} in filter ${which} slot ${index}`);
    }
    await carry(slot, null);
  }
  await fixture.raw(`click-at ${centre((await part(p => p.button === 'pickup_mode')).rect)}`);
  await until(magnet, v => v.ae2_terminal.magnet.allow?.[0] === true && v.ae2_terminal.magnet.allow?.[1] === false, 'the pickup filter allows, the insert filter denies');
  await fixture.raw('close');
  const terminal = chat(await fixture.command('data get entity @s Inventory'));
  assert.match(terminal, /pickup_config/); assert.match(terminal, /insert_config/); assert.match(terminal, /pickup_mode/);
  await fixture.command('clear @s minecraft:cobblestone'); await fixture.command('clear @s minecraft:dirt');
  await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  fixture.mark('the magnet filter screen sets the pickup and insert filters from the cursor and turns the pickup filter to allow');

  const mode = async () => Number(chat(await fixture.command('data get entity @s Inventory')).match(/magnet_settings\\?"?:\s*(-?\d+)b/)?.[1] ?? 0);
  const press = async (next: number) => {
    await fixture.raw('key m tap');
    await until(mode, value => value === next, `the hotkey turns the magnet to ${next}`);
  };
  const drop = (id: string, count: number, dx: number) =>
    fixture.command(`summon minecraft:item ${x + dx} ${y + 0.2} ${z + 2.5} {Item:{id:"${id}",count:${count}},PickupDelay:0s}`);
  const grounded = async (id: string) => ((await fixture.raw('entities 48')).entities ?? [])
    .filter((e: any) => e.type === 'minecraft:item' && JSON.stringify(e).includes(id)).length;
  const counts = async () => (await fixture.raw('inventory')).counts ?? {};
  const stored = async (id: string) => (await fixture.ae(`grid status ${drive}`)).items?.find((e: any) => e.item === id)?.count ?? 0;

  // Off: nothing moves.
  await drop('minecraft:stone', 2, 12.5); await drop('minecraft:cobblestone', 5, 12.5); await drop('minecraft:dirt', 3, 12.5);
  await until(() => grounded('minecraft:'), n => n >= 3, 'the three stacks on the ground');
  await wait(1500);
  assert.equal((await counts())['minecraft:cobblestone'], undefined, 'the magnet is off');
  assert.equal(await mode(), 0);

  // Pickup to the inventory: the pickup filter's items come from 11 blocks away, stone stays.
  await press(1);
  await until(counts, c => c['minecraft:cobblestone'] === 5 && c['minecraft:dirt'] === 3, 'cobblestone and dirt pulled into the inventory');
  assert.equal(await grounded('minecraft:stone'), 1, 'stone is not on the pickup filter');
  assert.equal(await stored('minecraft:cobblestone'), 0);
  fixture.mark('the hotkey turns the magnet on: what the pickup filter allows comes to the inventory from 11 blocks away');

  // Pickup to ME: cobblestone into the network; dirt, which the insert filter denies, into the inventory.
  await fixture.command('clear @s minecraft:cobblestone'); await fixture.command('clear @s minecraft:dirt');
  await press(3);
  await drop('minecraft:cobblestone', 5, 12.5); await drop('minecraft:dirt', 3, 12.5);
  await until(() => stored('minecraft:cobblestone'), n => n === 5, 'cobblestone picked up into the network');
  await until(counts, c => c['minecraft:dirt'] === 3 && !c['minecraft:cobblestone'], 'dirt in the inventory, no cobblestone');
  assert.equal(await stored('minecraft:dirt'), 0, 'the insert filter keeps dirt out of the network');
  assert.equal(await grounded('minecraft:stone'), 1);
  fixture.mark('pickup into the network respects the insert filter, the magnet the pickup filter');

  // Pickup to ME without the magnet: what the player touches goes in, the far stack stays.
  await press(2);
  await drop('minecraft:cobblestone', 4, 12.5);
  await until(() => grounded('minecraft:cobblestone'), n => n === 1, 'the far cobblestone on the ground');
  // At the player's feet, wherever they stand.
  const [fx, fy, fz] = (await fixture.raw('status')).player.position;
  await fixture.command(`summon minecraft:item ${fx} ${fy + 0.2} ${fz} {Item:{id:"minecraft:cobblestone",count:2},PickupDelay:0s}`);
  await until(() => stored('minecraft:cobblestone'), n => n === 7, 'the touched cobblestone into the network');
  await wait(1000);
  assert.equal(await grounded('minecraft:cobblestone'), 1, 'the magnet is off');
  await press(0);
  fixture.mark('the hotkey turns the card through its four modes; without the magnet touched items still go into the network');

  // WirelessTerminalSettingsScreen over the crafting terminal: the magnet's two checkboxes set its bits.
  await fixture.select(0, 'ae2:wireless_crafting_terminal');
  await fixture.raw('use-item');
  const term = await until(() => fixture.raw('screen'), v => v.ae2_terminal?.widgets?.some((w: any) => w.widget === 'WirelessSettings'), 'the crafting terminal with its settings button');
  await fixture.raw(`click-at ${centre(term.ae2_terminal.widgets.find((w: any) => w.widget === 'WirelessSettings').rect)}`);
  const box = (view: any, i: number) => view.ae2_terminal?.checks?.find((c: any) => c.check === `Wireless(${i})`);
  const sub = await until(() => fixture.raw('screen'), v => v.ae2_terminal?.sub === 'Wireless' && v.ae2_terminal.checks?.length === 5, 'the terminal settings screen');
  assert(box(sub, 3).active && !box(sub, 3).selected, 'the magnet box: the card is in, the magnet off');
  assert(!box(sub, 1).active, 'craft if missing waits for pick block');
  await fixture.raw(`click-at ${centre(box(sub, 3).rect)}`);
  await until(mode, value => value === 1, 'the magnet box turns the magnet on');
  await fixture.raw(`click-at ${centre(box(sub, 4).rect)}`);
  await until(mode, value => value === 3, 'the pickup box adds pickup to ME');
  assert(box(await fixture.raw('screen'), 4).selected);
  await fixture.raw('close');
  fixture.mark('the wireless terminal settings screen sets the magnet and pickup-to-ME bits');

  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
