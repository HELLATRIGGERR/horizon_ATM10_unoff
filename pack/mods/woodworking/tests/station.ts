/** Woodworking station: a stonecutter-like interface over an external station block.
 *
 * - Opening the station brings up a 2-slot screen (input=0, output=1) with a 4x3 recipe grid.
 * - Placing an oak log in the input slot populates the grid with oak-family recipes.
 * - Clicking the first recipe cell selects and executes a recipe: one log is consumed and the
 *   output slot fills; taking the output moves it to the inventory.
 * - Needs the staged native build; `pack/scripts/woodworking.ts world` sets WOODWORKING_STATION env var. */
import assert from 'node:assert/strict';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

/** The station block id is passed by the build script; default is for local runs only. */
const stationBlock: string = process.env['WOODWORKING_STATION'] ?? (() => {
  throw new Error('WOODWORKING_STATION env var is not set; run via pack/scripts/woodworking.ts world');
})();

/** The carry of the station block's mod, so the game registers the block. */
const carry: string = process.env['WOODWORKING_CARRY'] ?? (() => {
  throw new Error('WOODWORKING_CARRY env var is not set; run via pack/scripts/woodworking.ts world');
})();

const fixture = new WorldFixture('woodworking-station', 'server-thread', { devMods: [carry] });
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);
const ownSlots = (view: any) => (view.slots as any[] | undefined)
  ?.filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300) ?? [];

try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 2, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 2} ${x + 4} ${y + 4} ${z + 2} minecraft:air`);
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 2} ${x + 4} ${y - 1} ${z + 2} minecraft:stone`);

  const pos = `${x} ${y} ${z}`;
  await fixture.command(`setblock ${pos} ${stationBlock}`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === stationBlock, `${stationBlock} loads on the client`);
  await fixture.raw(`interact ${pos} south`);

  const opened = await until(() => fixture.raw('screen'), v => v.screen === 'container', 'woodworking screen opens');
  assert.deepEqual(ownSlots(opened), [0, 1], 'only input (0) and output (1) own slots');
  fixture.mark('woodworking station screen opens with 2 own slots');

  await fixture.replace('hotbar.0', 'minecraft:oak_log', 4);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:oak_log', 'oak log on cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried && !!slot(v, 300)?.item, 'oak log in input slot');
  fixture.mark('oak log placed in input slot');

  // Take a screenshot of the station screen with a log in the input slot.
  const screenshotPath = fixture.run + '/station-screen.png';
  await fixture.raw(`screenshot ${screenshotPath}`);
  fixture.mark(`screenshot saved: ${screenshotPath}`);

  // Recipe cells are drawn by native_screen_draw, not by slots; click-at hits native_screen_click.
  // Screen origin at (152,52) in the 480x270 window; GRID_X=52, GRID_Y=14; CELL=18 → first cell centre (213,75).
  await fixture.raw('click-at 213 75');
  const afterSelect = await until(() => fixture.raw('screen'), v => !!slot(v, 301)?.item, 'output slot fills', 10000);
  assert(slot(afterSelect, 301)?.item, 'output slot is not empty after recipe selection');
  fixture.mark(`station produced: ${JSON.stringify(slot(afterSelect, 301))}`);
  const input = slot(afterSelect, 300);
  assert.equal(input?.item, 'minecraft:oak_log', 'the input slot still holds oak logs');
  assert.equal(input?.count, 3, 'one of the four logs was consumed');
  fixture.mark('one oak log consumed');

  await fixture.raw('click-slot 301');
  await until(() => fixture.raw('screen'), v => !!v.carried, 'output on cursor');
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => !v.carried, 'output in inventory');
  fixture.mark('output moved to inventory');

  await fixture.raw('close');
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
