/** JEI's config (bundle `jei`, slice 5) in a world: a new `config/jei/jei-client.ini` is written in JEI 19.70's format
 * (MezzConfig's comments, ATM10's own `maxRows`/`maxColumns`); the item list's wrench opens JEI's config screen over
 * the inventory (its categories on the left, a button per value); a search mode changed there is in the file at once
 * and the search follows it; cheat mode is turned on there (the wrench's icon turns red) and, as in JEI, is off again
 * after a restart, while the file and the search mode are kept. What the client holds comes from the bundle's probe
 * (`/jeiclient`, `JEI_DEV=1`); screenshots in the run folder. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

// JEI's sprites come with the carry of the player's jar (JEI_CARRY, `jei.ts carry`); without it the fallback draws.
const carry = process.env.JEI_CARRY;
const devMods = carry && existsSync(join(carry, 'horizon-mod.toml')) ? [carry] : [];
const fixture = new WorldFixture('jei-config', 'server-thread', { devMods });
type Row = { name: string; value: string };
type Config = {
  file: string | null; cheat: boolean; bookmarks: boolean; wrench: number[] | null;
  screen: { category: string; rows: Row[] } | null;
};
type Probe = { loaded: boolean; query: string; shown: number; cheat: boolean; config: Config };
type Button = { label: string; x: number; y: number; w: number; h: number };

async function jei(): Promise<Probe> {
  // The control's `command` runs a client mod's command and replies with the client's chat; `chat` on a server replies with none.
  const reply = await fixture.raw('command jeiclient');
  const line = (reply.chat ?? []).findLast((t: string) => t.startsWith('JEI {'));
  assert(line, `client probe: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(4));
}
const shot = (name: string) => fixture.raw(`screenshot ${join(fixture.run, name).replaceAll('\\', '/')}`);
async function button(label: string): Promise<Button> {
  const buttons: Button[] = (await fixture.raw('screen')).buttons ?? [];
  const b = buttons.find(b => b.label === label);
  assert(b, `${label} is on the screen: ${buttons.map(b => b.label).filter(l => !/ \d+:\d+$/.test(l)).join(', ')}`);
  return b;
}
const centre = (b: Button) => `${Math.round(b.x + b.w / 2)} ${Math.round(b.y + b.h / 2)}`;
const click = async (label: string, right = false) => fixture.raw(`click-at ${centre(await button(label))}${right ? ' right' : ''}`);
const row = (p: Probe, name: string) => p.config.screen?.rows.find(r => r.name === name)?.value;
async function search(query: string): Promise<Probe> {
  const box = await button('Search...');
  await fixture.raw(`click-at ${centre(box)} right`);
  if (query) await fixture.raw(`type ${query}`);
  return until(() => jei(), p => p.query === query, `search "${query}"`, 10000);
}
async function openConfig(): Promise<Probe> {
  const p = await until(() => jei(), p => p.config.wrench !== null, 'the wrench is on the item list', 10000);
  const [x, y, w, h] = p.config.wrench!;
  await fixture.raw(`click-at ${Math.round(x + w / 2)} ${Math.round(y + h / 2)}`);
  return until(() => jei(), p => p.config.screen !== null, 'the wrench opens JEI\'s config screen', 10000);
}

try {
  await fixture.start();
  fixture.assertHidden(await fixture.raw('status'));
  const first = await until(() => jei(), p => p.loaded, 'the list loads with the world', 30000);
  const file = first.config.file;
  assert(file && existsSync(file), `a new jei-client.ini is written: ${file}`);
  const text = readFileSync(file, 'utf8');
  assert.match(text, /^\[search\]\n\t# Name: Search Mod Names\n\t# Description: .+\n\t# Valid Values: \[ENABLED, REQUIRE_PREFIX, DISABLED\]\n\t# Default Value: REQUIRE_PREFIX\n\tmodNameSearchMode = REQUIRE_PREFIX\n/);
  assert.match(text, /\ttagSearchMode = REQUIRE_PREFIX\n/);
  const list = text.slice(text.indexOf('[ingredientList]')).split(/^\[/m)[1]!;
  assert.match(list, /\tmaxRows = 99\n/, 'ATM10\'s own list rows');
  assert.match(list, /\tmaxColumns = 18\n/, 'ATM10\'s own list columns');
  assert.equal(first.cheat, false, 'cheat mode starts off');
  fixture.mark('a new jei-client.ini in JEI 19.70\'s format, with ATM10\'s own values');

  // A search mode changed on the config screen: in the file at once, and the search follows it.
  await fixture.raw('open-inventory');
  await Bun.sleep(500);
  const before = await search('ingots');
  let p = await openConfig();
  assert.equal(p.config.screen!.category, 'search', 'the screen opens on Search');
  assert.equal(row(p, 'Search Ingredient Tags'), 'Require Prefix');
  await shot('config-search.png');
  await click('Search Ingredient Tags', true);
  p = await until(() => jei(), p => row(p, 'Search Ingredient Tags') === 'Enabled', 'a right click goes back to Enabled', 10000);
  assert.match(readFileSync(file, 'utf8'), /\ttagSearchMode = ENABLED\n/, 'the file has it at once');
  assert.match(readFileSync(file, 'utf8'), /\t# Default Value: REQUIRE_PREFIX\n\ttagSearchMode = ENABLED\n/, 'its comments are kept');
  p = await until(() => jei(), p => p.shown > before.shown, `"ingots" now finds the tag's items (was ${before.shown})`, 10000);
  const tagged = p.shown;
  fixture.mark(`tag search Require Prefix -> Enabled: "ingots" finds ${before.shown} -> ${tagged}`);

  // Cheat mode on the Cheats page; the wrench shows JEI's cheat icon.
  await click('JEI Config: Cheats');
  p = await until(() => jei(), p => p.config.screen?.category === 'cheating', 'the Cheats page', 10000);
  assert.equal(row(p, 'Enable Cheat Mode'), 'OFF');
  await click('Enable Cheat Mode');
  p = await until(() => jei(), p => p.cheat && row(p, 'Enable Cheat Mode') === 'ON', 'cheat mode on', 10000);
  await shot('config-cheats.png');
  await click('Done');
  p = await until(() => jei(), p => p.config.screen === null, 'Done closes the screen', 10000);
  await shot('cheat-wrench.png');
  // Escape closes it too, and leaves the inventory open.
  await openConfig();
  await fixture.raw('key escape tap');
  await until(() => jei(), p => p.config.screen === null && p.config.wrench !== null, 'Escape closes only the config screen', 10000);
  fixture.mark('cheat mode turned on from the config screen; Done and Escape close it');

  // A restart keeps the file and the search mode; cheat mode is off again (JEI doesn't keep it).
  await fixture.restart();
  p = await until(() => jei(), p => p.loaded, 'the list loads after the restart', 30000);
  assert.equal(p.cheat, false, 'cheat mode is off after a restart');
  assert.match(readFileSync(file, 'utf8'), /\ttagSearchMode = ENABLED\n/);
  await fixture.raw('open-inventory');
  await Bun.sleep(500);
  p = await search('ingots');
  assert.equal(p.shown, tagged, 'the search still finds the tag\'s items');
  p = await openConfig();
  assert.equal(row(p, 'Search Ingredient Tags'), 'Enabled');
  await shot('after-restart.png');
  fixture.mark('after a restart the ini and the search mode are kept; cheat mode is off');
} catch (error) {
  await shot('failure.png').catch(() => {});
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
