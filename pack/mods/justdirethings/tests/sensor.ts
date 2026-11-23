/** Just Dire Things' Sensor T1, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar
 * (the machine on the furnace template) make it.
 *
 * - Its screen opens on a click with one filter slot; its target is blocks and its signal weak.
 * - A stone in front of it: within its 20 tick clock a lamp above it lights; the stone gone, the lamp goes out.
 * - A stone beside it with a lamp behind that stone: a weak signal does not light that lamp; the strong button does.
 * - The target button once (air): with the front free the lamp lights. Seven more clicks (items): an item entity in front
 *   of it lights the lamp, and it goes out when the item is gone.
 * - A picture of its screen goes to the run folder (`sensor.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-sensor', 'server-thread', { devMods: [content] });
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

  const lamp = `${x} ${y + 1} ${z}`;
  const farLamp = `${x + 2} ${y} ${z}`;
  await fixture.command(`setblock ${pos} justdirethings:sensort1[facing=north]`);
  await fixture.command(`setblock ${lamp} minecraft:redstone_lamp`);
  await open('justdirethings:sensort1');
  const lit = (p: string, on: boolean) => is(p, `minecraft:redstone_lamp[lit=${on}]`);
  let view = await screen();
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `one slot of its own: ${JSON.stringify(view.slots)}`);
  let nbt = await saved(pos);
  assert.equal(field(nbt, 'senseTarget'), 0, 'blocks by default');
  assert.equal(field(nbt, 'strongSignal'), 0, 'weak by default');
  await Bun.sleep(2500);
  assert(await lit(lamp, false), 'nothing in front: the lamp is dark');

  await fixture.command(`setblock ${front} minecraft:stone`);
  await until(() => lit(lamp, true), v => v, 'a block in front lights the lamp above', 20000);
  await fixture.command(`setblock ${front} minecraft:air`);
  await until(() => lit(lamp, false), v => v, 'the lamp goes out with the block', 20000);
  fixture.mark('target blocks: a stone in front lights the lamp above it, and it goes out when the stone goes');

  await fixture.command(`setblock ${lamp} minecraft:air`);
  await fixture.command(`setblock ${x + 1} ${y} ${z} minecraft:stone`);
  await fixture.command(`setblock ${farLamp} minecraft:redstone_lamp`);
  await fixture.command(`setblock ${front} minecraft:stone`);
  await Bun.sleep(3500);
  assert(await lit(farLamp, false), 'a weak signal does not light the lamp behind a stone');
  await fixture.raw(at(20 + 8, 58 + 8));
  await until(() => saved(pos), s => field(s, 'strongSignal') === 1, 'the strong button');
  await until(() => lit(farLamp, true), v => v, 'a strong signal lights the lamp behind the stone', 20000);
  await fixture.raw(at(20 + 8, 58 + 8));
  await until(() => saved(pos), s => field(s, 'strongSignal') === 0, 'weak again');
  await until(() => lit(farLamp, false), v => v, 'weak again, the lamp goes out', 20000);
  await fixture.command(`setblock ${x + 1} ${y} ${z} minecraft:air`);
  await fixture.command(`setblock ${farLamp} minecraft:air`);
  await fixture.command(`setblock ${front} minecraft:air`);
  await Bun.sleep(2000); // the sensor's next sense: it stops emitting
  await fixture.command(`setblock ${lamp} minecraft:redstone_lamp`);
  fixture.mark('the strong signal reaches the lamp through the stone; the weak one does not');

  const target = at(56 + 8, 58 + 8);
  await fixture.raw(target);
  await until(() => saved(pos), s => field(s, 'senseTarget') === 1, 'target air');
  await until(() => lit(lamp, true), v => v, 'air in front lights the lamp', 20000);
  for (let i = 2; i <= 8; i++) {
    await fixture.raw(target);
    await until(() => saved(pos), s => field(s, 'senseTarget') === i, `target ${i}`);
  }
  await until(() => lit(lamp, false), v => v, 'no item in front: dark', 20000);
  await fixture.command(`summon minecraft:item ${x + 0.5} ${y + 0.1} ${z - 0.5} {Item:{id:"minecraft:cobblestone",count:1},PickupDelay:32767}`);
  await until(() => lit(lamp, true), v => v, 'an item entity in front lights the lamp', 20000);
  await shoot('sensor.png');
  await fixture.command('kill @e[type=minecraft:item]');
  await until(() => lit(lamp, false), v => v, 'the item gone, the lamp goes out', 20000);
  fixture.mark('target air, then items: an item entity in front lights the lamp');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS sensor', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS sensor', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
