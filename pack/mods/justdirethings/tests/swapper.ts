/** Just Dire Things' Block Swapper T1, as the `justdirethings` native bundle and the content mod made from the carry of the
 * player's jar (the swapper on the furnace template) make it.
 *
 * - Two swappers and a Ferricore Wrench: the wrench used (sneaking) on one, then on the other, binds them (`boundTo` saved on both).
 * - A pulse (a redstone block set beside the first): the blocks in front of the two trade places.
 * - The entity button (one right click: "all"): a cow standing in front of the first is in front of the second after the
 *   next pulse.
 * - The blocks button off: the next pulse leaves the blocks where they are.
 * - A picture of its screen goes to the run folder (`swapper.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-swapper', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => Number(nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?\\d+)`))?.[1] ?? NaN);

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
  const fa = `${x} ${y} ${z - 1}`, fb = `${x + 6} ${y} ${z - 1}`;
  const power = (on: boolean) => fixture.command(`setblock ${x} ${y + 1} ${z} ${on ? 'minecraft:redstone_block' : 'minecraft:air'}`);
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;

  await fixture.command(`setblock ${a} justdirethings:blockswappert1[facing=north]`);
  await fixture.command(`setblock ${b} justdirethings:blockswappert1[facing=north]`);
  await fixture.command(`setblock ${fa} minecraft:gold_block`);
  await fixture.command(`setblock ${fb} minecraft:dirt`);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${a}`), v => v.block === 'justdirethings:blockswappert1', 'the swapper reaches the client');
  await until(() => fixture.raw(`block ${b}`), v => v.block === 'justdirethings:blockswappert1', 'the partner reaches the client');
  assert.equal(field(await saved(a), 'redstoneMode'), 3, 'pulse mode by default');

  // The wrench binds them.
  await fixture.replace('hotbar.0', 'justdirethings:ferricore_wrench', 1);
  await fixture.sneakInteract(`${a} south`);
  await Bun.sleep(600);
  await fixture.sneakInteract(`${b} south`);
  await until(() => saved(a), s => s.includes('boundTo'), 'the first swapper is bound');
  await until(() => saved(b), s => s.includes('boundTo'), 'the second swapper is bound');
  fixture.mark('the wrench used on the two swappers binds them');

  // A pulse: the blocks trade places.
  await power(true);
  await until(() => is(fa, 'minecraft:dirt'), v => v, 'dirt in front of the first', 20000);
  assert(await is(fb, 'minecraft:gold_block'), 'gold in front of the second');
  fixture.mark('a pulse swaps the blocks in front');

  // Entities: one right click on the entity button reads "all".
  await fixture.command('item replace entity @s hotbar.0 with minecraft:air');
  await power(false);
  await fixture.raw(`interact ${a} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the swapper screen opens');
  await fixture.raw(`${at(106 + 8, 58 + 8)} right`);
  await until(() => saved(a), s => field(s, 'swap_entity_type') === 8, 'the entity button sets all');
  await Bun.sleep(800);
  await fixture.raw(`${at(88 + 8, 58 + 8)}`);
  await until(() => saved(a), s => field(s, 'swapBlocks') === 0, 'the blocks button turns blocks off');
  const shot = join(fixture.run, 'swapper.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('key escape tap');
  await fixture.command(`summon minecraft:cow ${x + 0.5} ${y} ${z - 0.5} {NoAI:1b}`);
  await power(true);
  const inB = `execute if entity @e[type=minecraft:cow,x=${x + 6},y=${y},z=${z - 1},dx=0.9,dy=2,dz=0.9]`;
  await until(async () => passed(await fixture.command(inB)), v => v, 'the cow is in front of the second swapper', 20000);
  assert(await is(fa, 'minecraft:dirt') && await is(fb, 'minecraft:gold_block'), 'the blocks stayed with the blocks button off');
  fixture.mark('the entity button (all) moves the cow; the blocks button off leaves the blocks');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS swapper', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS swapper', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
