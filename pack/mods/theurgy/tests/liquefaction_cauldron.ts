/** Theurgy's liquefaction cauldron: an item + a solvent fluid, heat from below, alchemical sulfur out.
 *
 * - Liquefaction recipe: acacia_log + 10 mB sal_ammoniac → alchemical_sulfur_acacia_log.
 * - The cauldron is a two-block apparatus with a solvent tank (2,000 mB).
 * - A bucket of sal_ammoniac clicked fills the tank; then the input is clicked in.
 * - A picture of the lit cauldron goes to the run folder (`liquefaction_cauldron.png`). */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, carried, field, hand, handCount, isBlock, saved, slotCount, slotItem, tank, theurgyFixture, until } from './support/common';

const f = theurgyFixture('theurgy-liquefaction-cauldron');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const brazier = at(0, 0, 0);
  const cauldron = at(0, 1, 0);
  const cauldronUpper = at(0, 2, 0);

  await f.command(`setblock ${brazier} theurgy:pyromantic_brazier`);
  await hand(f, 'minecraft:coal', 64);
  await f.raw(`interact ${brazier} up`);
  await until(() => isBlock(f, brazier, 'theurgy:pyromantic_brazier[lit=true]'), ok => ok, 'brazier lights', 20_000);

  await f.command(`setblock ${cauldron} theurgy:liquefaction_cauldron[half=lower]`);
  await f.command(`setblock ${cauldronUpper} theurgy:liquefaction_cauldron[half=upper]`);

  await f.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 10`);

  // Fill the solvent tank: a bucket of sal_ammoniac.
  await hand(f, 'theurgy:sal_ammoniac_bucket', 1);
  await f.raw(`interact ${cauldron} up`);
  await until(
    () => saved(f, cauldron),
    s => tank(s, 'solventTank').fluid === 'theurgy:sal_ammoniac',
    'tank fills with sal_ammoniac',
    10_000,
  );
  const filled = tank(await saved(f, cauldron), 'solventTank');
  log.tankFluid = filled.fluid;
  log.tankAmount = filled.amount;
  assert.equal(filled.amount, 1000, `a bucket is 1,000 mB: got ${filled.amount}`);
  f.mark('sal_ammoniac bucket fills the solvent tank');

  // Click in the input: acacia_log.
  await hand(f, 'minecraft:acacia_log', 1);
  await f.raw(`interact ${cauldron} up`);
  await until(() => handCount(f), h => h.count === 0, 'acacia_log taken', 5_000);
  f.mark('acacia_log clicked into the cauldron');

  // Wait for the craft: alchemical_sulfur_acacia_log appears in the output.
  const result = await until(
    () => saved(f, cauldron),
    s => slotItem(s, 1) === 'theurgy:alchemical_sulfur_acacia_log',
    'liquefaction finishes',
    120_000,
  );
  log.output = slotItem(result, 1);
  assert.equal(slotCount(result, 1), 1);
  const afterTank = tank(result, 'solventTank');
  log.tankAfter = afterTank.amount;
  assert(afterTank.amount < filled.amount, `the solvent was consumed: ${afterTank.amount} < ${filled.amount}`);
  f.mark('liquefaction produced alchemical_sulfur_acacia_log, solvent consumed');

  await f.command(`tp @s ${x + 0.5} ${y + 1} ${z + 3.5} 180 25`);
  await Bun.sleep(1500);
  const shot = join(f.run, 'liquefaction_cauldron.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY liquefaction_cauldron', JSON.stringify(log));
  f.report('passed');
} catch (error) {
  console.log('THEURGY liquefaction_cauldron', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
