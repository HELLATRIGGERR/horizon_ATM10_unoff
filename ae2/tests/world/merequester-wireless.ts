/** ME Requester's Wireless Requester Terminal (with AE2WTLib): it opens the Requester Terminal's menu over the grid of the
 * access point it reaches, twelve blocks from the requester, and edits its rows; the combine recipe puts it into a
 * universal terminal, whose selector and hotkey turn between it and the crafting terminal. Needs the ae2wtlib and
 * merequester content (ae2/scripts/prepare-addon.ts) and a client built with both. */
import assert from 'node:assert/strict';
import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('merequester-wireless', 'server-thread', { addons: ['ae2wtlib', 'merequester'] });
appendFileSync(join(fixture.game, 'options.txt'), 'version:5023\nkey_key.ae2.wireless_requester_terminal:key.keyboard.k\n');
const chat = (reply: any) => JSON.stringify(reply.chat ?? '');
const centre = ([cx, cy, w, h]: number[]) => `${cx + w / 2} ${cy + h / 2}`;
const UNIVERSAL = 'ae2wtlib:wireless_universal_terminal', WIRELESS = 'merequester:wireless_requester_terminal';
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 32, y = Math.ceil(py) + 5, z = Math.floor(pz) + 32;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const access = at(1), drive = at(-1), requester = at(0, 1);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  for (const [dx, dz] of [[-3, -3], [-3, 4], [16, -3], [16, 4]] as const)
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), block => block.loaded, 'fixture chunks loaded', 30000);
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(16, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(16, -1, 4)} minecraft:stone`);
  // The requester sits on the energy cell: its front (north) faces away from the grid.
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [access, 'ae2:wireless_access_point[facing=east]'], [drive, 'ae2:drive'], [requester, 'merequester:requester']] as const)
    await fixture.command(`setblock ${pos} ${id}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'powered wireless access point', 30000);
  const status = () => fixture.ae(`requester status ${requester}`);
  await until(status, v => v.online && v.powered, 'the requester on the grid', 30000);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');

  // Twelve blocks from the access point, far out of reach of the requester.
  const link = `ae2:wireless_link_target={dimension:"minecraft:overworld",pos:[I;${x + 1},${y},${z}]},ae2:stored_energy=1000000.0d`;
  await fixture.replace('hotbar.0', `${WIRELESS}[${link}]`);
  await fixture.replace('hotbar.1', 'minecraft:stick');
  await fixture.command(`tp @s ${x + 13.5} ${y} ${z + 0.5}`);
  await fixture.select(0, WIRELESS); await fixture.raw('look 0 -30');
  await fixture.raw('use-item');
  let screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.kind === 'merequester:requester_terminal' && v.addon_screen.lines?.length === 6, 'the wireless terminal lists the requester', 30000);
  assert.equal(screen.addon_screen.lines[0].header, 'ME Requester');
  assert.equal(screen.addon_screen.universal, null, 'a plain wireless terminal has no selector');
  // Its toggle switches row 0 off; the stick on the cursor goes into row 0's ghost slot.
  const [ox, oy] = screen.addon_screen.origin, box = screen.addon_screen.lines[1].state_box;
  await fixture.raw(`click-at ${ox + box[0] + 7} ${oy + box[1] + 7}`);
  await until(status, v => v.rows[0].state === false, 'row 0 switched off at range');
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:stick', 'a stick on the cursor');
  await fixture.raw('click-slot 301');
  await until(status, v => v.rows[0].key?.id === 'minecraft:stick', 'row 0 asks for sticks');
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('inventory'), s => !s.carried, 'the stick back');
  await fixture.raw('close');
  fixture.mark('the Wireless Requester Terminal opens and edits a requester at range');

  // Out of range: no menu.
  await fixture.command(`tp @s ${x + 40.5} ${y} ${z + 0.5}`);
  await fixture.raw('use-item');
  await new Promise(resolve => setTimeout(resolve, 1500));
  assert.notEqual((await fixture.raw('screen')).addon_screen?.kind, 'merequester:requester_terminal', 'out of range it does not open');
  await fixture.command(`tp @s ${x + 13.5} ${y} ${z + 0.5}`);

  // combine_crafting.json: the requester terminal and a wireless crafting terminal in the 2 x 2 grid.
  await fixture.replace('hotbar.2', `ae2:wireless_crafting_terminal[${link}]`);
  await fixture.raw('open-inventory');
  await until(() => fixture.raw('screen'), s => slot(s, 100), 'inventory crafting grid');
  for (const [from, to] of [[0, 100], [2, 101]]) {
    await fixture.raw(`click-slot ${from}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${to}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await until(() => fixture.raw('screen'), s => slot(s, 200)?.item === UNIVERSAL, 'the two combine into a universal terminal');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  await until(() => fixture.raw('inventory'), s => s.counts?.[UNIVERSAL] === 1 && !s.counts?.[WIRELESS], 'the universal terminal taken');
  const text = chat(await fixture.command('data get entity @s Inventory'));
  assert.match(text, /has_requester_terminal/); assert.match(text, /has_crafting_terminal/);
  assert.match(text, /ae2:energy_capacity[^0-9]*3200000/, 'a battery for each terminal');
  fixture.mark('combine: the requester terminal and the crafting terminal in one universal terminal');

  // The universal terminal: the requester terminal's hotkey turns it to the requester and opens it; its selector goes
  // back to the crafting terminal.
  const wut = (await fixture.raw('inventory')).hotbar.findIndex((s: any) => s?.item === UNIVERSAL);
  await fixture.select(wut, UNIVERSAL);
  await fixture.raw('key k tap');
  screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.kind === 'merequester:requester_terminal' && v.addon_screen.universal?.current === 'requester', 'the hotkey opens the requester terminal');
  assert.deepEqual(screen.addon_screen.universal.terminals, ['crafting', 'requester']);
  const [ux, uy] = screen.addon_screen.origin;
  await fixture.raw(`click-at ${ux + screen.addon_screen.universal.button[0] + 8} ${uy + screen.addon_screen.universal.button[1] + 8}`);
  screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.universal?.panel?.length === 2, 'the selector\'s panel');
  const crafting = screen.addon_screen.universal.panel[0];
  await fixture.raw(`click-at ${centre([ux + crafting[0], uy + crafting[1], crafting[2], crafting[3]])}`);
  await until(() => fixture.raw('screen'), v => v.ae2_terminal?.widgets?.some((w: any) => w.widget === 'Terminal'), 'the crafting terminal again');
  assert.match(chat(await fixture.command('data get entity @s Inventory')), /current_terminal[^,}]*crafting/);
  await fixture.raw('close');
  fixture.mark('the universal terminal turns between the requester terminal and the crafting terminal');

  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
