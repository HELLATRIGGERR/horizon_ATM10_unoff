/** Theurgy's fermentation vat: water, a beef sulfur and a log, lid closed, one abundant animal niter out. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { arena, isBlock, handCount, hand, saved, slotCount, slotItem, tank, theurgyFixture, until, stand } from './support/common';

const f = theurgyFixture('theurgy-fermentation-vat');
const log: Record<string, unknown> = {};
try {
  const { x, y, z, at } = await arena(f);
  const vat = at(0, 0, 0);
  const isOpen = () => isBlock(f, vat, `theurgy:fermentation_vat[open=true]`);
  await f.command(`setblock ${vat} theurgy:fermentation_vat[open=true]`);
  await stand(f, x + 0.5, y + 1, z + 2.5, 180, 25);

  await hand(f, 'minecraft:water_bucket', 1);
  await f.raw(`interact ${vat} up`);
  await until(() => saved(f, vat), s => tank(s, 'fluidTank').fluid === 'minecraft:water', 'tank fills', 10_000);
  f.mark('the bucket fills the vat');

  for (const [item, count] of [["theurgy:alchemical_sulfur_beef",1],["minecraft:oak_log",1]] as [string, number][]) {
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
  assert.equal(slotCount(result, 3), 1);
  log.output = slotItem(result, 3);
  log.tankAfter = tank(result, 'fluidTank').amount;
  await until(() => isOpen(), open => open, 'the lid opens again', 10_000);
  f.mark('crafted theurgy:alchemical_niter_animals_abundant; the lid opens again');

  await stand(f, x + 0.5, y + 1, z + 3.5, 180, 25);
  await Bun.sleep(1500);
  const shot = join(f.run, 'fermentation-vat.png');
  await f.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  console.log('THEURGY fermentation-vat', JSON.stringify(log));
  f.report('passed');

} catch (error) {
  console.log('THEURGY fermentation-vat', JSON.stringify(log));
  f.report('failed', error);
  throw error;
} finally {
  await f.close();
}
