/** The rest of `FluidType`, part one (docs/mod-fluids.md, slice 7a), with Java's facts from the oracle runs of ATM10
 * 8.4 (the fluids-only server run: `isVaporizedOnPlacement` in each dimension, NeoForge's `CauldronFluidContent`):
 *
 * - Vaporizing (`BucketItem.emptyContents`, `FluidType.isVaporizedOnPlacement`/`onVaporize`): Create Dragons Plus'
 *   red dye vaporizes where water evaporates (its `DyeFluidType`: the Nether) and is placed in the overworld;
 *   Mekanism's heavy water never vaporizes (`MekanismFluidType`), so it is placed in the Nether; water keeps 26.3's
 *   rule; each bucket is emptied.
 * - Cauldrons as NeoForge's `CauldronWrapper` (the fluid handler of every `CauldronFluidContent` block): an AE2 import
 *   bus with a speed card (8 operations of 125 mB) takes a full water cauldron's 1000 mB and leaves an empty cauldron,
 *   takes nothing from one two levels full (water moves only by whole buckets), and empties Create Dragons Plus'
 *   dragon's breath cauldron (four levels of 250 mB); a bus without a card (125 mB) takes nothing from a full one; an
 *   export bus with a card fills an empty cauldron with a bucket of water.
 * - NeoForge's milk (`NeoForgeMod.enableMilkFluid()`, a content mod's `milk_fluid`): a milk bucket goes into a fluid
 *   cell as 1000 mB of `minecraft:milk`.
 *
 * Needs ATM10_CARRIES: carries with slice 7a's declarations (`vaporizes`, `cauldron`, from the fluids-only probe),
 * its `mekanism` and `create_dragons_plus`. */
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../ae2/tests/world/harness';
import { root } from '../../../ae2/scripts/common';

const carries = resolve(process.env.ATM10_CARRIES ?? '');
for (const name of ['mekanism', 'create_dragons_plus']) assert(existsSync(join(carries, name, 'horizon-mod.toml')), `ATM10_CARRIES has no ${name}`);
const declaration = (carry: string, fluid: string) => {
  const [ns, path] = fluid.split(':');
  return JSON.parse(readFileSync(join(carries, carry, `data/${ns}/horizon/fluid/${path}.json`), 'utf8'));
};
// Java's values, as the oracle wrote them.
assert.equal(declaration('mekanism', 'mekanism:heavy_water').vaporizes, 'never');
assert.equal(declaration('create_dragons_plus', 'create_dragons_plus:red_dye').vaporizes, 'ultrawarm');
assert.deepEqual(declaration('create_dragons_plus', 'create_dragons_plus:dragon_breath').cauldron,
  { block: 'create_dragons_plus:dragon_breath_cauldron', level_property: 'level', max_level: 4, total_amount: 1000 });

// A content mod that turns NeoForge's milk on, as Create's or Mekanism's `NeoForgeMod.enableMilkFluid()` does, with
// NeoForge's own c:milk.
const tests = join(root, '.local/world-tests'); mkdirSync(tests, { recursive: true });
const milkMod = mkdtempSync(join(tests, 'mod-fluid-vaporize-cauldron-milk-'));
writeFileSync(join(milkMod, 'horizon-mod.toml'), 'id = "milk_on"\nname = "Milk on"\nversion = "1.0.0"\napi = "0.2"\nside = "both"\ncapabilities = ["registries"]\nmilk_fluid = true\n');
mkdirSync(join(milkMod, 'data/c/tags/fluid'), { recursive: true });
writeFileSync(join(milkMod, 'data/c/tags/fluid/milk.json'), JSON.stringify({ values: [{ id: 'minecraft:milk', required: false }, { id: 'minecraft:flowing_milk', required: false }] }));

const fixture = new WorldFixture('mod-fluid-vaporize-cauldron', 'server-thread',
  { devMods: [join(carries, 'mekanism'), join(carries, 'create_dragons_plus'), milkMod] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => line.includes('Test passed'));
const isBlock = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
const block = async (pos: string) => (await fixture.raw(`getblock ${pos}`)).block as string;
async function gridFluid(host: string, fluid: string): Promise<number> {
  const status = await fixture.ae(`fluid grid status ${host}`);
  return (status.entries ?? []).filter((e: any) => e.fluid === fluid).reduce((n: number, e: any) => n + e.amount_mb, 0);
}
/** Steps the held server in `step`-tick runs until `done`, at most `limit` ticks. */
async function stepUntil(done: () => Promise<boolean>, what: string, step = 10, limit = 1200): Promise<number> {
  for (let ticks = 0; ticks <= limit; ticks += step) {
    if (await done()) return ticks;
    await fixture.stepServer(step);
  }
  throw new Error(`${what}: not after ${limit} ticks`);
}
/** Empties `bucket` from the hand onto the top of the floor block under `pos`. */
async function pour(bucket: string, pos: string, floor: string) {
  await fixture.replace('hotbar.0', bucket); await fixture.select(0, bucket);
  await fixture.raw(`look-at block ${floor} up`); await fixture.raw('use-item');
  await until(() => fixture.raw('inventory'), v => v.hotbar?.[0]?.item === 'minecraft:bucket', `${bucket} is emptied`);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-6, -6], [-6, 8], [8, -6], [8, 8]] as const) {
    await until(async () => passed(await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)), v => v, 'fixture corner loaded', 30000);
  }
  await fixture.command(`fill ${at(-6, 0, -6)} ${at(8, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-6, -1, -6)} ${at(8, -1, 8)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');

  // The overworld: the dye is placed (no water evaporates here).
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5} 180 60`);
  await pour('create_dragons_plus:red_dye_bucket', at(0), at(0, -1));
  await until(() => isBlock(at(0), 'create_dragons_plus:red_dye[level=0]'), v => v, 'the dye is placed in the overworld');
  await fixture.command(`setblock ${at(0)} minecraft:air`);
  fixture.mark('in the overworld the red dye bucket places a source of create_dragons_plus:red_dye');

  // Cauldrons: an import bus each on a full water cauldron (east of H1), one two levels full (above H1), a full
  // dragon's breath cauldron (east of H2), each with a speed card, and one without on a full water cauldron (above
  // H2); a fluid cell to take what comes.
  const power = at(-1), h1 = at(0), drive = at(-1, 0, 1), h2 = at(0, 0, 1), h3 = at(0, 0, 2);
  const full = at(1), part = at(0, 1, 0), breath = at(1, 0, 1), slow = at(0, 1, 1), empty = at(1, 0, 2);
  await fixture.command('gamemode creative @s');
  await fixture.command(`setblock ${power} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:fluid_storage_cell_4k`);
  for (const host of [h1, h2, h3]) await fixture.command(`setblock ${host} ae2:cable_bus`);
  await fixture.hold();
  await fixture.command(`setblock ${full} minecraft:water_cauldron[level=3]`);
  await fixture.command(`setblock ${part} minecraft:water_cauldron[level=2]`);
  await fixture.command(`setblock ${breath} create_dragons_plus:dragon_breath_cauldron[level=4]`);
  await fixture.command(`setblock ${slow} minecraft:water_cauldron[level=3]`);
  await fixture.command(`setblock ${empty} minecraft:cauldron`);
  await fixture.stepServer(2);
  await fixture.hold(false);
  const attach = async (item: string, host: string, face: string) => {
    await fixture.replace('hotbar.0', item); await fixture.select(0, item);
    assert.equal((await fixture.ae(`grid part ${host} ${face}`)).attached, true, `${item} on ${host} ${face}`);
  };
  const card = async (host: string, face: string) => {
    await fixture.replace('hotbar.0', 'ae2:speed_card'); await fixture.select(0, 'ae2:speed_card');
    await fixture.ae(`bus install ${host} ${face}`);
    assert.equal((await fixture.ae(`bus status ${host} ${face}`)).state.speed_cards, 1, `a speed card in ${host} ${face}`);
  };
  // The inventory moves only while the server runs: the buses go in on a running server, so the full cauldrons may
  // start draining before the last one is in (what is checked is where they end).
  for (const [host, face] of [[h1, 'east'], [h1, 'up'], [h2, 'east']]) {
    await attach('ae2:import_bus', host, face); await card(host, face);
  }
  await attach('ae2:import_bus', h2, 'up');
  await until(() => fixture.ae(`grid status ${h1}`), s => s.online && s.powered, 'the network is online', 30000);

  await fixture.hold();
  try {
    log.import_ticks = await stepUntil(async () => (await gridFluid(h1, 'minecraft:water')) === 1000
      && (await gridFluid(h1, 'create_dragons_plus:dragon_breath')) === 1000, 'the import buses empty the full cauldrons');
  } catch (error) {
    log.grid = (await fixture.ae(`fluid grid status ${h1}`)).entries;
    log.bus = await fixture.ae(`bus status ${h1} east`);
    log.blocks = [await block(full), await block(breath), await block(part), await block(slow)];
    throw error;
  }
  assert.match(await block(full), /^minecraft:cauldron$/, 'a drained water cauldron is an empty cauldron');
  assert.match(await block(breath), /^minecraft:cauldron$/, 'a drained dragon\'s breath cauldron is an empty cauldron');
  fixture.mark('an import bus drains a full water cauldron and a full dragon\'s breath cauldron (1000 mB each) to empty cauldrons');
  await fixture.stepServer(100);
  assert.ok(await isBlock(part, 'minecraft:water_cauldron[level=2]'), `two levels of water stay: ${await block(part)}`);
  assert.ok(await isBlock(slow, 'minecraft:water_cauldron[level=3]'), `a bus of 125 mB takes no bucket: ${await block(slow)}`);
  assert.equal(await gridFluid(h1, 'minecraft:water'), 1000, 'no part of a bucket left a cauldron');
  fixture.mark('a cauldron two levels full gives nothing, nor a full one to a bus without a card: water moves only by whole buckets');
  await fixture.hold(false);

  // An export bus (water, a speed card) fills the empty cauldron with the bucket it has.
  await attach('ae2:export_bus', h3, 'east'); await card(h3, 'east');
  await fixture.replace('hotbar.0', 'minecraft:water_bucket'); await fixture.select(0, 'minecraft:water_bucket');
  await fixture.ae(`bus filter_fluid ${h3} east 0`);
  await fixture.command('item replace entity @s hotbar.0 with minecraft:air');
  await fixture.hold();
  log.export_ticks = await stepUntil(() => isBlock(empty, 'minecraft:water_cauldron[level=3]'), 'the export bus fills the empty cauldron');
  assert.equal(await gridFluid(h1, 'minecraft:water'), 0, 'the bucket left the cell');
  await fixture.hold(false);
  fixture.mark('an export bus fills an empty cauldron with 1000 mB of water: a full water cauldron');

  // NeoForge's milk: a milk bucket is 1000 mB of minecraft:milk to the cell.
  await fixture.replace('hotbar.1', 'minecraft:milk_bucket'); await fixture.select(1, 'minecraft:milk_bucket');
  assert.equal((await fixture.ae(`fluid grid insert ${h1}`)).inserted_mb, 1000, 'the milk bucket goes in');
  assert.equal(await gridFluid(h1, 'minecraft:milk'), 1000);
  fixture.mark('with milk turned on, a milk bucket goes into a fluid cell as 1000 mB of minecraft:milk');

  // The Nether: the dye vaporizes, heavy water is placed, water evaporates as in 26.3.
  await fixture.command('gamemode survival @s');
  await fixture.command('execute in minecraft:the_nether run tp @s 0.5 101 4.5 180 60');
  const n = (dx: number, dy = 100, dz = 0) => `${dx} ${dy} ${dz}`;
  await until(async () => passed(await fixture.command(`execute if loaded ${n(3, 100, 3)}`)), v => v, 'Nether fixture chunk loaded', 30000);
  await fixture.command(`fill ${n(-3, 99, -3)} ${n(4, 99, 6)} minecraft:stone`);
  await fixture.command(`fill ${n(-3, 100, -3)} ${n(4, 104, 6)} minecraft:air`);
  await fixture.command('tp @s 0.5 100 4.5 180 60');
  await pour('create_dragons_plus:red_dye_bucket', n(0), n(0, 99));
  assert.match(await block(n(0)), /^minecraft:air$/, 'the dye vaporizes: nothing placed');
  fixture.mark('in the Nether the red dye bucket vaporizes (DyeFluidType: where water evaporates) and is emptied');
  await pour('minecraft:water_bucket', n(0), n(0, 99));
  assert.match(await block(n(0)), /^minecraft:air$/, 'water evaporates');
  await pour('mekanism:heavy_water_bucket', n(0), n(0, 99));
  await until(() => isBlock(n(0), 'mekanism:heavy_water[level=0]'), v => v, 'heavy water is placed in the Nether');
  fixture.mark('in the Nether heavy water is placed (MekanismFluidType never vaporizes) and water evaporates as in 26.3');

  fixture.assertHidden(await fixture.raw('status'));
  console.log(`FLUIDS 7a ${JSON.stringify(log)}`);
  fixture.report('passed');
} catch (error) { console.log(`FLUIDS 7a ${JSON.stringify(log)}`); fixture.report('failed', error); throw error; }
finally { await fixture.close(); rmSync(milkMod, { recursive: true, force: true }); }
