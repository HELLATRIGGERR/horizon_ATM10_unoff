/** AE2WTLib's Restock, Pick Block, hotbar counts and trash on a wireless crafting terminal: placing, dropping, eating and
 * shooting keep the used stack full from the network, the hotbar shows the network's stock, the restock key turns it off
 * and on, the stow key puts the hand's stack into the network, a pick takes a block from the network or opens its craft
 * amount, and the trash loses what it holds when it closes. */
import assert from 'node:assert/strict';
import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('ae2wtlib-restock', 'server-thread', { addons: ['ae2wtlib'] });
appendFileSync(join(fixture.game, 'options.txt'), 'version:5023\nkey_key.ae2.ae2wtlib_restock:key.keyboard.r\nkey_key.ae2.ae2wtlib_stow:key.keyboard.g\n');
const chat = (reply: any) => JSON.stringify(reply.chat ?? '');
const centre = ([cx, cy, w, h]: number[]) => `${cx + w / 2} ${cy + h / 2}`;
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 32, y = Math.ceil(py) + 5, z = Math.floor(pz) + 32;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const access = at(1), drive = at(-1), provider = at(0, 1);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  for (const [dx, dz] of [[-3, -3], [-3, 6], [8, -3], [8, 6]] as const)
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), block => block.loaded, 'fixture chunks loaded', 30000);
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(8, 4, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(8, -1, 6)} minecraft:stone`);
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [access, 'ae2:wireless_access_point[facing=east]'], [drive, 'ae2:drive'], [provider, 'ae2:pattern_provider']] as const)
    await fixture.command(`setblock ${pos} ${id}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k`);
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'powered wireless access point', 30000);
  await fixture.command('clear @s');
  // Back on the new floor, the provider in sight.
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  await until(() => fixture.raw('status'), v => Math.abs(v.player.position[1] - y) < 0.01, 'standing on the floor');
  const stored = async (id: string) => (await fixture.ae(`grid status ${drive}`)).items?.find((e: any) => e.item === id)?.count ?? 0;
  const deposit = async (item: string, amount: number) => {
    await fixture.replace('hotbar.0', item, amount); await fixture.select(0, item);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, amount);
  };
  // Planks are craftable: a pattern of one oak log in the provider.
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  assert.equal((await fixture.ae('crafting encode minecraft:oak_log - - - - - - - -')).encoded, true);
  await until(() => fixture.raw('inventory'), s => s.hotbar[0]?.item === 'ae2:crafting_pattern', 'the planks pattern');
  await fixture.raw(`interact ${provider}`);
  const menu = await until(() => fixture.raw('screen'), s => s.slots?.some((v: any) => v.slot < 36 && v.item === 'ae2:crafting_pattern'), 'provider menu');
  await fixture.raw(`click-slot ${menu.slots.find((v: any) => v.slot < 36 && v.item === 'ae2:crafting_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), s => !s.counts?.['ae2:crafting_pattern'], 'pattern installed'); await fixture.raw('close');
  for (let i = 0; i < 3; i++) await deposit('minecraft:cobblestone', 64);
  await deposit('minecraft:golden_apple', 10); await deposit('minecraft:arrow', 64);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');

  // A crafting terminal with restock, pick block and craft if missing on.
  const link = `ae2:wireless_link_target={dimension:"minecraft:overworld",pos:[I;${x + 1},${y},${z}]},ae2:stored_energy=1000000.0d`;
  await fixture.replace('hotbar.8', `ae2:wireless_crafting_terminal[${link},ae2wtlib:restock=true,ae2wtlib:pick_block=true,ae2wtlib:craft_if_missing=true]`);
  await fixture.replace('hotbar.1', 'minecraft:cobblestone', 10); await fixture.select(1, 'minecraft:cobblestone');
  const inv = () => fixture.raw('inventory');
  const hotbar = async (i: number) => (await inv()).hotbar[i];
  // GuiMixin: the stack's count and the network's.
  await until(inv, s => s.hotbar_counts?.[1] === '202', 'the hotbar shows 10 + 192 cobblestone');
  assert.equal((await inv()).hotbar_counts[8], null, 'a stack of one keeps its look');
  fixture.mark('the hotbar count shows the stack and the network\'s stock');

  // Placing: RightClickBlock fills the stack, useItemOn's return tops it up again.
  await fixture.raw(`place ${at(4, 0, 2)}`);
  await until(() => fixture.raw(`block ${at(4, 0, 2)}`), b => JSON.stringify(b).includes('cobblestone'), 'the block placed');
  await until(() => hotbar(1), s => s?.count === 64, 'the stack restocked to 64');
  assert.equal(await stored('minecraft:cobblestone'), 192 - 55, 'from the network: 54 before the use, 1 after');
  await until(inv, s => s.hotbar_counts?.[1] === String(64 + 137), 'the count follows');
  fixture.mark('restock refills the hotbar stack from the network after placing a block');

  // The drop key: one thrown, the stack restocked.
  await fixture.raw('drop');
  await until(() => stored('minecraft:cobblestone'), n => n === 136, 'the dropped one replaced from the network');
  assert.equal((await hotbar(1)).count, 64);
  fixture.mark('the drop key restocks the stack');

  // The restock key turns it off: a placed block is not replaced. Again, on.
  const restock = async () => /ae2wtlib:restock[^,}]*1b|restock\\?"?:\s*1b/.test(chat(await fixture.command('data get entity @s Inventory')));
  await fixture.raw('key r tap');
  await until(restock, on => !on, 'the restock key turns restock off');
  await fixture.raw(`place ${at(4, 0, 3)}`);
  await until(() => hotbar(1), s => s?.count === 63, 'the stack used, not restocked');
  assert.equal(await stored('minecraft:cobblestone'), 136);
  await fixture.raw('key r tap');
  await until(restock, on => on, 'and on again');
  fixture.mark('the restock key turns restock off and on');

  // The stow key: the hand's stack into the network.
  await fixture.raw('key g tap');
  await until(() => hotbar(1), s => !s?.item, 'the hand emptied');
  assert.equal(await stored('minecraft:cobblestone'), 136 + 63);
  fixture.mark('the stow key puts the hand\'s stack into the network');

  // Eating: a golden apple eaten, the stack filled from the network (LivingEntityUseItemEvent.Finish).
  await fixture.replace('hotbar.2', 'minecraft:golden_apple', 2); await fixture.select(2, 'minecraft:golden_apple');
  await fixture.raw('use-item --hold 2.5');
  await until(() => hotbar(2), s => s?.count === 11, 'one eaten, the ten in the network added');
  assert.equal(await stored('minecraft:golden_apple'), 0);
  // A bow: the arrows filled as it is drawn (ArrowNockEvent), one shot.
  await fixture.replace('hotbar.3', 'minecraft:arrow', 5); await fixture.replace('hotbar.4', 'minecraft:bow'); await fixture.select(4, 'minecraft:bow');
  await fixture.raw('use-item --hold 1.5');
  await until(() => hotbar(3), s => s?.count === 63, 'the arrows restocked, then one shot');
  assert.equal(await stored('minecraft:arrow'), 64 - 59);
  fixture.mark('eating and shooting restock the used stack');

  // Pick Block: the placed cobblestone, none in the inventory, comes from the network into the selected empty slot.
  await fixture.raw('hotbar 2'); await fixture.raw(`look-at block ${at(4, 0, 2)}`);
  await fixture.raw('mouse middle click');
  await until(() => hotbar(1), s => s?.item === 'minecraft:cobblestone' && s.count === 64, 'a stack of cobblestone picked from the network');
  assert.equal((await inv()).selected, 2);
  assert.equal(await stored('minecraft:cobblestone'), 136 + 63 - 64);
  fixture.mark('pick block takes the block from the network');

  // Craft if missing: oak planks, none stored, a pattern makes them: AE2's craft amount for planks.
  await fixture.command(`setblock ${at(4, 0, 4)} minecraft:oak_planks`);
  await fixture.raw(`look-at block ${at(4, 0, 4)}`);
  await fixture.raw('mouse middle click');
  const amount = await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Amount'), 'the craft amount menu');
  assert(amount.slots?.some((v: any) => v.slot === 300 && v.item === 'minecraft:oak_planks'), 'for oak planks');
  assert(!(await inv()).counts?.['minecraft:oak_planks'], 'nothing picked');
  await fixture.raw('close');
  fixture.mark('pick block with craft if missing opens the craft amount menu');

  // The trash: from the terminal's toolbar; dirt put in is gone once it goes back to the terminal.
  await fixture.replace('hotbar.5', 'minecraft:dirt', 5);
  await fixture.select(8, 'ae2:wireless_crafting_terminal'); await fixture.raw('use-item');
  const term = await until(() => fixture.raw('screen'), v => v.ae2_terminal?.widgets?.some((w: any) => w.widget === 'Trash'), 'the crafting terminal with its trash button');
  await fixture.raw(`click-at ${centre(term.ae2_terminal.widgets.find((w: any) => w.widget === 'Trash').rect)}`);
  const trash = await until(() => fixture.raw('screen'), v => v.addon_screen?.trash, 'the trash screen');
  await fixture.raw('click-slot 5');
  await until(inv, s => s.carried?.item === 'minecraft:dirt', 'dirt on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot === 300 && s.item === 'minecraft:dirt'), 'the dirt in the trash');
  assert(!(await inv()).counts?.['minecraft:dirt']);
  await fixture.raw(`click-at ${centre(trash.addon_screen.trash.back)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_terminal?.widgets?.some((w: any) => w.widget === 'Trash'), 'back to the terminal');
  assert(!(await inv()).counts?.['minecraft:dirt'], 'the dirt is gone');
  await fixture.raw(`click-at ${centre(term.ae2_terminal.widgets.find((w: any) => w.widget === 'Trash').rect)}`);
  const again = await until(() => fixture.raw('screen'), v => v.addon_screen?.trash, 'the trash again');
  assert(!again.slots?.some((s: any) => s.slot >= 300 && s.item), 'empty');
  await fixture.raw('close');
  fixture.mark('the trash slot takes items and loses them when it closes');

  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
