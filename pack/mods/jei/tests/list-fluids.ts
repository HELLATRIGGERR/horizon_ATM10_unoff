/** Fluids in JEI's ingredient list (bundle `jei`, hook points API 441: `native_panel_ingredients`, `native_fluids`):
 * water and lava sit after Minecraft's items (JEI's `MOD_NAME, INGREDIENT_TYPE` order), drawn as their still sprites
 * in their tints, with JEI's tooltip (name, mod name); the search finds them; U on one in the list shows its uses, a
 * click its recipes; A bookmarks it and the bookmark list draws it. Screenshots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const fixture = new WorldFixture('jei-list-fluids', 'server-thread', { devMods });
type Recipes = { focus: string; mode: string; category: string; tabs: string[]; recipes: number; history: number };
type Probe = {
  loaded: boolean; query: string; total: number; shown: number; first: string[]; recipes: Recipes | null;
  kinds: { fluid: number; chemical: number }; shownTyped: string[]; bookmarks: { items: string[] } | null;
};
type Button = { label: string; x: number; y: number; w: number; h: number };

async function jei(): Promise<Probe> {
  const reply = await fixture.raw('chat /jeiclient');
  const line = (reply.chat ?? []).findLast((t: string) => t.startsWith('JEI {'));
  assert(line, `client probe: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(4));
}
const shot = (name: string) => fixture.raw(`screenshot ${join(fixture.run, name).replaceAll('\\', '/')}`);
async function buttons(): Promise<Button[]> {
  return (await fixture.raw('screen')).buttons ?? [];
}
async function button(match: (label: string) => boolean, what: string): Promise<Button> {
  const all = await buttons();
  const b = all.find(b => match(b.label));
  assert(b, `${what} is on the screen: ${all.map(b => b.label).slice(0, 80).join(' | ')}`);
  return b;
}
const centre = (b: Button) => `${Math.round(b.x + b.w / 2)} ${Math.round(b.y + b.h / 2)}`;
const hover = (b: Button) => fixture.raw(`scroll-at ${centre(b)} 0`);
const key = (name: string) => fixture.raw(`key ${name} tap`);
const page = (what: string, ok: (r: Recipes) => boolean) =>
  until(() => jei(), p => !!p.recipes && ok(p.recipes), what, 10000).then(p => p.recipes!);
async function search(query: string): Promise<void> {
  const box = await button(l => l === 'Search...', 'the search box');
  await fixture.raw(`click-at ${centre(box)} right`);
  if (query) await fixture.raw(`type ${query}`);
  await until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
  await fixture.raw(`click-at ${Math.round(box.x + box.w / 2)} 150`);
}
const WATER = 'fluid minecraft:water';
const LAVA = 'fluid minecraft:lava';
// The list's water: its widget is its name alone (a recipe window's slot is `Water i:k`); left of the bookmarks.
const listWater = async () => {
  const area = (await jei() as Probe & { area: number[] | null }).area;
  assert(area, 'the list is drawn');
  return button(l => l === 'Water', 'the water in the list').then(b => {
    assert(b.x >= area[0] - 1, `the water is in the list: ${JSON.stringify(b)} ${area}`);
    return b;
  });
};

try {
  await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  let p = await until(() => jei(), p => p.loaded && p.shownTyped.includes(WATER), 'the list loads with its fluids', 30000);
  // Water and lava are in the list, after Minecraft's items and before the first mod's.
  assert(p.kinds.fluid >= 2, `fluids in the list: ${JSON.stringify(p.kinds)}`);
  assert.deepEqual(p.shownTyped.slice(0, 2), [WATER, LAVA], `the first ingredients that are not items: ${p.shownTyped}`);
  fixture.mark(`the list holds ${p.kinds.fluid} fluids and ${p.total} ingredients; first typed ${p.shownTyped.slice(0, 2).join(', ')}`);
  await fixture.raw('open-inventory');
  await Bun.sleep(500);

  // The search finds the water (by name) and the lava (by its id with &), drawn as fluids with JEI's tooltip.
  await search('water');
  p = await jei();
  assert(p.shownTyped.includes(WATER), `"water" finds the water: ${p.shownTyped}`);
  await hover(await listWater());
  await Bun.sleep(300);
  await shot('list-water-tooltip.png');
  await search('lava');
  p = await jei();
  assert(p.shownTyped.includes(LAVA), `"lava" finds the lava: ${p.shownTyped}`);
  await hover(await button(l => l === 'Lava', 'the lava in the list'));
  await Bun.sleep(300);
  await shot('list-lava-tooltip.png');
  fixture.mark('the search finds the water and the lava');

  // U on the water in the list: its uses (AE2's transformation drops items into water); a click: its recipes.
  await search('water');
  await hover(await listWater());
  await key('u');
  let r = await page('U on the water in the list shows its uses', r => r.focus === WATER && r.mode === 'uses');
  assert(r.tabs.includes('ae2:item_transformation'), `the water's uses: ${r.tabs}`);
  await Bun.sleep(300);
  await shot('list-u-water.png');
  fixture.mark(`U on the water in the list: ${r.tabs.join(' ')}, ${r.recipes} recipes`);
  await key('escape');
  await until(() => jei(), p => p.recipes === null, 'Escape closes the window', 10000);
  await fixture.raw(`click-at ${centre(await listWater())} right`);
  r = await page('a right click on the water in the list shows its uses', r => r.focus === WATER && r.mode === 'uses');
  await key('escape');
  await until(() => jei(), p => p.recipes === null, 'Escape closes the window again', 10000);
  fixture.mark('a right click on the water in the list shows its uses');

  // A bookmarks the water: the bookmark list draws it (named, as a fluid); A over the bookmark takes it off.
  await hover(await listWater());
  await key('a');
  p = await until(() => jei(), p => !!p.bookmarks?.items.includes(WATER), 'A bookmarks the water', 10000);
  const mark = await button(l => l === 'Water bookmark', 'the water bookmark');
  await hover(mark);
  await Bun.sleep(300);
  await shot('water-bookmark.png');
  fixture.mark(`A bookmarks the water: ${p.bookmarks!.items.join(', ')}`);
  await key('a');
  await until(() => jei(), p => !p.bookmarks?.items.includes(WATER), 'A over the bookmark takes it off', 10000);
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
