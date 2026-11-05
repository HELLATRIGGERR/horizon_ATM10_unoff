/** Mekanism's advanced machines (the Osmium Compressor, Purification Chamber and Chemical Injection Chamber) with their
 * chemical secondary, as the `mekanism` native bundle and its content mods make them (docs/mekanism-chemicals.md,
 * slice 4), under ATM10's configs. All the Ores' carry is loaded too (`MEK_ATO_CARRY`, else beside `MEK_CARRY`): the
 * pack's purifying and injecting recipes make its clumps and shards (here from Mekanism's raw osmium: the pack's
 * `c:ores/*` tags name no ore of these two mods). The numbers are those a NeoForge 1.21.1 server with
 * Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (chem-work/m4 a1):
 *
 * - The compressor: 210 mB of osmium and two dusts (the oracle's glowstone, refined obsidian here, the same kind of
 *   recipe): 200 ticks of 100 J, 1 mB a tick, `used_so_far` counting up and back to 0 with the ingot; 8 speed
 *   upgrades 16 mB a tick for 12 ticks of 25 600 J (80 000 J last three); 4 speed and 8 energy 4 mB a tick for 50
 *   ticks of 100 J: 200 mB an ingot whatever its upgrades. An empty tank waits with its progress; an osmium ingot in
 *   its chemical slot adds 200 mB only when all of it fits.
 * - The chambers (`randomizedConsumption`): a draw a tick of 0 to 3 mB (`StatUtils.inversePoisson(1)`, 0.91 on
 *   average: 186 and 165 mB in the oracle's 200 ticks); 8 speed upgrades draw 256 on average, more than the tank holds,
 *   and stall; 8 chemical upgrades draw 1/16 on average. Their chemical goes in on every side and comes out on none.
 * - Their screens show their slots. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const ato = resolve(process.env.MEK_ATO_CARRY ?? join(dirname(resolve(process.env.MEK_CARRY ?? join(root, '../ports/atm10/mods/mekanism/carry'))), 'alltheores'));
if (!existsSync(join(ato, 'horizon-mod.toml'))) throw new Error(`All the Ores' carry is needed for the chambers' recipes: none at ${ato} (MEK_ATO_CARRY)`);
const fixture = new WorldFixture('mekanism-advanced', 'server-thread', { devMods: [content, genContent, ato] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
}

const CO = 'mekanism:osmium_compressor', PU = 'mekanism:purification_chamber', IN = 'mekanism:chemical_injection_chamber';
const OS = 'mekanism:osmium', O = 'mekanism:oxygen', HCL = 'mekanism:hydrogen_chloride';
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** progress, Joules, the tank's mB and `used_so_far`; the slots. */
async function row(pos: string): Promise<{ r: number[]; slots: any[]; p: any }> {
  const p = await mek(`mek probe ${pos}`);
  return { r: [p.progress, p.energy, p.tank?.amount ?? 0, p.used_so_far], slots: p.slots ?? [], p };
}
const count = (slots: any[], i: number) => slots[i]?.count ?? 0;
const tank = (chemical: string, mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:${mb}L}}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const upgrades = (list: [number, number][]) => `{component_upgrade:{upgrades:[${list.map(([t, n]) => `{type:${t},amount:${n}}`).join(',')}]}}`;
const items = (list: [number, string, number][]) => `{Items:[${list.map(([s, id, n]) => `{Slot:${s}b,id:"${id}",count:${n}}`).join(',')}]}`;
async function place(pos: string, block: string, ...nbt: string[]) {
  await fixture.command(`setblock ${pos} ${block}[facing=north]`);
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

  // A: the compressor, no upgrades; B: 8 speed; C: 4 speed and 8 energy; D: an empty tank.
  const a = at(0), b = at(2), c = at(4), d = at(6);
  const dust = items([[0, 'mekanism:dust_refined_obsidian', 2]]);
  await place(a, CO, tank(OS, 210), dust, energy(80000));
  await place(b, CO, tank(OS, 210), dust, upgrades([[0, 8]]), energy(80000));
  await place(c, CO, tank(OS, 210), dust, upgrades([[0, 4], [1, 8]]), energy(80000));
  await place(d, CO, dust, '{progress:50,used_so_far:50L}', energy(80000));
  const rows: number[][] = [], fast: number[] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    rows.push((await row(a)).r);
  }
  for (let t = 4; t <= 13; t++) await fixture.stepServer(1);
  const fastEnd = (await row(b)).r;
  log.compressor = { rows, fastEnd, probe: (await row(a)).p };
  assert.deepEqual(rows, [[1, 79900, 209, 1], [2, 79800, 208, 2], [3, 79700, 207, 3]]);
  assert.deepEqual(fastEnd, [3, 3200, 162, 48], '8 speed: 16 mB a tick, 3 ticks of 25 600 J, then short of energy');
  assert.deepEqual((await row(d)).r, [50, 80000, 0, 50], 'an empty tank waits with its progress');
  await fixture.stepServer(37);
  const half = await row(c);
  assert.deepEqual([half.r[0], half.r[1], half.r[2], count(half.slots, 2)], [0, 75000, 10, 1], '4 speed, 8 energy: 50 ticks of 100 J, 4 mB each');
  await fixture.stepServer(149);
  const t199 = (await row(a)).r;
  await fixture.stepServer(1);
  const t200 = await row(a);
  log.compressor_end = { half: half.r, t199, t200: t200.r, slots: t200.slots };
  assert.deepEqual(t199, [199, 60100, 11, 199]);
  assert.deepEqual(t200.r, [0, 60000, 10, 0], 'the ingot: 200 mB, used_so_far back to 0');
  assert.deepEqual([count(t200.slots, 0), t200.slots[2]?.item, count(t200.slots, 2)], [1, 'mekanism:ingot_refined_obsidian', 1]);
  fixture.mark('the compressor takes 200 mB an ingot, 1 a tick, 16 with 8 speed upgrades, 4 with 4 speed and 8 energy');

  // E: an osmium ingot in its chemical slot: 200 mB when all of it fits.
  const e = at(8);
  await place(e, CO, tank(OS, 20), items([[1, 'mekanism:ingot_osmium', 2]]));
  await fixture.stepServer(1);
  const kept = await row(e);
  await fixture.command(`data merge block ${e} ${tank(OS, 10)}`);
  await fixture.stepServer(1);
  const filled = await row(e);
  log.conversion = { kept: [kept.r[2], count(kept.slots, 1)], filled: [filled.r[2], count(filled.slots, 1)] };
  assert.deepEqual([kept.r[2], count(kept.slots, 1), filled.r[2], count(filled.slots, 1)], [20, 2, 210, 1]);
  // Its chemical sides (`OUT_NO_EJECT`, facing north): its right (west) gives, the rest take osmium, not oxygen.
  assert.equal((await mek(`mek chemical ${e} west extract 10`)).got.amount, 10);
  assert.equal((await mek(`mek chemical ${e} east insert ${OS} 5`)).rest.amount, 0);
  assert.equal((await mek(`mek chemical ${e} east insert ${O} 5`)).rest.amount, 5, 'no compressing recipe takes oxygen');
  assert.equal((await mek(`mek chemical ${e} east extract 5`)).got.amount, 0, 'an input side gives nothing');
  fixture.mark('an osmium ingot adds 200 mB only when all of it fits; its right gives osmium, its other sides take it');

  // F: purification at random, kept topped up by flint (10 mB each); G: 8 speed; H: 8 chemical upgrades.
  const f = at(10), g = at(12), h = at(14);
  await place(f, PU, tank(O, 210), items([[0, 'mekanism:raw_osmium', 2], [1, 'minecraft:flint', 64]]), energy(80000));
  await place(g, PU, tank(O, 210), items([[0, 'mekanism:raw_osmium', 4]]), upgrades([[0, 8], [1, 8]]), energy(1000000));
  await place(h, PU, tank(O, 210), items([[0, 'mekanism:raw_osmium', 2]]), upgrades([[3, 8]]), energy(80000));
  const fProbe = (await row(f)).p;
  log.purification_probe = fProbe;
  assert(fProbe.recipes > 0, 'All the Ores\' clumps give it its recipes');
  const uses: number[] = [];
  let [lastTank, lastFlint] = [210, 64];
  let usedAt100 = 0;
  for (let t = 1; t <= 200; t++) {
    await fixture.stepServer(1);
    const now = await row(f);
    const flint = count(now.slots, 1);
    uses.push(lastTank + 10 * (lastFlint - flint) - now.r[2]);
    [lastTank, lastFlint] = [now.r[2], flint];
    if (t === 100) usedAt100 = now.r[3];
  }
  const fEnd = await row(f);
  const total = uses.reduce((s, u) => s + u, 0);
  log.purification = { uses, total, usedAt100, end: fEnd.r, slots: fEnd.slots };
  assert(uses.every(u => u >= 0 && u <= 3), `0 to 3 mB a tick: ${uses}`);
  assert(uses.some(u => u === 0) && uses.some(u => u >= 2), 'drawn at random');
  assert.equal(usedAt100, uses.slice(0, 100).reduce((s, u) => s + u, 0), 'used_so_far counts what it took');
  assert(total >= 130 && total <= 235, `about 181 mB in 200 ticks: ${total}`);
  assert.deepEqual([fEnd.r[0], fEnd.r[1], fEnd.r[3], count(fEnd.slots, 0), fEnd.slots[2]?.item, count(fEnd.slots, 2)], [0, 40000, 0, 1, 'alltheores:osmium_clump', 2]);
  const stall = await row(g);
  assert(stall.r[0] <= 1 && count(stall.slots, 2) === 0, `8 speed upgrades: 256 mB a tick on average, more than 210: it stalls (${stall.r})`);
  const slow = await row(h);
  log.purification_upgrades = { stall: stall.r, slow: slow.r };
  assert(210 - slow.r[2] <= 40, `8 chemical upgrades: 1/16 mB a tick on average (${slow.r})`);
  // Its chemical side takes oxygen on every side, gives none.
  const room = 210 - (await row(h)).r[2];
  assert(room > 0, 'it took some oxygen');
  assert.equal((await mek(`mek chemical ${h} west insert ${O} ${room}`)).rest.amount, 0, 'a side fills it');
  assert.equal((await mek(`mek chemical ${h} south insert ${O} 1`)).rest.amount, 1, 'to its 210 mB');
  assert.equal((await mek(`mek chemical ${h} up extract 10`)).got.amount, 0, 'only the chamber takes out');
  assert.equal((await mek(`mek chemical ${h} north insert ${HCL} 10`)).rest.amount, 10, 'no purifying recipe takes hydrogen chloride');
  fixture.mark('the purification chamber draws 0 to 3 mB of oxygen a tick, stalls with 8 speed upgrades, saves with chemical ones');

  // I: injection at random, topped up by salt (2 mB each).
  const i = at(16);
  await place(i, IN, tank(HCL, 210), items([[0, 'mekanism:raw_osmium', 6], [1, 'mekanism:salt', 64]]), energy(160000));
  const inj: number[] = [];
  [lastTank, lastFlint] = [210, 64];
  for (let t = 1; t <= 200; t++) {
    await fixture.stepServer(1);
    const now = await row(i);
    const salt = count(now.slots, 1);
    inj.push(lastTank + 2 * (lastFlint - salt) - now.r[2]);
    [lastTank, lastFlint] = [now.r[2], salt];
  }
  const iEnd = await row(i);
  const iTotal = inj.reduce((s, u) => s + u, 0);
  log.injection = { inj, iTotal, end: iEnd.r, slots: iEnd.slots };
  assert(inj.every(u => u >= 0 && u <= 3) && iTotal >= 130 && iTotal <= 235, `0 to 3 mB a tick, about 181 in all: ${iTotal}`);
  assert.deepEqual([iEnd.r[0], iEnd.r[1], iEnd.slots[2]?.item, count(iEnd.slots, 2)], [0, 80000, 'alltheores:osmium_shard', 8]);
  fixture.mark('the injection chamber draws its hydrogen chloride the same way at 400 J a tick');

  // Their screens (the world running again).
  await fixture.hold(false);
  for (const [dx, block, name] of [[0, CO, 'compressor'], [14, PU, 'purification']] as [number, string, string][]) {
    const pos = at(dx);
    await fixture.command(`tp @s ${x + dx + 0.5} ${y} ${z + 2.5}`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === block, `the ${name} reaches the client`);
    await fixture.raw(`interact ${pos} south`);
    const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), `the ${name} screen opens`);
    const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
    log[`${name}_slots`] = own;
    assert.deepEqual(own, [0, 1, 2, 3, 4, 5], `the ${name}'s input, chemical, output, energy and upgrade slots`);
    await Bun.sleep(1500);
    await fixture.raw(`screenshot ${join(fixture.run, `${name}.png`)}`);
    await fixture.raw('close');
    await Bun.sleep(300);
  }
  fixture.mark('their screens show their six slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK advanced', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK advanced', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
