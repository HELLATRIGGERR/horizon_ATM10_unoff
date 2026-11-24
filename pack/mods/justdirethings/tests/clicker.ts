/** Just Dire Things' Clicker T1, as the `justdirethings` native bundle and the content mod made from the carry of the
 * player's jar (the clicker on the furnace template) make it: the game's fake player clicks with the stack in its slot.
 *
 * - Its screen opens on a click with one slot; a bucket goes in. On its clock (every 20 ticks) its fake player right
 *   clicks the water source in front: the water is gone and the slot holds a water bucket.
 * - A second clicker gets a diamond sword; its click type button set to a left click and its target button to hostile
 *   mobs, its fake player hits the husk in front of it (its health drops).
 * - A Clicker T2 with a bucket clicks nothing without energy; given Forge Energy (its buffer written), it takes the water
 *   in front for 250 FE.
 * - A picture of its screen goes to the run folder (`clicker.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-clicker', 'server-thread', { devMods: [content] });
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
async function health(): Promise<number> {
  const reply = await fixture.command('data get entity @e[type=minecraft:husk,limit=1] Health');
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following entity data'));
  return Number(line?.match(/data: ?(-?[\d.]+)/)?.[1] ?? NaN);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 3} ${y} ${z - 3} ${x + 11} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 3} ${x + 11} ${y - 1} ${z + 4} minecraft:stone`);
  await fixture.command('gamemode survival @s');
  await fixture.command('time set midnight');
  const pos = `${x} ${y} ${z}`, front = `${x} ${y} ${z - 1}`;
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;

  // A bucket, and the water in front: walled in, so it stays a source.
  await fixture.replace('hotbar.0', 'minecraft:bucket', 1);
  await fixture.command(`fill ${x - 1} ${y} ${z - 2} ${x + 1} ${y} ${z - 2} minecraft:stone`);
  await fixture.command(`setblock ${x - 1} ${y} ${z - 1} minecraft:stone`);
  await fixture.command(`setblock ${x + 1} ${y} ${z - 1} minecraft:stone`);
  await fixture.command(`setblock ${pos} justdirethings:clickert1[facing=north]`);
  await fixture.command(`setblock ${front} minecraft:water`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:clickert1', 'the clicker reaches the client');
  await fixture.raw(`interact ${pos} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the clicker screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `one slot of its own: ${JSON.stringify(view.slots)}`);
  const nbt = await saved(pos);
  assert.equal(field(nbt, 'clickType'), 0, 'a right click by default');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:bucket', 'the bucket on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the bucket in the slot');
  fixture.mark('the screen shows one slot; a bucket goes in');

  await until(() => is(front, 'minecraft:air'), v => v, 'the fake player takes the water in front', 30000);
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'minecraft:water_bucket', 'the slot holds a water bucket');
  fixture.mark('on its clock the fake player right clicks the water with the bucket: the water is gone, a water bucket in the slot');
  const shot = join(fixture.run, 'clicker.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('key escape tap');

  // The second clicker: a sword, a left click, hostile mobs.
  const x2 = x + 4, pos2 = `${x2} ${y} ${z}`;
  await fixture.replace('hotbar.0', 'minecraft:diamond_sword', 1);
  await fixture.command(`setblock ${pos2} justdirethings:clickert1[facing=north]`);
  await fixture.command(`tp @s ${x2 + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos2}`), v => v.block === 'justdirethings:clickert1', 'the second clicker reaches the client');
  await fixture.raw(`interact ${pos2} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'its screen opens');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:diamond_sword', 'the sword on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the sword in the slot');
  await fixture.raw(at(38 + 8, 58 + 8));
  await until(() => saved(pos2), s => field(s, 'clickType') === 1, 'the click type button: a left click');
  await Bun.sleep(500);
  await fixture.raw(at(56 + 8, 58 + 8));
  await until(() => saved(pos2), s => field(s, 'clickTarget') === 1, 'the target button: air');
  await Bun.sleep(500);
  await fixture.raw(at(56 + 8, 58 + 8));
  await until(() => saved(pos2), s => field(s, 'clickTarget') === 2, 'the target button: hostile mobs');
  fixture.mark('a sword in the second clicker; its buttons set a left click on hostile mobs');
  await fixture.command(`summon minecraft:husk ${x2 + 0.5} ${y} ${z - 0.5} {NoAI:1b,PersistenceRequired:1b}`);
  const full = await until(() => health(), h => h > 0, 'the husk is there');
  log.full = full;
  const hit = await until(() => health(), h => h < full, 'the fake player hits the husk', 30000);
  log.hit = hit;
  fixture.mark(`the fake player hits the husk with the sword: ${full} to ${hit} health`);
  await fixture.raw('key escape tap');

  // The Clicker T2: energy first.
  const x3 = x + 8, pos3 = `${x3} ${y} ${z}`, front3 = `${x3} ${y} ${z - 1}`;
  await fixture.replace('hotbar.0', 'minecraft:bucket', 1);
  await fixture.command(`fill ${x3 - 1} ${y} ${z - 2} ${x3 + 1} ${y} ${z - 2} minecraft:stone`);
  await fixture.command(`setblock ${x3 - 1} ${y} ${z - 1} minecraft:stone`);
  await fixture.command(`setblock ${x3 + 1} ${y} ${z - 1} minecraft:stone`);
  await fixture.command(`setblock ${pos3} justdirethings:clickert2[facing=north]`);
  await fixture.command(`setblock ${front3} minecraft:water`);
  await fixture.command(`tp @s ${x3 + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos3}`), v => v.block === 'justdirethings:clickert2', 'the clicker T2 reaches the client');
  await fixture.raw(`interact ${pos3} south`);
  const view3 = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the clicker T2 screen opens');
  const own3 = (view3.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own3, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], `its slot and nine filter slots: ${JSON.stringify(view3.slots)}`);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:bucket', 'the bucket on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the bucket in the slot');
  await Bun.sleep(3000);
  assert(await is(front3, 'minecraft:water'), 'no energy, no click');
  await fixture.command(`data merge block ${pos3} {energy:1000}`);
  await until(() => is(front3, 'minecraft:air'), v => v, 'with energy its fake player takes the water', 30000);
  const energy3 = field(await saved(pos3), 'energy');
  log.t2energy = energy3;
  assert.equal(energy3, 750, 'a click that consumed costs 250 FE');
  fixture.mark('the Clicker T2 clicks nothing without energy; with it, its fake player takes the water for 250 FE');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS clicker', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS clicker', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
