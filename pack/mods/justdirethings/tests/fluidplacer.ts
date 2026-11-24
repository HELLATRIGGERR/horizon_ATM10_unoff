/** Just Dire Things' Fluid Placer T1, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar
 * (the machine on the furnace template) make it.
 *
 * - A water bucket in its slot: the tank takes a bucket of water and the bucket comes back empty.
 * - On its operation clock it places the water as a source block in front of it and the tank is empty again.
 * - A lava bucket swapped into the slot fills the tank with lava; with a stone in front it places nothing, and
 *   when the stone goes it places the lava.
 * - A picture of its screen goes to the run folder (`fluidplacer.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-fluidplacer', 'server-thread', { devMods: [content] });
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
  await fixture.command(`fill ${x - 3} ${y} ${z - 6} ${x + 3} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 6} ${x + 3} ${y - 1} ${z + 4} minecraft:stone`);
  await fixture.command('gamemode survival @s');
  await fixture.command('gamerule spawn_mobs false');
  const pos = `${x} ${y} ${z}`, front = `${x} ${y} ${z - 1}`;
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;
  const open = async (id: string) => {
    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === id, 'the machine reaches the client');
  };
  const screen = async () => {
    await fixture.raw(`interact ${pos} south`);
    return until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the machine screen opens');
  };
  const put = async (item: string, count = 1) => {
    await fixture.replace('hotbar.0', item, count);
    await fixture.raw('click-slot 0');
    await until(() => fixture.raw('screen'), v => v.carried?.item === item, `${item} on the cursor`);
    await fixture.raw('click-slot 300');
    await until(() => fixture.raw('screen'), v => !v.carried || v.carried.item !== item, `${item} in the slot`);
  };
  const shoot = async (file: string) => {
    await Bun.sleep(1000);
    const shot = join(fixture.run, file);
    await fixture.raw(`screenshot ${shot}`);
    log[file] = shot;
  };

  await fixture.command(`setblock ${pos} justdirethings:fluidplacert1[facing=north]`);
  await open('justdirethings:fluidplacert1');
  let view = await screen();
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `one slot of its own: ${JSON.stringify(view.slots)}`);
  await fixture.command(`setblock ${front} minecraft:stone`);
  await put('minecraft:water_bucket');
  let nbt = await until(() => saved(pos), s => tank(s).amount === 1000, 'a bucket of water in the tank');
  assert.equal(tank(nbt).id, 'minecraft:water');
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'minecraft:bucket', 'the bucket comes back empty');
  assert(await is(front, 'minecraft:stone'), 'the stone in front stays');
  fixture.mark('a water bucket in the slot fills the tank and comes back empty; a stone in front is left alone');
  await fixture.command(`setblock ${front} minecraft:air`);
  await until(() => is(front, 'minecraft:water[level=0]'), v => v, 'the placer places a water source in front', 20000);
  await until(() => saved(pos), s => tank(s).amount === 0, 'the tank is empty again');
  fixture.mark('on its clock it places the water as a source block in front of it: the tank is empty again');

  await fixture.command(`setblock ${front} minecraft:stone`);
  await fixture.replace('hotbar.0', 'minecraft:lava_bucket', 1);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:lava_bucket', 'the lava bucket on the cursor');
  await fixture.raw('click-slot 300');
  nbt = await until(() => saved(pos), s => tank(s).amount === 1000, 'a bucket of lava in the tank');
  assert.equal(tank(nbt).id, 'minecraft:lava');
  await Bun.sleep(2500);
  assert(await is(front, 'minecraft:stone'), 'the stone is still there');
  await shoot('fluidplacer.png');
  await fixture.command(`setblock ${front} minecraft:air`);
  await until(() => is(front, 'minecraft:lava[level=0]'), v => v, 'the lava goes down when the stone goes', 20000);
  fixture.mark('swapped for a lava bucket, it fills with lava and places it once the spot is free');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS fluidplacer', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS fluidplacer', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
