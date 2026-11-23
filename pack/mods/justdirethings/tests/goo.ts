/** Just Dire Things' goo blocks, as the `justdirethings` native bundle and the carry of the player's jar make them.
 *
 * - A living goo of each tier with an iron block on top turns it into raw ferricore ore facing up (the recipe
 *   `dire_iron_block`: 2,400 ticks for tier 1), in 2,401, 1,201, 481 and 241 game ticks (the tier's step of 1, 2, 5,
 *   10 a tick, and the tick that writes the block). While it runs, the block entity keeps Java's `sideCounters` and
 *   `sideDurations`.
 * - A dead goo (the block's default) starts nothing; an item of another tier's revive tag leaves it dead, sugar (tier
 *   1's tag) brings it to life and is used up (in survival), and it starts on its block.
 *
 * Reads blocks with `execute if block` and the saved compound with `data get block`. Needs the staged native build
 * (`AE2_TEST_BINARY`) and the content mod made from the carry of Just Dire Things (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-goo', 'server-thread', { devMods: [content] });

/** Whether the block at `pos` is `state` (`execute if block`). */
async function is(pos: string, state: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${state}`);
  return (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
}
/** The block entity's compound on the server, as `data get block` prints it. */
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
/** The `field` of side `side` in list `list` of the compound (`{side: 1, counter: 2399}`). */
function sideValue(nbt: string, list: string, side: number, field: string): number | undefined {
  const at = nbt.indexOf(`${list}: [`);
  if (at < 0) return undefined;
  const body = nbt.slice(at, nbt.indexOf(']', at));
  for (const entry of body.match(/\{[^}]*\}/g) ?? []) {
    const s = entry.match(/side: ?(-?\d+)/), v = entry.match(new RegExp(`${field}: ?(-?\\d+)`));
    if (s && Number(s[1]) === side && v) return Number(v[1]);
  }
  return undefined;
}
const gametime = async () => Number(((await fixture.command('time query gametime')).chat ?? []).join(' ').match(/(\d+)/)?.[1]);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 3;
  await fixture.command(`fill ${x - 1} ${y} ${z - 3} ${x + 14} ${y + 3} ${z + 7} minecraft:air`);
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 3} ${x + 14} ${y - 1} ${z + 7} minecraft:stone`);
  await fixture.command('gamerule advance_time false');

  // Four living goo blocks, one of each tier, an iron block on each.
  const tiers = [1, 2, 3, 4];
  const goo = (t: number) => `${x + (t - 1) * 3} ${y} ${z}`;
  const top = (t: number) => `${x + (t - 1) * 3} ${y + 1} ${z}`;
  for (const t of tiers) await fixture.command(`setblock ${top(t)} minecraft:iron_block`);
  const started: Record<number, number> = {};
  for (const t of [4, 3, 2, 1]) {
    await fixture.command(`setblock ${goo(t)} justdirethings:gooblock_tier${t}[alive=true]`);
    started[t] = await gametime();
  }
  assert(await is(goo(1), 'justdirethings:gooblock_tier1[alive=true]'), 'the goo is placed alive');
  let nbt = await until(() => saved(goo(1)), s => (sideValue(s, 'sideCounters', 1, 'counter') ?? -1) > 0, 'tier 1 starts on its iron block', 20000);
  log.running = nbt;
  assert.equal(sideValue(nbt, 'sideDurations', 1, 'duration'), 2400, 'the recipe dire_iron_block takes 2,400 ticks');
  assert.equal(sideValue(nbt, 'sideCounters', 0, 'counter'), -1, 'nothing runs on a side without a recipe');
  fixture.mark('a living goo starts on the iron block above it: sideCounters / sideDurations as Java saves them');

  const expected: Record<number, number> = { 1: 2401, 2: 1201, 3: 481, 4: 241 };
  const took: Record<number, number> = {};
  for (const t of [4, 3, 2, 1]) {
    await until(() => is(top(t), 'justdirethings:raw_ferricore_ore[facing=up]'), v => v, `tier ${t} converts its iron block`, expected[t] * 50 + 60000);
    took[t] = (await gametime()) - started[t];
    log[`tier${t}`] = took[t];
    assert(took[t] >= expected[t] - 5 && took[t] <= expected[t] + 60, `tier ${t} takes about ${expected[t]} ticks: ${took[t]}`);
    nbt = await saved(goo(t));
    assert.equal(sideValue(nbt, 'sideCounters', 1, 'counter'), -1, 'the side is free again');
  }
  fixture.mark(`the four tiers turn iron into raw ferricore ore facing away from the goo in ${[1, 2, 3, 4].map(t => took[t]).join(', ')} ticks`);

  // A dead goo starts nothing until its tier's revive item is used on it.
  const dead = `${x} ${y} ${z + 2}`, iron = `${x} ${y} ${z + 1}`;
  await fixture.command(`setblock ${iron} minecraft:iron_block`);
  await fixture.command(`setblock ${dead} justdirethings:gooblock_tier1`);
  assert(await is(dead, 'justdirethings:gooblock_tier1[alive=false]'), 'a placed goo is dead');
  await Bun.sleep(2500);
  nbt = await saved(dead);
  assert.equal(sideValue(nbt, 'sideCounters', 2, 'counter'), -1, 'a dead goo starts nothing');
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5} 180 30`);
  // In survival: a creative player keeps the item (`if (!player.isCreative()) itemStack.shrink(1)`).
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.1', 'minecraft:nether_wart', 3);
  await fixture.select(1, 'minecraft:nether_wart');
  await fixture.raw(`interact ${dead} up`);
  await Bun.sleep(500);
  assert(await is(dead, 'justdirethings:gooblock_tier1[alive=false]'), 'nether wart (tier 2) leaves a tier 1 goo dead');
  await fixture.replace('hotbar.0', 'minecraft:sugar', 3);
  await fixture.select(0, 'minecraft:sugar');
  await fixture.raw(`interact ${dead} up`);
  await until(() => is(dead, 'justdirethings:gooblock_tier1[alive=true]'), v => v, 'sugar revives the goo');
  const inv = await until(() => fixture.raw('inventory'), v => v.hotbar[0]?.count === 2, 'the sugar is used up');
  assert.equal(inv.hotbar[1]?.count, 3, 'the nether wart was not');
  nbt = await until(() => saved(dead), s => (sideValue(s, 'sideCounters', 2, 'counter') ?? -1) > 0, 'the revived goo starts on the iron to its north');
  fixture.mark('a dead goo starts nothing; another tier\'s item leaves it dead; sugar revives it, is used up, and it starts');

  const shot = join(fixture.run, 'goo.png');
  await fixture.command(`tp @s ${x + 4.5} ${y + 1} ${z + 5.5} 180 25`);
  await Bun.sleep(1000);
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS goo', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS goo', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
