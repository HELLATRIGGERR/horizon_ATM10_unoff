/** Theurgy's distiller: items go in, heat from a brazier below, mercury shards come out.
 *
 * - Distillation recipe: 2 beef → 1 mercury_shard.
 * - The distiller is a two-block apparatus; with heat, it processes and produces the output.
 * - A picture of the lit distiller goes to the run folder (`distiller.png`). */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, carried, hand, handCount, isBlock, saved, slotCount, slotItem, theurgyFixture, until } from './support/common';

const f = theurgyFixture('theurgy-distiller');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const brazier = at(0, 0, 0);
  const distiller = at(0, 1, 0);
  const distillerUpper = at(0, 2, 0);

  await f.command(`setblock ${brazier} theurgy:pyromantic_brazier`);
  await hand(f, 'minecraft:coal', 64);
  await f.raw(`interact ${brazier} up`);
  await until(() => isBlock(f, brazier, 'theurgy:pyromantic_brazier[lit=true]'), ok => ok, 'brazier lights', 20_000);

  await f.command(`setblock ${distiller} theurgy:distiller[half=lower]`);
  await f.command(`setblock ${distillerUpper} theurgy:distiller[half=upper]`);

  // The distillation of beef needs 2 beef.
  await f.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 10`);
  await hand(f, 'minecraft:beef', 2);
  await f.raw(`interact ${distiller} up`);
  await until(() => handCount(f), h => h.count === 0, 'beef taken', 5_000);
  f.mark('2 beef clicked into the distiller');

  // Wait for the craft: the output slot gets a mercury_shard.
  const result = await until(
    () => saved(f, distiller),
    s => slotItem(s, 1) === 'theurgy:mercury_shard',
    'distillation finishes',
    120_000,
  );
  log.output = slotItem(result, 1);
  log.outputCount = slotCount(result, 1);
  assert.equal(slotCount(result, 1), 1);
  f.mark('distillation produced mercury_shard');

  // An empty hand takes the output.
  await hand(f);
  await f.raw(`interact ${distiller} up`);
  await until(() => carried(f, 'theurgy:mercury_shard'), n => n >= 1, 'output taken');
  f.mark('empty hand takes the mercury shard');

  await f.command(`tp @s ${x + 0.5} ${y + 1} ${z + 3.5} 180 25`);
  await Bun.sleep(1500);
  const shot = join(f.run, 'distiller.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY distiller', JSON.stringify(log));
  f.report('passed');
} catch (error) {
  console.log('THEURGY distiller', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
