/** JEI's bookmarks (bundle `jei`, slice 3) in a world: A over an item of the list adds it to the bookmarks, which show
 * in JEI's list on the left of the inventory (JEI's `bookmark_list_*` sprites from the carry); a click on a bookmark
 * shows its recipes, a right click its uses; the world's `config/jei/world/local/<world>/bookmarks.json` holds them in
 * JEI 19.70's format (a JSON array: `{"version":2}`, then one element each); a legacy `bookmarks.ini` beside it is
 * imported once and moved to `bookmarks.ini.bak`; after a restart of the world they are still there; A over a
 * bookmark removes it, and A over a stack of the recipe window adds it. What the client holds comes from the bundle's
 * probe (`/jeiclient`, `JEI_DEV=1`); screenshots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

// JEI's sprites come with the carry of the player's jar (JEI_CARRY, `jei.ts carry`); without it the fallback draws.
const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const fixture = new WorldFixture('jei-bookmarks', 'server-thread', { devMods });
type Bookmarks = { items: string[]; page: number; pages: number; area: number[] | null; slots: number; file: string | null };
type Probe = { loaded: boolean; query: string; bookmarks: Bookmarks; recipes: { focus: string; mode: string } | null };
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
  assert(b, `${what} is on the screen: ${buttons.length + ' widgets: ' + buttons.map(b => b.label).filter(l => /ookmark| \d+:\d+$|Page|tab$/.test(l)).slice(0, 40).join(' | ')}`);
  return b;
}
const centre = (b: Button) => `${Math.round(b.x + b.w / 2)} ${Math.round(b.y + b.h / 2)}`;
/** The pointer onto `b` without a click (a scroll of nothing). */
const hover = (b: Button) => fixture.raw(`scroll-at ${centre(b)} 0`);
const key = (name: string) => fixture.raw(`key ${name} tap`);
async function search(query: string): Promise<void> {
  const box = await button(l => l === 'Search...', 'the search box');
  await fixture.raw(`click-at ${centre(box)} right`);
  if (query) await fixture.raw(`type ${query}`);
  await until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
  // A click on the list's empty area lets the box go of the keys, so A reaches JEI.
  // (With nothing typed the whole list shows there, and a click on an item would open its recipes: above the list.)
  await fixture.raw(`click-at ${Math.round(box.x + box.w / 2)} ${query ? 150 : 6}`);
}
const bookmarked = (what: string, ok: (b: Bookmarks) => boolean) =>
  until(() => jei(), p => ok(p.bookmarks), what, 10000).then(p => p.bookmarks);
/** The bookmark list's file: JEI's array, its version first, then one bookmark each. */
function file(path: string): any[] {
  assert(existsSync(path), `${path} is written`);
  return JSON.parse(readFileSync(path, 'utf8'));
}

try {
  await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  const first = await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  assert.deepEqual(first.bookmarks.items, [], 'a new world has no bookmarks');
  await fixture.raw('open-inventory');
  await Bun.sleep(500);

  // A over an item of the list adds it; the second one goes last.
  await search('iron ingot');
  await hover(await button(l => l === 'Iron Ingot', 'the iron ingot in the list'));
  await key('a');
  await bookmarked('A bookmarks the iron ingot', b => b.items.length === 1);
  await search('gold ingot');
  await hover(await button(l => l === 'Gold Ingot', 'the gold ingot in the list'));
  await key('a');
  const two = await bookmarked('A bookmarks the gold ingot', b => b.items.length === 2 && b.slots === 2);
  assert.deepEqual(two.items, ['minecraft:iron_ingot', 'minecraft:gold_ingot'], 'in the order they were added');
  assert(two.area && two.area[0] < 20, `the list is on the left: ${JSON.stringify(two)}`);
  assert.equal(two.pages, 1);
  fixture.mark('A adds the iron and the gold ingot; they show in the list on the left');
  await search('');
  await Bun.sleep(300);
  await shot('bookmarks.png');
  await button(l => l === 'Next Bookmark Page', 'the list\'s page arrows');
  await button(l => l === 'Gold Ingot bookmark', 'the gold ingot bookmark');

  // The file: JEI 19.70's per-world bookmarks.json.
  const path = two.file!;
  assert(path.endsWith('/config/jei/world/local/' + path.split('/').at(-2) + '/bookmarks.json'), `JEI's place: ${path}`);
  const lines = file(path);
  assert.deepEqual(lines[0], { version: 2 });
  assert.deepEqual(lines[1], { bookmarkType: 'INGREDIENT', type: 'item_stack', ingredient: { id: 'minecraft:iron_ingot', count: 1 } });
  assert.equal(lines[2].ingredient.id, 'minecraft:gold_ingot');
  assert.equal(lines.length, 3);
  fixture.mark(`bookmarks.json holds both in JEI's format (${path.split('/world/local/')[1]})`);

  // A click on a bookmark shows its recipes, a right click its uses; Escape closes the window.
  await fixture.raw('click-button Iron Ingot bookmark');
  await until(() => jei(), p => p.recipes?.focus === 'minecraft:iron_ingot' && p.recipes.mode === 'recipes', 'a click shows the recipes', 10000);
  await Bun.sleep(300);
  await shot('bookmark-click-recipes.png');
  await key('escape');
  await until(() => jei(), p => p.recipes === null, 'Escape closes the window', 10000);
  const gold = await button(l => l === 'Gold Ingot bookmark', 'the gold ingot bookmark');
  await fixture.raw(`click-at ${centre(gold)} right`);
  await until(() => jei(), p => p.recipes?.focus === 'minecraft:gold_ingot' && p.recipes.mode === 'uses', 'a right click shows the uses', 10000);
  await Bun.sleep(300);
  await shot('bookmark-click-uses.png');
  fixture.mark('a click on a bookmark shows its recipes, a right click its uses');

  // A over a stack of the recipe window bookmarks it too (any stack of the gold ingot's uses but the ingot itself).
  const stackLabel = (l: string) => / \d+:\d+$/.test(l) && !l.startsWith('Gold Ingot ');
  const target = await button(stackLabel, 'a stack of the window');
  const other = target.label.replace(/ \d+:\d+$/, '');
  await hover(target);
  await key('a');
  const three = await bookmarked('A over the window bookmarks its stack', b => b.items.length === 3);
  assert(!['minecraft:iron_ingot', 'minecraft:gold_ingot'].includes(three.items[2]), `another item was added: ${three.items}`);
  await key('escape');
  await until(() => jei(), p => p.recipes === null, 'Escape closes the window', 10000);
  await hover(await button(l => l === `${other} bookmark`, `the ${other} bookmark`));
  await key('a');
  await bookmarked('A over a bookmark takes it off', b => b.items.length === 2);
  fixture.mark(`A over a stack of the window adds it (${other}), A over a bookmark takes it off`);

  // A legacy bookmarks.ini beside the file is imported once and moved aside; the bookmarks survive a restart.
  const dir = dirname(path);
  writeFileSync(join(dir, 'bookmarks.ini'), 'T:{id:"minecraft:stick",count:1}\nI:minecraft:water\nT:{"id":"minecraft:gold_ingot","count":1}\n');
  await fixture.raw('close');
  await fixture.restart();
  await until(() => jei(), p => p.loaded, 'the list loads after the restart', 30000);
  const back = (await jei()).bookmarks;
  assert.deepEqual(back.items, ['minecraft:iron_ingot', 'minecraft:gold_ingot', 'minecraft:stick'], `the file, then the ini's new one: ${back.items}`);
  assert(!existsSync(join(dir, 'bookmarks.ini')) && existsSync(join(dir, 'bookmarks.ini.bak')), 'the ini is moved to bookmarks.ini.bak');
  assert.equal(file(path).length, 4, 'the import is written at once');
  await fixture.raw('open-inventory');
  await Bun.sleep(500);
  await bookmarked('the list shows them again', b => b.slots === 3);
  await shot('after-restart.png');
  fixture.mark('after a restart the bookmarks are still there; the legacy bookmarks.ini was imported and backed up');

  // A over a bookmark removes it, and the file follows.
  await hover(await button(l => l === 'Gold Ingot bookmark', 'the gold ingot bookmark'));
  await key('a');
  const left = await bookmarked('A removes the gold ingot', b => b.items.length === 2 && b.slots === 2);
  assert.deepEqual(left.items, ['minecraft:iron_ingot', 'minecraft:stick']);
  assert.deepEqual(file(path).slice(1).map((e: any) => e.ingredient.id), ['minecraft:iron_ingot', 'minecraft:stick']);
  await Bun.sleep(300);
  await shot('removed.png');
  fixture.mark('A over a bookmark removes it and bookmarks.json follows');
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
