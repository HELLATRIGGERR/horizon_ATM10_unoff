/** AE2WTLib's wireless terminals: the Wireless Pattern Encoding Terminal encodes a pattern and keeps its state in the item,
 * the Wireless Universal Terminal opens the terminal chosen in its selector, and its battery grows with the terminals it
 * holds and an Energy Card. */
import assert from 'node:assert/strict';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('ae2wtlib-terminals', 'server-thread', { addons: ['ae2wtlib'] });
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
const chat = (reply: any) => JSON.stringify(reply.chat ?? '');
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 32, y = Math.ceil(py) + 5, z = Math.floor(pz) + 32;
  const at = (dx: number) => `${x + dx} ${y} ${z}`;
  const access = at(1);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 2.5}`);
  for (const [dx, dz] of [[-2, -3], [-2, 3], [5, -3], [5, 3]] as const)
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), block => block.loaded, 'fixture chunks loaded', 30000);
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + 5} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + 5} ${y - 1} ${z + 3} minecraft:stone`);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${access} ae2:wireless_access_point[facing=east]`);
  await fixture.command(`setblock ${at(-1)} ae2:drive`);
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'powered wireless access point', 30000);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  const link = `ae2:wireless_link_target={dimension:"minecraft:overworld",pos:[I;${x + 1},${y},${z}]},ae2:stored_energy=100000.0d`;

  // The Wireless Pattern Encoding Terminal: the encoding terminal's menu on the item (WETMenuHost).
  const encoder = `ae2wtlib:wireless_pattern_encoding_terminal[${link}]`;
  await fixture.replace('hotbar.0', 'minecraft:oak_planks', 2);
  await fixture.replace('hotbar.1', 'ae2:blank_pattern');
  await fixture.replace('hotbar.8', encoder); await fixture.select(8, 'ae2wtlib:wireless_pattern_encoding_terminal');
  const openEncoder = async () => {
    await fixture.raw('use-item');
    return until(() => fixture.raw('screen'), view => String(slot(view, 346)?.components ?? '').includes('Mode: crafting'), 'the wireless encoding terminal opens on its encoding grid');
  };
  const opened = await openEncoder();
  assert.equal(opened.ae2_terminal?.kind, 'Encoding');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('inventory'), view => view.carried?.item === 'minecraft:oak_planks', 'oak template on cursor');
  await fixture.raw('click-slot 300');
  await fixture.raw('click-slot 303');
  await until(() => fixture.raw('screen'), view => [300, 303].every(index => slot(view, index)?.item === 'minecraft:oak_planks'), 'two oak ghost inputs');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('inventory'), view => !view.carried && view.hotbar?.[0]?.count === 2, 'oak templates returned to the player');
  await fixture.raw('click-slot 1 shift');
  await until(() => fixture.raw('screen'), view => slot(view, 349)?.item === 'ae2:blank_pattern', 'a physical blank in the wireless terminal');
  await fixture.raw('click-slot 313');
  await until(() => fixture.raw('screen'), view => slot(view, 350)?.item === 'ae2:crafting_pattern', 'the encoded stick pattern');
  await fixture.raw('click-slot 350 shift');
  await until(() => fixture.raw('inventory'), view => view.counts?.['ae2:crafting_pattern'] === 1, 'the encoded pattern in the inventory');
  await fixture.raw('close');
  const held = chat(await fixture.command('data get entity @s SelectedItem'));
  assert.match(held, /ae2wtlib:pattern_encoding_logic/);
  assert.match(held, /encodedInputs/); assert.match(held, /CRAFTING/);
  assert.doesNotMatch(held, /blankPattern|encodedPattern/, 'the blank was used and the pattern taken out');
  fixture.mark('the wireless pattern encoding terminal encodes a pattern from a physical blank and keeps its grid in the item');

  // The Wireless Universal Terminal: crafting, encoding and access in one; the current one opens.
  const universal = (extra = '') => `ae2wtlib:wireless_universal_terminal[ae2wtlib:has_crafting_terminal={},ae2wtlib:has_pattern_encoding_terminal={},ae2wtlib:has_pattern_access_terminal={},ae2wtlib:current_terminal="pattern_encoding",${link}${extra}]`;
  await fixture.replace('hotbar.7', universal()); await fixture.select(7, 'ae2wtlib:wireless_universal_terminal');
  await fixture.raw('use-item');
  const wut = await until(() => fixture.raw('screen'), view => view.ae2_terminal?.kind === 'Encoding', 'the universal terminal opens its current terminal');
  const button = wut.ae2_terminal.widgets.find((w: any) => w.widget === 'Terminal');
  assert(button, 'the terminal selector on the toolbar');
  await fixture.raw(`click-at ${centre(button.rect)}`);
  const panel = await until(() => fixture.raw('screen'), view => view.ae2_terminal?.widgets?.filter((w: any) => /^SelectTerminal/.test(w.widget)).length === 3, 'the selector lists the three terminals');
  await fixture.raw(`click-at ${centre(panel.ae2_terminal.widgets.find((w: any) => w.widget === 'SelectTerminal(0)').rect)}`);
  await until(() => fixture.raw('screen'), view => view.ae2_terminal?.kind === 'Crafting', 'the crafting terminal chosen in the selector opens');
  assert.match(chat(await fixture.command('data get entity @s SelectedItem')), /current_terminal[^,}]*crafting/);
  const crafting = await fixture.raw('screen');
  await fixture.raw(`click-at ${centre(crafting.ae2_terminal.widgets.find((w: any) => w.widget === 'Terminal').rect)}`);
  const again = await until(() => fixture.raw('screen'), view => view.ae2_terminal?.widgets?.some((w: any) => w.widget === 'SelectTerminal(2)'), 'the selector again');
  await fixture.raw(`click-at ${centre(again.ae2_terminal.widgets.find((w: any) => w.widget === 'SelectTerminal(2)').rect)}`);
  await until(() => fixture.raw('screen'), view => /Pattern Access/i.test(JSON.stringify(view.title ?? '')), 'the pattern access terminal opens wirelessly');
  await fixture.raw('close');
  fixture.mark('the universal terminal opens its current terminal and the one chosen in its selector, the pattern access terminal included');

  // Shift and the wheel with no screen open turn it to the next terminal it holds.
  await fixture.raw('key shift down');
  await fixture.raw('scroll-at 100 100 -1');
  await fixture.raw('key shift up');
  await until(async () => chat(await fixture.command('data get entity @s SelectedItem')), text => /current_terminal[^,}]*pattern_encoding/.test(text), 'down the wheel turns the universal terminal back from pattern access to pattern encoding');
  fixture.mark('shift and the wheel cycle the held universal terminal');

  // Its battery: one AE2 battery for each terminal it holds, one more for an Energy Card (ItemWUT.onUpgradesChanged).
  await fixture.replace('hotbar.0', 'ae2:energy_card');
  await fixture.raw('open-inventory');
  await until(() => fixture.raw('screen'), s => slot(s, 100), 'inventory crafting grid');
  for (const [from, to] of [[7, 100], [0, 101]]) {
    await fixture.raw(`click-slot ${from}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${to}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await until(() => fixture.raw('screen'), s => slot(s, 200)?.item === 'ae2wtlib:wireless_universal_terminal', 'the card goes into the universal terminal');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  await until(() => fixture.raw('inventory'), s => s.counts?.['ae2wtlib:wireless_universal_terminal'] === 1 && !s.counts?.['ae2:energy_card'], 'the upgraded universal terminal');
  const inventory = chat(await fixture.command('data get entity @s Inventory'));
  assert.match(inventory, /ae2:energy_capacity[^0-9]*6400000/, 'three terminals and one card: (3 + 1) × 1,600,000');
  fixture.mark('the universal terminal\'s battery holds the terminals\' and the card\'s');

  await fixture.restart();
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'reloaded wireless access point', 30000);
  await fixture.select(8, 'ae2wtlib:wireless_pattern_encoding_terminal');
  const reopened = await openEncoder();
  assert.equal(slot(reopened, 300)?.item, 'minecraft:oak_planks', 'the ghost grid survives a restart in the item');
  await fixture.raw('close');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.mark('the wireless encoding terminal\'s grid survives a restart');
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
