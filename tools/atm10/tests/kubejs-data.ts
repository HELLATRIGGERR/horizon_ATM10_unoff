/** KubeJS's data pack over the carries (docs/atm10-port.md, "Pack logic"): ATM10's `kubejs/data` turns most mods' ore
 * generation off, so that All the Ores' ores stand for them, and sets All the Ores' own veins; `final-data` lays those
 * files over the carries. Modern Industrialization's and All the Ores' carries are the sample. ATM10 8.4 turns off eight
 * of MI's eleven ores (bauxite, iridium, lead, lignite coal, nickel, salt, tin, uranium, in stone and in deepslate),
 * keeps antimony, monazite and tungsten, and gives ten of All the Ores' ores their own configured features.
 *
 * - The carries' files: MI's eight are `neoforge:none`, the three kept are MI's own `add_features`; All the Ores' ten
 *   features are the pack's, in 26.3's form (no `config`: 26.3 skips a feature that keeps it).
 * - A world with the two carries (seed 1234): the game runs the pack's ten features (none is in its list of features
 *   that cannot run), the nine chunks around spawn hold All the Ores' ores, and none of MI's eight. Each
 *   ore is counted by `fill ... air replace <ore>` over every column of the nine chunks (vanilla's limit of 32,768
 *   blocks a command: one 16 x 128 x 16 column at a time, y -64 to 63).
 *   MI's own ore features don't decode in the engine today (`invalid world generation field: targets`), so MI's kept
 *   ores are no control here: All the Ores' are, as the features the pack's files replaced.
 * - A screenshot: one chunk with its stone and deepslate taken away, its ores left in the air, seen from above.
 *
 * Needs ATM10_CARRIES: the carries folder `final-data` wrote with `--instance` (its `modern_industrialization` and
 * `alltheores`). */
import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../ae2/tests/world/harness';

const carries = resolve(process.env.ATM10_CARRIES ?? '');
const mi = join(carries, 'modern_industrialization'), ato = join(carries, 'alltheores');
for (const c of [mi, ato]) assert(existsSync(join(c, 'horizon-mod.toml')), `ATM10_CARRIES has no ${c}`);
const off = ['bauxite', 'iridium', 'lead', 'lignite_coal', 'nickel', 'salt', 'tin', 'uranium'];
const kept = ['antimony', 'monazite', 'tungsten'];
const modifiers = join(mi, 'data/modern_industrialization/neoforge/biome_modifier');
const modifier = (name: string) => JSON.parse(readFileSync(join(modifiers, `${name}.json`), 'utf8'));
for (const ore of off) for (const name of [`ore_generator_${ore}`, `deepslate_ore_generator_${ore}`]) {
  assert.deepEqual(modifier(name), { type: 'neoforge:none' }, `${name} is off`);
}
for (const ore of kept) for (const name of [`ore_generator_${ore}`, `deepslate_ore_generator_${ore}`]) {
  const m = modifier(name);
  assert.equal(m.type, 'neoforge:add_features', `${name} is MI's own`);
  assert.equal(m.features, `modern_industrialization:${name}`);
}
assert.equal(readdirSync(modifiers).filter(f => modifier(f.slice(0, -5)).type === 'neoforge:none').length, 2 * off.length, 'no other modifier is off');
const blocks = (ores: string[]) => ores.flatMap(o => [`modern_industrialization:${o}_ore`, `modern_industrialization:deepslate_${o}_ore`]);
// Iridium has no ore of its own in stone: MI's iridium generator is only the pack's file, turned off.
const declared = (id: string) => existsSync(join(mi, `data/modern_industrialization/horizon/block/${id.split(':')[1]}.json`));
const offBlocks = blocks(off).filter(declared), keptBlocks = blocks(kept);
for (const id of keptBlocks) assert(declared(id), `${id} is declared`);

const atoOres = ['cinnabar', 'iridium', 'lead', 'nickel', 'osmium', 'platinum', 'silver', 'tin', 'uranium', 'zinc'];
for (const ore of atoOres) {
  const f = JSON.parse(readFileSync(join(ato, `data/alltheores/worldgen/configured_feature/ore_${ore}.json`), 'utf8'));
  assert.equal(f.type, 'minecraft:ore'); assert(f.config === undefined && f.size > 0 && f.targets?.length, `ore_${ore} in 26.3's form`);
}
const atoDeclared = (id: string) => existsSync(join(ato, `data/alltheores/horizon/block/${id.split(':')[1]}.json`));
const atoBlocks = atoOres.flatMap(o => [`alltheores:${o}_ore`, `alltheores:deepslate_${o}_ore`]).filter(atoDeclared);

const fixture = new WorldFixture('kubejs-data', 'server-thread', { devMods: [mi, ato] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => line.includes('Test passed'));
/** How many blocks a `fill` changed: 0 when it says none were. */
const filled = async (command: string) => {
  const reply = await until(() => fixture.raw('command ' + command), r => !(r.chat ?? []).some((l: string) => l.includes('not loaded')), `${command}: loaded`, 30000);
  const text = (reply.chat ?? []).join(' ');
  const n = text.match(/filled (\d+)/i);
  if (n) return Number(n[1]);
  assert(/No blocks were filled/.test(text), `${command}: ${JSON.stringify(reply)}`);
  return 0;
};
try {
  const start = await fixture.start(), [px, , pz] = start.player.position;
  const log = readdirSync(fixture.run).filter(f => /^game-\d+\.log$/.test(f)).map(f => readFileSync(join(fixture.run, f), 'utf8')).join('\n');
  const skipped = [...log.matchAll(/^Horizon: {3}(\S+): /gm)].map(m => m[1]);
  for (const ore of atoOres) assert(!skipped.includes(`alltheores:ore_${ore}_placed`), `the pack's ore_${ore} runs: skipped ${skipped.join(', ')}`);
  fixture.mark(`the game runs the ten features of the pack's (skipped, carried from the jars: ${skipped.join(', ') || 'none'})`);
  const cx = Math.floor(px / 16), cz = Math.floor(pz / 16);
  const chunks: [number, number][] = [];
  for (let dx = -1; dx <= 1; dx++) for (let dz = -1; dz <= 1; dz++) chunks.push([(cx + dx) * 16, (cz + dz) * 16]);
  for (const [x, z] of chunks) {
    await until(async () => passed(await fixture.command(`execute if loaded ${x} 0 ${z}`)), v => v, `chunk at ${x} ${z} loaded`, 60000);
  }
  const counts: Record<string, number> = {};
  for (const id of [...atoBlocks, ...keptBlocks, ...offBlocks]) {
    counts[id] = 0;
    for (const [x, z] of chunks) counts[id] += await filled(`fill ${x} -64 ${z} ${x + 15} 63 ${z + 15} minecraft:air replace ${id}`);
  }
  writeFileSync(join(fixture.run, 'ore-counts.json'), JSON.stringify(counts, null, 1));
  console.log('ores in nine chunks:', JSON.stringify(counts));
  const atoTotal = atoBlocks.reduce((n, id) => n + counts[id], 0);
  assert(atoTotal > 0, "All the Ores' ores generate");
  fixture.mark(`All the Ores' ores generate from the pack's features: ${atoTotal} blocks (${atoBlocks.filter(id => counts[id]).map(id => `${id.split(':')[1]} ${counts[id]}`).join(', ')})`);
  for (const id of offBlocks) assert.equal(counts[id], 0, `${id} does not generate`);
  fixture.mark(`none of the ${offBlocks.length} MI ores ATM10 turns off generates in the same nine chunks`);

  // The picture: a chunk further out, its stone and deepslate gone, its ores left standing, seen from above.
  const [x, z] = [(cx + 2) * 16, cz * 16];
  await until(async () => passed(await fixture.command(`execute if loaded ${x} 0 ${z}`)), v => v, 'the picture\'s chunk loaded', 60000);
  for (const [lo, hi] of [[-64, -1], [0, 63], [64, 127]]) {
    for (const stone of ['stone', 'deepslate', 'granite', 'diorite', 'andesite', 'tuff', 'dirt', 'gravel', 'grass_block', 'water', 'lava'].map(b => `minecraft:${b}`)) {
      await filled(`fill ${x} ${lo} ${z} ${x + 15} ${hi} ${z + 15} minecraft:air replace ${stone}`);
    }
  }
  await fixture.command('time set noon'); await fixture.command('weather clear');
  await fixture.command('gamemode spectator @s');
  // A spectator's teleport, not waited for by where it lands: the picture is taken after the client has it.
  await fixture.raw(`command tp @s ${x + 8} 40 ${z - 20} 0 50`);
  await Bun.sleep(3000);
  const shot = join(fixture.run, 'mi-ores.png').replaceAll('\\', '/');
  await fixture.raw(`screenshot ${shot}`);
  assert(existsSync(shot), 'screenshot written');
  fixture.mark(`screenshot: ${shot}`);
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
