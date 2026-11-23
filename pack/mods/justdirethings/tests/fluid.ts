/** Just Dire Things' fluid drop and time fluid, as the `justdirethings` native bundle and the carry of the player's jar
 * make them (the recipes `justdirethings:fluiddrop` of the world's tables, the carry's fluids).
 *
 * - Two polymorphic catalysts dropped into a water source turn it into polymorphic fluid, using one up; the other
 *   stays an item.
 * - A time crystal dropped into the polymorphic fluid turns it into time fluid and is used up; one dropped into plain
 *   water makes nothing and stays.
 * - Time fluid flows a level less a block where the basin opens, and an empty bucket takes the source up as a time
 *   fluid bucket.
 * - A picture of the basins goes to the run folder (`fluid.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-fluid', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
const items = async (id: string, count?: number) =>
  passed(await fixture.command(`execute if entity @e[type=minecraft:item,nbt={Item:{id:"${id}"${count ? `,count:${count}` : ''}}}]`));
const drop = (at: [number, number, number], id: string, count: number) =>
  fixture.command(`summon minecraft:item ${at[0] + 0.5} ${at[1] + 0.3} ${at[2] + 0.5} {Item:{id:"${id}",count:${count}},PickupDelay:32767}`);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 4;
  await fixture.command(`fill ${x - 3} ${y} ${z - 3} ${x + 8} ${y + 3} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 3} ${x + 8} ${y - 1} ${z + 3} minecraft:stone`);
  // Two one-block basins of water, walled with stone.
  const a: [number, number, number] = [x, y, z], b: [number, number, number] = [x + 4, y, z];
  for (const [bx, by, bz] of [a, b]) {
    await fixture.command(`fill ${bx - 1} ${by} ${bz - 1} ${bx + 1} ${by} ${bz + 1} minecraft:stone`);
    await fixture.command(`setblock ${bx} ${by} ${bz} minecraft:water`);
  }
  const A = a.join(' '), B = b.join(' ');
  await fixture.command(`tp @s ${x + 2.5} ${y + 1} ${z + 4.5} 180 40`);

  await drop(a, 'justdirethings:polymorphic_catalyst', 2);
  await until(() => is(A, 'justdirethings:polymorphic_fluid_block[level=0]'), v => v, 'the catalyst turns the water into polymorphic fluid', 20000);
  await until(() => items('justdirethings:polymorphic_catalyst', 1), v => v, 'one catalyst is used up, one stays');
  await fixture.command('kill @e[type=minecraft:item]');
  fixture.mark('polymorphic catalysts in a water source make polymorphic fluid, one item used up');

  await drop(b, 'justdirethings:time_crystal', 1);
  await drop(a, 'justdirethings:time_crystal', 1);
  await until(() => is(A, 'justdirethings:time_fluid_block[level=0]'), v => v, 'a time crystal turns polymorphic fluid into time fluid', 20000);
  await Bun.sleep(2000);
  assert(await is(B, 'minecraft:water[level=0]'), 'a time crystal in plain water makes nothing');
  assert(await items('justdirethings:time_crystal', 1), 'the crystal in the water is still an item');
  await fixture.command(`kill @e[type=minecraft:item,distance=..30]`);
  assert(!(await items('justdirethings:time_crystal')), 'no crystal left');
  fixture.mark('a time crystal in polymorphic fluid makes time fluid and is used up; in water it makes nothing');

  // Open the basin to the east: the time fluid flows a level less a block, and stays one source.
  await fixture.command(`setblock ${x + 1} ${y} ${z} minecraft:air`);
  await until(() => is(`${x + 1} ${y} ${z}`, 'justdirethings:time_fluid_block[level=1]'), v => v, 'time fluid flows out', 20000);
  await until(() => is(`${x + 2} ${y} ${z}`, 'justdirethings:time_fluid_block[level=2]'), v => v, 'a level less a block', 20000);
  assert(await is(A, 'justdirethings:time_fluid_block[level=0]'), 'the source stays');
  fixture.mark('time fluid flows a level less a block from its source');

  const shot = join(fixture.run, 'fluid.png');
  await Bun.sleep(1000);
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;

  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.0', 'minecraft:bucket', 1);
  await fixture.select(0, 'minecraft:bucket');
  await fixture.command(`tp @s ${x + 0.5} ${y + 1} ${z + 2.5}`);
  await fixture.raw(`look-at ${x + 0.5} ${y + 0.5} ${z + 0.5}`);
  await fixture.raw('use-item');
  await until(() => fixture.raw('inventory'), v => v.hotbar?.[0]?.item === 'justdirethings:time_fluid_bucket', 'the empty bucket takes the time fluid up');
  fixture.mark('an empty bucket takes the time fluid source up as a time fluid bucket');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS fluid', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS fluid', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
