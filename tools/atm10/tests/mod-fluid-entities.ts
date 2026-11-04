/** Entities in carried pack fluids (docs/mod-fluids.md, slice 4): NeoForge's `FluidType` over an entity, with Java's
 * facts from the oracle runs of ATM10 8.4 (`FluidType`'s `canSwim`, `canDrownIn`, `motionScale`,
 * `getFallDistanceModifier`, asked of a pig; the client extension's `getRenderOverlayTexture`, `modifyFogColor` and
 * `modifyFogRender`), in a world with the Mekanism and All the Ores carries alone. Two pools, four deep:
 *
 * - Mekanism's heavy water (its type is `FluidType.Properties`' defaults: swim, drown, push at 0.014, fall distance
 *   halved): a player in it travels as in water (a slow sink, not a fall), falls ten blocks into it unhurt, loses air
 *   with the eyes under, sees Mekanism's fog (its tint, from 8 behind to 24 ahead) and the underwater overlay, and is
 *   pushed along by its flow; a pig in it floats up (`FloatGoal`) and loses air.
 * - All the Ores' molten aluminum (`canSwim` and `canDrownIn` false, lava's push): a player keeps all its air with the
 *   eyes under, sees its fog (0.178 grey, to 16 blocks); a pig sinks and keeps its air.
 *
 * Needs ATM10_CARRIES: carries with slice 4's declarations (`entity`, `render_overlay`, `fog_color`, `fog_range`, from
 * the client oracle run with the fog probe), its `mekanism` and `alltheores`. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../ae2/tests/world/harness';

const carries = resolve(process.env.ATM10_CARRIES ?? '');
for (const name of ['mekanism', 'alltheores']) assert(existsSync(join(carries, name, 'horizon-mod.toml')), `ATM10_CARRIES has no ${name}`);
const declaration = (carry: string, fluid: string) => {
  const [ns, path] = fluid.split(':');
  return JSON.parse(readFileSync(join(carries, carry, `data/${ns}/horizon/fluid/${path}.json`), 'utf8'));
};
// Java's values, as the oracle runs wrote them.
const heavy = declaration('mekanism', 'mekanism:heavy_water');
assert.deepEqual(heavy.entity, { can_swim: true, can_drown: true, can_push: true, motion_scale: 0.014, fall_distance_modifier: 0.5, can_extinguish: false, can_hydrate: false, supports_boating: false });
assert.equal(heavy.render_overlay, 'minecraft:textures/misc/underwater.png');
assert.deepEqual(heavy.fog_range, [-8, 24]);
const molten = declaration('alltheores', 'alltheores:molten_aluminum');
assert.equal(molten.entity.can_swim, false); assert.equal(molten.entity.can_drown, false);
assert.deepEqual(molten.fog_range, [-8, 16]);
const near = (a: number[], b: number[], eps = 1e-3) => a.length === b.length && a.every((v, i) => Math.abs(v - b[i]) < eps);

const fixture = new WorldFixture('mod-fluid-entities', 'server-thread', { devMods: ['mekanism', 'alltheores'].map(c => join(carries, c)) });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => line.includes('Test passed'));
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-6, -6], [-6, 26], [16, -6], [16, 26]] as const) {
    await until(async () => passed(await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)), v => v, 'fixture corner loaded', 30000);
  }
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await fixture.command('time set noon'); await fixture.command('gamerule doDaylightCycle false'); await fixture.command('weather clear');
  await fixture.command('gamerule doMobSpawning false');
  await fixture.command(`fill ${at(-6, -1, -6)} ${at(16, 14, 26)} minecraft:air`);
  await fixture.command(`fill ${at(-6, -1, -6)} ${at(16, -1, 26)} minecraft:stone`);
  // Two pools of 5x5, four deep, in glass: heavy water at x 0..4, molten aluminum at x 8..12.
  const pool = async (dx: number, block: string) => {
    await fixture.command(`fill ${at(dx - 1, 0, -1)} ${at(dx + 5, 3, 5)} minecraft:glass`);
    await fixture.command(`fill ${at(dx, 0, 0)} ${at(dx + 4, 3, 4)} ${block}[level=0]`);
  };
  await pool(0, 'mekanism:heavy_water'); await pool(8, 'alltheores:molten_aluminum');
  const state = () => fixture.raw('state');
  const player = async () => (await state()).player;
  const into = async (dx: number, dy: number) => { await fixture.command(`tp @s ${x + dx + 2.5} ${y + dy} ${z + 2.5} 0 0`); };

  // Heavy water: the water branch of LivingEntity.travel, not a fall.
  await into(0, 3.0);
  await fixture.step(20);
  let p = await player();
  assert(p.position[1] > y + 2.0, `a second in heavy water sinks slowly (y ${p.position[1] - y} of 3)`);
  assert(p.velocity[1] > -0.1 && p.velocity[1] < 0, `water's slow sink: ${p.velocity[1]}`);
  fixture.mark('a player in heavy water travels as in water: a slow sink, not a fall');

  // The eyes under: air goes, and the camera sees Mekanism's fog and the underwater overlay.
  await into(0, 0.0);
  await until(state, s => s.eye_mod_fluid?.fluid === 'mekanism:heavy_water', 'the eyes in heavy water');
  const air0 = (await player()).air;
  await fixture.step(40);
  let s = await until(state, v => v.player.air < air0 - 20, 'air used up in heavy water', 20000);
  assert.equal(s.eye_mod_fluid.overlay, 'minecraft:textures/misc/underwater.png');
  s = await until(state, v => v.fog && near(v.fog.environmental, [-8, 24]), "Mekanism's fog range");
  assert(near(s.fog.color.slice(0, 3), heavy.fog_color), `Mekanism's fog colour, its tint: ${s.fog.color} against ${heavy.fog_color}`);
  fixture.mark("eyes in heavy water: air goes down, Mekanism's fog (its tint, -8 to 24) and the underwater overlay");

  // Molten aluminum: no drowning (canDrownIn false), and its own fog.
  await fixture.command('effect clear @s');
  await into(8, 0.0);
  s = await until(state, v => v.eye_mod_fluid?.fluid === 'alltheores:molten_aluminum', 'the eyes in molten aluminum');
  const air1 = s.player.air;
  await fixture.step(60);
  s = await state();
  assert(s.player.air >= air1, `no air lost in molten aluminum (${air1} to ${s.player.air})`);
  s = await until(state, v => v.fog && near(v.fog.environmental, [-8, 16]), "molten aluminum's fog range");
  assert(near(s.fog.color.slice(0, 3), molten.fog_color), `its fog colour: ${s.fog.color} against ${molten.fog_color}`);
  assert.equal(s.eye_mod_fluid.overlay ?? null, null, 'no overlay of its own');
  fixture.mark('eyes in molten aluminum: no drowning where Java says so, and its own fog');

  // A fall of ten blocks into heavy water: the fall distance halves each tick in it (getFallDistanceModifier 0.5).
  await fixture.command('effect give @s minecraft:instant_health 1 5 true'); await fixture.step(5);
  const healthy = (await player()).health;
  await into(0, 13.0);
  await until(player, v => v.on_ground && v.position[1] < y + 0.5, 'landed on the bottom of the heavy water', 20000);
  assert.equal((await player()).health, healthy, 'no fall damage after ten blocks into heavy water');
  fixture.mark('a ten-block fall into heavy water hurts nothing');

  // The flow pushes: a channel of heavy water flowing east from a source, the player standing in it.
  await fixture.command(`fill ${at(0, 0, 14)} ${at(12, 1, 16)} minecraft:glass`);
  await fixture.command(`fill ${at(0, 0, 15)} ${at(11, 1, 15)} minecraft:air`);
  await fixture.command(`setblock ${at(0, 0, 15)} mekanism:heavy_water[level=0]`);
  await until(async () => passed(await fixture.command(`execute if block ${at(5, 0, 15)} mekanism:heavy_water`)), v => v, 'the channel flows', 20000);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 15.5} 90 0`);
  await fixture.step(40);
  p = await player();
  assert(p.position[0] > x + 3.0, `pushed east along the flow (x ${p.position[0] - x} from 2.5)`);
  fixture.mark('the flow of heavy water pushes a player along it');

  // Pigs: one floats up in heavy water and loses air; one stays down in molten aluminum and keeps it.
  await fixture.command(`tp @s ${x + 2.5} ${y + 6} ${z + 8.5}`);
  await fixture.command(`summon minecraft:pig ${x + 2.5} ${y} ${z + 2.5} {Tags:["in_heavy"]}`);
  await fixture.command(`summon minecraft:pig ${x + 10.5} ${y} ${z + 2.5} {Tags:["in_molten"]}`);
  const pig = async (tag: string, path: string) => (await fixture.command(`data get entity @e[tag=${tag},limit=1] ${path}`)).chat?.join(' ') ?? '';
  const pigY = async (tag: string) => Number((await pig(tag, 'Pos[1]')).match(/(-?\d+(?:\.\d+)?)d/)?.[1]);
  const pigAir = async (tag: string) => Number((await pig(tag, 'Air')).match(/(-?\d+)s/)?.[1]);
  await fixture.step(100);
  const floated = await pigY('in_heavy'), sunk = await pigY('in_molten');
  assert(floated > y + 2.5, `the pig in heavy water floats up (y ${floated - y})`);
  assert(sunk < y + 0.5, `the pig in molten aluminum stays down (y ${sunk - y})`);
  assert.equal(await pigAir('in_molten'), 300, 'and keeps its air');
  fixture.mark('a pig floats up in heavy water (canSwim) and stays down in molten aluminum, keeping its air');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
