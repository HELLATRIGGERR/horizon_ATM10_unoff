/** Oritech's processing machines, as the `oritech` native bundle and the carry of the player's jar make them.
 *
 * - A pulverizer holding a bone and 25,000 FE pulverizes it in 100 ticks at 32 FE/t (the mod's defaults, no
 *   ATM10 config), consuming 3,200 FE and putting 6 bone meal in the first output. The screen shows the energy bar,
 *   the progress arrow and the slot frames.
 * - An assembler with three machine_core_1 blocks, 50,000 FE and four blaze powder assembles a blaze rod in
 *   96 ticks at 128 FE/t.
 * - The /oritech recipes command reports how many of the world's recipes each type reads.
 *
 * Uses only vanilla items in ingredients (not convention tags that need extra data sources). Reads the saved
 * compound (`data get block`, the bundle's own keys). Screenshots of the pulverizer's screen (running and idle)
 * go to the run folder. Needs the staged native build (`AE2_TEST_BINARY`) and the content mod made from the carry
 * of Oritech (`ORITECH_CONTENT_DIR`); `pack/scripts/oritech.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.ORITECH_CONTENT_DIR ?? join(root, '../.build/content/oritech'));
const fixture = new WorldFixture('oritech-machines', 'server-thread', { devMods: [content] });

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

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 4} ${x + 16} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 4} ${x + 16} ${y - 1} ${z + 4} minecraft:stone`);
  const at = (n: number) => `${x + n * 3} ${y} ${z}`;

  // --- Pulverizer: bone → 6 bone meal, 100 ticks at 32 FE/t ---
  const pulvPos = at(0);
  await fixture.command(`setblock ${pulvPos} oritech:pulverizer_block`);
  await fixture.command(`data merge block ${pulvPos} {"oritech.machine_energy":25000L}`);
  await until(() => fixture.raw(`block ${pulvPos}`), v => v.block === 'oritech:pulverizer_block', 'pulverizer placed');
  await fixture.command(`data modify block ${pulvPos} Items set value [{id:"minecraft:bone",count:1,Slot:0b}]`);
  fixture.mark('pulverizer placed with a bone and 25,000 FE');

  const t0 = await gametime();
  let nbt = await until(() => saved(pulvPos), s => field(s, 'oritech.machine_progress') >= 10, 'the pulverizer starts', 30000);
  log.pulvStart = { progress: field(nbt, 'oritech.machine_progress'), energy: field(nbt, 'oritech.machine_energy') };

  // Screenshot the running pulverizer's screen.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${pulvPos} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'pulverizer screen opens');
  await Bun.sleep(500);
  const shotRunning = join(fixture.run, 'pulverizer-running.png');
  await fixture.raw(`screenshot ${shotRunning}`);
  log.screenshotRunning = shotRunning;
  await fixture.raw('close');

  nbt = await until(() => saved(pulvPos), s => {
    return s.includes('bone_meal') && field(s, 'oritech.machine_progress') === 0;
  }, 'the pulverizer finishes', 90000);
  const elapsed = (await gametime()) - t0;
  log.pulvElapsed = elapsed;
  log.pulvEnergy = field(nbt, 'oritech.machine_energy');
  assert(elapsed >= 95 && elapsed <= 120, `a pulverize takes about 100 ticks: ${elapsed}`);
  assert(nbt.includes('bone_meal'), `the output is bone meal: ${nbt.slice(0, 400)}`);
  const energyLeft = field(nbt, 'oritech.machine_energy');
  assert.equal(energyLeft, 25000 - 100 * 32, `100 ticks at 32 FE/t: ${energyLeft}`);
  fixture.mark(`the pulverizer made bone meal in ${elapsed} ticks, energy left ${energyLeft}`);

  // Screenshot the idle pulverizer's screen.
  await fixture.raw(`interact ${pulvPos} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'pulverizer screen reopens');
  await Bun.sleep(300);
  const shotIdle = join(fixture.run, 'pulverizer-idle.png');
  await fixture.raw(`screenshot ${shotIdle}`);
  log.screenshotIdle = shotIdle;
  await fixture.raw('close');

  // --- Assembler: a multiblock with 3 machine_core_1 blocks, 4× blaze powder → blaze rod ---
  const asmPos = at(2);
  // The assembler faces north: cores at (z+1,y,x), (z,y+1,x), (z+1,y+1,x) relative to it.
  // Facing north: rotate([0,0,1], North) = [1,0,0]; rotate([0,1,0], North) = [0,1,0]; rotate([0,1,1], North) = [1,1,0]
  const ax = x + 6, ay = y, az = z;
  await fixture.command(`setblock ${ax} ${ay} ${az} oritech:assembler_block[facing=north]`);
  await fixture.command(`setblock ${ax + 1} ${ay} ${az} oritech:machine_core_1`);
  await fixture.command(`setblock ${ax} ${ay + 1} ${az} oritech:machine_core_1`);
  await fixture.command(`setblock ${ax + 1} ${ay + 1} ${az} oritech:machine_core_1`);
  await fixture.command(`data merge block ${asmPos} {"oritech.machine_energy":50000L}`);
  await until(() => fixture.raw(`block ${asmPos}`), v => v.block === 'oritech:assembler_block', 'assembler placed');

  // Check the multiblock assembled.
  nbt = await until(() => saved(asmPos), s => s.includes('connectedCores') && field(s, 'connectedCores') !== 0, 'the assembler assembles its cores', 15000).catch(() => saved(asmPos));
  log.asmCores = nbt.includes('connectedCores');

  // Give the assembler its inputs: 4× blaze powder, one per slot (the recipe takes four ingredients).
  await fixture.command(`data modify block ${asmPos} Items set value [{id:"minecraft:blaze_powder",count:1,Slot:0b},{id:"minecraft:blaze_powder",count:1,Slot:1b},{id:"minecraft:blaze_powder",count:1,Slot:2b},{id:"minecraft:blaze_powder",count:1,Slot:3b}]`);
  fixture.mark('assembler placed with cores and 4 blaze powder');

  const t1 = await gametime();
  nbt = await until(() => saved(asmPos), s => {
    return s.includes('blaze_rod') && field(s, 'oritech.machine_progress') === 0;
  }, 'the assembler finishes', 120000);
  const asmElapsed = (await gametime()) - t1;
  log.asmElapsed = asmElapsed;
  log.asmEnergy = field(nbt, 'oritech.machine_energy');
  assert(asmElapsed >= 90 && asmElapsed <= 120, `an assemble takes about 96 ticks: ${asmElapsed}`);
  assert(nbt.includes('blaze_rod'), `the output is a blaze rod`);
  fixture.mark(`the assembler made a blaze rod in ${asmElapsed} ticks`);

  // --- /oritech recipes ---
  const reply = await fixture.command('oritech recipes');
  const chat = (reply.chat ?? []).join(' ');
  log.recipes = chat;
  assert.match(chat, /Oritech recipes read/, `/oritech recipes works: ${chat}`);
  assert.match(chat, /oritech:pulverizer \d+\/\d+/, 'pulverizer recipes counted');
  fixture.mark(`/oritech recipes: ${chat}`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('ORITECH machines', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('ORITECH machines', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
