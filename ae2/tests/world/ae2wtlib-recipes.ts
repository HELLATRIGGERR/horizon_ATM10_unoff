/** AE2WTLib's crafting-grid recipes: `ae2wtlib:combine` makes a Wireless Universal Terminal of two terminals, their cards
 * and energy in it; `ae2wtlib:upgrade` adds a terminal to one that does not hold it yet. */
import assert from 'node:assert/strict';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('ae2wtlib-recipes', 'server-thread', { addons: ['ae2wtlib'] });
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
const chat = (reply: any) => JSON.stringify(reply.chat ?? '');
const UNIVERSAL = 'ae2wtlib:wireless_universal_terminal';
try {
  await fixture.start();
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  // The 2 x 2 grid of the inventory screen: places 100..103, the result 200.
  const grid = async (pairs: [number, number][], result: string, label: string) => {
    await fixture.raw('open-inventory');
    await until(() => fixture.raw('screen'), s => slot(s, 100), 'inventory crafting grid');
    for (const [from, to] of pairs) {
      await fixture.raw(`click-slot ${from}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
      await fixture.raw(`click-slot ${to}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
    }
    await until(() => fixture.raw('screen'), s => slot(s, 200)?.item === result, label);
    await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  };
  const inventory = async () => chat(await fixture.command('data get entity @s Inventory'));

  // ce.json: the crafting terminal with an Energy Card and the encoding terminal, each with energy.
  await fixture.replace('hotbar.0', 'ae2:wireless_crafting_terminal[ae2:stored_energy=1000000.0d]');
  await fixture.replace('hotbar.1', 'ae2:energy_card');
  await grid([[0, 100], [1, 101]], 'ae2:wireless_crafting_terminal', 'the card goes into the crafting terminal (AE2\'s upgrade recipe)');
  await until(() => fixture.raw('inventory'), s => s.counts?.['ae2:wireless_crafting_terminal'] === 1 && !s.counts?.['ae2:energy_card'], 'the crafting terminal with its card');
  const crafting = (await fixture.raw('inventory')).hotbar.findIndex((s: any) => s?.item === 'ae2:wireless_crafting_terminal');
  await fixture.replace('hotbar.2', 'ae2wtlib:wireless_pattern_encoding_terminal[ae2:stored_energy=300000.0d,minecraft:custom_name="Mine"]');
  await grid([[2, 100], [crafting, 103]], UNIVERSAL, 'the two terminals combine into a universal terminal');
  await until(() => fixture.raw('inventory'), s => s.counts?.[UNIVERSAL] === 1 && !s.counts?.['ae2:wireless_crafting_terminal'] && !s.counts?.['ae2wtlib:wireless_pattern_encoding_terminal'], 'the universal terminal taken');
  let text = await inventory();
  assert.match(text, /has_crafting_terminal/); assert.match(text, /has_pattern_encoding_terminal/);
  assert.doesNotMatch(text, /has_pattern_access_terminal/);
  assert.match(text, /ae2:energy_card/, 'the crafting terminal\'s card');
  assert.match(text, /ae2:energy_capacity[^0-9]*4800000/, 'two terminals and a card: 3 x 1,600,000');
  assert.match(text, /ae2:stored_energy[^0-9]*1300000/, 'both terminals\' energy');
  assert.match(text, /Mine/, 'the encoding terminal\'s other components');
  fixture.mark('combine: the crafting and encoding terminals, the card and their energy in one universal terminal');

  // upgrade_pattern_acces.json: the pattern access terminal into it.
  const wut = (await fixture.raw('inventory')).hotbar.findIndex((s: any) => s?.item === UNIVERSAL);
  await fixture.replace('hotbar.3', 'ae2wtlib:wireless_pattern_access_terminal[ae2:stored_energy=200000.0d]');
  await grid([[wut, 101], [3, 102]], UNIVERSAL, 'the pattern access terminal goes into the universal terminal');
  await until(() => fixture.raw('inventory'), s => s.counts?.[UNIVERSAL] === 1 && !s.counts?.['ae2wtlib:wireless_pattern_access_terminal'], 'the upgraded universal terminal');
  text = await inventory();
  assert.match(text, /has_pattern_access_terminal/); assert.match(text, /has_crafting_terminal/);
  assert.match(text, /ae2:energy_capacity[^0-9]*6400000/, 'three terminals and a card');
  assert.match(text, /ae2:stored_energy[^0-9]*1500000/);
  fixture.mark('upgrade: the pattern access terminal added to the universal terminal');

  // A universal terminal holding the terminal already takes no other.
  const again = (await fixture.raw('inventory')).hotbar.findIndex((s: any) => s?.item === UNIVERSAL);
  await fixture.replace('hotbar.4', 'ae2wtlib:wireless_pattern_access_terminal');
  await fixture.raw('open-inventory');
  await until(() => fixture.raw('screen'), s => slot(s, 100), 'inventory crafting grid');
  for (const [from, to] of [[again, 100], [4, 101]]) {
    await fixture.raw(`click-slot ${from}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${to}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await new Promise(resolve => setTimeout(resolve, 1500));
  assert(!slot(await fixture.raw("screen"), 200)?.item, "no result for a terminal it holds");
  await fixture.raw('close');
  fixture.mark('upgrade: no result for a terminal the universal terminal already holds');

  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
