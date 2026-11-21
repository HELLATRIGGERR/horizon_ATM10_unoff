/** JEI's "+" recipe transfer (bundle `jei`, slice 4) in a world: beside each recipe of the window when the open screen
 * takes it. At a crafting table the crafting table's recipe goes into the grid from the inventory and the window
 * closes; the chest's, short of planks, has its button grey with the missing slots listed; in the inventory's 2x2
 * grid the chest is too large; at a furnace an iron smelting recipe moves one raw iron, ore or deepslate ore (whichever the page shows first) into the input.
 * What the client holds comes from the bundle's probe (`/jeiclient`, `JEI_DEV=1`); screenshots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const fixture = new WorldFixture('jei-transfer', 'server-thread', { devMods });
type Answer = { recipe: number; handler: string | null; too_large: boolean; missing: number[]; craftable: number[] };
type Recipes = { focus: string; category: string; window: { shown: number[]; transfers: Answer[] } | null };
type Probe = { loaded: boolean; query: string; recipes: Recipes | null };
type Button = { label: string; x: number; y: number; w: number; h: number };

async function jei(): Promise<Probe> {
  // The control's `command` runs a client mod's command and replies with the client's chat; `chat` on a server replies with none.
  const reply = await fixture.raw('command jeiclient');
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
async function search(query: string): Promise<void> {
  const box = await button(l => l === 'Search...', 'the search box');
  await fixture.raw(`click-at ${centre(box)} right`);
  if (query) await fixture.raw(`type ${query}`);
  await until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
  await fixture.raw(`click-at ${Math.round(box.x + box.w / 2)} 6`);
}
/** R over `label` in the list: its recipes, with the screen's answers for the recipes shown. */
async function recipes(query: string, label: string, focus: string): Promise<Recipes> {
  await search(query);
  await hover(await button(l => l === label, `${label} in the list`));
  await fixture.raw('key r tap');
  const p = await until(() => jei(), p => p.recipes?.focus === focus && (p.recipes.window?.transfers.length ?? 0) > 0, `R shows ${focus} with its transfers`, 10000);
  return p.recipes!;
}
const closeWindow = () => fixture.raw('key escape tap').then(() => until(() => jei(), p => p.recipes === null, 'the window closes', 10000));

try {
  const start = await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 2, y = Math.ceil(py), z = Math.floor(pz);
  await fixture.command(`fill ${x - 1} ${y} ${z - 1} ${x} ${y + 2} ${z + 2} minecraft:air`);
  await fixture.command(`setblock ${x} ${y} ${z} minecraft:crafting_table`);
  await fixture.command(`setblock ${x} ${y} ${z + 1} minecraft:furnace[facing=west]`);
  await fixture.command('clear @s');
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.0', 'minecraft:oak_planks', 4);
  const ores = ['minecraft:raw_iron', 'minecraft:iron_ore', 'minecraft:deepslate_iron_ore'];
  for (const [i, ore] of ores.entries()) await fixture.replace(`hotbar.${i + 1}`, ore, 2);

  // A crafting table: the crafting table's recipe, the four planks on hand.
  await fixture.raw(`interact ${x} ${y} ${z} west`);
  await until(() => fixture.raw('screen'), s => !!slot(s, 108) || (s.slots ?? []).some((e: any) => e.slot === 200), 'the crafting table opens', 10000);
  let r = await recipes('crafting table', 'Crafting Table', 'minecraft:crafting_table');
  assert.equal(r.category, 'minecraft:crafting');
  const ready = r.window!.transfers.find(t => t.handler === 'CraftingGrid' && t.missing.length === 0 && !t.too_large);
  assert(ready, `the crafting table's recipe can be moved: ${JSON.stringify(r.window)}`);
  const plus = await button(l => l === `Move Items ${ready.recipe}`, 'the recipe\'s "+"');
  await hover(plus);
  await Bun.sleep(300);
  await shot('transfer-ready.png');
  await fixture.raw(`click-at ${centre(plus)}`);
  await until(() => jei(), p => p.recipes === null, 'the transfer closes the window', 10000);
  await until(() => fixture.raw('screen'), s => slot(s, 200)?.item === 'minecraft:crafting_table', 'the planks in the grid make a crafting table', 10000);
  const inv = await fixture.raw('inventory');
  assert(!inv.counts?.['minecraft:oak_planks'], `the planks left the inventory: ${JSON.stringify(inv.counts)}`);
  await shot('transfer-done.png');
  fixture.mark('crafting table: "+" moves the crafting table\'s planks into the grid and closes the window');

  // The chest needs eight planks: four on hand (in the grid, which JEI counts), so its "+" is grey.
  r = await recipes('chest', 'Chest', 'minecraft:chest');
  const chest = r.window!.transfers.find(t => t.handler === 'CraftingGrid');
  assert(chest && chest.missing.length === 4 && !chest.too_large, `four planks missing for the chest: ${JSON.stringify(r.window)}`);
  await hover(await button(l => l === `Move Items ${chest.recipe}`, 'the chest\'s "+"'));
  await Bun.sleep(300);
  await shot('transfer-missing.png');
  fixture.mark('crafting table: the chest\'s "+" is grey with the four missing slots');
  await closeWindow();
  await fixture.raw('close');

  // The inventory's 2x2 grid: the chest is too large; its "+" is grey.
  await fixture.raw('open-inventory');
  await Bun.sleep(500);
  r = await recipes('chest', 'Chest', 'minecraft:chest');
  const small = r.window!.transfers.find(t => t.handler === 'CraftingGrid');
  assert(small?.too_large, `the chest is too large for the 2x2 grid: ${JSON.stringify(r.window)}`);
  await hover(await button(l => l === `Move Items ${small.recipe}`, 'the chest\'s "+" in the inventory'));
  await Bun.sleep(300);
  await shot('transfer-too-large.png');
  fixture.mark('inventory: the chest is too large for the 2x2 grid');
  await closeWindow();
  await fixture.raw('close');

  // A furnace: a smelting recipe of iron ore (raw iron, ore or deepslate ore, whichever the page shows) moves one into its input; crafting recipes get no "+".
  await fixture.raw(`interact ${x} ${y} ${z + 1} west`);
  await until(() => fixture.raw('screen'), s => (s.slots ?? []).some((e: any) => e.slot === 302), 'the furnace opens', 10000);
  await search('iron ingot');
  await hover(await button(l => l === 'Iron Ingot', 'the iron ingot in the list'));
  await fixture.raw('key r tap');
  await until(() => jei(), p => p.recipes?.focus === 'minecraft:iron_ingot', 'the iron ingot\'s recipes', 10000);
  await fixture.raw(`click-at ${centre(await button(l => l === 'Furnace tab', 'the smelting tab'))}`);
  const smelt = await until(() => jei(), p => p.recipes?.category === 'minecraft:smelting'
    && !!p.recipes.window?.transfers.some(t => t.handler === 'Furnace' && t.missing.length === 0), 'smelting with an iron ore recipe ready', 10000);
  const iron = smelt.recipes!.window!.transfers.find(t => t.handler === 'Furnace' && t.missing.length === 0)!;
  await fixture.raw(`click-at ${centre(await button(l => l === `Move Items ${iron.recipe}`, 'the smelting recipe\'s "+"'))}`);
  await until(() => fixture.raw('screen'), s => ores.includes(slot(s, 300)?.item) && slot(s, 300)?.count === 1, 'one iron ore in the furnace', 10000);
  const counts = (await fixture.raw('inventory')).counts ?? {};
  assert.equal(ores.reduce((n, o) => n + (counts[o] ?? 0), 0), 5, `one of the six iron ores moved: ${JSON.stringify(counts)}`);
  await shot('transfer-furnace.png');
  fixture.mark('furnace: "+" moves one iron ore into the input');

  fixture.report('passed');
} catch (error) { await shot('transfer-fail.png').catch(() => {}); fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
