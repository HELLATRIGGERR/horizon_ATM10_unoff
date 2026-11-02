/** Powah's Furnator, as the `powah` native bundle and its content mod make it.
 *
 * - The content: every block of the carry can be placed, the recipes are known, the seven tiers answer `/powah probe`
 *   with the numbers of ATM10's `config/powah.json5`.
 * - A starter burns one coal for 600 ticks beside an AE2 Energy Acceptor: 80 FE a tick, 40 AE a tick (2 FE = 1 AE),
 *   24,000 AE from the coal's 48,000 FE, then nothing. AE2 reads it as an `IEnergyStorage` that gives and never takes.
 * - Each tier burns a coal at its own generation, and pushes its own transfer rate from a full buffer, per side.
 * - Hoppers put fuel into the fuel slot and leave what doesn't burn; a lava bucket leaves its bucket.
 * - Energy, fuel and burn progress survive a save and a restart; it keeps burning afterwards.
 * - A player's break with a pickaxe drops an item with the energy and the fuel burning, as
 *   `powah:stored_block_entity_state`; placing that item gives them back to the block. A command's break drops a
 *   plain item, as an explosion's does in Java.
 *
 * Energy is read from the saved compound (`data get block`, Powah's own keys), AE from the energy cell. A picture of
 * the seven tiers, the left ones burning, goes to the run folder (`furnators.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod from `powah-content` (`POWAH_CONTENT_DIR`);
 * `pack/scripts/powah.ts world` sets both. With `ATM10_DIR` the world reads ATM10's own `config/powah.json5`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.POWAH_CONTENT_DIR ?? join(root, '../.build/content/powah'));
const fixture = new WorldFixture('powah-furnator', 'server-thread', { devMods: [content] });
const atm10 = process.env.ATM10_DIR;
const fromAtm10 = !!atm10 && existsSync(join(atm10, 'config/powah.json5'));
if (fromAtm10) {
  const config = join(fixture.game, 'config');
  mkdirSync(config, { recursive: true });
  copyFileSync(join(atm10!, 'config/powah.json5'), join(config, 'powah.json5'));
}

// ATM10's config/powah.json5, which are also the mod's defaults.
const TIERS = [
  { name: 'starter', capacity: 20_000, transfer: 240, generation: 80 },
  { name: 'basic', capacity: 80_000, transfer: 480, generation: 160 },
  { name: 'hardened', capacity: 200_000, transfer: 1_600, generation: 400 },
  { name: 'blazing', capacity: 800_000, transfer: 4_000, generation: 1_000 },
  { name: 'niotic', capacity: 2_000_000, transfer: 8_000, generation: 2_000 },
  { name: 'spirited', capacity: 8_000_000, transfer: 32_000, generation: 8_000 },
  { name: 'nitro', capacity: 40_000_000, transfer: 160_000, generation: 20_000 },
];
const COAL_FE = 48_000;

/** The block entity's compound on the server, as `data get block` prints it. */
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => {
  const found = nbt.match(new RegExp(`${name}: ?(-?[\\d.]+)`));
  return found ? Number(found[1]) : 0;
};
const stored = async (pos: string) => field(await saved(pos), 'energy_stored_main_energy');
/** What a Furnator keeps: energy, what the fuel still gives and gave, burning, and how much coal waits. */
async function state(pos: string) {
  const nbt = await saved(pos);
  const coal = nbt.match(/\{[^{}]*minecraft:coal[^{}]*\}/)?.[0];
  return {
    energy: field(nbt, 'energy_stored_main_energy'),
    carbon: field(nbt, 'carbon_ticks'),
    carbon_max: field(nbt, 'carbon_max_ticks'),
    burning: field(nbt, 'burning') !== 0,
    coal: coal ? field(coal, 'count') : 0,
  };
}
/** Whether the block there is in a state, on the server. */
async function isState(pos: string, block: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${block}`);
  return (reply.chat ?? []).join(' ').includes('Test passed');
}
/** `/powah probe` as the player types it; its answer comes back in chat. Once per position. */
async function probe(pos: string): Promise<any> {
  const needle = `"pos":[${pos.split(' ').join(',')}]`;
  await fixture.raw(`chat /powah probe ${pos}`);
  const found = (lines: string[] = []) => lines.findLast(text => text.startsWith('POWAH {') && text.includes(needle));
  const view = await until(() => fixture.raw('chat-log 20'), v => !!found(v.chat), `/powah probe ${pos} answers in chat`);
  const value = JSON.parse(found(view.chat)!.slice(6));
  assert(!value.error, JSON.stringify(value));
  return value;
}
/** AE in the grid of the energy cell at `cell`. */
async function gridAe(cell: string): Promise<number> {
  const s = await fixture.ae(`power ${cell} north`);
  return s.energy + (s.energy_service?.buffer ?? 0);
}
const nearly = (actual: number, expected: number, what: string) =>
  assert(Math.abs(actual - expected) < 1, `${what}: ${actual}, expected ${expected}`);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.ceil(py) + 3, z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(12, 4, 12)} minecraft:air`);

  // P0: every block of the carry places as itself, and the recipes are there.
  const blocks = readdirSync(join(content, 'data/powah/horizon/block')).filter(f => f.endsWith('.json')).map(f => f.slice(0, -5));
  log.blocks = blocks.length;
  assert.equal(blocks.length, 106, 'blocks: 92 from the carry, 7 cables and 7 transmitters added');
  const test = at(-2, 0, -2);
  for (const block of blocks) {
    await fixture.command(`setblock ${test} powah:${block}`);
    assert(await isState(test, `powah:${block}`), `powah:${block} places`);
  }
  await fixture.command(`setblock ${test} minecraft:air`);
  const recipes = (await fixture.command(`recipe give @s powah:crafting/furnator_starter`)).chat ?? [];
  assert(recipes.join(' ').includes('Unlocked'), `the recipe is known: ${JSON.stringify(recipes)}`);
  fixture.mark(`the carry loads: ${blocks.length} blocks place, the Furnator's recipe is known`);

  // The tiers, with the world running, as the player's own command answers.
  for (const [i, tier] of TIERS.entries()) {
    const pos = at(i * 2, 0, 10);
    await fixture.command(`setblock ${pos} powah:furnator_${tier.name}`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === `powah:furnator_${tier.name}`, `${tier.name} reaches the client`);
    const value = await probe(pos);
    assert.equal(value.tier, tier.name);
    assert.deepEqual([value.capacity, value.transfer, value.generation], [tier.capacity, tier.transfer, tier.generation], tier.name);
    assert.equal(value.energy_per_fuel_tick, 30);
    assert.equal(value.from_file, fromAtm10);
    assert.equal(value.facing, 'north');
    assert.equal(value.lit, 'false');
  }
  fixture.mark(`seven tiers answer with their numbers${fromAtm10 ? ' from ATM10\'s powah.json5' : ''}`);

  // Furnator north of an acceptor north of a dense cell: the row is along z.
  const furnator = at(0, 0, 2), acceptor = at(0, 0, 1), cell = at(0);
  await fixture.command(`setblock ${cell} ae2:dense_energy_cell`);
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`);
  await fixture.command(`setblock ${furnator} powah:furnator_starter`);
  await until(() => fixture.ae(`power ${cell} north`), s => s.energy_service?.members > 0, 'the acceptor and cell form a grid', 30000);
  await fixture.hold();

  // AE2 sees what Java's capability shows: it gives, it takes nothing.
  const fe = (await fixture.ae(`power ${furnator} north`)).fe;
  log.furnator_fe = fe;
  assert.deepEqual(fe, { stored: 0, capacity: 20_000, can_receive: false, can_extract: true });
  fixture.mark('AE2 reads the Furnator as IEnergyStorage: 0 of 20,000 FE, gives and takes nothing');

  // One coal in the fuel slot, as a hopper or a click would put it.
  await fixture.command(`item replace block ${furnator} container.1 with minecraft:coal 1`);
  const ae0 = await gridAe(cell);
  const perTick: number[] = [];
  let last = ae0;
  for (let i = 0; i < 5; i++) {
    await fixture.stepServer(1);
    const now = await gridAe(cell);
    perTick.push(Math.round((now - last) * 1000) / 1000); last = now;
  }
  log.ae_per_tick = perTick;
  for (const gain of perTick) nearly(gain, 40, `80 FE a tick are 40 AE: ${perTick}`);
  assert.deepEqual(await state(furnator), { energy: 0, carbon: COAL_FE - 400, carbon_max: COAL_FE, burning: true, coal: 0 });
  assert(await isState(furnator, 'powah:furnator_starter[lit=true]'), 'burning shows lit');
  fixture.mark('a starter makes 80 FE a tick, which leaves the same tick: the grid gains 40 AE a tick');

  await fixture.stepServer(595);
  const burnt = await gridAe(cell);
  log.ae_after_600_ticks = burnt - ae0;
  nearly(burnt - ae0, COAL_FE / 2, 'the coal, 600 ticks');
  assert.deepEqual(await state(furnator), { energy: 0, carbon: 0, carbon_max: 0, burning: true, coal: 0 });
  await fixture.stepServer(1);
  assert.deepEqual(await state(furnator), { energy: 0, carbon: 0, carbon_max: 0, burning: false, coal: 0 });
  assert(await isState(furnator, 'powah:furnator_starter[lit=false]'), 'out of fuel shows unlit');
  await fixture.stepServer(20);
  nearly(await gridAe(cell), burnt, 'no fuel, no energy');
  fixture.mark(`one coal gave 48,000 FE in 600 ticks: ${Math.round(burnt - ae0)} AE, then it stops`);

  // Each tier: a coal at its generation, then a full buffer at its transfer rate.
  const rates: Record<string, unknown> = {};
  for (const tier of TIERS) {
    await fixture.command(`setblock ${furnator} powah:furnator_${tier.name}`);
    await fixture.command(`item replace block ${furnator} container.1 with minecraft:coal 1`);
    const before = await gridAe(cell);
    await fixture.stepServer(1);
    const first = await gridAe(cell) - before;
    nearly(first, tier.generation / 2, `${tier.name} generation`);
    const ticks = Math.ceil(COAL_FE / tier.generation);
    if (ticks > 1) await fixture.stepServer(ticks - 1);
    const coal = await gridAe(cell) - before;
    nearly(coal, COAL_FE / 2, `${tier.name}: a coal in ${ticks} ticks`);
    const s = await state(furnator);
    assert.deepEqual([s.energy, s.carbon], [0, 0], `${tier.name} ${JSON.stringify(s)}`);

    await fixture.command(`data merge block ${furnator} {energy_stored_main_energy:${tier.capacity}L}`);
    const full = await gridAe(cell);
    await fixture.stepServer(1);
    const pushed = await gridAe(cell) - full;
    nearly(pushed, tier.transfer / 2, `${tier.name} transfer`);
    assert.equal(await stored(furnator), tier.capacity - tier.transfer);
    rates[tier.name] = { ticks_per_coal: ticks, generation_ae: first, transfer_ae: pushed };
  }
  log.rates = rates;
  fixture.mark('each of the seven tiers burns a coal at its generation and pushes its transfer rate');

  // The rate is per side: a second acceptor and cell on the other side.
  const acceptor2 = at(0, 0, 3), cell2 = at(0, 0, 4);
  await fixture.command(`setblock ${furnator} powah:furnator_starter`);
  await fixture.command(`setblock ${cell2} ae2:energy_cell`);
  await fixture.command(`setblock ${acceptor2} ae2:energy_acceptor`);
  await fixture.hold(false);
  await until(() => fixture.ae(`power ${cell2} north`), s => s.energy_service?.members > 0, 'the second grid forms', 30000);
  await fixture.hold();
  await fixture.command(`data merge block ${furnator} {energy_stored_main_energy:20000L}`);
  const sides = [await gridAe(cell), await gridAe(cell2)];
  const before = await stored(furnator);
  await fixture.stepServer(1);
  const taken = before - await stored(furnator);
  log.two_sides = { taken, north: await gridAe(cell2) - sides[1], south: await gridAe(cell) - sides[0] };
  assert.equal(taken, 480, 'two sides take 240 FE each');
  nearly(await gridAe(cell) - sides[0], 120, 'one side');
  nearly(await gridAe(cell2) - sides[1], 120, 'the other side');
  fixture.mark('two neighbours take the tier\'s rate each: 240 + 240 FE out of a starter');

  // Hoppers: fuel in, the rest stays in the hopper. Java's trace: down from a hopper above, nothing but fuel.
  const hopped = at(8, 0, 2), hopper = at(8, 1, 2);
  await fixture.command(`setblock ${hopped} powah:furnator_starter`);
  await fixture.command(`setblock ${hopper} minecraft:hopper[facing=down]`);
  await fixture.command(`data merge block ${hopper} {Items:[{Slot:0b,id:"minecraft:dirt",count:1},{Slot:1b,id:"minecraft:coal",count:2}]}`);
  await fixture.stepServer(40);
  const fed = await state(hopped);
  const hopperLeft = await saved(hopper);
  log.hopper = { fed, hopper: hopperLeft };
  assert(hopperLeft.includes('minecraft:dirt'), `the dirt stays in the hopper: ${hopperLeft}`);
  assert(!hopperLeft.includes('minecraft:coal'), `the coal leaves the hopper: ${hopperLeft}`);
  assert(!(await saved(hopped)).includes('minecraft:dirt'), 'no dirt in the Furnator');
  assert.equal(fed.coal, 1, 'one coal burning, one waiting');
  assert.equal(fed.carbon_max, COAL_FE);
  await fixture.command(`setblock ${hopper} minecraft:air`);
  fixture.mark('a hopper feeds coal and leaves dirt behind');

  // A hopper below takes the waiting coal, and whatever sits in the charge slot (Java's traces Q and R).
  const pulled = at(8, 0, 9), below = at(8, -1, 9);
  await fixture.command(`setblock ${pulled} powah:furnator_starter`);
  await fixture.command(`item replace block ${pulled} container.1 with minecraft:coal 3`);
  await fixture.stepServer(1);
  await fixture.command(`setblock ${below} minecraft:hopper[facing=down]`);
  await fixture.stepServer(11);
  const afterPull = await state(pulled);
  const belowHolds = await saved(below);
  log.hopper_below = { afterPull, belowHolds };
  assert.equal(afterPull.coal, 0, `the hopper below takes the waiting coal: ${JSON.stringify(afterPull)} ${belowHolds}`);
  assert.equal(afterPull.carbon, COAL_FE - 12 * 80, 'the coal burning goes on');
  assert(/count: ?2\b/.test(belowHolds) && belowHolds.includes('minecraft:coal'), belowHolds);
  fixture.mark('a hopper below takes the waiting coal; the coal burning goes on');

  // A lava bucket burns 20,000 fuel ticks and leaves its bucket in the slot.
  const bucket = at(10, 0, 2);
  await fixture.command(`setblock ${bucket} powah:furnator_starter`);
  await fixture.command(`item replace block ${bucket} container.1 with minecraft:lava_bucket 1`);
  await fixture.stepServer(2);
  const lava = await saved(bucket);
  log.lava = lava;
  assert(/minecraft:bucket/.test(lava), `the empty bucket stays: ${lava}`);
  assert.equal(field(lava, 'carbon_max_ticks'), 600_000, '20,000 fuel ticks of lava');
  fixture.mark('a lava bucket gives 600,000 FE of burning and leaves its bucket');

  // Through a save and a restart: energy, burn progress, the coal waiting, the nitro's big number, the grid's push.
  const keep = at(8, 0, 6), big = at(10, 0, 6);
  await fixture.command(`setblock ${keep} powah:furnator_starter`);
  await fixture.command(`item replace block ${keep} container.1 with minecraft:coal 3`);
  await fixture.command(`setblock ${big} powah:furnator_nitro`);
  await fixture.command(`data merge block ${big} {energy_stored_main_energy:39000000L}`);
  await fixture.stepServer(10);
  const kept = await state(keep);
  assert.deepEqual(kept, { energy: 800, carbon: COAL_FE - 800, carbon_max: COAL_FE, burning: true, coal: 2 });
  await fixture.command(`item replace block ${furnator} container.1 with minecraft:coal 1`);
  await fixture.hold(false);
  // The world runs until the save: read after the restart, and compare with the tick count it ran.
  await fixture.restart();
  await fixture.hold();
  const back = await state(keep);
  log.after_restart = back;
  assert(back.burning, 'still burning');
  assert.equal(back.energy + back.carbon, COAL_FE, `no energy lost or made: ${JSON.stringify(back)}`);
  assert(back.energy >= 800, `energy kept: ${back.energy}`);
  assert.equal(back.carbon_max, COAL_FE);
  assert.equal(back.coal, 2, 'the waiting coal kept');
  assert.equal(await stored(big), 39_000_000, 'a nitro keeps its 39,000,000 FE');
  assert(await isState(keep, 'powah:furnator_starter[lit=true]'), 'still lit');
  await fixture.stepServer(5);
  const on = await state(keep);
  assert.deepEqual([on.energy, on.carbon], [back.energy + 400, back.carbon - 400], 'it keeps burning after the restart');
  fixture.mark(`energy ${back.energy}, burn progress ${back.carbon} and the waiting coal survive a save and a restart; it keeps burning`);

  // And it pushes into the grid again.
  await fixture.hold(false);
  await until(() => fixture.ae(`power ${cell} north`), s => s.energy_service?.members > 0, 'the grid forms after the restart', 30000);
  await fixture.hold();
  await fixture.command(`data merge block ${furnator} {energy_stored_main_energy:20000L}`);
  const again = [await gridAe(cell), await gridAe(cell2)];
  const start2 = await stored(furnator);
  await fixture.stepServer(1);
  assert.equal(start2 - await stored(furnator), 480);
  nearly(await gridAe(cell) - again[0], 120, 'the grid after a restart');
  fixture.mark('after the restart it pushes into both grids again');

  // A command's or an explosion's break, as in Java's trace: the fuel and the loot table's plain item.
  await fixture.command(`kill @e[type=minecraft:item,distance=..64]`);
  await fixture.command(`setblock ${keep} minecraft:air destroy`);
  const itemAt = async (pos: string, id: string, path = '') => (await fixture.command(`execute positioned ${pos} run data get entity @e[type=minecraft:item,nbt={Item:{id:"${id}"}},limit=1,sort=nearest,distance=..4] Item${path}`)).chat?.join(' ') ?? '';
  const plain = await itemAt(keep, 'powah:furnator_starter');
  log.plain_drop = plain;
  assert(plain.includes('powah:furnator_starter') && !plain.includes('stored_block_entity_state'), `a command's break keeps nothing: ${plain}`);
  assert(/\b2\b/.test(await itemAt(keep, 'minecraft:coal', '.count')), 'the waiting coal drops');
  await fixture.command(`kill @e[type=minecraft:item,distance=..64]`);
  fixture.mark('broken by a command it drops its fuel and a plain item, as in Java');

  // A player's break with a pickaxe, as in Powah's Block.playerDestroy: the item keeps the energy and the fuel burning.
  // A full one, so nothing moves while the player mines it.
  await fixture.hold(false);
  const mined = at(4, 0, 6), standAt = `${x + 4.5} ${y} ${z + 3.5}`;
  await fixture.command(`fill ${at(2, -1, 3)} ${at(6, -1, 8)} minecraft:stone`);
  await fixture.command(`setblock ${mined} powah:furnator_starter`);
  await fixture.command(`data merge block ${mined} {energy_stored_main_energy:20000L,carbon_ticks:1000.0d,carbon_max_ticks:48000.0d}`);
  await fixture.command(`tp @s ${standAt}`);
  await fixture.replace('hotbar.0', 'minecraft:diamond_pickaxe', 1);
  await fixture.raw('hotbar 1');
  await fixture.command('gamemode survival @s');
  await until(() => fixture.raw(`block ${mined}`), v => v.block === 'powah:furnator_starter', 'the Furnator reaches the client');
  await until(async () => (await saved(mined)), s => /energy_stored_main_energy: ?20000L/.test(s), 'the Furnator is full');
  await fixture.raw(`break ${mined}`);
  await until(() => fixture.raw(`block ${mined}`), v => v.block === 'minecraft:air', 'the player breaks it');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 6.5}`);
  const stacks = (v: any): any[] => [...(v.hotbar ?? []), ...(v.main ?? [])];
  const held = await until(() => fixture.raw('inventory'), v => stacks(v).some(s => s?.item === 'powah:furnator_starter'), 'the player picks the item up');
  const picked = stacks(held).find(s => s?.item === 'powah:furnator_starter');
  log.picked = picked;
  const kept3 = String(picked.components);
  assert(/stored_block_entity_state/.test(kept3) && /energy_stored_main_energy: ?20000L/.test(kept3) && /carbon_ticks: ?1000(\.0)?d/.test(kept3), `the item keeps the energy and the fuel burning: ${kept3}`);
  fixture.mark('broken by a player with a pickaxe, the item keeps 20,000 FE and the fuel burning as powah:stored_block_entity_state');

  // Placed again from the hand: the block has them back. It faces the player, as a furnace does.
  await fixture.command(`tp @s ${standAt} 0 30`);
  await fixture.raw(`hotbar ${picked.slot + 1}`);
  await fixture.raw(`place ${mined}`);
  await until(() => fixture.raw(`block ${mined}`), v => v.block === 'powah:furnator_starter', 'the item places');
  const placed = await until(() => state(mined), s => s.energy === 20_000, 'the placed block has the energy back');
  assert.deepEqual([placed.carbon, placed.carbon_max], [1000, COAL_FE], 'and the fuel burning');
  const facing = (await fixture.raw(`block ${mined}`)).state;
  log.placed = { placed, state: facing };
  assert(String(facing).includes('facing=north'), `it faces the player, who looks south: ${facing}`);
  await fixture.command('gamemode creative @s');
  fixture.mark(`the item placed again gives the block its ${placed.energy} FE and burning fuel back, facing the player`);

  // A picture of the seven tiers, burning from the left to the middle.
  try {
    for (const [i, tier] of TIERS.entries()) {
      if (i < 4) await fixture.command(`item replace block ${at(i * 2, 0, 10)} container.1 with minecraft:coal 64`);
      void tier;
    }
    await fixture.command(`fill ${at(-1, -1, 4)} ${at(13, -1, 14)} minecraft:stone`);
    await fixture.command(`fill ${at(-1, 0, 11)} ${at(13, 3, 16)} minecraft:air`);
    await fixture.command(`fill ${at(-1, -1, 11)} ${at(13, -1, 16)} minecraft:stone`);
    await fixture.command(`tp @s ${x + 6.5} ${y + 1} ${z + 14.5} 180 15`);
    await Bun.sleep(11000);
    const shot = join(fixture.run, 'furnators.png');
    await fixture.raw(`screenshot ${shot}`);
    log.screenshot = shot;
  } catch (error) { log.screenshot = `not taken: ${error}`; }

  fixture.assertHidden(await fixture.raw('status'));
  console.log('POWAH numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('POWAH numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
