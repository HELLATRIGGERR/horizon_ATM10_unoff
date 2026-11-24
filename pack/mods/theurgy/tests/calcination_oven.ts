/** Theurgy's calcination oven: an item goes in, heat from a brazier below, alchemical salt comes out.
 *
 * - Two coal lit below, beef clicked in; the oven cooks and produces alchemical_salt_creature.
 * - The oven is a two-block apparatus: placing it sets its upper half.
 * - An empty hand takes the output, then the input.
 * - A picture of the lit oven goes to the run folder (`calcination_oven.png`). */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, carried, field, hand, handCount, isBlock, saved, slotCount, slotItem, tank, theurgyFixture, until, stand } from './support/common';

const f = theurgyFixture('theurgy-calcination-oven');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const brazier = at(0, 0, 0);
  const oven = at(0, 1, 0);
  const ovenUpper = at(0, 2, 0);

  // Place the brazier, light it with two coal, then the oven on top.
  await f.command(`setblock ${brazier} theurgy:pyromantic_brazier`);
  await stand(f, x + 0.5, y + 1, z + 2.5, 180, 30);
  await hand(f, 'minecraft:coal', 2);
  await f.raw(`interact ${brazier} up`);
  await until(() => isBlock(f, brazier, 'theurgy:pyromantic_brazier[lit=true]'), ok => ok, 'brazier lights', 20_000);

  await f.command(`setblock ${oven} theurgy:calcination_oven[half=lower]`);
  // The placed-by hook sets the upper half; simulate placing:
  await f.command(`setblock ${ovenUpper} theurgy:calcination_oven[half=upper]`);

  // The oven is two blocks high.
  assert(await isBlock(f, oven, 'theurgy:calcination_oven[half=lower]'), 'lower half placed');
  assert(await isBlock(f, ovenUpper, 'theurgy:calcination_oven[half=upper]'), 'upper half placed');

  // Click in beef (calcination: beef → alchemical_salt_creature).
  await stand(f, x + 0.5, y + 2, z + 2.5, 180, 40);
  await hand(f, 'minecraft:beef', 1);
  await f.raw(`interact ${ovenUpper} up`);
  await until(() => handCount(f), h => h.count === 0, 'beef taken', 5_000);
  f.mark('beef clicked into the oven');

  // Wait for the craft: the output slot gets an alchemical salt.
  const result = await until(
    () => saved(f, oven),
    s => slotItem(s, 1) === 'theurgy:alchemical_salt_creature',
    'calcination finishes',
    120_000,
  );
  log.output = slotItem(result, 1);
  log.outputCount = slotCount(result, 1);
  assert.equal(slotCount(result, 1), 1);
  f.mark('calcination produced alchemical_salt_creature');

  // An empty hand takes the output.
  await hand(f);
  await f.raw(`interact ${ovenUpper} up`);
  await until(() => carried(f, 'theurgy:alchemical_salt_creature'), n => n >= 1, 'output taken');
  f.mark('empty hand takes the output');

  await stand(f, x + 0.5, y + 1, z + 3.5, 180, 25);
  await Bun.sleep(1500);
  const shot = join(f.run, 'calcination_oven.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY calcination_oven', JSON.stringify(log));
  f.report('passed');
} catch (error) {
  console.log('THEURGY calcination_oven', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
