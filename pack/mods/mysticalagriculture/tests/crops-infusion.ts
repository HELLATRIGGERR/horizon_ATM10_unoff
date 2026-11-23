/** Mystical Agriculture's crops and infusion altar, as a player uses them, on the carry of the player's own jar.
 *
 * - Inferium seeds plant their crop on Tertium farmland (the carry's `place_block`, from the oracle's facts).
 * - The crop grows on random ticks; ripe (age 7), a survival break drops what `InferiumCropBlock.getDrops` gives on a
 *   tier 3 farmland: 2 Inferium Essence and 1 seed (ATM10's `secondarySeedDrops = false`).
 * - A Growth Accelerator gets its timer.
 * - The infusion altar, its eight pedestals filled by right clicks and a redstone signal make ATM10's
 *   `mysticalagriculture:augment/absorption_i` (an Unattuned Augment, 4 golden apples, 4 Inferium Essence) in
 *   100 ticks: the pedestals are emptied and a right click gives the Absorption I Augment.
 *
 * - A seed recipe, whose ingredients are MA's own `mysticalagriculture:crop_component` (resolved from the crop table):
 *   `mysticalagriculture:seed/infusion/stone` takes a Prosperity Seed Base on the altar (the resource crops' crafting
 *   seed), 4 Stone (the crop's material) and 4 Inferium Essence (its tier's) on the pedestals and gives Stone Seeds.
 *   The game's log says how many of the carry's 123 infusion recipes the altar read.
 *
 * A screenshot of the ritual goes to the run folder (`infusion-altar.png`). Needs MA_CARRY (the carry);
 * ATM10_DIR, when set, gives ATM10's `config/mysticalagriculture-common.toml` and MysticalCustomization's files. */
import assert from 'node:assert/strict';
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const carry = resolve(process.env.MA_CARRY ?? '');
assert(existsSync(join(carry, 'horizon-mod.toml')), `MA_CARRY is not a carry: ${carry}`);
const fixture = new WorldFixture('ma-crops-infusion', 'server-thread', { devMods: [carry] });

// ATM10's settings the bundle reads; without an instance, the one that matters here.
const config = join(fixture.game, 'config');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/mysticalagriculture-common.toml'))) {
  cpSync(join(atm10, 'config/mysticalagriculture-common.toml'), join(config, 'mysticalagriculture-common.toml'));
  if (existsSync(join(atm10, 'config/mysticalcustomization'))) cpSync(join(atm10, 'config/mysticalcustomization'), join(config, 'mysticalcustomization'), { recursive: true });
} else {
  writeFileSync(join(config, 'mysticalagriculture-common.toml'), '[General]\n\tsecondarySeedDrops = false\n');
}

const count = (view: any, item: string) =>
  [...(view.hotbar ?? []), ...(view.main ?? view.inventory ?? [])].filter((s: any) => s?.item === item).reduce((n: number, s: any) => n + (s.count ?? 0), 0);
const age = (v: any) => Number(String(v.state ?? '').match(/age=(\d)/)?.[1] ?? -1);
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 2, y = Math.floor(py), z = Math.floor(pz);
  await fixture.command('time set day');
  await fixture.command('gamerule advance_time false');
  await fixture.command('gamerule random_tick_speed 0');
  await fixture.command(`fill ${x - 6} ${y} ${z - 6} ${x + 10} ${y + 4} ${z + 6} minecraft:air`);
  await fixture.command(`fill ${x - 6} ${y - 3} ${z - 6} ${x + 10} ${y - 1} ${z + 6} minecraft:stone`);

  // ---- The crop -------------------------------------------------------------------------------------------------
  const crop = `${x} ${y} ${z}`;
  await fixture.command(`setblock ${x} ${y - 1} ${z} mysticalagriculture:tertium_farmland[moisture=7]`);
  await fixture.command(`setblock ${x + 1} ${y - 1} ${z} minecraft:water`);
  await fixture.command(`setblock ${x} ${y - 2} ${z} mysticalagriculture:inferium_growth_accelerator`);
  // Light for the crop wherever the spawn is (a crop grows at light 9 or more).
  await fixture.command(`setblock ${x} ${y} ${z - 1} minecraft:glowstone`);
  await fixture.command(`tp @s ${x - 1.5} ${y} ${z + 0.5}`);
  await fixture.replace('hotbar.0', 'mysticalagriculture:inferium_seeds', 4);
  await fixture.select(0, 'mysticalagriculture:inferium_seeds');
  await fixture.raw(`interact ${x} ${y - 1} ${z} up`);
  const block = await until(() => fixture.raw(`block ${crop}`), v => String(v.state ?? v.block).startsWith('mysticalagriculture:inferium_crop'), 'the seed plants its crop');
  log.planted = block.state;
  assert.equal(age(block), 0, `a new crop: ${JSON.stringify(block)}`);
  fixture.mark(`Inferium seeds plant ${block.state} on Tertium farmland`);

  log.accelerator = await saved(`${x} ${y - 2} ${z}`);
  assert.match(String(log.accelerator), /Cooldown: ?\d+/, 'the accelerator has its timer');
  fixture.mark('the Growth Accelerator has its block entity and timer');

  await fixture.command('gamerule random_tick_speed 1000');
  // Growth on random ticks is the wheat template's odds; two ages show it, then the crop is set ripe.
  const grown = await until(() => fixture.raw(`block ${crop}`), v => age(v) >= 2, 'the crop grows on random ticks', 170000);
  await fixture.command('gamerule random_tick_speed 0');
  log.grown = grown.state;
  fixture.mark(`the crop grows on random ticks (${grown.state})`);
  await fixture.command(`setblock ${crop} mysticalagriculture:inferium_crop[age=7]`);

  await fixture.command('gamemode survival');
  const broke = await fixture.raw(`break ${crop}`);
  log.harvest = broke.drops;
  assert.equal(broke.block_before, 'mysticalagriculture:inferium_crop[age=7]', JSON.stringify(broke));
  assert.deepEqual(broke.drops, { 'mysticalagriculture:inferium_essence': 2, 'mysticalagriculture:inferium_seeds': 1 },
    `Inferium on tier 3 farmland: ${JSON.stringify(broke.drops)}`);
  fixture.mark('the ripe crop drops 2 Inferium Essence and 1 seed on Tertium farmland');
  await fixture.command('gamemode creative');

  // ---- The infusion ritual ---------------------------------------------------------------------------------------
  const [ax, az] = [x + 5, z];
  const altar = `${ax} ${y} ${az}`;
  const offsets = [[3, 0], [0, 3], [-3, 0], [0, -3], [2, 2], [2, -2], [-2, 2], [-2, -2]];
  await fixture.command(`setblock ${altar} mysticalagriculture:infusion_altar`);
  for (const [dx, dz] of offsets) await fixture.command(`setblock ${ax + dx} ${y} ${az + dz} mysticalagriculture:infusion_pedestal`);
  await fixture.command(`tp @s ${ax + 0.5} ${y + 1} ${az + 0.5}`);
  await fixture.replace('hotbar.0', 'mysticalagriculture:unattuned_augment', 1);
  await fixture.replace('hotbar.1', 'minecraft:golden_apple', 4);
  await fixture.replace('hotbar.2', 'mysticalagriculture:inferium_essence', 4);
  await fixture.select(0, 'mysticalagriculture:unattuned_augment');
  await fixture.raw(`interact ${altar} up`);
  await until(() => saved(altar), s => s.includes('mysticalagriculture:unattuned_augment'), 'the augment goes on the altar');
  for (const [i, [dx, dz]] of offsets.entries()) {
    const item = i % 2 === 0 ? 'minecraft:golden_apple' : 'mysticalagriculture:inferium_essence';
    await fixture.select(i % 2 === 0 ? 1 : 2, item);
    const at = `${ax + dx} ${y} ${az + dz}`;
    await fixture.raw(`interact ${at} up`);
    await until(() => saved(at), s => s.includes(item), `${item} on the pedestal at ${at}`);
  }
  const held = await fixture.raw('inventory');
  assert.equal(count(held, 'minecraft:golden_apple') + count(held, 'mysticalagriculture:inferium_essence'), 0, 'each right click put one item on');
  fixture.mark('right clicks put the augment on the altar and one item on each pedestal');

  // From a block above and behind the ring, so no pedestal hides the altar; chat lines fade after 200 ticks, so the
  // command feedback is gone from the picture.
  await fixture.command(`setblock ${ax - 5} ${y + 3} ${az + 5} minecraft:stone`);
  await fixture.command(`tp @s ${ax - 5 + 0.5} ${y + 4} ${az + 5 + 0.5} -135 38`);
  await Bun.sleep(12000);
  const shot = join(fixture.run, 'infusion-altar.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await Bun.sleep(1500);
  assert.match(await saved(altar), /Progress: ?0/, 'no signal, no progress');

  await fixture.command(`setblock ${ax} ${y - 1} ${az} minecraft:redstone_block`);
  const done = await until(() => saved(altar), s => s.includes('mysticalagriculture:absorption_i_augment'), 'the altar crafts', 30000);
  log.altar = done;
  assert(!done.includes('mysticalagriculture:unattuned_augment'), `the augment is used: ${done}`);
  for (const [dx, dz] of offsets) {
    const left = await saved(`${ax + dx} ${y} ${az + dz}`);
    assert(!/golden_apple|inferium_essence/.test(left), `the pedestal is emptied: ${left}`);
  }
  fixture.mark('a redstone signal starts the ritual: the Absorption I Augment, the pedestals emptied');

  await fixture.command(`tp @s ${ax + 0.5} ${y + 1} ${az + 0.5}`);
  await fixture.raw(`interact ${altar} up`);
  await until(() => fixture.raw('inventory'), v => count(v, 'mysticalagriculture:absorption_i_augment') === 1, 'a right click takes the output');
  assert(!(await saved(altar)).includes('absorption_i_augment'), 'the output slot is empty');
  fixture.mark('a right click gives the Absorption I Augment');

  // ---- A seed recipe: crop_component ingredients ---------------------------------------------------------------------
  // The signal is taken away first, so the altar starts when the ring is full, as in the first ritual.
  await fixture.command(`setblock ${ax} ${y - 1} ${az} minecraft:stone`);
  await fixture.replace('hotbar.0', 'mysticalagriculture:prosperity_seed_base', 1);
  await fixture.replace('hotbar.1', 'minecraft:stone', 4);
  await fixture.replace('hotbar.2', 'mysticalagriculture:inferium_essence', 4);
  await fixture.select(0, 'mysticalagriculture:prosperity_seed_base');
  await fixture.raw(`interact ${altar} up`);
  await until(() => saved(altar), s => s.includes('mysticalagriculture:prosperity_seed_base'), 'the seed base goes on the altar');
  for (const [i, [dx, dz]] of offsets.entries()) {
    const item = i % 2 === 0 ? 'minecraft:stone' : 'mysticalagriculture:inferium_essence';
    await fixture.select(i % 2 === 0 ? 1 : 2, item);
    const at = `${ax + dx} ${y} ${az + dz}`;
    await fixture.raw(`interact ${at} up`);
    await until(() => saved(at), s => s.includes(item), `${item} on the pedestal at ${at}`);
  }
  await Bun.sleep(1500);
  assert.match(await saved(altar), /Progress: ?0/, 'no signal, no progress');
  await fixture.command(`setblock ${ax} ${y - 1} ${az} minecraft:redstone_block`);
  const seeds = await until(() => saved(altar), s => s.includes('mysticalagriculture:stone_seeds'), 'the altar crafts the seed', 30000);
  log.seedAltar = seeds;
  assert(!seeds.includes('prosperity_seed_base'), `the seed base is used: ${seeds}`);
  for (const [dx, dz] of offsets) {
    const left = await saved(`${ax + dx} ${y} ${az + dz}`);
    assert(!/stone|inferium_essence/.test(left), `the pedestal is emptied: ${left}`);
  }
  await fixture.raw(`interact ${altar} up`);
  await until(() => fixture.raw('inventory'), v => count(v, 'mysticalagriculture:stone_seeds') === 1, 'a right click takes the Stone Seeds');
  fixture.mark('the seed recipe (crop_component ingredients) crafts Stone Seeds on the altar');

  // How much of the carry's 123 infusion recipes the altar read (logged when it first looked at them).
  const gameLog = readFileSync(join(fixture.run, 'game-1.log'), 'utf8');
  const books = [...gameLog.matchAll(/\[mysticalagriculture\] (\d+) infusion recipes, (\d+) left out/g)].map(m => [Number(m[1]), Number(m[2])]);
  assert(books.length > 0, 'the altar logs its recipe count');
  const [read, left] = books[books.length - 1];
  log.recipes = { read, left };
  assert.equal(read + left, 123, `the carry's infusion recipes: ${read} read, ${left} left out`);
  assert(read > 57, `the seed recipes load (${read} of 123 read; 57 before)`);
  fixture.mark(`the altar read ${read} of the carry's 123 infusion recipes (${left} left out)`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MA crops-infusion', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MA crops-infusion', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
