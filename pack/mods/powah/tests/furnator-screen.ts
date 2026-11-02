/** A Furnator's screen (`powah_ui`), as a player uses it.
 *
 * - It shows the charge slot and the fuel slot and none of the hopper menu's other three, under the block's name.
 * - The fuel slot takes coal by click and by shift-click; neither slot takes cobblestone, and the charge slot takes
 *   no coal. The click is refused on the server, and the cobblestone stays on the cursor.
 * - The menu's data shows the energy gauge, the burn bar and the lit bolt, with their tooltips.
 *
 * Screenshots go to the run folder (`furnator-*.png`).
 * Needs what `furnator.ts` needs; `pack/scripts/powah.ts world` runs both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.POWAH_CONTENT_DIR ?? join(root, '../.build/content/powah'));
const fixture = new WorldFixture('powah-furnator-screen', 'server-thread', { devMods: [content] });

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  const pos = `${x} ${y} ${z}`;
  await fixture.command(`fill ${x - 1} ${y} ${z - 2} ${x + 5} ${y + 3} ${z + 2} minecraft:air`);
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 2} ${x + 5} ${y - 1} ${z + 2} minecraft:stone`);
  await fixture.command(`setblock ${pos} powah:furnator_basic`);
  await fixture.command(`data merge block ${pos} {energy_stored_main_energy:40000L}`);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 0.5}`);
  await fixture.replace('hotbar.0', 'minecraft:coal', 8);
  await fixture.replace('hotbar.1', 'minecraft:cobblestone', 5);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'powah:furnator_basic', 'the Furnator reaches the client');

  await fixture.raw(`interact ${pos} east`);
  let view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 301), 'the Furnator screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.container_slots = own;
  assert.deepEqual(own, [0, 1], `only the charge and fuel slots: ${JSON.stringify(view.slots)}`);
  const title = (view.title ?? []).map((span: any) => span.text).join('');
  log.title = title;
  assert.match(title, /Furnator/);
  const shots: string[] = [];
  log.screenshots = shots;
  const shot = async (name: string) => {
    await Bun.sleep(1500);
    const path = join(fixture.run, `furnator-${name}.png`);
    await fixture.raw(`screenshot ${path}`);
    shots.push(path);
  };
  await Bun.sleep(1500);
  await shot('half-full');
  fixture.mark(`the screen shows the charge and fuel slots only, titled "${title}"`);

  // The 960x540 fixture window at vanilla's automatic GUI scale (2): the 176x166 screen is centred in 480x270.
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 166) / 2)];
  const hover = (dx: number, dy: number) => fixture.raw(`click-at ${ox + dx} ${oy + dy}`);

  // Tooltips, the mouse over an empty part of each gauge.
  await hover(10, 20);
  await shot('energy-tooltip');
  await hover(100, 50);
  await shot('generation-tooltip');
  fixture.mark('the energy gauge and the bolt show their tooltips (see the screenshots)');

  // Cobblestone goes nowhere, on the cursor or by shift-click.
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:cobblestone', 'cobblestone on the cursor');
  await fixture.raw('click-slot 301');
  await fixture.raw('click-slot 300');
  await Bun.sleep(1000);
  view = await fixture.raw('screen');
  log.refused = { carried: view.carried, charge: slot(view, 300), fuel: slot(view, 301) };
  assert.equal(view.carried?.item, 'minecraft:cobblestone', `the cobblestone stays on the cursor: ${JSON.stringify(log.refused)}`);
  assert(!slot(view, 300)?.item && !slot(view, 301)?.item, `no slot took it: ${JSON.stringify(log.refused)}`);
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the cobblestone back in its slot');
  await fixture.raw('click-slot 1 shift');
  await Bun.sleep(1000);
  view = await fixture.raw('screen');
  assert(!slot(view, 300)?.item && !slot(view, 301)?.item, 'shift-click moves no cobblestone in');
  assert(!(await saved(pos)).includes('minecraft:cobblestone'), 'nor on the server');
  fixture.mark('neither slot takes cobblestone, by click or shift-click');

  // Coal: not in the charge slot, into the fuel slot.
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:coal', 'coal on the cursor');
  await fixture.raw('click-slot 300');
  await Bun.sleep(1000);
  view = await fixture.raw('screen');
  assert.equal(view.carried?.item, 'minecraft:coal', 'the charge slot takes no coal');
  await fixture.raw('click-slot 301');
  view = await until(() => fixture.raw('screen'), v => !v.carried && slot(v, 301)?.item === 'minecraft:coal', 'the fuel slot takes coal');
  await until(() => saved(pos), s => /burning: ?1b/.test(s), 'it starts burning');
  assert(!slot(view, 300)?.item, 'nothing in the charge slot');
  await Bun.sleep(4000);
  await shot('burning');
  await hover(112, 26);
  await shot('fuel-tooltip');
  // Taking the coal back out: a player can.
  await fixture.raw('click-slot 301');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:coal' && !slot(v, 301)?.item, 'the coal comes back out');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the coal back in its slot');
  await fixture.raw('click-slot 0 shift');
  await until(() => fixture.raw('screen'), v => slot(v, 301)?.item === 'minecraft:coal', 'shift-click puts coal in the fuel slot');
  fixture.mark('coal goes into the fuel slot by click and shift-click, comes back out, and it burns');
  log.saved = await saved(pos);
  await fixture.raw('close');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('POWAH screen', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('POWAH screen', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
