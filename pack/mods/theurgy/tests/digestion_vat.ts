/** Theurgy's digestion vat: sal ammoniac and a common animal niter, lid closed, four abundant out. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, isBlock, handCount, hand, saved, slotCount, slotItem, tank, theurgyFixture, until, stand } from './support/common';

const f = theurgyFixture('theurgy-digestion-vat');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const vat = at(0, 0, 0);
  const isOpen = () => isBlock(f, vat, `theurgy:digestion_vat[open=true]`);
  await f.command(`setblock ${vat} theurgy:digestion_vat[open=true]`);
  await stand(f, x + 0.5, y + 1, z + 2.5, 180, 25);

  await hand(f, 'theurgy:sal_ammoniac_bucket', 1);
  await f.raw(`interact ${vat} up`);
  await until(() => saved(f, vat), s => tank(s, 'fluidTank').fluid === 'theurgy:sal_ammoniac', 'tank fills', 10_000);
  f.mark('the bucket fills the vat');

  for (const [item, count] of [["theurgy:alchemical_niter_animals_common",1]] as [string, number][]) {
    await hand(f, item, count);
    await f.raw(`interact ${vat} up`);
    await until(() => handCount(f), h => h.count === 0, `${item} taken`, 5_000);
  }
  f.mark('the inputs are clicked in');

  // An empty hand, sneaking, closes the lid once a recipe can craft.
  await hand(f);
  await f.sneakInteract(`${vat} up`);
  await until(() => isOpen(), open => !open, 'the lid closes', 5_000);
  f.mark('the lid is closed');

  const result = await until(() => saved(f, vat), s => slotItem(s, 3) === 'theurgy:alchemical_niter_animals_abundant', 'the vat crafts', 120_000);
  assert.equal(slotCount(result, 3), 4);
  log.output = slotItem(result, 3);
  log.tankAfter = tank(result, 'fluidTank').amount;
  await until(() => isOpen(), open => open, 'the lid opens again', 10_000);
  f.mark('crafted theurgy:alchemical_niter_animals_abundant; the lid opens again');

  await stand(f, x + 0.5, y + 1, z + 3.5, 180, 25);
  await Bun.sleep(1500);
  const shot = join(f.run, 'digestion-vat.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY digestion-vat', JSON.stringify(log));
  f.report('passed');

} catch (error) {
  console.log('THEURGY digestion-vat', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
