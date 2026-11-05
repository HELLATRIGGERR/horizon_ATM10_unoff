/** The rest of `FluidType`, part two (docs/mod-fluids.md, slice 7b), with Java's facts from the fluids-only oracle run
 * of ATM10 8.4 (`FluidInteractionFacts`: each `FluidInteractionRegistry` interaction run on a grid of worlds; each
 * type's `getDripInfo`, `setItemMovement`, `canHydrate` and `supportsBoating`; each liquid block's light in place):
 *
 * - `FluidInteractionRegistry.canInteract` (`LiquidBlock.shouldSpreadLiquid`), the pack mod's files: lava placed beside
 *   Allthemodium's soul lava becomes gilded blackstone (a source) or blackstone (flowing); All the Ores' molten iron
 *   beside water an iron block or cobblestone; the Undergarden's virulent mix beside water depthrock; lava beside
 *   molten iron obsidian; NeoForge's own lava and water still obsidian; lava beside stone stays.
 * - Light: a liquid block lights by its own block's light, never its type's (`FluidType.getLightLevel` reaches no
 *   light engine in NeoForge 1.21.1): soul lava (block 15, type 0) lights its cell, molten iron (block 0, type 15)
 *   leaves its cell dark.
 * - The dripstone hook (`getDripInfo`): virulent mix over a stalactite fills the empty cauldron below with the
 *   Undergarden's virulent mix cauldron, Create Dragons Plus' dragon's breath with its dragon's breath cauldron.
 * - Hydration by type (`canBeHydrated`, `canHydrate`): EvilCraft's blood keeps farmland moist and a sponge takes it
 *   (a wet sponge); Mekanism's heavy water does neither.
 * - A dropped item (`setItemMovement`) floats up in heavy water as in water, where it used to sink.
 * - Boats (`canBoatInFluid`, `supportsBoating`): a boat floats on blood and sinks in heavy water.
 *
 * Needs ATM10_CARRIES: carries with slice 7b's declarations (`drip`, `item_movement`; the pack mod's
 * `fluid_interaction` files), its `alltheores`, `allthemodium`, `undergarden`, `create_dragons_plus`, `mekanism`,
 * `evilcraft` and `atm10-pack`. */
import assert from 'node:assert/strict';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../ae2/tests/world/harness';
import { root } from '../../../ae2/scripts/common';

const carries = resolve(process.env.ATM10_CARRIES ?? '');
const mods = ['alltheores', 'allthemodium', 'undergarden', 'create_dragons_plus', 'mekanism', 'evilcraft'];
for (const name of [...mods, 'atm10-pack']) assert(existsSync(join(carries, name, 'horizon-mod.toml')), `ATM10_CARRIES has no ${name}`);
const declaration = (carry: string, fluid: string) => {
  const [ns, path] = fluid.split(':');
  return JSON.parse(readFileSync(join(carries, carry, `data/${ns}/horizon/fluid/${path}.json`), 'utf8'));
};
const interactions = join(carries, 'atm10-pack/data/kubejs/horizon/fluid_interaction');
const rules = (fluid: string) => {
  const [ns, path] = fluid.split(':');
  return JSON.parse(readFileSync(join(interactions, ns, `${path}.json`), 'utf8')).interactions as any[];
};
// Java's values, as the oracle wrote them.
const meets = (list: any[], fluid: string) => list.find(r => (r.fluids ?? []).includes(fluid));
assert.deepEqual([meets(rules('minecraft:lava'), 'allthemodium:soul_lava').source, meets(rules('minecraft:lava'), 'allthemodium:soul_lava').flowing],
  ['minecraft:gilded_blackstone', 'minecraft:blackstone']);
assert.deepEqual([meets(rules('alltheores:molten_iron'), 'minecraft:water').source, meets(rules('alltheores:molten_iron'), 'minecraft:water').flowing],
  ['minecraft:iron_block', 'minecraft:cobblestone']);
assert.equal(meets(rules('undergarden:virulent_mix_source'), 'minecraft:water').source, 'undergarden:depthrock');
assert.equal(rules('minecraft:lava').some(r => (r.fluids ?? []).includes('minecraft:water')), false, "NeoForge's own interactions are the engine's");
assert.deepEqual(declaration('undergarden', 'undergarden:virulent_mix_source').drip,
  { chance: 0.05859375, particle: 'undergarden:dripping_virulent', cauldron: 'undergarden:virulent_mix_cauldron' });
assert.equal(declaration('create_dragons_plus', 'create_dragons_plus:dragon_breath').drip.cauldron, 'create_dragons_plus:dragon_breath_cauldron');
assert.equal(declaration('create_dragons_plus', 'create_dragons_plus:dragon_breath').item_movement.horizontal, 0.949999988079071);
assert.equal(declaration('mekanism', 'mekanism:heavy_water').item_movement, undefined, "NeoForge's default");
assert.deepEqual([declaration('evilcraft', 'evilcraft:blood').entity.can_hydrate, declaration('evilcraft', 'evilcraft:blood').entity.supports_boating], [true, true]);
assert.deepEqual([declaration('mekanism', 'mekanism:heavy_water').entity.can_hydrate, declaration('mekanism', 'mekanism:heavy_water').entity.supports_boating], [false, false]);

// The pack mod's interaction files, in a content mod of their own (the whole pack mod names every carry).
const tests = join(root, '.local/world-tests'); mkdirSync(tests, { recursive: true });
const interactionMod = mkdtempSync(join(tests, 'mod-fluid-interactions-files-'));
writeFileSync(join(interactionMod, 'horizon-mod.toml'), 'id = "interactions_on"\nname = "Interactions"\nversion = "1.0.0"\napi = "0.2"\nside = "both"\ncapabilities = ["registries"]\n');
cpSync(interactions, join(interactionMod, 'data/interactions_on/horizon/fluid_interaction'), { recursive: true });

const fixture = new WorldFixture('mod-fluid-interactions', 'server-thread',
  { devMods: [...mods.map(c => join(carries, c)), interactionMod] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => line.includes('Test passed'));
const isBlock = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
const block = async (pos: string) => (await fixture.raw(`getblock ${pos}`)).block as string;
const entityY = async (tag: string) => {
  const reply = await fixture.command(`data get entity @e[tag=${tag},limit=1] Pos[1]`);
  return Number((reply.chat?.join(' ') ?? '').match(/(-?\d+(?:\.\d+)?)d/)?.[1]);
};

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-6, -6], [-6, 26], [26, -6], [26, 26]] as const) {
    await until(async () => passed(await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)), v => v, 'fixture corner loaded', 30000);
  }
  await fixture.command('gamemode creative @s');
  await fixture.command('gamerule random_tick_speed 0');
  await fixture.command('time set noon'); await fixture.command('gamerule advance_time false'); await fixture.command('weather clear');
  await fixture.command(`fill ${at(-6, 0, -6)} ${at(26, 8, 26)} minecraft:air`);
  await fixture.command(`fill ${at(-6, -1, -6)} ${at(26, -1, 26)} minecraft:stone`);
  await fixture.command(`tp @s ${x + 10.5} ${y + 12} ${z - 4.5} 0 45`);

  // Interactions: each neighbour placed, then the fluid that meets it, on a held server (nothing flows between).
  await fixture.hold();
  const cases: [number, string, string, string][] = [
    [0, 'allthemodium:soul_lava[level=0]', 'minecraft:lava[level=0]', 'minecraft:gilded_blackstone'],
    [3, 'allthemodium:soul_lava[level=0]', 'minecraft:lava[level=2]', 'minecraft:blackstone'],
    [6, 'minecraft:water[level=0]', 'alltheores:molten_iron[level=0]', 'minecraft:iron_block'],
    [9, 'minecraft:water[level=0]', 'alltheores:molten_iron[level=2]', 'minecraft:cobblestone'],
    [12, 'minecraft:water[level=0]', 'undergarden:virulent_mix[level=0]', 'undergarden:depthrock'],
    [15, 'alltheores:molten_iron[level=0]', 'minecraft:lava[level=0]', 'minecraft:obsidian'],
    [18, 'minecraft:water[level=0]', 'minecraft:lava[level=0]', 'minecraft:obsidian'],
    [21, 'minecraft:stone', 'minecraft:lava[level=0]', 'minecraft:lava'],
  ];
  for (const [dx, neighbour, fluid] of cases) {
    await fixture.command(`setblock ${at(dx, 0, 1)} ${neighbour}`);
    await fixture.command(`setblock ${at(dx, 0, 0)} ${fluid}`);
  }
  const met: string[] = [];
  for (const [dx, neighbour, fluid, result] of cases) {
    const got = await block(at(dx, 0, 0));
    met.push(`${fluid} by ${neighbour}: ${got}`);
    assert(got.startsWith(result), `${fluid} beside ${neighbour}: ${got}, not ${result}`);
  }
  log.interactions = met;
  // Cleared before the server runs again: nothing of the row flows on.
  await fixture.command(`fill ${at(-1, 0, -1)} ${at(23, 0, 3)} minecraft:air`);
  await fixture.hold(false);
  fixture.mark('lava beside soul lava: gilded blackstone or blackstone; molten iron beside water: an iron block or cobblestone; virulent mix beside water: depthrock (the pack\'s FluidInteractionRegistry), lava beside water obsidian as ever');

  // Light: each fluid in a pit in the floor of a closed stone cell; the light above it.
  const cell = (dx: number, fluid: string) => [
    `fill ${at(dx, 0, 6)} ${at(dx + 4, 4, 10)} minecraft:stone`,
    `fill ${at(dx + 1, 1, 7)} ${at(dx + 3, 3, 9)} minecraft:air`,
    `setblock ${at(dx + 2, 0, 8)} ${fluid}`,
  ];
  for (const c of [...cell(0, 'allthemodium:soul_lava[level=0]'), ...cell(6, 'alltheores:molten_iron[level=0]')]) await fixture.command(c);
  const light = (dx: number) => fixture.raw(`getblock ${at(dx + 2, 1, 8)}`);
  const lit = await until(() => light(0), v => v.block_light === 14 && v.sky_light === 0, 'soul lava lights its cell', 30000);
  await fixture.step(20);
  const dark = await light(6);
  log.light = { soul_lava: lit, molten_iron: dark };
  assert.deepEqual([dark.block_light, dark.sky_light], [0, 0], `molten iron leaves its cell dark: ${JSON.stringify(dark)}`);
  fixture.mark('soul lava (its block\'s light 15) lights its cell, molten iron (its type\'s 15, its block\'s 0) leaves it dark');

  // Dripstone, hydration: random ticks fast, each fluid in a pit over a stalactite with a cauldron below; farmland
  // and sponges beside blood and beside heavy water.
  const drip = (dx: number, fluid: string) => [
    `fill ${at(dx - 1, 5, 13)} ${at(dx + 1, 6, 15)} minecraft:stone`,
    `setblock ${at(dx, 6, 14)} ${fluid}`,
    `setblock ${at(dx, 4, 14)} minecraft:pointed_dripstone[vertical_direction=down,thickness=tip]`,
    `setblock ${at(dx, 0, 14)} minecraft:cauldron`,
  ];
  for (const c of [...drip(0, 'undergarden:virulent_mix[level=0]'), ...drip(6, 'create_dragons_plus:dragon_breath[level=0]')]) await fixture.command(c);
  await fixture.command(`setblock ${at(0, 5, 14)} minecraft:dripstone_block`);
  await fixture.command(`setblock ${at(6, 5, 14)} minecraft:dripstone_block`);
  const pit = (dx: number, dz: number, fluid: string) => [
    `fill ${at(dx - 1, -2, dz - 1)} ${at(dx + 1, -1, dz + 1)} minecraft:stone`,
    `setblock ${at(dx, -1, dz)} ${fluid}`,
  ];
  for (const [dx, fluid] of [[12, 'evilcraft:blood[level=0]'], [18, 'mekanism:heavy_water[level=0]']] as const) {
    for (const c of pit(dx, 14, fluid)) await fixture.command(c);
    await fixture.command(`setblock ${at(dx + 1, -1, 14)} minecraft:farmland[moisture=7]`);
    for (const c of pit(dx, 18, fluid)) await fixture.command(c);
  }
  await fixture.command(`setblock ${at(12, 0, 18)} minecraft:sponge`);
  await fixture.command(`setblock ${at(18, 0, 18)} minecraft:sponge`);
  await until(() => block(at(12, 0, 18)), b => b === 'minecraft:wet_sponge', 'a sponge takes blood');
  assert.match(await block(at(12, -1, 18)), /^minecraft:air$/, 'the blood is gone');
  assert.equal(await block(at(18, 0, 18)), 'minecraft:sponge', 'a sponge leaves heavy water');
  assert.match(await block(at(18, -1, 18)), /^mekanism:heavy_water/);
  fixture.mark('a sponge takes EvilCraft\'s blood (canHydrate) and leaves Mekanism\'s heavy water');
  await fixture.command('gamerule random_tick_speed 200');
  try {
    await until(async () => (await block(at(0, 0, 14))) !== 'minecraft:cauldron' && (await block(at(6, 0, 14))) !== 'minecraft:cauldron',
      v => v, 'both stalactites drip into their cauldrons', 120000);
    await until(() => block(at(19, -1, 14)), b => !b.includes('moisture=7'), 'farmland beside heavy water dries', 120000);
  } finally {
    await fixture.command('gamerule random_tick_speed 0');
  }
  const cauldrons = [await block(at(0, 0, 14)), await block(at(6, 0, 14))];
  log.cauldrons = cauldrons;
  assert.equal(cauldrons[0], 'undergarden:virulent_mix_cauldron');
  assert.match(cauldrons[1], /^create_dragons_plus:dragon_breath_cauldron/);
  fixture.mark('virulent mix and dragon\'s breath drip from their stalactites into their own cauldrons (getDripInfo)');
  log.farmland = [await block(at(13, -1, 14)), await block(at(19, -1, 14))];
  assert.match(await block(at(13, -1, 14)), /^minecraft:farmland\[moisture=7\]/, 'farmland beside blood stays moist');
  fixture.mark('farmland beside blood stays moist, beside heavy water it dries (canHydrate)');

  // A dropped item at the bottom of a pool of heavy water floats up; boats on blood and heavy water.
  const pool = (dx: number, fluid: string) => [
    `fill ${at(dx - 2, -1, 21)} ${at(dx + 2, 3, 25)} minecraft:stone`,
    `fill ${at(dx - 1, 0, 22)} ${at(dx + 1, 3, 24)} ${fluid}`,
  ];
  for (const c of [...pool(2, 'mekanism:heavy_water[level=0]'), ...pool(8, 'evilcraft:blood[level=0]'), ...pool(14, 'mekanism:heavy_water[level=0]')]) {
    await fixture.command(c);
  }
  await fixture.command(`summon minecraft:item ${x + 2.5} ${y + 0.05} ${z + 23.5} {Item:{id:"minecraft:stone",count:1},PickupDelay:32767,Tags:["in_heavy"]}`);
  await fixture.command(`summon minecraft:oak_boat ${x + 8.5} ${y + 3.6} ${z + 23.5} {Tags:["on_blood"]}`);
  await fixture.command(`summon minecraft:oak_boat ${x + 14.5} ${y + 3.6} ${z + 23.5} {Tags:["on_heavy"]}`);
  await fixture.step(100);
  const item = await entityY('in_heavy'), floats = await entityY('on_blood'), sinks = await entityY('on_heavy');
  log.entities = { item: item - y, boat_on_blood: floats - y, boat_in_heavy_water: sinks - y };
  assert(item > y + 0.5, `the item floats up in heavy water (y ${item - y})`);
  fixture.mark('a dropped item floats up in heavy water (setItemMovement, as in water)');
  assert(floats > y + 3, `the boat floats on blood (y ${floats - y})`);
  assert(sinks < y + 1, `the boat sinks in heavy water (y ${sinks - y})`);
  fixture.mark('a boat floats on blood (supportsBoating) and sinks in heavy water');

  fixture.assertHidden(await fixture.raw('status'));
  console.log(`FLUIDS 7b ${JSON.stringify(log)}`);
  fixture.report('passed');
} catch (error) { console.log(`FLUIDS 7b ${JSON.stringify(log)}`); fixture.report('failed', error); throw error; }
finally { await fixture.close(); rmSync(interactionMod, { recursive: true, force: true }); }
