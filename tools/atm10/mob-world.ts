/** Mod mobs as data (docs/mobs.md): carries whose entity types copy a vanilla mob template, in a real world.
 * A spawn egg and `/summon` spawn them; they tick and move with the template's AI; they have the health the carry
 * declares (the oracle's `max_health`); a screenshot shows them drawn with their template's model (and their own
 * texture where they have one); killed, they drop their own loot table.
 *
 *   bun run tools/atm10/mob-world.ts
 *
 * - MOB_CARRIES: the carries `horizon-port-pack carry` wrote with the oracle's entity facts (its work folder's
 *   `mods`, each carry in `<key>/carry`), or a folder of carries (`<key>/horizon-mod.toml`). MOB_MODS picks the
 *   carries to load (default `merrymaking,occultism`; `all`: every carry that declares a living type; an entry may also
 *   be a carry's own folder, outside MOB_CARRIES). The test
 *   wants Merrymaking's `christmas_creeper` (a creeper with its own texture, its own spawn egg) and Occultism's
 *   `possessed_skeleton`, `wild_zombie` and `wild_spider` (drawn by their template's own renderer).
 * - MOB_ALL=1: then `/summon`s every living type the loaded carries declare, one at a time, and prints how many
 *   spawned (`spawned N of M declared types`) with their health against the declared `max_health`.
 * - The screenshot is `mobs.png` in the fixture's run folder (printed): look at it. */
import assert from 'node:assert/strict';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const { WorldFixture, until } = await import('../../ae2/tests/world/harness');
const carries = resolve(process.env.MOB_CARRIES ?? '');
assert(process.env.MOB_CARRIES && existsSync(carries), 'MOB_CARRIES names the folder of carries to load');
const carryOf = (id: string) => [join(carries, id, 'carry'), join(carries, id), resolve(id)]
  .find(dir => existsSync(join(dir, 'horizon-mod.toml')));
// A carry's living types: its `data/<ns>/horizon/entity_type/<path>.json` declarations with a living template.
const declaredIn = (dir: string): string[] => existsSync(join(dir, 'data')) ? readdirSync(join(dir, 'data')).flatMap(ns => {
  const types = join(dir, 'data', ns, 'horizon', 'entity_type');
  return existsSync(types) ? readdirSync(types).filter(f => f.endsWith('.json'))
    .filter(f => JSON.parse(readFileSync(join(types, f), 'utf8')).template !== 'minecraft:snowball')
    .map(f => `${ns}:${f.slice(0, -'.json'.length)}`) : [];
}) : [];
const wanted = process.env.MOB_MODS === 'all'
  ? readdirSync(carries).filter(id => { const dir = carryOf(id); return dir && declaredIn(dir).length > 0; }).sort()
  : (process.env.MOB_MODS ?? 'merrymaking,occultism').split(',').filter(Boolean);
const folders = wanted.map(carryOf);
for (const [i, id] of wanted.entries()) assert(folders[i], `MOB_CARRIES has the ${id} carry`);
// A declaration's `minecraft:max_health` is the health a new mob has.
const declared = (type: string) => {
  const [ns, path] = type.split(':');
  const file = folders.map(dir => join(dir!, 'data', ns, 'horizon', 'entity_type', `${path}.json`)).find(existsSync);
  assert(file, `${type} is declared`);
  return JSON.parse(readFileSync(file, 'utf8'));
};
process.env.AE2_TEST_STARTUP_MS = String(Math.max(Number(process.env.AE2_TEST_STARTUP_MS ?? 0), 900_000));

const EGGED = 'merrymaking:christmas_creeper';
const SUMMONED = ['occultism:possessed_skeleton', 'occultism:wild_zombie', 'occultism:wild_spider'];
const fixture = new WorldFixture('mob-world', 'server-thread', { devMods: folders as string[] });
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
  const eggHealth = declared(EGGED).attributes?.['minecraft:max_health'];
  for (const m of egged) if (eggHealth !== undefined) assert.equal(m.health, eggHealth, `${EGGED} has its declared max_health`);
  fixture.mark(`${egged.length} ${EGGED} from the spawn egg, health ${egged.map((m: any) => m.health).join(', ')}`);
  for (const [i, type] of SUMMONED.entries()) await fixture.command(`summon ${type} ${x - 3 + 3 * i} ${y} ${z + 3}`);
  const all = [EGGED, ...SUMMONED];
  for (const type of SUMMONED) {
    const [m] = await until(() => mobs(type), list => list.length === 1, `${type} summoned`);
    assert(m.health > 0, `${type} is alive`);
    const health = declared(type).attributes?.['minecraft:max_health'];
    if (health !== undefined) assert.equal(m.health, health, `${type} has its declared max_health`);
    fixture.mark(`${type}: health ${m.health}${health === undefined ? ' (its template\'s)' : ' as declared'}`);
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

  // Every declared living type of the loaded carries, by `/summon`, one at a time: what spawns, with what health.
  if (process.env.MOB_ALL) {
    const types = [...new Set(folders.flatMap(dir => declaredIn(dir!)))].sort();
    const spawned: string[] = [], missing: string[] = [], otherHealth: string[] = [];
    for (const type of types) {
      await fixture.command(`summon ${type} ${x} ${y} ${z + 3}`);
      const list = await until(() => mobs(type), l => l.length >= 1, `${type} summoned`, 8_000).catch(() => []);
      if (list.length === 0) {
        missing.push(type);
        continue;
      }
      spawned.push(type);
      const health = declared(type).attributes?.['minecraft:max_health'];
      if (health !== undefined && list[0].health !== health) otherHealth.push(`${type} ${list[0].health} (declared ${health})`);
      await fixture.command(`kill @e[type=${type}]`);
      await until(() => mobs(type), l => l.length === 0, `${type} gone`, 8_000).catch(() => []);
    }
    console.log(`not spawned: ${missing.join(', ') || 'none'}`);
    console.log(`health other than declared: ${otherHealth.join('; ') || 'none'}`);
    fixture.mark(`spawned ${spawned.length} of ${types.length} declared types of ${wanted.length} carries`);
  }
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
