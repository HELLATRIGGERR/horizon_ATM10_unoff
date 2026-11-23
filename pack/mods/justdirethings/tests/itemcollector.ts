/** Just Dire Things' Item Collector, as the `justdirethings` native bundle and the content mod made from the carry of
 * the player's jar (the collector on the furnace template) make it.
 *
 * - Facing a chest, on its operation clock (every 20 ticks) it takes the items lying in its area (the block behind it)
 *   into the chest: three cobblestone go in and their item entity is gone; an item a block further away stays.
 * - Its screen opens on a click with nine filter slots; a dirt put in the first and the allowlist button pressed, it
 *   takes dirt and leaves cobblestone.
 * - The radius button on X widens the area to the item further away.
 * - A picture of its screen goes to the run folder (`itemcollector.png`).
 *
 * Reads the saved compounds (`data get block`). Needs the staged native build (`AE2_TEST_BINARY`) and the content mod
 * (`JUSTDIRETHINGS_CONTENT_DIR`); `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-itemcollector', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => Number(nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?[\\d.]+)`))?.[1] ?? NaN);
/** How many of `item` the chest's saved `Items` hold. */
const inChest = (nbt: string, item: string) => {
  let n = 0;
  for (const m of nbt.matchAll(/\{[^{}]*\}/g)) {
    if (m[0].includes(`"${item}"`)) n += Number(m[0].match(/count: ?(\d+)/)?.[1] ?? 1);
  }
  return n;
};
const items = async (id: string) =>
  passed(await fixture.command(`execute if entity @e[type=minecraft:item,nbt={Item:{id:"${id}"}}]`));
const drop = (at: [number, number, number], id: string, count: number) =>
  fixture.command(`summon minecraft:item ${at[0] + 0.5} ${at[1] + 0.1} ${at[2] + 0.5} {Item:{id:"${id}",count:${count}},PickupDelay:32767}`);
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 3;
  await fixture.command(`fill ${x - 3} ${y} ${z - 3} ${x + 3} ${y + 3} ${z + 5} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 3} ${x + 3} ${y - 1} ${z + 5} minecraft:stone`);
  await fixture.command('gamerule spawn_mobs false');
  const pos = `${x} ${y} ${z}`, chest = `${x} ${y} ${z - 1}`;
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;

  await fixture.command(`setblock ${chest} minecraft:chest[facing=south]`);
  await fixture.command(`setblock ${pos} justdirethings:itemcollector[facing=north]`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:itemcollector', 'the collector reaches the client');
  let nbt = await until(() => saved(pos), s => s.includes('xRadiusDouble'), 'the collector has ticked');
  assert.equal(field(nbt, 'zOffset'), 1, 'the area is the block behind it (south)');

  // Three cobblestone behind it, one two blocks behind.
  const behind: [number, number, number] = [x, y, z + 1], far: [number, number, number] = [x + 1, y, z + 1];
  await drop(behind, 'minecraft:cobblestone', 3);
  await drop(far, 'minecraft:andesite', 1);
  await until(async () => inChest(await saved(chest), 'minecraft:cobblestone'), n => n === 3, 'three cobblestone in the chest', 20000);
  assert(!(await items('minecraft:cobblestone')), 'the cobblestone item entity is gone');
  await Bun.sleep(2500);
  assert(await items('minecraft:andesite'), 'the andesite outside the area stays');
  fixture.mark('facing a chest, it takes the items behind it into the chest; one outside its area stays');

  // The screen: nine filter slots; dirt in the first, then the allowlist.
  await fixture.replace('hotbar.0', 'minecraft:dirt', 1);
  await fixture.raw(`interact ${pos} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the collector screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5, 6, 7, 8], `nine filter slots: ${JSON.stringify(view.slots)}`);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:dirt', 'the dirt on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'minecraft:dirt', 'the dirt in the first filter slot');
  await fixture.raw(at(8 + 8, 82 + 8));
  nbt = await until(() => saved(pos), s => field(s, 'allowlist') === 1, 'the allowlist button');
  await Bun.sleep(1000);
  const shot = join(fixture.run, 'itemcollector.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('key escape tap');
  await until(() => fixture.raw('screen'), v => v.screen !== 'container', 'the screen closes');

  await drop(behind, 'minecraft:cobblestone', 2);
  await drop(behind, 'minecraft:dirt', 4);
  await until(async () => inChest(await saved(chest), 'minecraft:dirt'), n => n === 4, 'the dirt goes in', 20000);
  await Bun.sleep(2500);
  assert(await items('minecraft:cobblestone'), 'the cobblestone stays: it is not on the allowlist');
  assert.equal(inChest(await saved(chest), 'minecraft:cobblestone'), 3);
  fixture.mark('with dirt in its filter and the allowlist on, it takes dirt and leaves cobblestone');

  // The X radius's plus, twice: one block either side; the andesite is in the area now (and on no allowlist, so the
  // denylist goes back first).
  await fixture.raw(`interact ${pos} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the screen opens again');
  await fixture.raw(at(8 + 8, 82 + 8));
  await until(() => saved(pos), s => field(s, 'allowlist') === 0, 'back to a denylist');
  await Bun.sleep(500);
  await fixture.raw(at(25 + 30 + 6, 32 + 6));
  await until(() => saved(pos), s => field(s, 'xRadiusDouble') === 0.5, 'radius X 0.5');
  await Bun.sleep(500);
  await fixture.raw(at(25 + 30 + 6, 32 + 6));
  await until(() => saved(pos), s => field(s, 'xRadiusDouble') === 1, 'radius X 1.0');
  await fixture.raw('key escape tap');
  await until(async () => inChest(await saved(chest), 'minecraft:andesite'), n => n === 1, 'the andesite goes in', 20000);
  fixture.mark('the X radius button widens its area: the andesite a block aside goes in (the dirt in the filter now denies dirt only)');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS itemcollector', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS itemcollector', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
