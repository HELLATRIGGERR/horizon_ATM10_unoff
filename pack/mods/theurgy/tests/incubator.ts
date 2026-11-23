/** Theurgy's incubator: mercury, salt and sulfur vessels beside it, heat from below, result in its output.
 *
 * - Incubation recipe: mercury_shard + alchemical_salt_plant + alchemical_sulfur_acacia_log → acacia_log.
 * - The incubator is a two-block apparatus; its multiblock check finds the three vessels among its four sides.
 * - A picture of the working incubator goes to the run folder (`incubator.png`). */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, carried, hand, handCount, isBlock, saved, slotCount, slotItem, theurgyFixture, until } from './support/common';

const f = theurgyFixture('theurgy-incubator');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const brazier = at(0, 0, 0);
  const incubator = at(0, 1, 0);
  const incubatorUpper = at(0, 2, 0);
  // Vessels placed on three sides (north, east, south).
  const mercury = at(0, 1, -1);   // north
  const salt = at(1, 1, 0);       // east
  const sulfur = at(0, 1, 1);     // south

  // Light the brazier.
  await f.command(`setblock ${brazier} theurgy:pyromantic_brazier`);
  await hand(f, 'minecraft:coal', 64);
  await f.raw(`interact ${brazier} up`);
  await until(() => isBlock(f, brazier, 'theurgy:pyromantic_brazier[lit=true]'), ok => ok, 'brazier lights', 20_000);

  // Place the incubator and its vessels.
  await f.command(`setblock ${incubator} theurgy:incubator[half=lower]`);
  await f.command(`setblock ${incubatorUpper} theurgy:incubator[half=upper]`);
  await f.command(`setblock ${mercury} theurgy:incubator_mercury_vessel`);
  await f.command(`setblock ${salt} theurgy:incubator_salt_vessel`);
  await f.command(`setblock ${sulfur} theurgy:incubator_sulfur_vessel`);

  await f.command(`tp @s ${x + 0.5} ${y} ${z + 3.5} 180 10`);

  // Fill the vessels: click one of each ingredient into its vessel.
  await hand(f, 'theurgy:mercury_shard', 1);
  await f.raw(`interact ${mercury} up`);
  await until(() => handCount(f), h => h.count === 0, 'mercury placed', 5_000);

  await hand(f, 'theurgy:alchemical_salt_plant', 1);
  await f.raw(`interact ${salt} up`);
  await until(() => handCount(f), h => h.count === 0, 'salt placed', 5_000);

  await hand(f, 'theurgy:alchemical_sulfur_acacia_log', 1);
  await f.raw(`interact ${sulfur} up`);
  await until(() => handCount(f), h => h.count === 0, 'sulfur placed', 5_000);
  f.mark('three vessel ingredients placed');

  // Wait for the craft: the incubator's output slot gets an acacia_log.
  const result = await until(
    () => saved(f, incubator),
    s => slotItem(s, 0) === 'minecraft:acacia_log',
    'incubation finishes',
    120_000,
  );
  log.output = slotItem(result, 0);
  log.outputCount = slotCount(result, 0);
  assert.equal(slotCount(result, 0), 1);
  f.mark('incubation produced acacia_log');

  // The vessel inputs were consumed (one of each).
  for (const [label, pos] of [['mercury', mercury], ['salt', salt], ['sulfur', sulfur]] as const) {
    const nbt = await saved(f, pos);
    assert.equal(slotCount(nbt, 0), 0, `${label} vessel's input consumed`);
  }
  f.mark('vessel inputs consumed');

  // An empty hand takes the output.
  await hand(f);
  await f.raw(`interact ${incubator} up`);
  await until(() => carried(f, 'minecraft:acacia_log'), n => n >= 1, 'output taken');
  f.mark('empty hand takes the acacia_log');

  await f.command(`tp @s ${x + 0.5} ${y + 2} ${z + 4.5} 180 30`);
  await Bun.sleep(1500);
  const shot = join(f.run, 'incubator.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY incubator', JSON.stringify(log));
  f.report('passed');
} catch (error) {
  console.log('THEURGY incubator', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
