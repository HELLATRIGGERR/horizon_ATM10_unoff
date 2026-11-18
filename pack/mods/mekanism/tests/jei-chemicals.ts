/** Mekanism's chemical and fluid slots in JEI's recipe window (needs the bundles `jei` and `mekanism` in one build, hook
 * points API 440; `docs/jei.md`): U on the Electrolytic Separator shows its category, a water slot in and two chemical
 * slots out, drawn as their sprites in their tints; hovering one shows JEI's tooltip (name, amount, mod), U on a chemical
 * slot shows its uses. Needs JEI_CARRY (the JEI textures) and JEI_DEV=1 (its `/jeiclient` probe). Shots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const ato = resolve(process.env.MEK_ATO_CONTENT_DIR ?? join(root, '../.build/content/alltheores'));
const carry = process.env.JEI_CARRY;
const fixture = new WorldFixture('mekanism-jei-chemicals', 'server-thread', { devMods: [content, genContent, ato, ...(carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [])] });
type Recipes = { focus: string; mode: string; category: string; tabs: string[]; recipes: number; history: number; window: { ingredients: string[] } | null };
type Probe = { loaded: boolean; query: string; recipes: Recipes | null };
type Button = { label: string; x: number; y: number; w: number; h: number };
const CHEMICAL = 'mekanism:chemical';

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

try {
  await fixture.start();
  await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  await fixture.raw('open-inventory');
  await Bun.sleep(500);
  await search('electrolytic separator');
  await hover(await button(l => l === 'Electrolytic Separator', 'the separator in the list'));
  await key('u');
  let r = await page('U shows the separator\'s category', r => r.tabs.includes('mekanism:separating'));
  if (r.category !== 'mekanism:separating') {
    await fixture.raw(`click-button ${(await button(l => /Electrolytic Separator tab/.test(l), 'the separator tab')).label}`);
    r = await page('the separating tab', r => r.category === 'mekanism:separating');
  }
  const drawn = (await until(() => jei(), p => !!p.recipes?.window?.ingredients?.some(i => i.startsWith(CHEMICAL)), 'chemicals drawn as slots', 10000)).recipes!.window!.ingredients;
  assert(drawn.includes('fluid minecraft:water'), `the water slot: ${drawn}`);
  assert(drawn.includes(`${CHEMICAL} mekanism:hydrogen`) && drawn.includes(`${CHEMICAL} mekanism:oxygen`), `the chemical slots: ${drawn}`);
  fixture.mark(`the separating category's slots: ${[...new Set(drawn)].join(', ')}`);
  await shot('separating.png');

  const hydrogen = await button(l => /^Hydrogen \d+:\d+$/.test(l), 'a hydrogen slot');
  await hover(hydrogen);
  await Bun.sleep(300);
  await shot('hydrogen-tooltip.png');
  await key('u');
  r = await page('U on hydrogen shows its uses', r => r.focus === `${CHEMICAL} mekanism:hydrogen` && r.mode === 'uses');
  assert(r.recipes > 0 && r.history === 1, `hydrogen's uses: ${JSON.stringify(r)}`);
  await Bun.sleep(300);
  await shot('hydrogen-uses.png');
  fixture.mark(`U on the hydrogen slot: ${r.tabs.join(' ')}, ${r.recipes} recipes`);
  await key('escape');
  await until(() => jei(), p => p.recipes === null, 'Escape closes the window', 10000);
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
