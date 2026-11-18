/** Fluids as JEI ingredients in a recipe (bundle `jei`, hook points API 440: `NativeStack::kind`): AE2's
 * transformation (items thrown into water, `TransformCategory`) shows the water as a fluid slot, drawn as its still
 * sprite in its tint, with JEI's tooltip (name, amount in mB, mod name); U over that slot and a right click on it show
 * the water's uses, R its recipes (none: nothing makes water here, so the page stays). Mekanism's chemical slots
 * need a build with the Mekanism bundle beside JEI (docs/jei.md). Screenshots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const fixture = new WorldFixture('jei-fluids', 'server-thread', { devMods });
type Window = { page: number; pages: number; ingredients: string[] };
type Recipes = { focus: string; mode: string; category: string; tabs: string[]; recipes: number; history: number; window: Window | null };
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
  assert(b, `${what} is on the screen: ${buttons.map(b => b.label).slice(0, 80).join(' | ')}`);
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
  await fixture.raw(`type ${query}`);
  await until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
  await fixture.raw(`click-at ${Math.round(box.x + box.w / 2)} 150`);
}
const WATER = 'fluid minecraft:water';
const water = () => button(l => /^Water \d+:\d+$/.test(l), 'the water slot');

try {
  await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  await fixture.raw('open-inventory');
  await Bun.sleep(500);

  // R on the Fluix Crystal: AE2's transformation (charged certus, redstone and quartz thrown into water).
  await search('fluix crystal');
  await hover(await button(l => l === 'Fluix Crystal', 'the fluix crystal in the list'));
  await key('r');
  let r = await page('R shows the fluix crystal\'s recipes', r => r.focus === 'ae2:fluix_crystal');
  assert(r.tabs.includes('ae2:item_transformation'), `tabs: ${r.tabs}`);
  if (r.category !== 'ae2:item_transformation') {
    await fixture.raw('click-button Water Bucket tab');
    r = await page('the transformation tab', r => r.category === 'ae2:item_transformation');
  }
  const drawn = await until(() => jei(), p => !!p.recipes?.window?.ingredients?.includes(WATER), 'the water drawn as a fluid', 10000);
  fixture.mark(`a fluid slot: ${drawn.recipes!.window!.ingredients.join(', ')}`);

  const slot = await water();
  await hover(slot);
  await Bun.sleep(300);
  await shot('water-slot-tooltip.png');

  // U over the fluid slot: the water's uses, the crystal's page onto the history.
  await key('u');
  r = await page('U on the water shows its uses', r => r.focus === WATER && r.mode === 'uses');
  assert(r.tabs.includes('ae2:item_transformation'), `the water's uses: ${r.tabs}`);
  assert.equal(r.history, 1, 'the crystal\'s page is in the history');
  await Bun.sleep(300);
  await shot('u-water.png');
  fixture.mark(`U on the water slot: ${r.tabs.join(' ')}, ${r.recipes} recipes`);

  // Backspace comes back to the crystal; a right click on the water slot shows its uses the same way.
  await key('backspace');
  r = await page('Backspace goes back to the crystal', r => r.focus === 'ae2:fluix_crystal' && r.history === 0);
  await fixture.raw(`click-at ${centre(await water())} right`);
  r = await page('a right click on the water shows its uses', r => r.focus === WATER && r.mode === 'uses' && r.history === 1);
  fixture.mark('a right click on the water slot shows its uses');
  await key('escape');
  await until(() => jei(), p => p.recipes === null, 'Escape closes the window', 10000);
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
