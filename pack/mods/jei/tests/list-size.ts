/** JEI's ingredient list sized from ATM10's config (bundle `jei`, slice 6) in a world, on a 1920x1080 window at GUI
 * scale 2 (960x540 GUI): `[ingredientList] maxColumns = 18, maxRows = 99` (ATM10's own `jei-client.ini`) fill the
 * room right of the inventory up to 18 columns (the engine's `native_panel_grid`, API 434); the mod name line of
 * `jei-mod-id-format.ini` (JEI's `blue italic`); ATM10's `blacklist.json` (its 14 Mekanism chemicals, plus bedrock
 * here) hides its items until "Show Hidden Ingredients"; `hideIngredientsWhenNotSearching` keeps the search box with
 * an empty list. Probe `/jeiclient` (`JEI_DEV=1`); screenshots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, appendFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

// The harness asks for 960x540 physical pixels; this suite wants twice that, at GUI scale 2 (the options file).
if (!process.env.AE2_TEST_DISPLAY_SCALE) {
  let scale = 1;
  if (process.platform === 'win32') {
    const query = Bun.spawnSync(['reg', 'query', 'HKCU\\Control Panel\\Desktop\\WindowMetrics', '/v', 'AppliedDPI']);
    const dpi = /AppliedDPI\s+REG_DWORD\s+0x([0-9a-f]+)/i.exec(query.stdout.toString());
    if (dpi) scale = parseInt(dpi[1], 16) / 96;
  }
  process.env.AE2_TEST_DISPLAY_SCALE = String(scale / 2);
}
const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const fixture = new WorldFixture('jei-list-size', 'server-thread', { devMods });
appendFileSync(join(fixture.game, 'options.txt'), 'guiScale:2\n');
// ATM10's blacklist.json as the pack ships it (its first entries), and an item to see it work on.
const jeiDir = join(fixture.game, 'config', 'jei');
mkdirSync(jeiDir, { recursive: true });
const chemical = (id: string) => `{"hide_mode":"WILDCARD","ingredient":{"ingredient":"mekanism:${id}","type":"mekanism.api.chemical.ChemicalStack"}}`;
writeFileSync(join(jeiDir, 'blacklist.json'), `[\n  {"version":2},\n  ${['dirty_iron', 'clean_iron', 'clean_gold', 'dirty_gold'].map(chemical).join(',\n  ')},\n  {"hide_mode":"WILDCARD","ingredient":{"ingredient":"minecraft:bedrock","type":"item_stack"}}\n]\n`);

type Probe = {
  loaded: boolean; query: string; shown: number; displayed: number; area: [number, number, number, number] | null;
  blacklist: { entries: number; items: number }; modNameFormat: string; hideUnsearched: boolean;
};
type Button = { label: string; x: number; y: number; w: number; h: number };
async function jei(args = ''): Promise<Probe> {
  const reply = await fixture.raw(`chat /jeiclient${args ? ' ' + args : ''}`);
  const line = (reply.chat ?? []).findLast((t: string) => t.startsWith('JEI {'));
  assert(line, `client probe: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(4));
}
const shot = (name: string) => fixture.raw(`screenshot ${join(fixture.run, name).replaceAll('\\', '/')}`);
const buttons = async (): Promise<Button[]> => (await fixture.raw('screen')).buttons ?? [];
const centre = (b: Button) => `${Math.round(b.x + b.w / 2)} ${Math.round(b.y + b.h / 2)}`;
async function search(query: string): Promise<Probe> {
  const box = (await buttons()).find(b => b.label === 'Search...');
  assert(box, 'the search box is a widget of the screen');
  await fixture.raw(`click-at ${centre(box)} right`);
  if (query) await fixture.raw(`type ${query}`);
  return until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
}

try {
  await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  await fixture.raw('open-inventory');
  const wide = await until(() => jei(), p => p.area !== null, 'the list shows beside the inventory', 10000);
  const area = wide.area!;
  const columns = Math.round(area[2] / 18);
  // 960 GUI pixels: the inventory leaves 21 slots of room right of it, so ATM10's 18 columns all show.
  assert(columns > 9, `more than 9 columns on a wide window: ${JSON.stringify(area)}`);
  assert.equal(columns, 18, `ATM10's maxColumns = 18: ${JSON.stringify(area)}`);
  await Bun.sleep(500);
  await shot('wide-inventory.png');
  fixture.mark(`ATM10's 18-column list on a 960x540 GUI (${columns} columns, area ${area.join(' ')})`);

  // jei-mod-id-format.ini: written in JEI's layout, `blue italic` = §9§o; the line under an item's tooltip.
  const format = join(jeiDir, 'jei-mod-id-format.ini');
  assert(existsSync(format), 'a new jei-mod-id-format.ini');
  assert.match(readFileSync(format, 'utf8'), /^\[modname\]\n(\t#.*\n)+\tmodNameFormat = blue italic\n/);
  assert.equal(wide.modNameFormat, '§9§o');
  const stone = (await buttons()).find(b => b.label === 'Stone');
  assert(stone, 'stone in the list');
  await fixture.raw(`scroll-at ${centre(stone)} 0`);
  await Bun.sleep(300);
  await shot('mod-name-tooltip.png');
  fixture.mark('jei-mod-id-format.ini: blue italic mod name line');

  // blacklist.json: the chemicals are read, bedrock is hidden from the list and its search, until the Cheats page
  // shows hidden ingredients.
  assert.deepEqual(wide.blacklist, { entries: 5, items: 1 });
  const hidden = await search('bedrock');
  assert.equal(hidden.shown, 0, 'bedrock is hidden');
  await jei('set cheating showHiddenIngredients true');
  await until(() => jei(), p => p.shown > 0, '"Show Hidden Ingredients" brings bedrock back', 10000);
  await jei('set cheating showHiddenIngredients false');
  await until(() => jei(), p => p.shown === 0, 'hidden again', 10000);
  fixture.mark('blacklist.json hides bedrock; Show Hidden Ingredients brings it back');

  // hideIngredientsWhenNotSearching: no items until a search, the box stays.
  await search('');
  await jei('set ingredientList hideIngredientsWhenNotSearching true');
  const empty = await until(() => jei(), p => p.hideUnsearched && p.displayed === 0, 'no items while not searching', 10000);
  assert(empty.area !== null, 'the panel (its search box) stays');
  assert((await buttons()).some(b => b.label === 'Search...'), 'the search box is still there');
  await Bun.sleep(300);
  await shot('hidden-until-search.png');
  const found = await search('iron ingot');
  assert(found.displayed > 0 && found.displayed < 20, `a search shows items: ${found.displayed}`);
  await shot('hidden-searched.png');
  await search('');
  await until(() => jei(), p => p.displayed === 0 && p.area !== null, 'cleared: empty again, box kept', 10000);
  await jei('set ingredientList hideIngredientsWhenNotSearching false');
  await until(() => jei(), p => p.displayed > 1000, 'every item again', 10000);
  fixture.mark('hideIngredientsWhenNotSearching keeps the search box with an empty list');

  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
