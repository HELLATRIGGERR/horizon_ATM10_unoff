/** Just Dire Things' Block Breaker T1, as the `justdirethings` native bundle and the content mod made from the carry of
 * the player's jar (the breaker on the furnace template) make it.
 *
 * - Its screen opens on a click: one tool slot (the hopper menu's four others hidden); an iron pickaxe goes in.
 * - It mines the stone in front of it as Java's fake player does: it starts on its operation clock (every 20 ticks)
 *   and an iron pickaxe (speed 6) takes 8 ticks for stone (hardness 1.5, correct tool: 6 / 1.5 / 30 a tick), so a
 *   stone put back at once breaks 20 ticks after the one before; the stone drops cobblestone and the pickaxe takes one
 *   point of wear a block (in survival).
 * - The redstone button set to "high" with no signal stops it, and the block's `active` turns false.
 * - A picture of its screen goes to the run folder (`breaker.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-breaker', 'server-thread', { devMods: [content] });

async function is(pos: string, state: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${state}`);
  return (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
}
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const gametime = async () => Number(((await fixture.command('time query gametime')).chat ?? []).join(' ').match(/(\d+)/)?.[1]);
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

  await fixture.replace('hotbar.0', 'minecraft:iron_pickaxe', 1);
  await fixture.command(`setblock ${pos} justdirethings:blockbreakert1[facing=north]`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:blockbreakert1', 'the breaker reaches the client');
  await fixture.raw(`interact ${pos} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the breaker screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `the tool slot only: ${JSON.stringify(view.slots)}`);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:iron_pickaxe', 'the pickaxe on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the pickaxe in the tool slot');
  await Bun.sleep(300);
  const shot = join(fixture.run, 'breaker.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  fixture.mark('the screen shows one tool slot; an iron pickaxe goes in');

  // Two stones, one after the other: the second breaks one operation (20 ticks) after the first.
  await fixture.command(`setblock ${front} minecraft:stone`);
  await until(() => is(front, 'minecraft:air'), v => v, 'the breaker mines the stone', 20000);
  const first = await gametime();
  await fixture.command(`setblock ${front} minecraft:stone`);
  await until(() => is(front, 'minecraft:air'), v => v, 'the breaker mines the second stone', 20000);
  const second = await gametime();
  log.period = second - first;
  assert(second - first >= 18 && second - first <= 24, `a stone every operation of 20 ticks: ${second - first}`);
  const drops = await fixture.command(`execute if entity @e[type=minecraft:item,x=${x},y=${y},z=${z - 1},distance=..3,nbt={Item:{id:"minecraft:cobblestone"}}]`);
  assert((drops.chat ?? []).some((l: string) => /Test passed/.test(l)), `the stone drops cobblestone: ${JSON.stringify(drops.chat)}`);
  const nbt = await saved(pos);
  log.nbt = nbt;
  assert.match(nbt, /"minecraft:damage": ?2\b/, 'the pickaxe took a point of wear for each stone');
  fixture.mark(`it mines stone with an iron pickaxe, one every ${second - first} ticks, dropping cobblestone and wearing the pickaxe`);

  // "high" with no signal: it stops, and the block shows it.
  const button = [ox + 104 + 8, oy + 58 + 8];
  await fixture.raw(`click-at ${button[0]} ${button[1]}`);
  await until(() => saved(pos), s => /redstoneMode: ?1\b/.test(s), 'the button sets "low"');
  // The button cycles from the mode the screen shows: let the menu's data reach the client first.
  await Bun.sleep(1000);
  await fixture.raw(`click-at ${button[0]} ${button[1]}`);
  await until(() => saved(pos), s => /redstoneMode: ?2\b/.test(s), 'the button sets "high"');
  await until(() => is(pos, 'justdirethings:blockbreakert1[active=false]'), v => v, 'the breaker shows itself inactive');
  await fixture.command(`setblock ${front} minecraft:stone`);
  await Bun.sleep(3000);
  assert(await is(front, 'minecraft:stone'), '"high" with no signal mines nothing');
  fixture.mark('in "high" with no signal it mines nothing and its block turns inactive');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS breaker', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS breaker', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
