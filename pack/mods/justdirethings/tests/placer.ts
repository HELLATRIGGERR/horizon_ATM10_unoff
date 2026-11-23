/** Just Dire Things' Block Placer T1, as the `justdirethings` native bundle and the content mod made from the carry of
 * the player's jar (the placer on the furnace template) make it.
 *
 * - Its screen opens on a click with one slot (the hopper menu's four others hidden); two oak logs go in.
 * - On its operation clock (every 20 ticks) it places one in front of it as its fake player would, clicking the face
 *   its direction setting names: down by default, so the log stands (`axis=y`), and one log stays in the slot.
 * - The direction button clicked twice (down, up, north), the next log lies along z and the slot is empty.
 * - The tick speed button: a left click 21 ticks, a right click back to 20.
 * - A picture of its screen goes to the run folder (`placer.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-placer', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => Number(nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?\\d+)`))?.[1] ?? NaN);
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + 2} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + 2} ${y - 1} ${z + 4} minecraft:stone`);
  await fixture.command('gamemode survival @s');
  const pos = `${x} ${y} ${z}`, front = `${x} ${y} ${z - 1}`;
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;

  await fixture.replace('hotbar.0', 'minecraft:oak_log', 2);
  await fixture.command(`setblock ${pos} justdirethings:blockplacert1[facing=north]`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:blockplacert1', 'the placer reaches the client');
  await fixture.raw(`interact ${pos} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the placer screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `one slot of its own: ${JSON.stringify(view.slots)}`);
  let nbt = await saved(pos);
  assert.equal(field(nbt, 'direction'), 0, 'down by default');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:oak_log', 'the logs on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the logs in the slot');
  fixture.mark('the screen shows one slot; two oak logs go in');

  await until(() => is(front, 'minecraft:oak_log[axis=y]'), v => v, 'the placer places a log standing up (it clicks the face below)', 20000);
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.count === 1, 'one log used up');
  fixture.mark('on its clock it places a log in front, clicking down: axis=y, one log used');

  // The direction button: down, up, north.
  const direction = at(122 + 8, 58 + 8);
  await fixture.raw(direction);
  await until(() => saved(pos), s => field(s, 'direction') === 1, 'the button sets up');
  await Bun.sleep(1000);
  await fixture.raw(direction);
  nbt = await until(() => saved(pos), s => field(s, 'direction') === 2, 'the button sets north');
  await Bun.sleep(1000);
  const shot = join(fixture.run, 'placer.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.command(`setblock ${front} minecraft:air`);
  await until(() => is(front, 'minecraft:oak_log[axis=z]'), v => v, 'clicking north, the next log lies along z', 20000);
  await until(() => fixture.raw('screen'), v => !slot(v, 300)?.item, 'the slot is empty');
  fixture.mark('set to north, the next log lies along z and the slot is empty');

  const ticks = at(144 + 12, 60 + 6);
  await fixture.raw(ticks);
  await until(() => saved(pos), s => field(s, 'tickspeed') === 21, 'a left click: 21 ticks');
  await Bun.sleep(1000);
  await fixture.raw(`${ticks} right`);
  await until(() => saved(pos), s => field(s, 'tickspeed') === 20, 'a right click: back to 20');
  fixture.mark('the tick speed button: left one more, right one less');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS placer', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS placer', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
