/** Theurgy's sal ammoniac accumulator and tank: water evaporates into sal ammoniac fluid in the tank below.
 *
 * - Accumulation recipe: 1,000 mB water → 100 mB sal ammoniac (time 100 ticks, no heat needed).
 * - The accumulator sits on a sal ammoniac tank (a fluid handler below it).
 * - A water bucket clicked empties into the accumulator's water tank; the result flows into the tank below.
 * - A picture of the working setup goes to the run folder (`accumulator.png`). */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, hand, handCount, saved, tank, theurgyFixture, until, stand } from './support/common';

const f = theurgyFixture('theurgy-accumulator');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const tankPos = at(0, 0, 0);
  const accumulator = at(0, 1, 0);

  // Place the tank, then the accumulator on top.
  await f.command(`setblock ${tankPos} theurgy:sal_ammoniac_tank`);
  await f.command(`setblock ${accumulator} theurgy:sal_ammoniac_accumulator`);
  await stand(f, x + 0.5, y + 1, z + 2.5, 180, 30);

  // A bucket empties into the accumulator's water tank. Theurgy's crafting behaviour starts with a total time of 0
  // (the water tank never reports an input change), so the first tick crafts at once: the 1,000 mB evaporate into
  // 100 mB of sal ammoniac in the tank below.
  await hand(f, 'minecraft:water_bucket', 1);
  await f.raw(`interact ${accumulator} up`);
  assert.equal((await handCount(f)).item, 'minecraft:bucket', 'the bucket was emptied into the accumulator');
  f.mark('water bucket emptied into the accumulator');
  const first = await until(
    () => saved(f, tankPos),
    s => tank(s, 'tank').fluid === 'theurgy:sal_ammoniac' && tank(s, 'tank').amount > 0,
    'accumulation produces sal ammoniac',
    20_000,
  );
  log.firstFluid = tank(first, 'tank').fluid;
  log.firstAmount = tank(first, 'tank').amount;
  assert.equal(tank(first, 'tank').amount, 100, `one craft is 100 mB: got ${tank(first, 'tank').amount}`);
  const left = tank(await saved(f, accumulator), 'waterTank');
  log.waterAfter = left.amount;
  assert.equal(left.amount, 0, `the bucket was evaporated: ${left.amount} mB left`);
  f.mark('first craft: 100 mB sal ammoniac, water tank empty');

  // The second bucket takes the recipe's 100 ticks.
  await hand(f, 'minecraft:water_bucket', 1);
  await f.raw(`interact ${accumulator} up`);
  const mid = tank(await saved(f, tankPos), 'tank').amount;
  const second = await until(
    () => saved(f, tankPos),
    s => tank(s, 'tank').amount >= 200,
    'second craft after the recipe time',
    60_000,
  );
  log.secondAmount = tank(second, 'tank').amount;
  assert.equal(tank(second, 'tank').amount, 200, `two crafts are 200 mB: got ${tank(second, 'tank').amount}`);
  assert(mid < 200, 'the second craft is not instant');
  f.mark('second craft after the recipe time: 200 mB sal ammoniac');

  await stand(f, x + 0.5, y + 2, z + 3.5, 180, 25);
  await Bun.sleep(1500);
  const shot = join(f.run, 'accumulator.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY accumulator', JSON.stringify(log));
  f.report('passed');
} catch (error) {
  console.log('THEURGY accumulator', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
