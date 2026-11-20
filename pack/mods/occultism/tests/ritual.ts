/** Occultism's spirit fire and a golden bowl ritual, as the `occultism` native bundle runs them on the carry of the
 * player's own jar.
 *
 * - Spirit fire: five andesite thrown into it become five otherstone (ATM10's `occultism:spirit_fire/otherstone`;
 *   `SpiritFireRecipe.assemble` keeps the stack's count). Flint and steel used on stone with datura beside it lights a
 *   new fire above the stone and uses the datura up.
 * - The ritual: ATM10's `occultism:ritual/craft_fragile_soul_gem` on the pentacle `occultism:craft_foliot`, drawn
 *   from the carry's own multiblock file, with an iron ingot (put in by a click), an egg and glass on three
 *   sacrificial bowls. The bound foliot book starts it on the golden bowl. Its numbers, from the source: the default
 *   duration 30, one unit every 20 game ticks on a golden bowl, an ingredient whenever floor(time / 7.5) grows (at 8,
 *   15 and 23), the result at 30; so the bowls empty one by one and the soul gem drops 581 to 600 ticks after the start.
 *   A glyph taken away interrupts a second start, which gives the book back.
 *
 * The `c:` tags the recipe names are ATM10's (from the `atm10-pack` carry, `OCCULTISM_PACK_CARRY`), cut to vanilla's
 * items for a world with only this carry; without it, the same three vanilla items. A picture of the pentacle, its
 * bowls and the fires goes to the run folder (`occultism.png`). */
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const carry = resolve(process.env.OCCULTISM_CARRY ?? join(root, '../ports/atm10/mods/occultism/carry'));
assert(existsSync(join(carry, 'horizon-mod.toml')), `OCCULTISM_CARRY ${carry} is the carry of Occultism`);

// The recipe's tags, as ATM10's pack mod has them, kept to the items a world with only this carry knows.
const tags = join(root, '.local/world-tests/occultism-tags');
rmSync(tags, { recursive: true, force: true });
const TAGS: Record<string, string[]> = { 'ingots/iron': ['minecraft:iron_ingot'], eggs: ['minecraft:egg'], glass_blocks: ['minecraft:glass'] };
for (const [tag, fallback] of Object.entries(TAGS)) {
  const pack = process.env.OCCULTISM_PACK_CARRY && join(process.env.OCCULTISM_PACK_CARRY, 'data/c/tags/item', `${tag}.json`);
  const values = pack && existsSync(pack)
    ? (JSON.parse(readFileSync(pack, 'utf8')).values as string[]).filter(v => v.startsWith('minecraft:'))
    : fallback;
  const file = join(tags, 'data/c/tags/item', `${tag}.json`);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, JSON.stringify({ values }));
}
writeFileSync(join(tags, 'horizon-mod.toml'),
  'id = "occultism_test_tags"\nname = "Occultism test tags"\nversion = "0.1.0"\napi = "0.2"\nside = "both"\ncapabilities = ["registries"]\n');

const fixture = new WorldFixture('occultism-ritual', 'server-thread', { devMods: [carry, tags] });
const log: Record<string, unknown> = {};

const chat = async (command: string) => ((await fixture.command(command)).chat ?? []).join('\n');
/** The block entity's compound on the server, as `data get block` prints it. */
const saved = async (pos: string) => chat(`data get block ${pos}`);
const isBlock = async (pos: string, block: string) => (await chat(`execute if block ${pos} ${block}`)).includes('Test passed');
/** The nearest item entity's stack within `r` of `pos`, as `data get entity` prints it ('' when none). */
const itemNear = async (pos: string, r = 2) => {
  const text = await chat(`execute positioned ${pos} run data get entity @e[type=minecraft:item,limit=1,sort=nearest,distance=..${r}] Item`);
  return text.includes('No entity was found') ? '' : text;
};
const countOf = (nbt: string) => Number(nbt.match(/count: ?(\d+)/)?.[1] ?? 0);

try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.ceil(py) + 2, z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-10, -10], [-10, 12], [12, -10], [12, 12]] as const) {
    await until(async () => chat(`execute if loaded ${x + dx} ${y} ${z + dz}`), t => t.includes('Test passed'), 'loaded corner', 60_000);
  }
  for (const c of [
    'gamerule doMobSpawning false', 'gamerule doDaylightCycle false', 'gamerule doFireTick false', 'time set noon',
    `fill ${at(-10, -1, -10)} ${at(12, -1, 12)} minecraft:stone`, `fill ${at(-10, 0, -10)} ${at(12, 6, 12)} minecraft:air`,
  ]) await fixture.command(c);

  // --- Spirit fire: andesite becomes otherstone, the whole stack.
  const fire = at(6, 0, 0);
  await fixture.command(`setblock ${fire} occultism:spirit_fire`);
  assert(await isBlock(fire, 'occultism:spirit_fire'), 'spirit fire places');
  await fixture.command(`summon minecraft:item ${x + 6.5} ${y + 0.2} ${z + 0.5} {Item:{id:"minecraft:andesite",count:5},PickupDelay:32767}`);
  const turned = await until(() => itemNear(fire), t => t.includes('occultism:otherstone'), 'the andesite turns in the spirit fire', 20_000);
  log.spirit_fire = turned;
  assert.equal(countOf(turned), 5, `five andesite give five otherstone: ${turned}`);
  fixture.mark('spirit fire turns five andesite into five otherstone (occultism:spirit_fire/otherstone)');

  // Lit by flint and steel over datura: the fire comes above the stone clicked, the datura is used up.
  const lit = at(6, 0, 4);
  await fixture.command(`summon minecraft:item ${x + 6.5} ${y + 0.1} ${z + 4.5} {Item:{id:"occultism:datura",count:1},PickupDelay:32767}`);
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 1.5} 0 60`);
  await fixture.replace('hotbar.0', 'minecraft:flint_and_steel');
  await fixture.select(0, 'minecraft:flint_and_steel');
  await fixture.raw(`interact ${at(6, -1, 4)} up`);
  await until(async () => isBlock(lit, 'occultism:spirit_fire'), ok => ok, 'flint and steel over datura lights spirit fire', 20_000);
  assert.equal(await itemNear(lit, 1.5), '', 'the datura is used up');
  fixture.mark('flint and steel used beside datura lights spirit fire and uses the datura up');

  // --- The pentacle, from the carry's own file: the top layer at y, the display floor below it.
  const pentacle = JSON.parse(readFileSync(join(carry, 'data/occultism/modonomicon/multiblocks/craft_foliot.json'), 'utf8'));
  const blockFor = (c: string): string | undefined => {
    const m = pentacle.mapping[c];
    if (!m) return undefined;
    if (m.type === 'modonomicon:block') return m.block;
    if (m.display) return m.display;
    if (m.tag === '#minecraft:candles') return 'minecraft:candle';
    throw new Error(`no block for ${c}: ${JSON.stringify(m)}`);
  };
  const layers: string[][] = pentacle.pattern;
  let center: [number, number] | undefined;
  layers.forEach((rows, i) => rows.forEach((row, j) => [...row].forEach((c, k) => { if (c === '0') center = [k, j]; })));
  assert(center, 'the pentacle has a center');
  let placed = 0;
  for (const [i, rows] of layers.entries()) {
    const dy = layers.length - i - 1 - (layers.length - 1); // the top layer at y, the floor at y - 1
    for (const [j, row] of rows.entries()) for (const [k, c] of [...row].entries()) {
      const block = blockFor(c);
      if (!block) continue;
      await fixture.command(`setblock ${at(k - center[0], dy, j - center[1])} ${block}`);
      placed++;
    }
  }
  log.pentacle_blocks = placed;
  const golden = at(0);
  assert(await isBlock(golden, 'occultism:golden_sacrificial_bowl'), 'the golden bowl is the center');
  // Three sacrificial bowls south of the pentacle, within the 8 blocks the ritual searches.
  const bowls = [at(-2, 0, 6), at(0, 0, 6), at(2, 0, 6)];
  for (const b of bowls) await fixture.command(`setblock ${b} occultism:sacrificial_bowl`);

  // The first bowl takes an iron ingot by a click (one of the two held), the others by command.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 8.5} 180 60`);
  await fixture.replace('hotbar.0', 'minecraft:iron_ingot', 2);
  await fixture.select(0, 'minecraft:iron_ingot');
  await fixture.raw(`interact ${bowls[0]} up`);
  const first = await until(() => saved(bowls[0]), t => t.includes('minecraft:iron_ingot'), 'the bowl takes the iron ingot');
  assert.equal(countOf(first), 1, `a bowl holds one: ${first}`);
  assert.equal((await fixture.raw('inventory')).hotbar[0]?.count, 1, 'one ingot stays in the hand');
  await fixture.command(`item replace block ${bowls[1]} container.0 with minecraft:egg`);
  await fixture.command(`item replace block ${bowls[2]} container.0 with minecraft:glass`);
  fixture.mark('sacrificial bowls hold one item each; a click puts one of the held stack in');

  // A picture: the pentacle, its bowls, the two fires; the chat fades first (10 s).
  await fixture.command(`setblock ${at(1, 7, 10)} minecraft:barrier`); // stood on while the chat fades
  await fixture.command(`tp @s ${x + 1.5} ${y + 8} ${z + 10.5} 180 52`);
  const shot = join(fixture.run, 'occultism.png');
  await Bun.sleep(11_000);
  await fixture.raw(`screenshot ${shot}`);
  assert(existsSync(shot));
  console.log(`screenshot ${shot}`);

  // The bound foliot book starts the ritual on the golden bowl.
  const book = 'occultism:book_of_binding_bound_foliot';
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 60`);
  await fixture.replace('hotbar.0', book);
  await fixture.select(0, book);
  await fixture.raw(`interact ${golden} up`);
  const started = await until(() => saved(golden), t => t.includes('craft_fragile_soul_gem'), 'the ritual starts', 20_000);
  assert(started.includes(book), `the bowl holds the book: ${started}`);
  fixture.mark('the bound foliot book starts occultism:ritual/craft_fragile_soul_gem on the drawn craft_foliot pentacle');

  // Its clock, held and stepped: the bowls empty at times 8, 15 and 23, the soul gem comes at 30.
  await fixture.hold();
  const time = (nbt: string) => Number(nbt.match(/currentTime: ?(\d+)/)?.[1] ?? -1);
  const full = async () => (await Promise.all(bowls.map(saved))).filter(t => /minecraft:(iron_ingot|egg|glass)/.test(t)).length;
  const trace: [number, number, number][] = [];
  let ticks = 0;
  while (ticks < 700) {
    await fixture.stepServer(5);
    ticks += 5;
    const nbt = await saved(golden);
    if (!nbt.includes('currentRitual')) break;
    const t = time(nbt), left = await full();
    trace.push([ticks, t, left]);
    const due = Math.min(3, Math.floor(t / 7.5));
    assert.equal(left, 3 - due, `at time ${t} ${due} ingredients are taken: ${JSON.stringify(trace.slice(-3))}`);
  }
  log.trace = trace.filter(([, , left], i) => i === 0 || left !== trace[i - 1][2]);
  assert(ticks >= 580 && ticks <= 605, `the ritual took ${ticks} ticks, 30 units of 20`);
  const gem = await until(() => itemNear(golden, 3), t => t.includes('occultism:fragile_soul_gem'), 'the soul gem drops');
  assert.equal(countOf(gem), 1);
  const after = await saved(golden);
  assert(!after.includes(book) && !after.includes('currentRitual'), `the book is used up and the ritual over: ${after}`);
  log.ritual_ticks = ticks;
  fixture.mark(`the ritual took ${ticks} ticks, the bowls emptied at times 8, 15 and 23, and dropped one fragile soul gem`);

  // A second start, then a glyph taken away: the ritual stops and gives back the book.
  await fixture.hold(false);
  await fixture.command(`item replace block ${bowls[0]} container.0 with minecraft:iron_ingot`);
  await fixture.command(`item replace block ${bowls[1]} container.0 with minecraft:egg`);
  await fixture.command(`item replace block ${bowls[2]} container.0 with minecraft:glass`);
  await fixture.command(`kill @e[type=minecraft:item]`);
  await fixture.replace('hotbar.0', book);
  await fixture.select(0, book);
  await fixture.raw(`interact ${golden} up`);
  await until(() => saved(golden), t => t.includes('craft_fragile_soul_gem'), 'the ritual starts again', 20_000);
  const [gx, gz] = [layers[0][1].indexOf('W'), 1];
  await fixture.command(`setblock ${at(gx - center[0], 0, gz - center[1])} minecraft:air`);
  await until(() => saved(golden), t => !t.includes('currentRitual'), 'a broken pentacle stops the ritual', 20_000);
  const back = await until(() => itemNear(golden, 3), t => t.includes(book), 'the book comes back');
  assert(back.includes(book));
  fixture.mark('a glyph taken away interrupts the ritual and gives the book back');

  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error);
  throw error;
} finally {
  console.log(JSON.stringify(log));
  await fixture.close();
}
