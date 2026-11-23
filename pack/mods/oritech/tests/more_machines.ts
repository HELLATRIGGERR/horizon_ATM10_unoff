/** Oritech's centrifuge, fragment forge (the grinder recipes), foundry and generators, as the `oritech` native bundle and
 * the carry of the player's jar make them. The test world has no ATM10 config: the mod's defaults (the centrifuge takes
 * 64 FE a tick, the fragment forge 256, the foundry 128, the basic generator makes 32, the bio generator 64).
 *
 * - A centrifuge (one core above) turns a sculk block into enderic compound in 400 ticks at 64 FE/t.
 * - A fragment forge (seven cores) grinds a bone into 8 bone meal in 40 ticks at 256 FE/t.
 * - A foundry (three cores) melts two iron gems into 3 iron ingots in 80 ticks at 128 FE/t.
 * - A powered furnace (one core above) smelts cobblestone into stone in 100 ticks at 32 FE/t.
 * - A basic generator burns a coal and fills the pulverizer beside it; a bio generator (a core above) burns packed
 *   wheat and fills its own.
 *
 * Reads the saved compound (`data get block`, the bundle's own keys). Screenshots of the centrifuge's and the basic
 * generator's screens go to the run folder. Needs the staged native build and the content mod of the carry. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.ORITECH_CONTENT_DIR ?? join(root, '../.build/content/oritech'));
const fixture = new WorldFixture('oritech-more-machines', 'server-thread', { devMods: [content] });

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => {
  const found = nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?[\\d.]+)`));
  return found ? Number(found[1]) : 0;
};
const gametime = async () => Number(((await fixture.command('time query gametime')).chat ?? []).join(' ').match(/(\d+)/)?.[1]);
const energy = (nbt: string) => field(nbt, 'oritech.machine_energy');
const progress = (nbt: string) => field(nbt, 'oritech.machine_progress');

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 4} ${x + 24} ${y + 4} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 4} ${x + 24} ${y - 1} ${z + 4} minecraft:stone`);
  const place = async (pos: string, block: string) => {
    await fixture.command(`setblock ${pos} ${block}`);
  };

  // --- Centrifuge: sculk → enderic compound, 400 ticks at 64 FE/t; the core is above ---
  const cen = `${x} ${y} ${z}`;
  await place(cen, 'oritech:centrifuge_block[facing=north]');
  await place(`${x} ${y + 1} ${z}`, 'oritech:machine_core_1');
  await fixture.command(`data merge block ${cen} {"oritech.machine_energy":60000L}`);
  await fixture.command(`data modify block ${cen} Items set value [{id:"minecraft:sculk",count:1,Slot:0b}]`);
  const c0 = await gametime();
  let nbt = await until(() => saved(cen), s => progress(s) >= 20, 'the centrifuge starts', 30000);
  log.cenStart = { progress: progress(nbt), energy: energy(nbt) };
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${cen} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'centrifuge screen opens');
  await Bun.sleep(500);
  const shotCen = join(fixture.run, 'centrifuge-running.png');
  await fixture.raw(`screenshot ${shotCen}`);
  log.screenshotCentrifuge = shotCen;
  await fixture.raw('close');
  nbt = await until(() => saved(cen), s => s.includes('enderic_compound') && progress(s) === 0, 'the centrifuge finishes', 90000);
  const cenTicks = (await gametime()) - c0;
  log.cenTicks = cenTicks;
  assert(cenTicks >= 395 && cenTicks <= 460, `a centrifuge run takes about 400 ticks: ${cenTicks}`);
  assert.equal(energy(nbt), 60000 - 400 * 64, 'the energy of 400 ticks at 64 FE/t');
  assert(!nbt.includes('sculk'), 'the sculk was used');
  fixture.mark(`the centrifuge made enderic compound in ${cenTicks} ticks, ${energy(nbt)} FE left`);

  // --- Fragment forge: bone → 8 bone meal, 40 ticks at 256 FE/t; seven cores in a 2x2x2 cube ---
  const fx = x + 4;
  const forge = `${fx} ${y} ${z}`;
  await place(forge, 'oritech:fragment_forge_block[facing=north]');
  for (const [dx, dy, dz] of [[0, 1, 0], [1, 0, 0], [1, 1, 0], [1, 0, 1], [1, 1, 1], [0, 0, 1], [0, 1, 1]]) {
    await place(`${fx + dx} ${y + dy} ${z + dz}`, 'oritech:machine_core_1');
  }
  await fixture.command(`data merge block ${forge} {"oritech.machine_energy":50000L}`);
  nbt = await until(() => saved(forge), s => s.includes('connectedCores') && !/connectedCores: ?\[\]/.test(s), 'the fragment forge assembles', 20000);
  await fixture.command(`data modify block ${forge} Items set value [{id:"minecraft:bone",count:1,Slot:0b}]`);
  const f0 = await gametime();
  nbt = await until(() => saved(forge), s => s.includes('bone_meal') && progress(s) === 0, 'the fragment forge finishes', 60000);
  const forgeTicks = (await gametime()) - f0;
  log.forgeTicks = forgeTicks;
  assert(forgeTicks >= 38 && forgeTicks <= 80, `a grind takes about 40 ticks: ${forgeTicks}`);
  assert(/count: ?8/.test(nbt), `eight bone meal: ${nbt.slice(0, 500)}`);
  assert.equal(energy(nbt), 50000 - 40 * 256, 'the energy of 40 ticks at 256 FE/t');
  fixture.mark(`the fragment forge ground a bone in ${forgeTicks} ticks, ${energy(nbt)} FE left`);

  // --- Foundry: two iron gems → 3 iron ingots, 80 ticks at 128 FE/t; three cores ---
  const ox = x + 8;
  const foundry = `${ox} ${y} ${z}`;
  await place(foundry, 'oritech:foundry_block[facing=north]');
  for (const [dx, dy] of [[1, 0], [0, 1], [1, 1]]) await place(`${ox + dx} ${y + dy} ${z}`, 'oritech:machine_core_1');
  await fixture.command(`data merge block ${foundry} {"oritech.machine_energy":50000L}`);
  nbt = await until(() => saved(foundry), s => s.includes('connectedCores') && !/connectedCores: ?\[\]/.test(s), 'the foundry assembles', 20000);
  await fixture.command(`data modify block ${foundry} Items set value [{id:"oritech:iron_gem",count:1,Slot:0b},{id:"oritech:iron_gem",count:1,Slot:1b}]`);
  const o0 = await gametime();
  nbt = await until(() => saved(foundry), s => s.includes('iron_ingot') && progress(s) === 0, 'the foundry finishes', 60000);
  const foundryTicks = (await gametime()) - o0;
  log.foundryTicks = foundryTicks;
  assert(foundryTicks >= 78 && foundryTicks <= 130, `a melt takes about 80 ticks: ${foundryTicks}`);
  assert(/count: ?3/.test(nbt), `three iron ingots: ${nbt.slice(0, 500)}`);
  assert.equal(energy(nbt), 50000 - 80 * 128, 'the energy of 80 ticks at 128 FE/t');
  fixture.mark(`the foundry made 3 iron ingots in ${foundryTicks} ticks, ${energy(nbt)} FE left`);

  // --- Powered furnace (one core above): cobblestone → stone, 200 ticks of smelting at 0.5 speed, 32 FE/t ---
  const ux = x + 20;
  const furnace = `${ux} ${y} ${z}`;
  await place(furnace, 'oritech:powered_furnace_block[facing=north]');
  await place(`${ux} ${y + 1} ${z}`, 'oritech:machine_core_1');
  await fixture.command(`data merge block ${furnace} {"oritech.machine_energy":10000L}`);
  nbt = await until(() => saved(furnace), s => s.includes('connectedCores') && !/connectedCores: ?\[\]/.test(s), 'the powered furnace assembles', 20000);
  await fixture.command(`data modify block ${furnace} Items set value [{id:"minecraft:cobblestone",count:1,Slot:0b}]`);
  const u0 = await gametime();
  nbt = await until(() => saved(furnace), s => s.includes('minecraft:stone') && progress(s) === 0, 'the powered furnace finishes', 60000);
  const furnaceTicks = (await gametime()) - u0;
  log.furnaceTicks = furnaceTicks;
  assert(furnaceTicks >= 98 && furnaceTicks <= 140, `a smelt takes about 100 ticks: ${furnaceTicks}`);
  assert.equal(energy(nbt), 10000 - 100 * 32, 'the energy of 100 ticks at 32 FE/t');
  fixture.mark(`the powered furnace smelted stone in ${furnaceTicks} ticks, ${energy(nbt)} FE left`);

  // --- Basic generator beside a pulverizer: a coal burns 1600 ticks at 32 FE/t and the pulverizer fills ---
  const gx = x + 12;
  const gen = `${gx} ${y} ${z}`;
  const beside = `${gx + 1} ${y} ${z}`;
  await place(gen, 'oritech:basic_generator_block[facing=north]');
  await place(beside, 'oritech:pulverizer_block');
  await until(() => fixture.raw(`block ${gen}`), v => v.block === 'oritech:basic_generator_block', 'generator placed');
  await fixture.command(`data modify block ${gen} Items set value [{id:"minecraft:coal",count:2,Slot:0b}]`);
  nbt = await until(() => saved(gen), s => progress(s) > 0 && progress(s) < 1590, 'the generator burns', 30000);
  await Bun.sleep(2000);
  const genNbt = await saved(gen);
  const pulvNbt = await saved(beside);
  log.generator = { progress: progress(genNbt), energy: energy(genNbt), storedBurn: field(genNbt, 'storedBurn'), pulverizer: energy(pulvNbt) };
  assert.equal(field(genNbt, 'storedBurn'), 1600, 'a coal burns 1600 ticks');
  assert(genNbt.includes('count: 1') && genNbt.includes('coal'), `one coal taken, one left: ${genNbt.slice(0, 400)}`);
  assert(energy(pulvNbt) >= 32 * 20, `the pulverizer beside it is filling: ${energy(pulvNbt)}`);
  assert(energy(pulvNbt) <= 32 * 1600, 'and not faster than the generator makes it');
  await fixture.command(`tp @s ${gx + 0.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${gen} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'generator screen opens');
  await Bun.sleep(500);
  const shotGen = join(fixture.run, 'basic-generator-running.png');
  await fixture.raw(`screenshot ${shotGen}`);
  log.screenshotGenerator = shotGen;
  await fixture.raw('close');
  fixture.mark(`the basic generator burns a coal and the pulverizer beside it holds ${energy(pulvNbt)} FE`);

  // --- Bio generator (a core above) beside a pulverizer: packed wheat burns 4000 ticks at 64 FE/t ---
  const bx = x + 16;
  const bio = `${bx} ${y} ${z}`;
  const bioBeside = `${bx + 1} ${y} ${z}`;
  await place(bio, 'oritech:bio_generator_block[facing=north]');
  await place(`${bx} ${y + 1} ${z}`, 'oritech:machine_core_1');
  await place(bioBeside, 'oritech:pulverizer_block');
  nbt = await until(() => saved(bio), s => s.includes('connectedCores') && !/connectedCores: ?\[\]/.test(s), 'the bio generator assembles', 20000);
  await fixture.command(`data modify block ${bio} Items set value [{id:"oritech:packed_wheat",count:1,Slot:0b}]`);
  nbt = await until(() => saved(bio), s => progress(s) > 0 && progress(s) < 3990, 'the bio generator burns', 30000);
  await Bun.sleep(1500);
  const bioNbt = await saved(bio);
  const bioPulv = await saved(bioBeside);
  log.bio = { progress: progress(bioNbt), storedBurn: field(bioNbt, 'storedBurn'), pulverizer: energy(bioPulv) };
  assert.equal(field(bioNbt, 'storedBurn'), 4000, 'packed wheat burns 4000 ticks');
  assert(energy(bioPulv) >= 64 * 20, `the pulverizer beside the bio generator is filling: ${energy(bioPulv)}`);
  fixture.mark(`the bio generator burns packed wheat and the pulverizer beside it holds ${energy(bioPulv)} FE`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('ORITECH more machines', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('ORITECH more machines', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
