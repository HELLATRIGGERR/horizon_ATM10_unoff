/** Just Dire Things' Block Placer T2 and Block Breaker T2, as the `justdirethings` native bundle and the content mod made
 * from the carry of the player's jar (both on the furnace template) make them: what every T2 machine shares (a 100,000 FE
 * buffer, an area, nine filter slots) and each one's work.
 *
 * - The Placer T2's screen opens with its slot and nine filter slots; with no energy it places nothing. Filled with
 *   Forge Energy (a Powah-free `data merge` of its buffer), it places the cobblestone of its slot in front of it, 500 FE
 *   a placement. Its area's X radius button made 1, it places three in a row, nearest first.
 * - The Breaker T2 with an iron pickaxe and energy breaks the stone in front of it, 500 FE a break; its X radius made
 *   1, it breaks a row of three, one a run.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-t2', 'server-thread', { devMods: [content] });
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
  await fixture.command(`fill ${x - 3} ${y} ${z - 3} ${x + 8} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 3} ${x + 8} ${y - 1} ${z + 4} minecraft:stone`);
  await fixture.command('gamemode survival @s');
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;
  // The X radius's plus (`value_button(0, 0)` + 30).
  const radiusX = at(25 + 30 + 6, 32 + 6);

  // ---- Placer T2 ----
  const pos = `${x} ${y} ${z}`, front = (dx: number) => `${x + dx} ${y} ${z - 1}`;
  await fixture.replace('hotbar.0', 'minecraft:cobblestone', 4);
  await fixture.command(`setblock ${pos} justdirethings:blockplacert2[facing=north]`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:blockplacert2', 'the placer reaches the client');
  await fixture.raw(`interact ${pos} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the placer T2 screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], `its slot and nine filter slots: ${JSON.stringify(view.slots)}`);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:cobblestone', 'the cobblestone on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the cobblestone in the slot');
  await Bun.sleep(3000);
  assert(await is(front(0), 'minecraft:air'), 'no energy, no placement');
  fixture.mark('the Placer T2 shows its slot and nine filter slots; without energy it places nothing');
  await fixture.command(`data merge block ${pos} {energy:100000}`);
  await until(() => is(front(0), 'minecraft:cobblestone'), v => v, 'with energy it places in front', 30000);
  const after = field(await saved(pos), 'energy');
  log.placerEnergy = after;
  assert(after <= 99500, `a placement takes 500 FE: ${after}`);
  fixture.mark(`with energy it places the cobblestone in front (${after} FE left)`);
  await fixture.raw(radiusX);
  await until(() => saved(pos), s => /xRadiusDouble: ?0\.5/.test(s), 'the X radius button: 0.5');
  await Bun.sleep(500);
  await fixture.raw(radiusX);
  await until(() => saved(pos), s => /xRadiusDouble: ?1\.0?d?/.test(s), 'the X radius button: 1');
  await until(async () => (await is(front(-1), 'minecraft:cobblestone')) && (await is(front(1), 'minecraft:cobblestone')), v => v, 'a row of three', 60000);
  fixture.mark('its area made three wide, it fills the row');
  await fixture.raw('key escape tap');

  // ---- Breaker T2 ----
  const x2 = x + 5, pos2 = `${x2} ${y} ${z}`, front2 = (dx: number) => `${x2 + dx} ${y} ${z - 1}`;
  await fixture.command(`fill ${x2 - 1} ${y} ${z - 1} ${x2 + 1} ${y} ${z - 1} minecraft:stone`);
  await fixture.replace('hotbar.0', 'minecraft:iron_pickaxe', 1);
  await fixture.command(`setblock ${pos2} justdirethings:blockbreakert2[facing=north]`);
  await fixture.command(`tp @s ${x2 + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos2}`), v => v.block === 'justdirethings:blockbreakert2', 'the breaker reaches the client');
  await fixture.raw(`interact ${pos2} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the breaker T2 screen opens');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:iron_pickaxe', 'the pickaxe on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the pickaxe in the slot');
  const shot = join(fixture.run, 't2.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await Bun.sleep(3000);
  assert(await is(front2(0), 'minecraft:stone'), 'no energy, no break');
  await fixture.command(`data merge block ${pos2} {energy:100000}`);
  await until(() => is(front2(0), 'minecraft:air'), v => v, 'with energy it breaks the stone in front', 30000);
  fixture.mark('the Breaker T2 with a pickaxe and energy breaks the stone in front');
  await fixture.raw(radiusX);
  await until(() => saved(pos2), s => /xRadiusDouble: ?0\.5/.test(s), 'the X radius button: 0.5');
  await Bun.sleep(500);
  await fixture.raw(radiusX);
  await until(() => saved(pos2), s => /xRadiusDouble: ?1\.0?d?/.test(s), 'the X radius button: 1');
  await until(async () => (await is(front2(-1), 'minecraft:air')) && (await is(front2(1), 'minecraft:air')), v => v, 'the row is broken', 60000);
  const left = field(await saved(pos2), 'energy');
  log.breakerEnergy = left;
  assert(left <= 98500, `three breaks take 1,500 FE: ${left}`);
  fixture.mark(`its area made three wide, it breaks the row (${left} FE left)`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS t2', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS t2', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
