/** Theurgy's mercurial reformation array: a mercury shard in a catalyst feeds the sulfuric flux emitter beside it, which
 * (with its pedestals picked by a click each, then placed) turns two crop niters and an animal niter into a
 * reformed one on the result pedestal. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, field, isBlock, handCount, hand, saved, slotCount, slotItem, theurgyFixture, until, stand } from './support/common';

const f = theurgyFixture('theurgy-reformation');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const catalyst = at(0, 0, 0), emitter = at(0, 1, 0);
  const src1 = at(-2, 0, 1), src2 = at(-2, 0, 2), target = at(2, 0, 1), result = at(2, 0, 2);
  await f.command(`setblock ${catalyst} theurgy:mercury_catalyst`);
  await f.command(`setblock ${src1} theurgy:reformation_source_pedestal`);
  await f.command(`setblock ${src2} theurgy:reformation_source_pedestal`);
  await f.command(`setblock ${target} theurgy:reformation_target_pedestal`);
  await f.command(`setblock ${result} theurgy:reformation_result_pedestal`);
  await stand(f, x + 0.5, y + 1, z + 3.5, 180, 25);

  const click = async (pos: string, item: string, face = 'up') => {
    await hand(f, item, 1);
    await f.raw(`interact ${pos} ${face}`);
    await until(() => handCount(f), h => h.count === 0, `${item} into ${pos}`, 5_000);
  };
  await click(src1, 'theurgy:alchemical_niter_crops_abundant');
  await click(src2, 'theurgy:alchemical_niter_crops_abundant');
  await click(target, 'theurgy:alchemical_niter_animals_abundant');
  await until(() => saved(f, src1), s => slotItem(s, 0) === 'theurgy:alchemical_niter_crops_abundant', 'the source pedestal holds its niter', 5_000);
  // The result pedestal refuses a hand's item; a plain stick is no niter for the source pedestal either (and places nothing, as a block would).
  await hand(f, 'minecraft:stick', 1);
  await f.raw(`interact ${result} up`);
  await f.raw(`interact ${src1} up`);
  assert.equal(slotItem(await saved(f, result), 0), '', 'the result pedestal takes nothing in');
  assert.equal((await handCount(f)).count, 1, 'the stick stays in the hand');
  f.mark('the pedestals hold their niters');

  // The shard in the catalyst, then the emitter: a click on each pedestal picks it, a click on the catalyst's top places it.
  await click(catalyst, 'theurgy:mercury_shard');  await hand(f, 'theurgy:sulfuric_flux_emitter', 1);
  for (const pedestal of [src1, src2, target, result]) await f.raw(`interact ${pedestal} up`);
  assert.equal((await handCount(f)).count, 1, 'the emitter is not placed on a pedestal');
  await f.raw(`interact ${catalyst} up`);
  await until(() => isBlock(f, emitter, 'theurgy:sulfuric_flux_emitter'), placed => placed, 'the emitter is placed', 5_000);
  const emitted = await saved(f, emitter);
  assert.equal((emitted.match(/blockPos/g) ?? []).length, 4, 'the emitter knows its four pedestals: ' + emitted);
  f.mark('the emitter knows its pedestals');

  const flux = await until(() => saved(f, emitter), s => field(s, 'mercuryFluxStorage') > 0, 'the catalyst feeds the emitter', 60_000);
  log.fluxFirst = field(flux, 'mercuryFluxStorage');
  const done = await until(() => saved(f, result), s => slotItem(s, 0) === 'theurgy:alchemical_niter_animals_abundant', 'the array reforms', 240_000);
  assert.equal(slotCount(done, 0), 1);
  const spent = await saved(f, src1);
  assert.equal(slotItem(spent, 0), '', 'the first source is used up');
  assert.equal(slotItem(await saved(f, src2), 0), '', 'the second source is used up');
  assert.equal(slotItem(await saved(f, target), 0), 'theurgy:alchemical_niter_animals_abundant', 'the target stays');
  log.output = slotItem(done, 0);
  log.fluxAfter = field(await saved(f, emitter), 'mercuryFluxStorage');
  log.catalyst = field(await saved(f, catalyst), 'mercuryFluxStorage');
  f.mark('crafted theurgy:alchemical_niter_animals_abundant on the result pedestal');

  await stand(f, x + 0.5, y + 1, z + 5.5, 180, 30);
  await Bun.sleep(1500);
  const shot = join(f.run, 'reformation.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY reformation', JSON.stringify(log));
  f.report('passed');

} catch (error) {
  console.log('THEURGY reformation', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
