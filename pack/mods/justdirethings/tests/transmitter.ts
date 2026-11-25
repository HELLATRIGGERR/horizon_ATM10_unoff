/** Just Dire Things' Energy Transmitter, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar (on the furnace template) make it.
 *
 * - Its screen opens with its nine filter slots only.
 * - A transmitter with 40,000 FE and a Block Placer T2 in its area (the block above, by default): every 50 ticks it scans, then
 *   each tick sends the machine 1,000 FE less one percent a block of distance, until its energy is gone: the machine ends with
 *   39,600 FE.
 * - Two transmitters, one holding 40,000 FE and the other seeing no one: the first, with the other in its area, balances their
 *   energy to 20,000 each.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-transmitter', 'server-thread', { devMods: [content] });
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

  const consumer = `${x} ${y + 1} ${z}`;
  await fixture.command(`setblock ${pos} justdirethings:energytransmitter[facing=north]`);
  await fixture.command(`setblock ${consumer} justdirethings:blockplacert2[facing=north]`);
  await open('justdirethings:energytransmitter');
  const view = await screen();
  assert.deepEqual(own(view), [1, 2, 3, 4, 5, 6, 7, 8, 9], `nine filter slots: ${JSON.stringify(view.slots)}`);
  await shoot('transmitter.png');
  await fixture.raw('key escape tap');
  let nbt = await saved(pos);
  assert.equal(field(nbt, 'tickspeed'), 50, 'it scans every 50 ticks');
  await fixture.command(`data merge block ${pos} {energy:40000}`);
  await until(async () => field(await saved(consumer), 'energy') >= 39000, v => v, 'the machine above fills', 60000);
  await Bun.sleep(2000);
  const got = field(await saved(consumer), 'energy');
  const left = field(await saved(pos), 'energy');
  log.consumer = got;
  log.transmitter = left;
  assert(got >= 39500 && got <= 39700, `1,000 FE a tick less a percent: ${got}`);
  assert(left <= 100, `the transmitter is emptied: ${left}`);
  fixture.mark(`the transmitter sends the machine above it 1,000 FE a tick, less 1%: ${got} FE of 40,000 arrive`);

  const a = `${x + 4} ${y} ${z}`, b = `${x + 6} ${y + 1} ${z}`;
  await fixture.command(`setblock ${a} justdirethings:energytransmitter[facing=north]`);
  await fixture.command(`setblock ${b} justdirethings:energytransmitter[facing=north]`);
  await Bun.sleep(1500);
  await fixture.command(`data merge block ${a} {energy:40000,xRadiusDouble:2.0d,yRadiusDouble:0.0d,zRadiusDouble:0.0d,xOffset:0,yOffset:1,zOffset:0,renderArea:0b,allowlist:0b,compareNBT:0b,blockitemfilter:-1}`);
  await until(async () => {
    const [ea, eb] = [field(await saved(a), 'energy'), field(await saved(b), 'energy')];
    return Math.abs(ea - 20000) <= 1 && Math.abs(eb - 20000) <= 1;
  }, v => v, 'the two transmitters balance to 20,000 each', 60000);
  fixture.mark('a transmitter that sees another balances their energy: 20,000 FE each');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS transmitter', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS transmitter', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
