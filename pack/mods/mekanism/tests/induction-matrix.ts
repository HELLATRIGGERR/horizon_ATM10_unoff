/** Mekanism's Induction Matrix, as the `mekanism` native bundle makes it under ATM10's configs (tiers.toml
 * `[induction]`: a basic cell holds 8 000 000 000 J, an advanced one 64 000 000 000, a basic provider moves 256 000 J
 * a tick each way). The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave
 * for the same build over RCON (local `mekm2-work/oracle`, x6 M1-M4):
 *
 * - A 3x5x3 of induction casings with a basic cell under two basic providers forms: 8 000 000 000 J of room,
 *   512 000 J a tick of transfer.
 * - In: a creative energy cube against the input port fills the cell by 512 000 J a tick, landing a tick after the
 *   cube gives it; Forge Energy reads the port as the matrix's FE.
 * - Out: the Configurator's sneaking use turns the other port to output; an ultimate cube against it then takes the
 *   transfer, 512 000 J a tick, out of the cell (a cube takes what others give it at any rate, `EnergyCubeEnergyContainer`;
 *   the oracle's matrix of one provider gave 256 000).
 * - Forge Energy: AE2's debug energy generator against the input port puts in 8 FE = 20 J a tick; an energy acceptor
 *   against the output port takes the matrix's Joules into its grid.
 * - The Joules stay in the cell: a casing broken and put back, the matrix forms again with them; an advanced cell in
 *   place of the basic one makes 64 000 000 000 J of room. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-induction-matrix', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const CASING = 'mekanism:induction_casing', PORT = 'mekanism:induction_port';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
async function joules(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((t: string) => t.includes('has the following block data')) ?? '';
  const stored = line.match(/stored: (\d+)L/);
  return stored ? Number(stored[1]) : 0;
}
async function is(pos: string, state: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${state}`);
  return (reply.chat ?? []).some((t: string) => /passed/i.test(t));
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 70), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-4, -1, -4)} ${at(8, 8, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-4, -2, -4)} ${at(8, -2, 8)} minecraft:stone`);
  await fixture.hold();
  await fixture.command(`fill ${at(0, 0, 0)} ${at(2, 4, 2)} ${CASING} hollow`);
  const cell = at(1, 1, 1), input = at(0, 1, 1), output = at(2, 1, 1);
  await fixture.command(`setblock ${cell} mekanism:basic_induction_cell`);
  await fixture.command(`setblock ${at(1, 2, 1)} mekanism:basic_induction_provider`);
  await fixture.command(`setblock ${at(1, 3, 1)} mekanism:basic_induction_provider`);
  await fixture.command(`setblock ${input} ${PORT}`);
  await fixture.command(`setblock ${output} ${PORT}`);
  await fixture.stepServer(2);
  const matrix = async () => (await mek(`mek probe ${input}`)).multiblocks?.[0];
  const formed = await matrix();
  log.formed = formed;
  assert(formed, `formed: ${JSON.stringify(await mek(`mek probe ${input}`))}`);
  assert.deepEqual(formed.size, [3, 5, 3]);
  assert.equal(formed.storage_cap, 8_000_000_000, 'a basic cell');
  assert.equal(formed.transfer_cap, 512_000, 'two basic providers');
  assert.equal(formed.cells.length, 1);
  assert.equal(formed.providers, 2);
  fixture.mark('a 3x5x3 with a basic cell and two basic providers: 8 000 000 000 J, 512 000 J a tick');

  // In: a creative cube against the input port, its front east.
  const west = at(-1, 1, 1);
  await fixture.command(`setblock ${west} mekanism:creative_energy_cube`);
  await fixture.command(`data merge block ${west} {facing:"east"}`);
  await fixture.stepServer(1);
  const ins: number[] = [];
  let last = await joules(cell);
  for (let i = 0; i < 3; i++) {
    await fixture.stepServer(1);
    const now = await joules(cell);
    ins.push(now - last); last = now;
  }
  log.input_per_tick = ins;
  assert.deepEqual(ins, [512_000, 512_000, 512_000], 'the transfer cap a tick');
  await fixture.command(`setblock ${west} minecraft:air`);
  await fixture.stepServer(2);
  const stored = await joules(cell);
  const fe = await fixture.ae(`power ${input} west`);
  log.fe_readback = fe.fe;
  assert.equal(fe.fe?.stored, Math.floor(stored / 2.5), `${stored} J in FE`);
  assert.equal(fe.fe?.capacity, 2147483647, '8 GJ are more FE than an int');
  fixture.mark(`a creative cube fills the cell by 512 000 J a tick; AE2 reads ${fe.fe.stored} FE through the port`);

  // Out: the Configurator turns the east port to output; an ultimate cube against it takes the transfer.
  await fixture.hold(false);
  await fixture.replace('hotbar.0', 'mekanism:configurator');
  await fixture.select(0, 'mekanism:configurator');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 1.5}`);
  await fixture.sneakInteract(`${output} east`);
  const turned = await until(() => mek(`mek probe ${output}`), p => p.port?.mode === 'Output', 'the port turned to output');
  assert(await is(output, `${PORT}[active=true]`), 'shown on the block');
  log.port = turned.port;
  await fixture.command(`tp @s ${x - 3.5} ${y} ${z - 3.5}`);
  await fixture.hold();
  const east = at(3, 1, 1);
  await fixture.command(`setblock ${east} mekanism:ultimate_energy_cube`);
  const outs: number[] = [], lost: number[] = [];
  let cellBefore = await joules(cell), cubeBefore = await joules(east);
  for (let i = 0; i < 3; i++) {
    await fixture.stepServer(1);
    const c = await joules(cell), d = await joules(east);
    outs.push(d - cubeBefore); lost.push(cellBefore - c);
    cellBefore = c; cubeBefore = d;
  }
  log.output = { outs, lost };
  assert.deepEqual(outs, [512_000, 512_000, 512_000], 'the transfer cap a tick');
  assert.deepEqual(lost.slice(1), [512_000, 512_000], 'out of the cell, a tick later');
  await fixture.command(`setblock ${east} minecraft:air`);
  fixture.mark('the Configurator turns a port to output; an ultimate cube against it takes 512 000 J a tick');

  // Forge Energy in: AE2's debug generator against the input port.
  await fixture.command(`setblock ${west} ae2:debug_energy_gen`);
  await fixture.stepServer(2);
  const fein: number[] = [];
  last = await joules(cell);
  for (let i = 0; i < 3; i++) {
    await fixture.stepServer(1);
    const now = await joules(cell);
    fein.push(now - last); last = now;
  }
  log.fe_in = fein;
  assert.deepEqual(fein, [20, 20, 20], '8 FE a tick are 20 J');
  await fixture.command(`setblock ${west} minecraft:air`);
  fixture.mark('AE2\'s debug energy generator puts 8 FE = 20 J a tick in through the input port');

  // Forge Energy out: an energy acceptor against the output port, an energy cell beside it, which fill at once.
  const c0 = await joules(cell);
  await fixture.hold(false);
  await fixture.command(`setblock ${at(4, 1, 1)} ae2:energy_cell`);
  await fixture.command(`setblock ${east} ae2:energy_acceptor`);
  const grid = () => fixture.ae(`power ${at(4, 1, 1)} west`);
  const total = (s: any) => s.energy + (s.energy_service?.buffer ?? 0);
  const filled = await until(grid, s => s.energy_service?.members > 0 && total(s) > 0, 'the acceptor and cell form a grid and take energy', 30000);
  await fixture.hold();
  const c1 = await joules(cell);
  log.fe_out = { grid: total(filled), cell: c0 - c1 };
  assert(c0 - c1 > 0, `out of the cell: ${c0} -> ${c1}`);
  await fixture.command(`setblock ${east} minecraft:air`);
  await fixture.command(`setblock ${at(4, 1, 1)} minecraft:air`);
  fixture.mark(`an energy acceptor against the output port takes the matrix's Joules: its grid holds ${Math.round(total(filled))} AE, the cell lost ${c0 - c1} J`);

  // The Joules stay in the cell when the matrix breaks, and come back with it: the creative cube fills it a little first.
  await fixture.command(`setblock ${west} mekanism:creative_energy_cube`);
  await fixture.command(`data merge block ${west} {facing:"east"}`);
  await fixture.stepServer(3);
  await fixture.command(`setblock ${west} minecraft:air`);
  await fixture.stepServer(2);
  const kept = await joules(cell);
  assert(kept >= 512_000, `the cell holds some: ${kept}`);
  await fixture.command(`setblock ${at(0, 0, 0)} minecraft:air`);
  await fixture.stepServer(2);
  assert.equal(await matrix(), undefined, 'broken: no matrix');
  assert.equal(await joules(cell), kept, 'the cell keeps its Joules');
  await fixture.command(`setblock ${at(0, 0, 0)} ${CASING}`);
  await fixture.stepServer(2);
  const back = await matrix();
  assert.equal(back?.energy, kept, 'formed again with them');
  fixture.mark(`a casing broken and put back: the matrix forms again with the cell's ${kept} J`);

  // An advanced cell in place of the basic one.
  await fixture.command(`setblock ${cell} mekanism:advanced_induction_cell`);
  await fixture.stepServer(2);
  const advanced = await matrix();
  assert.equal(advanced?.storage_cap, 64_000_000_000, 'an advanced cell');
  fixture.mark('an advanced cell in its place: 64 000 000 000 J of room');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK induction-matrix', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK induction-matrix', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
