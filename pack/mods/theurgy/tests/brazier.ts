/** Theurgy's pyromantic brazier, as the `theurgy` native bundle runs it on the carry of the player's own jar.
 *
 * - A click with coal puts the whole stack in (fuel only: a diamond stays in the hand); the brazier lights at once,
 *   uses one coal and is lit for its 1,600 ticks of burn time (`remainingLitTime`), then goes out with no fuel left.
 * - An empty hand takes the fuel back out.
 * - A lava bucket burns 20,000 ticks and leaves its bucket in the brazier, which a click with anything takes out
 *   (`brazierHasNonFuel`).
 * - A picture of the lit brazier goes to the run folder (`brazier.png`). */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, carried, field, hand, handCount, isBlock, saved, slotCount, slotItem, theurgyFixture, until, stand } from './support/common';

const f = theurgyFixture('theurgy-brazier');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const pos = at(0, 0, 0);
  await f.command(`setblock ${pos} theurgy:pyromantic_brazier`);
  await stand(f, x + 0.5, y + 1, z + 2.5, 180, 30);

  // A diamond is no fuel: it stays in the hand.
  await hand(f, 'minecraft:diamond', 1);
  await f.raw(`interact ${pos} up`);
  await Bun.sleep(500);
  assert.equal((await handCount(f)).count, 1, 'a diamond is refused');
  assert.equal(slotCount(await saved(f, pos), 0), 0);

  // Two coal go in; one lights it.
  await hand(f, 'minecraft:coal', 2);
  await f.raw(`interact ${pos} up`);
  await until(() => isBlock(f, pos, 'theurgy:pyromantic_brazier[lit=true]'), ok => ok, 'the brazier lights', 20_000);
  const lit = await saved(f, pos);
  assert.equal((await handCount(f)).count, 0, 'the whole stack went in');
  assert.equal(slotCount(lit, 0), 1, `one coal burns, one waits: ${lit}`);
  const left = field(lit, 'remainingLitTime');
  assert(left > 1500 && left <= 1600, `a coal burns 1,600 ticks: ${left} left`);
  log.lit = left;
  f.mark('coal in by a click (a diamond refused); it lights at once, one coal burning 1,600 ticks');

  // An empty hand takes the waiting coal back.
  await hand(f);
  await f.raw(`interact ${pos} up`);
  await until(() => saved(f, pos), s => slotCount(s, 0) === 0, 'the coal comes out');
  assert.equal(await carried(f, 'minecraft:coal'), 1, 'the coal is back in the inventory');
  f.mark('an empty hand takes the fuel out');

  // Out after its 1,600 ticks: held, then stepped.
  await f.hold();
  const before = field(await saved(f, pos), 'remainingLitTime');
  // One step is at most 1,000 ticks.
  for (let rest = before - 2; rest > 0; rest -= 1000) await f.stepServer(Math.min(rest, 1000));
  assert(await isBlock(f, pos, 'theurgy:pyromantic_brazier[lit=true]'), `still lit with 2 of ${before} ticks left`);
  await f.stepServer(4);
  assert(await isBlock(f, pos, 'theurgy:pyromantic_brazier[lit=false]'), 'out once the coal has burnt');
  await f.hold(false);
  f.mark(`the coal burnt out after its ticks (${before} were left)`);

  // Lava: 20,000 ticks, and its bucket stays in the brazier until a click takes it.
  const lava = at(2, 0, 0);
  await f.command(`setblock ${lava} theurgy:pyromantic_brazier`);
  await hand(f, 'minecraft:lava_bucket', 1);
  await f.raw(`interact ${lava} up`);
  const burning = await until(() => saved(f, lava), s => slotItem(s, 0) === 'minecraft:bucket', 'the lava burns and leaves its bucket', 20_000);
  assert(field(burning, 'remainingLitTime') > 19_900, `lava burns 20,000 ticks: ${burning}`);
  assert(await isBlock(f, lava, 'theurgy:pyromantic_brazier[lit=true]'));
  await hand(f, 'minecraft:coal', 1);
  await f.raw(`interact ${lava} up`);
  await until(() => saved(f, lava), s => slotCount(s, 0) === 0, 'a click takes the empty bucket out');
  assert.equal((await handCount(f)).count, 1, 'the coal stays in the hand while the bucket comes out');
  assert.equal(await carried(f, 'minecraft:bucket'), 1);
  f.mark('a lava bucket burns 20,000 ticks and leaves its bucket, which the next click takes out');

  await stand(f, x + 1.5, y + 1, z + 3.5, 180, 35);
  await Bun.sleep(1500);
  const shot = join(f.run, 'brazier.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY brazier', JSON.stringify(log));
  f.report('passed');
} catch (error) {
  console.log('THEURGY brazier', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
