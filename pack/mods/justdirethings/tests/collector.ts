/** Just Dire Things' Fluid Collector T1, as the `justdirethings` native bundle and the content mod made from the carry
 * of the player's jar (the collector on the furnace template) make it.
 *
 * - On its operation clock (every 20 ticks) it takes the water source in front of it into its tank, a bucket each
 *   (`Fluid`: 1,000, then 2,000 mB for a second source).
 * - A lava source in front stays: the tank holds water.
 * - Its screen opens on a click with one slot (the hopper menu's four others hidden); an empty bucket put in comes
 *   back a water bucket, and the tank is a bucket lighter.
 * - A picture of its screen (the fluid bar) goes to the run folder (`collector.png`).
 *
 * Reads the saved compound (`data get block`). Needs the staged native build (`AE2_TEST_BINARY`) and the content mod
 * (`JUSTDIRETHINGS_CONTENT_DIR`); `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-collector', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
/** The tank's amount and fluid from the saved `Fluid` compound (0 and none while it is empty). */
const tank = (nbt: string) => {
  const at = nbt.indexOf('Fluid:');
  if (at < 0) return { amount: 0, id: '' };
  const part = nbt.slice(at, at + 120);
  return { amount: Number(part.match(/amount: ?(\d+)/)?.[1] ?? 0), id: part.match(/id: ?"([^"]+)"/)?.[1] ?? '' };
};
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + 2} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + 2} ${y - 1} ${z + 4} minecraft:stone`);
  const pos = `${x} ${y} ${z}`, front = `${x} ${y} ${z - 1}`;
  // A one-block basin in front, walled with stone.
  await fixture.command(`fill ${x - 1} ${y} ${z - 2} ${x + 1} ${y} ${z - 1} minecraft:stone`);
  await fixture.command(`setblock ${pos} justdirethings:fluidcollectort1[facing=north]`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:fluidcollectort1', 'the collector reaches the client');

  await fixture.command(`setblock ${front} minecraft:water`);
  await until(() => is(front, 'minecraft:air'), v => v, 'the collector takes the water source', 20000);
  let nbt = await until(() => saved(pos), s => tank(s).amount === 1000, 'a bucket of water in the tank');
  assert.equal(tank(nbt).id, 'minecraft:water');
  await fixture.command(`setblock ${front} minecraft:water`);
  nbt = await until(() => saved(pos), s => tank(s).amount === 2000, 'a second bucket', 20000);
  assert(await is(front, 'minecraft:air'));
  fixture.mark('it takes a water source in front each operation: 1,000 then 2,000 mB of water');

  await fixture.command(`setblock ${front} minecraft:lava`);
  await Bun.sleep(3000);
  assert(await is(front, 'minecraft:lava[level=0]'), 'the lava source stays');
  assert.equal(tank(await saved(pos)).amount, 2000, 'the tank still holds 2,000 mB of water');
  await fixture.command(`setblock ${front} minecraft:stone`);
  fixture.mark('a lava source stays in front of a tank of water');

  await fixture.replace('hotbar.0', 'minecraft:bucket', 1);
  await fixture.raw(`interact ${pos} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the collector screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `one slot of its own: ${JSON.stringify(view.slots)}`);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:bucket', 'the bucket on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'minecraft:water_bucket', 'the bucket comes back full');
  nbt = await until(() => saved(pos), s => tank(s).amount === 1000, 'the tank a bucket lighter');
  log.tank = tank(nbt);
  fixture.mark('an empty bucket in its slot comes back a water bucket, 1,000 mB out of the tank');
  await Bun.sleep(1000);
  const shot = join(fixture.run, 'collector.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS collector', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS collector', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
