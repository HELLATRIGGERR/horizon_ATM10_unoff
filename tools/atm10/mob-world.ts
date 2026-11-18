/** Mod mobs as data, slice 1 (docs/mobs.md): carries whose entity types copy a vanilla mob template, in a real world.
 * A spawn egg and `/summon` spawn them; they tick and move with the template's AI; a screenshot shows them drawn
 * with their template's model (and their own texture where they have one); killed, they drop their own loot table.
 *
 *   bun run tools/atm10/mob-world.ts
 *
 * - MOB_CARRIES: a folder of carries with `data/<ns>/horizon/entity_type/` declarations (`mob-audit.py --declare`
 *   writes them beside copies of the carries; never into the generated ones). The test wants Merrymaking's and
 *   Occultism's: `merrymaking:christmas_creeper` (a creeper with its own texture, its own spawn egg) and Occultism's
 *   `possessed_skeleton`, `wild_zombie` and `wild_spider` (drawn by their template's own renderer).
 * - The screenshot is `mobs.png` in the fixture's run folder (printed): look at it. */
import assert from 'node:assert/strict';
import { existsSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

const { WorldFixture, until } = await import('../../ae2/tests/world/harness');
const carries = resolve(process.env.MOB_CARRIES ?? '');
assert(process.env.MOB_CARRIES && existsSync(carries), 'MOB_CARRIES names the folder of carries to load');
const mods = readdirSync(carries, { withFileTypes: true })
  .filter(d => d.isDirectory() && existsSync(join(carries, d.name, 'horizon-mod.toml')))
  .map(d => d.name).sort();
for (const id of ['merrymaking', 'occultism']) assert(mods.includes(id), `MOB_CARRIES has the ${id} carry`);
process.env.AE2_TEST_STARTUP_MS = String(Math.max(Number(process.env.AE2_TEST_STARTUP_MS ?? 0), 900_000));

const EGGED = 'merrymaking:christmas_creeper';
const SUMMONED = ['occultism:possessed_skeleton', 'occultism:wild_zombie', 'occultism:wild_spider'];
const fixture = new WorldFixture('mob-world', 'server-thread', { devMods: mods.map(id => join(carries, id)) });
const mobs = async (type?: string) =>
  ((await fixture.raw(type ? `entities 24 ${type}` : 'entities 24')).entities ?? []).filter((e: any) => e.kind === 'mob');
try {
  const spawn = await fixture.start(), [px, , pz] = spawn.player.position;
  // A lit pen in the sky near the player: a stone floor, glass walls and a glowstone roof (no mob burns or wanders off), day,
  // mobs only ours. Its corners are waited for: a world that has just opened is still loading its chunks.
  const [x, y, z] = [Math.floor(px) + 12, 200, Math.floor(pz) + 12];
  for (const [dx, dz] of [[-8, -8], [-8, 8], [8, -8], [8, 8]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${x + dx} ${y} ${z + dz}`)).chat ?? [],
      lines => lines.includes('Test passed'), 'loaded pen corner', 60_000);
  }
  for (const c of [
    'gamerule doMobSpawning false', 'gamerule doDaylightCycle false', 'time set noon', 'difficulty normal',
    `fill ${x - 8} ${y - 1} ${z - 8} ${x + 8} ${y - 1} ${z + 8} minecraft:stone`,
    `fill ${x - 8} ${y} ${z - 8} ${x + 8} ${y + 4} ${z + 8} minecraft:glass hollow`,
    `fill ${x - 7} ${y} ${z - 7} ${x + 7} ${y + 3} ${z + 7} minecraft:air`,
    `fill ${x - 8} ${y + 4} ${z - 8} ${x + 8} ${y + 4} ${z + 8} minecraft:glowstone`,
  ]) await fixture.command(c);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z - 2.5} 0 20`);

  // The egg: Merrymaking's own item, used on the floor.
  await fixture.replace('hotbar.0', `${EGGED}_spawn_egg`, 4);
  await fixture.select(0, `${EGGED}_spawn_egg`);
  for (const dx of [-2, 2]) await fixture.aimedInteract(`${x + dx} ${y - 1} ${z}`);
  const egged = await until(() => mobs(EGGED), list => list.length >= 2, `${EGGED} from its spawn egg`);
  fixture.mark(`${egged.length} ${EGGED} from the spawn egg`);
  for (const [i, type] of SUMMONED.entries()) await fixture.command(`summon ${type} ${x - 3 + 3 * i} ${y} ${z + 3}`);
  const all = [EGGED, ...SUMMONED];
  for (const type of SUMMONED) {
    const [m] = await until(() => mobs(type), list => list.length === 1, `${type} summoned`);
    assert(m.health > 0, `${type} is alive`);
  }
  fixture.mark(`summoned ${SUMMONED.join(', ')}`);

  // They tick and move: the template's goals stroll them about (RandomStrollGoal, LookAround).
  const start = new Map((await mobs()).map((m: any) => [m.id, m.pos]));
  const moved = await until(async () => {
    const now = await mobs();
    return all.map(type => now.filter((m: any) => m.type === type).some((m: any) => {
      const p = start.get(m.id);
      return p && Math.hypot(m.pos[0] - p[0], m.pos[2] - p[2]) > 0.5;
    }));
  }, ok => ok.every(Boolean), 'every type walked half a block', 60_000);
  assert(moved.every(Boolean));
  fixture.mark(`every type walked: ${all.join(', ')}`);

  // Drawn: the player looks at the pen; the screenshot is checked by eye.
  await fixture.command(`tp @s ${x + 0.5} ${y + 1} ${z - 6.5} 0 25`);
  const shot = join(fixture.run, 'mobs.png');
  await fixture.raw(`screenshot ${shot}`);
  assert(existsSync(shot));
  console.log(`screenshot ${shot}`);

  // Killed, each drops its own table: the possessed skeleton's always a skull (occultism:entities/possessed_skeleton).
  await fixture.command('kill @e[type=occultism:possessed_skeleton]');
  const items = await until(async () => ((await fixture.raw('entities 24 minecraft:item')).entities ?? []),
    list => list.some((e: any) => e.item?.item === 'minecraft:skeleton_skull'), 'the possessed skeleton\'s skull');
  fixture.mark(`the possessed skeleton dropped ${items.map((e: any) => e.item?.item).join(', ')}`);
  for (const type of all) await fixture.command(`kill @e[type=${type}]`);
  await until(() => mobs(), list => list.length === 0, 'every mob dead');
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
