/** Mekanism's Rotary Condensentrator, as the `mekanism` native bundle and its content mods make it
 * (docs/mekanism-chemicals.md, slice 5), under ATM10's configs. The numbers are those a NeoForge 1.21.1 server with
 * Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (chem-work/m5 c1, R1-R3):
 *
 * - Condensentrating (its mode `false`, the default): 1 mB of hydrogen into 1 mB of liquid hydrogen a tick for 50 J;
 *   3 speed upgrades 8 a tick of 400 J each.
 * - Decondensentrating (`mode: 1b`): the liquid back into the gas, 1 mB a tick.
 * - Decondensentrating, its chemical tank ejects out of its left (east, facing north) at 1024 mB a tick;
 *   condensentrating, it ejects none.
 * - Its chemical sides take hydrogen while condensentrating and give none; decondensentrating, the other way round.
 * - Its screen shows its slots, and its mode button toggles the mode. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-rotary', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
}

const RO = 'mekanism:rotary_condensentrator', H = 'mekanism:hydrogen';
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** The chemical tank's mB, the fluid tank's mB and fluid, the energy, the mode. */
async function probe(pos: string): Promise<any> {
  const p = await mek(`mek probe ${pos}`);
  return { gas: p.tanks?.[0]?.amount ?? 0, fluid: p.fluid?.amount ?? 0, fluidId: p.fluid?.id, energy: p.energy, mode: p.mode, recipes: p.recipes };
}
const tank = (mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${H}",amount:${mb}L}}]}`;
const liquid = (mb: number) => `{fluid_tanks:[{tank:0b,stored:{id:"${H}",amount:${mb}}}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
/** A basic chemical tank's mB, by `/mek probe`. */
const tankAmount = async (pos: string) => (await mek(`mek probe ${pos}`)).amount ?? 0;
const quietTank = '{component_config:{config2:[I;1,1,1,1,1,1],eject2:0b,config3:[I;0,0,0,0,0,0],eject3:0b}}';
async function place(pos: string, ...nbt: string[]) {
  await fixture.command(`setblock ${pos} ${RO}[facing=north]`);
  for (const n of nbt) await fixture.command(`data merge block ${pos} ${n}`);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(30, 5, 10)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(30, -2, 10)} minecraft:stone`);
  await fixture.hold();

  // R1: condensentrating 1000 mB of hydrogen, 20 000 J; R2: 3 speed upgrades; R3: decondensentrating 1000 mB.
  const a = at(0), b = at(2), c = at(4);
  await place(a, tank(1000), energy(20000));
  await place(b, tank(1000), '{component_upgrade:{upgrades:[{type:0,amount:3}]}}', energy(20000));
  await place(c, '{mode:1b}', liquid(1000), energy(20000));
  const rows: any[] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    const [p, q, r] = [await probe(a), await probe(b), await probe(c)];
    rows.push([p.gas, p.fluid, p.energy, q.gas, q.fluid, q.energy, r.gas, r.fluid, r.energy]);
  }
  log.rows = rows;
  log.probe = await mek(`mek probe ${a}`);
  assert((await probe(a)).recipes >= 10, 'Mekanism\'s rotary recipes');
  assert.deepEqual(rows, [1, 2, 3].map(t => [1000 - t, t, 20000 - 50 * t, 1000 - 8 * t, 8 * t, 20000 - 3200 * t, t, 1000 - t, 20000 - 50 * t]));
  assert.deepEqual([(await probe(a)).fluidId, (await probe(a)).mode, (await probe(c)).mode], [H, false, true]);
  fixture.mark('condensentrating: 1 mB of hydrogen into liquid a tick for 50 J, 3 speed 8 of 400 J; decondensentrating 1 mB back');

  // R4: decondensentrating, it ejects out of its left (east) at 1024 mB a tick; condensentrating, none.
  const d = at(8), east = at(9), e = at(12), eEast = at(13);
  await place(d, '{mode:1b}', liquid(5000), tank(3000), energy(20000));
  await place(e, tank(3000), energy(20000));
  for (const t of [east, eEast]) {
    await fixture.command(`setblock ${t} mekanism:basic_chemical_tank[facing=north]`);
    await fixture.command(`data merge block ${t} ${quietTank}`);
  }
  const ejected: number[][] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    ejected.push([(await probe(d)).gas, await tankAmount(east), await tankAmount(eEast)]);
  }
  log.ejected = ejected;
  assert.deepEqual(ejected, [[1977, 1024, 0], [954, 2048, 0], [1, 3002, 0]]);
  fixture.mark('decondensentrating, its chemical ejects out of its left at 1024 mB a tick; condensentrating, none');

  // R5: its chemical sides by mode.
  const f = at(16);
  await place(f, energy(0));
  assert.equal((await mek(`mek chemical ${f} south insert ${H} 100`)).rest.amount, 0, 'condensentrating, its back takes hydrogen');
  assert.equal((await mek(`mek chemical ${f} south extract 10`)).got.amount, 0, 'and gives none');
  assert.equal((await mek(`mek chemical ${f} south insert mekanism:sulfuric_acid 10`)).rest.amount, 10, 'no recipe takes acid');
  await fixture.command(`data merge block ${f} {mode:1b}`);
  assert.equal((await mek(`mek chemical ${f} south insert ${H} 10`)).rest.amount, 10, 'decondensentrating, it takes none');
  assert.equal((await mek(`mek chemical ${f} south extract 10`)).got.amount, 10, 'and gives');
  fixture.mark('its chemical sides take while condensentrating and give while decondensentrating');

  // Its screen, and its mode button (the world running again).
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z - 1.5}`);
  await until(() => fixture.raw(`block ${a}`), v => v.block === RO, 'the condensentrator reaches the client');
  await fixture.raw(`interact ${a} north`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [...Array(7).keys()], 'its five slots and the upgrade component\'s');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'rotary.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK rotary', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK rotary', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
