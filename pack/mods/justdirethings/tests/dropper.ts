/** Just Dire Things' Dropper T1, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar
 * (the machine on the furnace template) make it.
 *
 * - Its screen opens on a click with one slot (the hopper menu's four others hidden); three cobblestone go in.
 * - The drop count button (left click one more) set to 2: on its operation clock (every 20 ticks) it drops two cobblestone
 *   as one item entity in front of it, thrown down by default (it stays in the block), and one stays in the slot.
 * - The direction button clicked twice (down, up, north): the next drop is thrown north, out of that block.
 * - The tick speed button: a left click 21 ticks, a right click back to 20.
 * - A picture of its screen goes to the run folder (`dropper.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-dropper', 'server-thread', { devMods: [content] });
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

  const column = `@e[type=minecraft:item,x=${x},y=${y - 1},z=${z - 1},dx=1,dy=3,dz=1]`;
  const beyond = `@e[type=minecraft:item,x=${x - 1},y=${y - 1},z=${z - 6},dx=3,dy=4,dz=4.8]`;
  const exists = async (selector: string) => passed(await fixture.command(`execute if entity ${selector}`));
  await fixture.command(`setblock ${pos} justdirethings:droppert1[facing=north]`);
  await open('justdirethings:droppert1');
  let view = await screen();
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `one slot of its own: ${JSON.stringify(view.slots)}`);
  let nbt = await saved(pos);
  assert.equal(field(nbt, 'dropCount'), 1);
  assert.equal(field(nbt, 'direction'), 0, 'down by default');
  const count = at(50 + 12, 61 + 6);
  await fixture.raw(count);
  await until(() => saved(pos), s => field(s, 'dropCount') === 2, 'the count button sets 2');
  await put('minecraft:cobblestone', 3);
  fixture.mark('the screen shows one slot; three cobblestone go in, the drop count is 2');

  await until(() => exists(column), v => v, 'the dropper drops cobblestone in front of it', 20000);
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.count === 1, 'two cobblestone left the slot');
  assert(passed(await fixture.command(`execute if entity @e[type=minecraft:item,nbt={Item:{id:"minecraft:cobblestone",count:2}}]`)), 'one item entity of two');
  fixture.mark('on its clock it drops two cobblestone as one item entity in front of it, thrown down: one stays in the slot');
  await fixture.command('kill @e[type=minecraft:item]');

  const direction = at(122 + 8, 58 + 8);
  await fixture.raw(direction);
  await until(() => saved(pos), s => field(s, 'direction') === 1, 'the button sets up');
  await Bun.sleep(1000);
  await fixture.raw(direction);
  await until(() => saved(pos), s => field(s, 'direction') === 2, 'the button sets north');
  await Bun.sleep(1000);
  await shoot('dropper.png');
  await until(() => fixture.raw('screen'), v => !slot(v, 300)?.item, 'the last cobblestone is dropped');
  await fixture.command('kill @e[type=minecraft:item]');
  await put('minecraft:cobblestone', 3);
  await until(() => exists(beyond), v => v, 'thrown north, the drop lands out of the block in front', 20000);
  fixture.mark('set to north, the next drop is thrown out of the block in front of it');

  const ticks = at(144 + 12, 60 + 6);
  await fixture.raw(ticks);
  await until(() => saved(pos), s => field(s, 'tickspeed') === 21, 'a left click: 21 ticks');
  await Bun.sleep(1000);
  await fixture.raw(`${ticks} right`);
  await until(() => saved(pos), s => field(s, 'tickspeed') === 20, 'a right click: back to 20');
  fixture.mark('the tick speed button: left one more, right one less');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS dropper', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS dropper', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
