/** JEI's ingredient list (bundle `jei`, slice 1) in a world: beside the inventory, every item in JEI's order (Minecraft
 * first), in pages; JEI's search typed into the panel's box (words, `@mod`, `#tag`, `-` excludes, `|` or; a query
 * keeping nothing leaves the box to edit); Ctrl+O hides and shows it; cheat mode gives a clicked item in creative.
 * What the client holds comes from the bundle's probe (`/jeiclient`, `JEI_DEV=1`); screenshots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

// JEI's sprites come with the carry of the player's jar (JEI_CARRY, `jei.ts carry`); without it the fallback draws.
const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const fixture = new WorldFixture('jei-item-list', 'server-thread', { devMods });
type Probe = { loaded: boolean; hidden: boolean; cheat: boolean; query: string; total: number; shown: number; first: string[] };

async function jei(args = ''): Promise<Probe> {
  const reply = await fixture.raw(`chat /jeiclient${args ? ' ' + args : ''}`);
  const line = (reply.chat ?? []).findLast((t: string) => t.startsWith('JEI {'));
  assert(line, `client probe: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(4));
}
const shot = (name: string) => fixture.raw(`screenshot ${join(fixture.run, name).replaceAll('\\', '/')}`);
/** Empties the panel's search box with a right click on it (JEI's way) and types `query` into it. */
async function search(query: string): Promise<Probe> {
  const box = (await fixture.raw('screen')).buttons?.find((b: any) => b.label === 'Search...');
  assert(box, 'the search box is a widget of the screen');
  await fixture.raw(`click-at ${Math.round(box.x + box.w / 2)} ${Math.round(box.y + box.h / 2)} right`);
  if (query) await fixture.raw(`type ${query}`);
  return until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
}
async function ctrlO(): Promise<void> {
  await fixture.raw('key ctrl down');
  try { await fixture.raw('key o tap'); } finally { await fixture.raw('key ctrl up'); }
}

try {
  await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  const loaded = await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  assert(loaded.total > 1000 && loaded.shown === loaded.total, `every item: ${JSON.stringify(loaded)}`);
  assert(loaded.first.every(id => id.startsWith('minecraft:')), `Minecraft first: ${loaded.first}`);
  assert(!loaded.hidden && !loaded.cheat, 'shown, no cheat mode by default');
  fixture.mark(`the list holds ${loaded.total} items, Minecraft first`);

  await fixture.raw('open-inventory');
  await Bun.sleep(500);
  await shot('inventory.png');
  await fixture.raw('click-button Next Page');
  await fixture.raw('click-button Next Page');
  await fixture.raw('click-button Previous Page');
  await shot('page-2.png');
  fixture.mark('the panel pages beside the inventory');

  const cases: [string, (p: Probe) => boolean][] = [
    ['iron ingot', p => p.first.includes('minecraft:iron_ingot') && p.shown < 20],
    ['@minecraft log', p => p.shown > 0 && p.first.every(id => id.startsWith('minecraft:') && id.includes('log'))],
    ['#minecraft:logs', p => p.first.includes('minecraft:oak_log') && p.shown < 100],
    ['oak log | birch log', p => p.first.includes('minecraft:oak_log') && p.first.includes('minecraft:birch_log')],
    ['ingot -iron', p => p.first.includes('minecraft:gold_ingot') && !p.first.includes('minecraft:iron_ingot')],
    ['mekanism', p => p.shown === 0 || p.first.every(id => !id.startsWith('minecraft:'))],
    ['zzzqqq', p => p.shown === 0],
  ];
  for (const [query, ok] of cases) {
    const p = await search(query);
    assert(ok(p), `"${query}": ${JSON.stringify(p)}`);
    fixture.mark(`"${query}" keeps ${p.shown}: ${p.first.slice(0, 4).join(' ')}`);
  }
  // A query that keeps nothing leaves the box: the backspaces above already went through it.
  await search('iron');
  await shot('search-iron.png');
  assert.equal((await search('')).shown, loaded.total, 'an empty box shows every item again');

  await ctrlO();
  await until(() => jei(), p => p.hidden, 'Ctrl+O hides the list', 10000);
  await shot('hidden.png');
  await ctrlO();
  await until(() => jei(), p => !p.hidden, 'Ctrl+O shows it again', 10000);
  fixture.mark('Ctrl+O hides and shows the list');

  await fixture.raw('close');
  await fixture.command('gamemode creative');
  await jei('cheat on');
  await fixture.raw('open-inventory');
  await Bun.sleep(500);
  await search('diamond sword');
  await shot('creative-cheat.png');
  await fixture.raw('click-button Diamond Sword');
  await until(() => fixture.raw('inventory'), inv => JSON.stringify(inv).includes('diamond_sword'), 'cheat mode gives the sword', 10000);
  fixture.mark('cheat mode gives the clicked item in creative');
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
