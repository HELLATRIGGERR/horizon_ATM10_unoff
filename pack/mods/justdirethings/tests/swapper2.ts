/** Just Dire Things' Block Swapper T2, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar (on the furnace template) make it.
 *
 * - Two swappers bound by the wrench: a pulse with energy swaps the blocks in front, 50 FE for each side of each block.
 * - Without energy nothing swaps.
 * - The X radius button of one (pressed twice) widens its partner's area too; a pulse then swaps the row of three.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-swapper2', 'server-thread', { devMods: [content] });
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
  const a = `${x} ${y} ${z}`, b = `${x + 6} ${y} ${z}`;
  const row = (cx: number) => [-1, 0, 1].map(dx => `${cx + dx} ${y} ${z - 1}`);
  const power = (on: boolean) => fixture.command(`setblock ${x} ${y + 1} ${z} ${on ? 'minecraft:redstone_block' : 'minecraft:air'}`);
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;
  await fixture.command(`setblock ${a} justdirethings:blockswappert2[facing=north]`);
  await fixture.command(`setblock ${b} justdirethings:blockswappert2[facing=north]`);
  await fixture.command(`setblock ${x} ${y} ${z - 1} minecraft:gold_block`);
  await fixture.command(`setblock ${x + 6} ${y} ${z - 1} minecraft:dirt`);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${a}`), v => v.block === 'justdirethings:blockswappert2', 'the swapper reaches the client');
  await until(() => fixture.raw(`block ${b}`), v => v.block === 'justdirethings:blockswappert2', 'the partner reaches the client');
  await fixture.replace('hotbar.0', 'justdirethings:ferricore_wrench', 1);
  await fixture.raw(`interact ${a} south`);
  await Bun.sleep(600);
  await fixture.raw(`interact ${b} south`);
  await until(() => saved(a), s => s.includes('boundTo'), 'the first swapper is bound');
  await until(() => saved(b), s => s.includes('boundTo'), 'the second swapper is bound');
  await fixture.command('item replace entity @s hotbar.0 with minecraft:air');

  await power(true);
  await Bun.sleep(3000);
  assert(await is(`${x} ${y} ${z - 1}`, 'minecraft:gold_block'), 'no energy, no swap');
  await power(false);
  fixture.mark('the Swapper T2s bound by the wrench: without energy a pulse swaps nothing');
  await fixture.command(`data merge block ${a} {energy:100000}`);
  await Bun.sleep(1200);
  await power(true);
  await until(() => is(`${x} ${y} ${z - 1}`, 'minecraft:dirt'), v => v, 'with energy a pulse swaps the blocks in front', 30000);
  assert(await is(`${x + 6} ${y} ${z - 1}`, 'minecraft:gold_block'), 'gold in front of the second');
  const left = field(await saved(a), 'energy');
  log.energy = left;
  assert.equal(left, 99900, `a block swapped takes 50 FE on each side: ${left}`);
  fixture.mark(`with energy a pulse swaps the blocks in front, 100 FE for the block (${left} FE left)`);
  await power(false);

  await fixture.raw(`interact ${a} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the swapper T2 screen opens');
  const plus = at(25 + 30 + 6, 32 + 6);
  await fixture.raw(plus);
  await until(() => saved(a), s => /xRadiusDouble: ?0\.5/.test(s), 'the X radius button: 0.5');
  await Bun.sleep(500);
  await fixture.raw(plus);
  await until(() => saved(a), s => /xRadiusDouble: ?1\.0?d?/.test(s), 'the X radius button: 1');
  const shot = join(fixture.run, 'swapper2.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('key escape tap');
  await until(() => saved(b), s => /xRadiusDouble: ?1\.0?d?/.test(s), 'the partner takes the radius');
  for (const p of row(x)) await fixture.command(`setblock ${p} minecraft:gold_block`);
  for (const p of row(x + 6)) await fixture.command(`setblock ${p} minecraft:dirt`);
  await Bun.sleep(1200);
  await power(true);
  await until(async () => (await Promise.all(row(x).map(p => is(p, 'minecraft:dirt')))).every(Boolean), v => v, 'the row of three swaps', 30000);
  assert((await Promise.all(row(x + 6).map(p => is(p, 'minecraft:gold_block')))).every(Boolean), 'gold on the other side');
  assert.equal(field(await saved(a), 'energy'), 99900 - 300, 'six blocks, 50 FE each');
  fixture.mark('the widened area (the partner follows) swaps the row of three: 300 FE');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS swapper2', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS swapper2', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
