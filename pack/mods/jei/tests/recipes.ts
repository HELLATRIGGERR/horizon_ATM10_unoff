/** JEI's recipe screen (bundle `jei`, slices 2 and 2b) in a world, drawn as JEI's own window centred over the screen
 * beside the item list: R over an item of the list shows its recipes with a tab per category (crafting and smelting
 * for the iron ingot), a tab click switches category with its catalysts beside the recipes, a right click on an
 * ingredient shows its uses, Backspace walks the history back to the list, Escape closes the window and leaves the
 * inventory open; U on AE2's Inscriber shows its own category first with the machine as the catalyst, and a click on
 * the catalyst its recipes; U on the Charger shows its recipes with their notes.
 * What the client holds comes from the bundle's probe (`/jeiclient`, `JEI_DEV=1`); screenshots in the run folder. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const fixture = new WorldFixture('jei-recipes', 'server-thread');
type Window = { page: number; pages: number; area: number[]; list: number[] | null };
type Recipes = { focus: string; mode: string; category: string; title: string; tabs: string[]; catalysts: string[]; recipes: number; history: number; categories: number; window: Window | null };
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
/** The pointer onto `b` without a click (a scroll of nothing). */
const hover = (b: Button) => fixture.raw(`scroll-at ${centre(b)} 0`);
async function search(query: string): Promise<void> {
  const box = await button(l => l === 'Search...', 'the search box');
  await fixture.raw(`click-at ${centre(box)} right`);
  if (query) await fixture.raw(`type ${query}`);
  await until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
  // A click on the list's empty area lets the box go of the keys, so R, U and Backspace reach JEI.
  await fixture.raw(`click-at ${Math.round(box.x + box.w / 2)} 150`);
}
const page = (what: string, ok: (r: Recipes) => boolean) =>
  until(() => jei(), p => !!p.recipes && ok(p.recipes), what, 10000).then(p => p.recipes!);
const closed = (what: string) => until(() => jei(), p => p.recipes === null, what, 10000);
const key = (name: string) => fixture.raw(`key ${name} tap`);
/** The window drawn: centred (JEI's 198 px at least) and clear of the item list. */
async function drawn(what: string): Promise<Window> {
  const p = await until(() => jei(), p => !!p.recipes?.window && p.recipes.window.pages > 0, `${what} is drawn`, 10000);
  const w = p.recipes!.window!;
  const [x, , width] = w.area;
  assert(width >= 198, `JEI's width: ${JSON.stringify(w)}`);
  if (w.list) assert(x + width <= w.list[0], `the window keeps clear of the list: ${JSON.stringify(w)}`);
  return w;
}

try {
  await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  await fixture.raw('open-inventory');
  await Bun.sleep(500);

  await search('iron ingot');
  const ingot = await button(l => l === 'Iron Ingot', 'the iron ingot in the list');
  // U over it: the crafting uses (tools, armour, the block, ...) fill several pages, so the arrows and the page count show.
  await hover(ingot);
  await key('u');
  const uses = await page('U shows the uses of the iron ingot', r => r.focus === 'minecraft:iron_ingot' && r.mode === 'uses');
  assert(uses.recipes > 2, `the uses need more than one page: ${JSON.stringify(uses)}`);
  const w = await drawn('the uses');
  assert(w.pages > 1 && w.page === 0, `pages: ${JSON.stringify(w)}`);
  await Bun.sleep(300);
  await shot('u-iron-ingot.png');
  await fixture.raw('click-button Next Page');
  await until(() => jei(), p => p.recipes?.window?.page === 1, 'Next Page turns the page', 10000);
  await Bun.sleep(300);
  await shot('u-iron-ingot-page-2.png');
  fixture.mark(`U on the iron ingot: ${uses.recipes} crafting uses on ${w.pages} pages`);
  await key('backspace');
  await closed('Backspace closes the uses');

  // R over the iron ingot in the list: crafting (from nuggets, the block) and smelting/blasting (raw iron, ores).
  await hover(ingot);
  await key('r');
  let r = await page('R shows the iron ingot\'s recipes', r => r.focus === 'minecraft:iron_ingot');
  assert.equal(r.mode, 'recipes');
  assert(r.tabs.includes('minecraft:crafting') && r.tabs.includes('minecraft:smelting'), `tabs: ${r.tabs}`);
  assert.equal(r.tabs[0], 'minecraft:crafting', 'JEI\'s order: crafting first');
  assert(r.catalysts.includes('minecraft:crafting_table'), `catalysts: ${r.catalysts}`);
  await Bun.sleep(300);
  await shot('r-iron-ingot.png');
  fixture.mark(`R: ${r.tabs.join(' ')} (${r.categories} categories), ${r.recipes} crafting recipes`);

  await fixture.raw('click-button Furnace tab');
  r = await page('the smelting tab', r => r.category === 'minecraft:smelting');
  assert.deepEqual(r.catalysts, ['minecraft:furnace']);
  assert(r.recipes >= 2, `smelting makes the ingot from raw iron and ores: ${r.recipes}`);
  await Bun.sleep(300);
  await shot('smelting.png');
  fixture.mark(`the smelting tab: ${r.recipes} recipes, the furnace beside them`);

  // A right click on the first recipe's input: its uses, the ingot's page onto the history.
  const input = await button(l => / 0:0$/.test(l), 'the first recipe\'s input');
  await fixture.raw(`click-at ${centre(input)} right`);
  r = await page('a right click on an input shows its uses', r => r.mode === 'uses' && r.history === 1);
  await shot('uses-of-input.png');
  fixture.mark(`a right click on ${r.focus} shows its uses: ${r.tabs.join(' ')}`);

  await key('backspace');
  r = await page('Backspace goes back to the ingot', r => r.focus === 'minecraft:iron_ingot' && r.history === 0);
  assert.equal(r.category, 'minecraft:smelting', 'on the tab it was on');
  await key('backspace');
  const list = await closed('Backspace again closes the recipes');
  assert.equal(list.query, 'iron ingot', 'Backspace did not edit the search');
  await shot('back-to-list.png');
  fixture.mark('Backspace walks the history back to the list');

  // JEI's click on the list: left the recipes, right the uses.
  await fixture.raw(`click-at ${centre(ingot)}`);
  await page('a click on the list shows the recipes', r => r.focus === 'minecraft:iron_ingot' && r.mode === 'recipes');
  await key('backspace');
  await closed('and Backspace closes them');

  // Escape closes the window (JEI's onClose: back to the screen under it), the inventory stays: R works again.
  await hover(ingot);
  await key('r');
  await page('R again', r => r.focus === 'minecraft:iron_ingot');
  await drawn('the recipes');
  await key('escape');
  await closed('Escape closes the window');
  await hover(ingot);
  await key('u');
  await page('the inventory stayed open under it', r => r.mode === 'uses');
  await key('escape');
  await closed('and Escape closes it again');
  fixture.mark('Escape closes the window and leaves the inventory');

  // U over AE2's Inscriber: its own category first, the machine as the catalyst.
  await search('inscriber');
  const inscriber = await button(l => l === 'Inscriber', 'the Inscriber in the list');
  await hover(inscriber);
  await key('u');
  r = await page('U shows the Inscriber\'s uses', r => r.focus === 'ae2:inscriber' && r.mode === 'uses');
  assert.equal(r.category, 'ae2:inscriber', `the category it makes first: ${r.tabs}`);
  assert(r.catalysts.includes('ae2:inscriber') && r.recipes > 0, JSON.stringify(r));
  await Bun.sleep(300);
  await shot('u-inscriber.png');
  fixture.mark(`U on the Inscriber: ${r.tabs.join(' ')}; ${r.recipes} inscriber recipes`);

  await fixture.raw('click-button Inscriber catalyst');
  r = await page('a click on the catalyst shows its recipes', r => r.mode === 'recipes' && r.history === 1);
  assert.equal(r.focus, 'ae2:inscriber');
  await shot('inscriber-recipes.png');
  await key('backspace');
  await page('back to its uses', r => r.mode === 'uses' && r.category === 'ae2:inscriber');
  await key('backspace');
  await closed('and to the list');
  fixture.mark('a catalyst click shows its recipes; Backspace returns');

  // Another machine's category with notes: U on AE2's Charger, its energy note under each recipe, the recipe's id
  // (an advanced-tooltip line in JEI) left out.
  await search('charger');
  const charger = await button(l => l === 'Charger', 'the Charger in the list');
  await hover(charger);
  await key('u');
  r = await page("U shows the Charger's uses", r => r.focus === 'ae2:charger' && r.mode === 'uses');
  assert.equal(r.category, 'ae2:charger', `the category it makes first: ${r.tabs}`);
  await drawn('the charger recipes');
  await Bun.sleep(300);
  await shot('u-charger.png');
  fixture.mark(`U on the Charger: ${r.tabs.join(' ')}; ${r.recipes} charger recipes`);
  await key('escape');
  await closed('Escape closes it');
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
