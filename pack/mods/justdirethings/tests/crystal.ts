/** Just Dire Things' time crystal, as the `justdirethings` native bundle and the carry of the player's jar make it.
 *
 * - A living tier 4 goo turns the budding amethyst on it into the time crystal's budding block (the goo recipe
 *   `budding_time_amethyst`, 4,800 ticks at the tier's 10 a tick), which a random tick in the Overworld moves to stage
 *   1, and no further there; it grows nothing below stage 3, and none of budding amethyst's buds.
 * - In the Nether a stage 1 block moves to 2 (a stage 0 one stays), in the End a stage 2 block moves to 3.
 * - A stage 3 block grows time crystal clusters facing away from it, small, then medium (a 5% roll after each growth
 *   sends it back to stage 0; the test places a fresh one then).
 * - A picture of the clusters goes to the run folder (`crystal.png`).
 *
 * Random ticks run at `randomTickSpeed` 1024. Needs the staged native build (`AE2_TEST_BINARY`) and the content mod
 * (`JUSTDIRETHINGS_CONTENT_DIR`); `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-crystal', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
const SIDES: [string, number, number, number][] = [
  ['down', 0, -1, 0], ['up', 0, 1, 0], ['north', 0, 0, -1], ['south', 0, 0, 1], ['west', -1, 0, 0], ['east', 1, 0, 0],
];
/** The sides of `[x, y, z]` holding a cluster of `size` facing away from it. */
async function clusters(at: [number, number, number], size: string): Promise<string[]> {
  const found: string[] = [];
  for (const [side, dx, dy, dz] of SIDES) {
    if (await is(`${at[0] + dx} ${at[1] + dy} ${at[2] + dz}`, `justdirethings:time_crystal_cluster${size}[facing=${side}]`)) found.push(side);
  }
  return found;
}
const BUDDING = 'justdirethings:time_crystal_budding_block';

/** A platform in `dimension` around `[x, y, z]` with the player on it, loaded. */
async function visit(dimension: string, x: number, y: number, z: number): Promise<void> {
  await fixture.command(`execute in ${dimension} run tp @s ${x + 0.5} ${y} ${z + 0.5}`);
  await until(async () => passed(await fixture.command(`execute in ${dimension} if loaded ${x} ${y} ${z}`)), v => v, `${dimension} loaded`, 60000);
  await fixture.command(`execute in ${dimension} run fill ${x - 4} ${y} ${z - 4} ${x + 4} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`execute in ${dimension} run fill ${x - 4} ${y - 1} ${z - 4} ${x + 4} ${y - 1} ${z + 4} minecraft:obsidian`);
  await fixture.command(`execute in ${dimension} run tp @s ${x + 0.5} ${y} ${z + 0.5}`);
}
const isIn = async (dimension: string, pos: string, state: string) => passed(await fixture.command(`execute in ${dimension} if block ${pos} ${state}`));

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 4;
  await fixture.command(`fill ${x - 3} ${y} ${z - 3} ${x + 10} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 3} ${x + 10} ${y - 1} ${z + 3} minecraft:stone`);
  await fixture.command('gamerule advance_time false');
  await fixture.command('gamerule random_tick_speed 1024');
  await fixture.command(`tp @s ${x + 3.5} ${y + 1} ${z + 4.5} 180 30`);

  // A tier 4 goo with budding amethyst on it.
  const top = `${x} ${y + 1} ${z}`;
  await fixture.command(`setblock ${top} minecraft:budding_amethyst`);
  await fixture.command(`setblock ${x} ${y} ${z} justdirethings:gooblock_tier4[alive=true]`);
  await until(() => is(top, BUDDING), v => v, 'the goo makes the budding block', 120000);
  await until(() => is(top, `${BUDDING}[stage=1]`), v => v, 'the Overworld moves it to stage 1', 30000);
  await Bun.sleep(3000);
  assert(await is(top, `${BUDDING}[stage=1]`), 'stage 1 stays stage 1 in the Overworld');
  for (const size of ['_small', '_medium', '_large', '']) {
    assert.deepEqual(await clusters([x, y + 1, z], size), [], `no time crystal cluster${size} below stage 3`);
  }
  fixture.mark('a tier 4 goo turns budding amethyst into the time crystal budding block, which moves to stage 1 in the Overworld and grows nothing');

  // A stage 3 block grows clusters: small, then medium on the same side.
  const grower: [number, number, number] = [x + 6, y + 1, z];
  const G = grower.join(' ');
  let medium: string[] = [];
  let small: string[] = [];
  for (let attempt = 0; attempt < 4 && medium.length === 0; attempt++) {
    await fixture.command(`fill ${x + 5} ${y} ${z - 1} ${x + 7} ${y + 2} ${z + 1} minecraft:air`);
    await fixture.command(`setblock ${G} ${BUDDING}[stage=3]`);
    small = await until(() => clusters(grower, '_small'), v => v.length > 0, 'a small cluster grows', 60000);
    const end = Date.now() + 60000;
    while (Date.now() < end && medium.length === 0) {
      medium = await clusters(grower, '_medium');
      if (medium.length === 0 && await is(G, `${BUDDING}[stage=1]`)) {
        log[`depleted${attempt}`] = true;
        break;
      }
      await Bun.sleep(500);
    }
  }
  log.small = small;
  log.medium = medium;
  assert(medium.length > 0, 'a small cluster grows into a medium one');
  fixture.mark(`a stage 3 budding block grows time crystal clusters facing away from it: small (${small}), then medium (${medium})`);
  await fixture.command('gamerule random_tick_speed 0');
  await fixture.command(`tp @s ${x + 6.5} ${y + 1} ${z + 3.5} 180 20`);
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'crystal.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.command('gamerule random_tick_speed 1024');

  // The Nether: stage 1 to 2, a stage 0 block stays.
  const nether = 'minecraft:the_nether';
  await visit(nether, 8, 100, 8);
  await fixture.command(`execute in ${nether} run setblock 10 100 8 ${BUDDING}[stage=1]`);
  await fixture.command(`execute in ${nether} run setblock 6 100 8 ${BUDDING}[stage=0]`);
  await until(() => isIn(nether, '10 100 8', `${BUDDING}[stage=2]`), v => v, 'the Nether moves stage 1 to 2', 60000);
  assert(await isIn(nether, '6 100 8', `${BUDDING}[stage=0]`), 'a stage 0 block stays in the Nether');
  fixture.mark('in the Nether a stage 1 budding block moves to stage 2; a stage 0 one stays');

  // The End: stage 2 to 3.
  const end = 'minecraft:the_end';
  await visit(end, 120, 80, 8);
  await fixture.command(`execute in ${end} run setblock 122 80 8 ${BUDDING}[stage=2]`);
  await until(() => isIn(end, '122 80 8', `${BUDDING}[stage=3]`), v => v, 'the End moves stage 2 to 3', 60000);
  fixture.mark('in the End a stage 2 budding block moves to stage 3');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS crystal', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS crystal', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
