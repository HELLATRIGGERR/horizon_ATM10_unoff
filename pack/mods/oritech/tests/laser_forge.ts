/** Oritech's laser arm and atomic forge, as the `oritech` native bundle and the carry of the player's jar make them. The
 * test world has no ATM10 config: the mod's defaults (a laser shot costs 128 FE, the forge's buffer is 1024 FE a tick of
 * its recipe).
 *
 * - A laser arm (a core behind it) aimed at an atomic forge (a ring of eight cores) gives it its recipe's energy shot by
 *   shot, and the forge crafts a heisenberg compensator the tick its buffer is full.
 * - A laser arm aimed past a stone block mines it (1024 x sqrt(1.5) of energy), its cobblestone lands in the laser's
 *   inventory, and the next target is an amethyst cluster that the laser's recipe turns into fluxite.
 *
 * The laser is aimed with `/oritech aim` (the target designator item is not ported). Reads the saved compound.
 * Screenshots of the laser's screen go to the run folder. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.ORITECH_CONTENT_DIR ?? join(root, '../.build/content/oritech'));
const fixture = new WorldFixture('oritech-laser-forge', 'server-thread', { devMods: [content] });

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
const energy = (nbt: string) => field(nbt, 'oritech.machine_energy');
const assembled = (s: string) => s.includes('connectedCores') && !/connectedCores: ?\[\]/.test(s);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) + 14;
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 14} ${x + 10} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 14} ${x + 10} ${y - 1} ${z + 3} minecraft:stone`);
  const place = (pos: string, block: string) => fixture.command(`setblock ${pos} ${block}`);

  // --- Laser arm → atomic forge ---
  const laser = `${x} ${y} ${z}`;
  const forge = `${x} ${y} ${z - 8}`;
  await place(laser, 'oritech:laser_arm_block[facing=north]');
  await place(`${x} ${y} ${z - 1}`, 'oritech:machine_core_1');
  await place(forge, 'oritech:atomic_forge_block[facing=north]');
  for (const dx of [-1, 0, 1]) for (const dz of [-1, 0, 1]) {
    if (dx || dz) await place(`${x + dx} ${y} ${z - 8 + dz}`, 'oritech:machine_core_1');
  }
  await fixture.command(`data merge block ${laser} {"oritech.machine_energy":20000L}`);
  await until(() => saved(laser), assembled, 'the laser assembles', 20000);
  await until(() => saved(forge), assembled, 'the forge assembles', 20000);
  await fixture.command(`data modify block ${forge} Items set value [{id:"oritech:super_ai_chip",count:1,Slot:0b},{id:"oritech:adamant_ingot",count:2,Slot:1b}]`);
  const aimed = await fixture.command(`oritech aim ${laser} ${forge}`);
  log.aim = aimed.chat;
  // The laser holds 20,000 FE and the forge's recipe needs 61,440: the test refills the laser as a generator would.
  let nbt = await until(async () => {
    const laserNow = await saved(laser);
    if (energy(laserNow) < 4000) await fixture.command(`data merge block ${laser} {"oritech.machine_energy":20000L}`);
    return saved(forge);
  }, s => s.includes('heisenberg_compensator'), 'the forge crafts', 180000);
  log.forgeNbt = nbt.slice(0, 400);
  const laserNbt = await saved(laser);
  log.laserEnergy = energy(laserNbt);
  assert.equal(energy(nbt), 0, 'the forge buffer emptied on the craft');
  assert.equal(energy(laserNbt) % 128, 20000 % 128, 'whole shots of 128 FE');
  fixture.mark('the laser fed the forge 61,440 FE shot by shot and it crafted a compensator');

  // --- Laser arm mining ---
  const mx = x + 6;
  const miner = `${mx} ${y} ${z}`;
  await place(miner, 'oritech:laser_arm_block[facing=north]');
  await place(`${mx} ${y} ${z - 1}`, 'oritech:machine_core_1');
  await place(`${mx} ${y} ${z - 6}`, 'minecraft:stone');
  await place(`${mx} ${y} ${z - 9}`, 'minecraft:amethyst_cluster');
  await fixture.command(`data merge block ${miner} {"oritech.machine_energy":20000L}`);
  await until(() => saved(miner), assembled, 'the miner assembles', 20000);
  await fixture.command(`oritech aim ${miner} ${mx} ${y} ${z - 6}`);
  nbt = await until(() => saved(miner), s => s.includes('cobblestone'), 'the laser mines the stone', 60000);
  assert.equal((await fixture.raw(`block ${mx} ${y} ${z - 6}`)).block, 'minecraft:air', 'the stone is gone');
  const tp = `tp @s ${mx + 0.5} ${y} ${z + 2.5}`;
  await fixture.command(tp);
  await fixture.raw(`interact ${miner} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'laser screen opens');
  await Bun.sleep(500);
  const shot = join(fixture.run, 'laser-mined.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshotLaser = shot;
  await fixture.raw('close');
  nbt = await until(() => saved(miner), s => s.includes('oritech:fluxite'), 'the laser mines the cluster into fluxite', 90000);
  assert.equal((await fixture.raw(`block ${mx} ${y} ${z - 9}`)).block, 'minecraft:air', 'the cluster is gone');
  fixture.mark('the laser mined the stone into cobblestone and the amethyst cluster into fluxite');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('ORITECH laser and forge', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('ORITECH laser and forge', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
