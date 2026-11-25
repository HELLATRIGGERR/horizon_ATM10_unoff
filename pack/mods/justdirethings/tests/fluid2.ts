/** Just Dire Things' Fluid Placer T2 and Fluid Collector T2, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar (on the furnace template) make them.
 *
 * - The Placer T2's screen opens with its slot and nine filter slots. With water in its tank (a `data merge`) and energy it
 *   places a source in front of it each run, 500 FE a bucket; with its X radius 1 it goes on to the blocks beside.
 * - The Collector T2 with energy takes the three lava sources of its widened area into its tank, one a run, 500 FE each.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-fluid2', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => Number(nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?\\d+)`))?.[1] ?? NaN);
const tank = (nbt: string) => {
  const at = nbt.indexOf('Fluid:');
  if (at < 0) return { amount: 0, id: '' };
  const part = nbt.slice(at, at + 120);
  return { amount: Number(part.match(/amount: ?(\d+)/)?.[1] ?? 0), id: part.match(/id: ?"([^"]+)"/)?.[1] ?? '' };
};
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 3} ${y} ${z - 6} ${x + 9} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 6} ${x + 9} ${y - 1} ${z + 4} minecraft:stone`);
  await fixture.command('gamemode survival @s');
  await fixture.command('gamerule spawn_mobs false');
  const pos = `${x} ${y} ${z}`, front = `${x} ${y} ${z - 1}`;
  const open = async (id: string) => {
    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === id, 'the machine reaches the client');
  };
  const screen = async () => {
    await fixture.raw(`interact ${pos} south`);
    return until(() => fixture.raw('screen'), v => v.screen === 'container' && !!(slot(v, 300) || slot(v, 301)), 'the machine screen opens');
  };
  const put = async (item: string, count: number, n: number) => {
    await fixture.replace('hotbar.0', item, count);
    await fixture.raw('click-slot 0');
    await until(() => fixture.raw('screen'), v => v.carried?.item === item, `${item} on the cursor`);
    await fixture.raw(`click-slot ${300 + n}`);
    await until(() => fixture.raw('screen'), v => !v.carried || v.carried.item !== item, `${item} in the slot`);
  };
  const shoot = async (file: string) => {
    await Bun.sleep(1000);
    const shot = join(fixture.run, file);
    await fixture.raw(`screenshot ${shot}`);
    log[file] = shot;
  };
  const own = (view: any) => (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);

  await fixture.command(`setblock ${pos} justdirethings:fluidplacert2[facing=north]`);
  await open('justdirethings:fluidplacert2');
  const view = await screen();
  assert.deepEqual(own(view), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], `its slot and nine filter slots: ${JSON.stringify(view.slots)}`);
  await shoot('fluidplacer2.png');
  await fixture.raw('key escape tap');
  await fixture.command(`data merge block ${pos} {energy:100000,Fluid:{id:"minecraft:water",amount:2000}}`);
  await until(() => is(front, 'minecraft:water'), v => v, 'with energy it places the water in front', 30000);
  let nbt = await saved(pos);
  assert.equal(tank(nbt).amount, 1000, 'one bucket left');
  assert(field(nbt, 'energy') <= 99500, `a bucket takes 500 FE: ${field(nbt, 'energy')}`);
  fixture.mark('the Fluid Placer T2 with water and energy places a source in front: 500 FE a bucket');
  await fixture.command(`data merge block ${pos} {xRadiusDouble:1.0d}`);
  await until(async () => (await is(`${x - 1} ${y} ${z - 1}`, 'minecraft:water')) || (await is(`${x + 1} ${y} ${z - 1}`, 'minecraft:water')), v => v, 'its area made three wide, it places beside', 30000);
  fixture.mark('its area made three wide, it places the next bucket beside the first');

  const x2 = x + 5, pos2 = `${x2} ${y} ${z}`;
  await fixture.command(`fill ${x2 - 1} ${y} ${z - 1} ${x2 + 1} ${y} ${z - 1} minecraft:lava`);
  await fixture.command(`setblock ${pos2} justdirethings:fluidcollectort2[facing=north]`);
  await fixture.command(`data merge block ${pos2} {energy:100000,xRadiusDouble:1.0d,yRadiusDouble:0.0d,zRadiusDouble:0.0d,xOffset:0,yOffset:0,zOffset:-1,renderArea:0b,allowlist:0b,compareNBT:0b,blockitemfilter:-1}`);
  await until(async () => tank(await saved(pos2)).amount === 3000, v => v, 'the three lava sources are collected', 90000);
  nbt = await saved(pos2);
  assert.equal(tank(nbt).id, 'minecraft:lava');
  assert(field(nbt, 'energy') <= 98500, `three buckets take 1,500 FE: ${field(nbt, 'energy')}`);
  for (const dx of [-1, 0, 1]) assert(!(await is(`${x2 + dx} ${y} ${z - 1}`, 'minecraft:lava[level=0]')), 'the lava source is gone');
  fixture.mark(`the Fluid Collector T2 takes the three lava sources of its area into its tank (${field(nbt, 'energy')} FE left)`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS fluid2', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS fluid2', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
