/** JEI's "+" over AE2's terminals (bundle `jei`, slice 4b) in a world with a small ME network: a creative energy cell, a
 * drive with a 1k cell holding eight oak planks, a crafting terminal and a Pattern Encoding Terminal on cable buses.
 * The crafting terminal's "+" pulls the chest's eight planks from the network into its grid and closes the window; the
 * torch's, with a stick on hand and no coal anywhere, stays pressable with the coal slot missing (AE2's orange tint,
 * "Missing items will be skipped"). The Pattern Encoding Terminal's "+" encodes the chest (crafting mode, the planks
 * as ghosts, nothing taken) and an iron smelting recipe (processing mode). The client is remote (`server-thread`): the
 * answers and the moves go to the host as `ae2:jei_transfer`. Probe `/jeiclient` (`JEI_DEV=1`); screenshots in the
 * run folder. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const mode = process.env.JEI_MODE === 'in-frame' ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('jei-transfer-ae2', mode, { devMods });
type Answer = { recipe: number; handler: string | null; too_large: boolean; missing: number[]; craftable: number[] };
type Recipes = { focus: string; category: string; window: { shown: number[]; transfers: Answer[] } | null };
type Probe = { loaded: boolean; query: string; recipes: Recipes | null };
type Button = { label: string; x: number; y: number; w: number; h: number };

async function jei(): Promise<Probe> {
  const reply = await fixture.raw('chat /jeiclient');
  const line = (reply.chat ?? []).findLast((t: string) => t.startsWith('JEI {'));
  assert(line, `client probe: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(4));
}
const shot = (name: string) => fixture.raw(`screenshot ${join(fixture.run, name).replaceAll('\\', '/')}`);
async function button(match: (label: string) => boolean, what: string): Promise<Button> {
  const buttons: Button[] = (await fixture.raw('screen')).buttons ?? [];
  const b = buttons.find(b => match(b.label));
  assert(b, `${what} is on the screen: ${buttons.map(b => b.label).slice(0, 60).join(' | ')}`);
  return b;
}
const centre = (b: Button) => `${Math.round(b.x + b.w / 2)} ${Math.round(b.y + b.h / 2)}`;
const hover = (b: Button) => fixture.raw(`scroll-at ${centre(b)} 0`);
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
const items = (view: any, from: number, to: number) => (view.slots ?? []).filter((e: any) => e.slot >= from && e.slot <= to && e.item);
const stored = (status: any, item: string) => status.items?.find((v: any) => v.item === item)?.count ?? 0;
async function search(query: string): Promise<void> {
  const box = await button(l => l === 'Search...', 'the search box');
  await fixture.raw(`click-at ${centre(box)} right`);
  if (query) await fixture.raw(`type ${query}`);
  await until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
  await fixture.raw(`click-at ${Math.round(box.x + box.w / 2)} 6`);
}
/** R over `label` in the list: its recipes, once the screen's answer for one of them passes `ready`. */
async function recipes(query: string, label: string, focus: string, ready: (a: Answer) => boolean): Promise<Answer> {
  await search(query);
  await hover(await button(l => l === label, `${label} in the list`));
  await fixture.raw('key r tap');
  const p = await until(() => jei(), p => p.recipes?.focus === focus && !!p.recipes.window?.transfers.some(ready),
    `R shows ${focus} with the terminal's answer`, 10000);
  return p.recipes!.window!.transfers.find(ready)!;
}
const closeWindow = () => fixture.raw('key escape tap').then(() => until(() => jei(), p => p.recipes === null, 'the window closes', 10000));

try {
  const start = await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const encoder = at(-1), drive = at(1), terminal = at(2);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  for (const [dx, dz] of [[-3, -3], [-3, 3], [5, -3], [5, 3]] as const) await until(() => fixture.raw(`block ${at(dx, 0, dz)}`), v => v.loaded, 'fixture chunk loaded', 30000);
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(5, 4, 3)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(5, -1, 3)} minecraft:stone`);
  for (const [pos, id] of [[encoder, 'cable_bus'], [at(0), 'creative_energy_cell'], [drive, 'drive'], [terminal, 'cable_bus']] as const) await fixture.command(`setblock ${pos} ae2:${id}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.0', 'ae2:crafting_terminal'); await fixture.select(0, 'ae2:crafting_terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  await fixture.replace('hotbar.0', 'ae2:pattern_encoding_terminal'); await fixture.select(0, 'ae2:pattern_encoding_terminal');
  assert.equal((await fixture.ae(`grid part ${encoder} north`)).attached, true);
  const grid = () => fixture.ae(`grid status ${drive}`);
  await until(grid, v => v.online && v.powered, 'powered grid', 30000);
  await until(() => fixture.ae(`grid status ${encoder}`), v => v.online && v.powered, 'the encoding terminal on the grid', 30000);
  await fixture.replace('hotbar.0', 'minecraft:oak_planks', 8); await fixture.select(0, 'minecraft:oak_planks');
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 8);
  await fixture.command('clear @s');
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');

  // The crafting terminal: the chest's eight planks come from the network.
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  await until(() => fixture.raw('status'), v => Math.abs(v.player.position[1] - y) < 0.01 && Math.abs(v.player.position[2] - (z - 2.5)) < 0.01, 'the player at the terminal', 10000);
  await fixture.raw(`interact ${terminal} north`);
  await until(() => fixture.raw('screen'), v => items(v, 300, 345).some((e: any) => e.item === 'minecraft:oak_planks'), 'the crafting terminal shows the planks', 10000);
  const chest = await recipes('chest', 'Chest', 'minecraft:chest', a => a.handler === 'Ae2Crafting' && a.missing.length === 0);
  const plus = await button(l => l === `Move Items ${chest.recipe}`, 'the chest\'s "+"');
  await hover(plus);
  await Bun.sleep(300);
  await shot('ae2-craft-ready.png');
  await fixture.raw(`click-at ${centre(plus)}`);
  await until(() => jei(), p => p.recipes === null, 'the transfer closes the window', 10000);
  const filled = await until(() => fixture.raw('screen'), v => items(v, 327, 335).filter((e: any) => e.item === 'minecraft:oak_planks').length === 8,
    'eight planks in the terminal\'s grid', 10000).catch(async e => { throw new Error(`${e.message}: ${JSON.stringify(items(await fixture.raw('screen'), 300, 400))}`); });
  assert.equal(stored(await grid(), 'minecraft:oak_planks'), 0, 'the planks left the network');
  assert(!(await fixture.raw('inventory')).counts?.['minecraft:oak_planks'], 'no planks went to the inventory');
  assert.equal(slot(filled, 336)?.item, 'minecraft:chest', 'the grid makes a chest');
  await shot('ae2-craft-done.png');
  fixture.mark('crafting terminal: "+" pulls the chest\'s eight planks from the network into the grid');

  // The torch: the stick on hand, no coal anywhere; AE2 moves what there is, the coal slot missing.
  const torch = await recipes('torch', 'Torch', 'minecraft:torch', a => a.handler === 'Ae2Crafting' && a.missing.length === 1);
  assert.equal(torch.craftable.length, 0, 'nothing craftable without patterns');
  await hover(await button(l => l === `Move Items ${torch.recipe}`, 'the torch\'s "+"'));
  await Bun.sleep(300);
  await shot('ae2-craft-missing.png');
  fixture.mark('crafting terminal: the torch\'s "+" stays pressable with the coal missing');
  await closeWindow();
  await fixture.raw('close');

  // The Pattern Encoding Terminal: the chest encoded as a crafting pattern, nothing taken.
  await fixture.command(`tp @s ${x - 0.5} ${y} ${z - 2.5}`);
  await until(() => fixture.raw('status'), v => Math.abs(v.player.position[0] - (x - 0.5)) < 0.01, 'the player at the encoding terminal', 10000);
  await fixture.raw(`interact ${encoder} north`);
  await until(() => fixture.raw('screen'), v => String(slot(v, 346)?.components ?? '').includes('Mode: crafting'), 'the encoding terminal opens', 10000);
  const encode = await recipes('chest', 'Chest', 'minecraft:chest', a => a.handler === 'Ae2Encoding');
  const encodePlus = await button(l => l === `Move Items ${encode.recipe}`, 'the chest\'s "+" at the encoding terminal');
  await hover(encodePlus);
  await Bun.sleep(300);
  await shot('ae2-encode-ready.png');
  await fixture.raw(`click-at ${centre(encodePlus)}`);
  await until(() => jei(), p => p.recipes === null, 'the encoding closes the window', 10000);
  const pattern = await until(() => fixture.raw('screen'), v => items(v, 300, 308).length === 8 && slot(v, 313)?.item === 'minecraft:chest',
    'the chest\'s planks as ghosts and the chest as the preview', 10000).catch(async e => { throw new Error(`${e.message}: ${JSON.stringify(items(await fixture.raw('screen'), 300, 400))}`); });
  assert(items(pattern, 300, 308).every((e: any) => e.item === 'minecraft:oak_planks') && !slot(pattern, 304)?.item, 'the planks around an empty middle');
  assert.equal(stored(await grid(), 'minecraft:oak_planks'), 0, 'encoding takes nothing from the network');
  await shot('ae2-encode-crafting.png');
  fixture.mark('encoding terminal: "+" encodes the chest as a crafting pattern');

  // A smelting recipe: processing mode, its input and its output.
  await search('iron ingot');
  await hover(await button(l => l === 'Iron Ingot', 'the iron ingot in the list'));
  await fixture.raw('key r tap');
  await until(() => jei(), p => p.recipes?.focus === 'minecraft:iron_ingot', 'the iron ingot\'s recipes', 10000);
  await fixture.raw(`click-at ${centre(await button(l => l === 'Furnace tab', 'the smelting tab'))}`);
  const smelt = await until(() => jei(), p => p.recipes?.category === 'minecraft:smelting' && !!p.recipes.window?.transfers.some(t => t.handler === 'Ae2Encoding'), 'smelting at the encoding terminal', 10000);
  const smelting = smelt.recipes!.window!.transfers.find(t => t.handler === 'Ae2Encoding')!;
  await fixture.raw(`click-at ${centre(await button(l => l === `Move Items ${smelting.recipe}`, 'the smelting recipe\'s "+"'))}`);
  const processing = await until(() => fixture.raw('screen'), v => String(slot(v, 346)?.components ?? '').includes('Mode: processing') && !!slot(v, 300)?.item,
    'a processing pattern with the smelting input', 10000).catch(async e => { throw new Error(`${e.message}: ${JSON.stringify(items(await fixture.raw('screen'), 300, 400))}`); });
  assert.equal(items(processing, 300, 308).length, 1, 'one input');
  assert.equal(slot(processing, 318)?.item, 'minecraft:iron_ingot', 'the iron ingot as the output');
  await shot('ae2-encode-processing.png');
  fixture.mark('encoding terminal: "+" encodes a smelting recipe as a processing pattern');

  fixture.report('passed');
} catch (error) { await shot('transfer-ae2-fail.png').catch(() => {}); fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
