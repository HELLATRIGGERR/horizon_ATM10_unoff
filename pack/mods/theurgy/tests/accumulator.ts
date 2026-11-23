/** Theurgy's sal ammoniac accumulator and tank: water evaporates into sal ammoniac fluid in the tank below.
 *
 * - Accumulation recipe: 1,000 mB water → 100 mB sal ammoniac (time 100 ticks, no heat needed).
 * - The accumulator sits on a sal ammoniac tank (a fluid handler below it).
 * - A water bucket clicked fills the accumulator's water tank; the result flows into the tank below.
 * - A picture of the working setup goes to the run folder (`accumulator.png`). */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, hand, handCount, isBlock, saved, tank, theurgyFixture, until } from './support/common';

const f = theurgyFixture('theurgy-accumulator');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const tankPos = at(0, 0, 0);
  const accumulator = at(0, 1, 0);

  // Place the tank, then the accumulator on top.
  await f.command(`setblock ${tankPos} theurgy:sal_ammoniac_tank`);
  await f.command(`setblock ${accumulator} theurgy:sal_ammoniac_accumulator`);

  await f.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);

  // Fill the accumulator's water tank with a water bucket.
  await hand(f, 'minecraft:water_bucket', 1);
  await f.raw(`interact ${accumulator} up`);
  await until(
    () => saved(f, accumulator),
    s => tank(s, 'waterTank').fluid !== '' && tank(s, 'waterTank').amount > 0,
    'water tank fills',
    10_000,
  );
  const filled = tank(await saved(f, accumulator), 'waterTank');
  log.waterFluid = filled.fluid;
  log.waterAmount = filled.amount;
  assert.equal(filled.amount, 1000, `a bucket is 1,000 mB: got ${filled.amount}`);
  f.mark('water bucket fills the accumulator\'s tank');

  // Wait for the craft: sal ammoniac appears in the tank below.
  const result = await until(
    () => saved(f, tankPos),
    s => tank(s, 'tank').fluid === 'theurgy:sal_ammoniac' && tank(s, 'tank').amount > 0,
    'accumulation produces sal ammoniac',
    120_000,
  );
  const produced = tank(result, 'tank');
  log.salFluid = produced.fluid;
  log.salAmount = produced.amount;
  assert.equal(produced.fluid, 'theurgy:sal_ammoniac');
  assert(produced.amount >= 100, `at least 100 mB produced: got ${produced.amount}`);
  f.mark(`accumulator produced ${produced.amount} mB sal ammoniac`);

  // The water tank was consumed.
  const after = tank(await saved(f, accumulator), 'waterTank');
  log.waterAfter = after.amount;
  assert(after.amount < filled.amount, `water consumed: ${after.amount} < ${filled.amount}`);
  f.mark('water consumed from the accumulator');

  await f.command(`tp @s ${x + 0.5} ${y + 1} ${z + 3.5} 180 25`);
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
