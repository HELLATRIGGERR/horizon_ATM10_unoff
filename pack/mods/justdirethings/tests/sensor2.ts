/** Just Dire Things' Sensor T2, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar (on the furnace template) make it.
 *
 * - Its screen opens with its nine filter slots only.
 * - With no energy it senses nothing (a block in front leaves the lamp above it dark); with energy it lights the lamp, and it
 *   pays 2 FE a block of its area each sense.
 * - Dirt in its filter (a denylist): dirt in front is not counted, the lamp goes out.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-sensor2', 'server-thread', { devMods: [content] });
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

  const lamp = `${x} ${y + 1} ${z}`;
  const lit = (p: string, on: boolean) => is(p, `minecraft:redstone_lamp[lit=${on}]`);
  await fixture.command(`setblock ${pos} justdirethings:sensort2[facing=north]`);
  await fixture.command(`setblock ${lamp} minecraft:redstone_lamp`);
  await open('justdirethings:sensort2');
  const view = await screen();
  assert.deepEqual(own(view), [1, 2, 3, 4, 5, 6, 7, 8, 9], `nine filter slots: ${JSON.stringify(view.slots)}`);
  await fixture.command(`setblock ${front} minecraft:dirt`);
  await Bun.sleep(3000);
  assert(await lit(lamp, false), 'no energy: the lamp is dark');
  fixture.mark('the Sensor T2 shows nine filter slots; without energy it senses nothing');
  await fixture.command(`data merge block ${pos} {energy:1000}`);
  await until(() => lit(lamp, true), v => v, 'with energy a block in front lights the lamp', 30000);
  await Bun.sleep(2200);
  const left = field(await saved(pos), 'energy');
  log.energy = left;
  assert(left < 1000 && (1000 - left) % 2 === 0, `2 FE a sense: ${left}`);
  fixture.mark(`with energy it lights the lamp (${left} FE left)`);
  await put('minecraft:dirt', 1, 1);
  await until(() => lit(lamp, false), v => v, 'dirt in the denylist: the lamp goes out', 30000);
  await shoot('sensor2.png');
  fixture.mark('dirt in its filter is not counted: the lamp goes out');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS sensor2', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS sensor2', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
