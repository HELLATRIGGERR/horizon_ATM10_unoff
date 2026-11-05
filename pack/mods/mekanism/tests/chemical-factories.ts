/** Mekanism's compressing, purifying, injecting and infusing factories (`TileEntityItemStackChemicalToItemStackFactory`),
 * as the `mekanism` native bundle and its content mods make them (docs/mekanism-chemicals.md, slice 6), under ATM10's
 * configs. All the Ores' carry is loaded too (`MEK_ATO_CARRY`, else beside `MEK_CARRY`) for the pack's purifying and
 * injecting recipes. The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave
 * for the same steps over RCON (chem-work/m6 f1 to f3):
 *
 * - Their stores are `max(storage / 2, usage) * processes`: basic compressing and purifying 120 000 J, injecting
 *   240 000, infusing 30 000; their one chemical tank 210 mB a process (630, 1890 for the ultimate), the infusing
 *   one's 1000 (3000); the chemical slot after the processes (slot 6 of a basic one) fills it with a conversion only
 *   when all of it fits.
 * - A basic compressing factory, a dust in each process: 1 mB of osmium and 100 J a process a tick, `used_so_far`
 *   `[1, 1, 1]` after a tick, three ingots and 30 of 630 mB left on the 200th; 8 speed upgrades 16 mB and 25 600 J a
 *   process, the third tick's processes short of Joules; 2 mB run the first two processes, the third waits.
 * - The purifying and injecting factories draw each process's use at random (0 to 3 mB a tick each), the injecting
 *   one at 400 J a process.
 * - A basic infusing factory takes the recipe's 10 mB with each item on the 200th tick; with 15 mB only the first
 *   process finishes, the others start over.
 * - The compressing factory's right gives its chemical; the purifying one's sides only take.
 * - Tier installers turn the Osmium Compressor, Purification Chamber, Chemical Injection Chamber and Metallurgic
 *   Infuser into their basic factories (and a factory into the next tier) with their tank, its slot and the use so
 *   far in process 0 (`AdvancedMachineUpgradeData`). */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const ato = resolve(process.env.MEK_ATO_CARRY ?? join(dirname(resolve(process.env.MEK_CARRY ?? join(root, '../ports/atm10/mods/mekanism/carry'))), 'alltheores'));
if (!existsSync(join(ato, 'horizon-mod.toml'))) throw new Error(`All the Ores' carry is needed for the purifying and injecting recipes: none at ${ato} (MEK_ATO_CARRY)`);
const fixture = new WorldFixture('mekanism-chemical-factories', 'server-thread', { devMods: [content, genContent, ato] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
}

const F = (tier: string, kind: string) => `mekanism:${tier}_${kind}_factory`;
const OS = 'mekanism:osmium', O = 'mekanism:oxygen', HCL = 'mekanism:hydrogen_chloride', FUNGI = 'mekanism:fungi';
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** progress by process, Joules, the tank's mB and `used_so_far` by process; the slots. */
async function row(pos: string): Promise<{ r: any[]; slots: any[]; p: any }> {
  const p = await mek(`mek probe ${pos}`);
  return { r: [p.progress, p.energy, p.tank?.amount ?? 0, p.used_so_far], slots: p.slots ?? [], p };
}
const count = (slots: any[], i: number) => slots[i]?.count ?? 0;
const tank = (chemical: string, mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:${mb}L}}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const upgrades = (list: [number, number][]) => `{component_upgrade:{upgrades:[${list.map(([t, n]) => `{type:${t},amount:${n}}`).join(',')}]}}`;
const items = (list: [number, string, number][]) => `{Items:[${list.map(([s, id, n]) => `{Slot:${s}b,id:"${id}",count:${n}}`).join(',')}]}`;
const three = (item: string, n = 2) => items([[0, item, n], [2, item, n], [4, item, n]]);
async function place(pos: string, block: string, ...nbt: string[]) {
  await fixture.command(`setblock ${pos} ${block}[facing=north]`);
  for (const n of nbt) await fixture.command(`data merge block ${pos} ${n}`);
}
const sum = (list: number[]) => list.reduce((s, v) => s + v, 0);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(40, 5, 10)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(40, -2, 10)} minecraft:stone`);
  await fixture.hold();

  // A: the stores and tanks.
  const stores: Record<string, [number, number]> = {};
  const kinds: [string, string, number, number][] = [
    ['basic', 'compressing', 120000, 630], ['basic', 'purifying', 120000, 630], ['basic', 'injecting', 240000, 630], ['basic', 'infusing', 30000, 3000],
    ['ultimate', 'compressing', 360000, 1890], ['elite', 'injecting', 560000, 1470], ['ultimate', 'infusing', 90000, 9000],
  ];
  for (const [i, [tier, kind]] of kinds.entries()) {
    await place(at(2 * i, 0, 8), F(tier, kind), energy(99999999));
    const p = (await row(at(2 * i, 0, 8))).p;
    stores[`${tier}_${kind}`] = [p.capacity, p.tank?.capacity];
    assert.equal(p.processes, { basic: 3, elite: 7, ultimate: 9 }[tier]);
  }
  log.stores = stores;
  assert.deepEqual(Object.values(stores), kinds.map(k => [k[2], k[3]]));
  fixture.mark('their stores are max(storage / 2, usage) * processes; their one tank 210 mB a process, 1000 for infusing');

  // B: a basic compressing factory, a dust a process; C: 2 mB for three; D: 8 speed upgrades.
  const b = at(0), c = at(2), d = at(4);
  const dust = three('mekanism:dust_refined_obsidian');
  await place(b, F('basic', 'compressing'), tank(OS, 630), dust, energy(120000));
  await place(c, F('basic', 'compressing'), tank(OS, 2), dust, energy(120000));
  await place(d, F('basic', 'compressing'), tank(OS, 630), dust, upgrades([[0, 8]]), energy(120000));
  const rows: any[] = [], fast: any[] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    rows.push((await row(b)).r);
    fast.push((await row(d)).r);
  }
  const short = (await row(c)).r;
  log.compressing = { rows, fast, short };
  assert.deepEqual(rows, [[[1, 1, 1], 119700, 627, [1, 1, 1]], [[2, 2, 2], 119400, 624, [2, 2, 2]], [[3, 3, 3], 119100, 621, [3, 3, 3]]]);
  assert.deepEqual(fast.slice(0, 2), [[[1, 1, 1], 43200, 582, [16, 16, 16]], [[2, 1, 1], 17600, 566, [32, 16, 16]]], '8 speed: 16 mB and 25 600 J a process');
  assert.deepEqual(fast[2], fast[1], 'then short of Joules');
  assert.deepEqual(short, [[1, 1, 0], 119800, 0, [1, 1, 0]], 'the first two took the 2 mB; the third waits');
  await fixture.stepServer(196);
  const t199 = (await row(b)).r;
  await fixture.stepServer(1);
  const t200 = await row(b);
  log.compressing_end = { t199, t200: t200.r, slots: t200.slots };
  assert.deepEqual(t199, [[199, 199, 199], 60300, 33, [199, 199, 199]]);
  assert.deepEqual(t200.r, [[0, 0, 0], 60000, 30, [0, 0, 0]], 'an ingot a process: 200 mB each, the use so far back to 0');
  assert.deepEqual([1, 3, 5].map(s => [t200.slots[s]?.item, count(t200.slots, s)]), [1, 3, 5].map(() => ['mekanism:ingot_refined_obsidian', 1]));
  fixture.mark('a basic compressing factory takes 1 mB a process a tick from its one tank, 16 with 8 speed upgrades');

  // E: its chemical slot (slot 6): an osmium ingot adds 200 mB only when all of it fits; its right gives osmium.
  const e = at(6);
  await place(e, F('basic', 'compressing'), tank(OS, 431), items([[6, 'mekanism:ingot_osmium', 2]]));
  await fixture.stepServer(1);
  const kept = await row(e);
  await fixture.command(`data merge block ${e} ${tank(OS, 430)}`);
  await fixture.stepServer(1);
  const filled = await row(e);
  log.slot = { kept: [kept.r[2], count(kept.slots, 6)], filled: [filled.r[2], count(filled.slots, 6)] };
  assert.deepEqual([kept.r[2], count(kept.slots, 6), filled.r[2], count(filled.slots, 6)], [431, 2, 630, 1]);
  assert.equal((await mek(`mek chemical ${e} west extract 10`)).got.amount, 10, 'its right (west) gives');
  assert.equal((await mek(`mek chemical ${e} east insert ${OS} 5`)).rest.amount, 0, 'its other sides take osmium');
  assert.equal((await mek(`mek chemical ${e} east insert ${O} 5`)).rest.amount, 5, 'no compressing recipe takes oxygen');
  fixture.mark('its chemical slot fills the tank when all of a conversion fits; its right gives the chemical');

  // F: a basic purifying factory, raw osmium a process (one an operation), at random; G: a basic injecting factory
  // (three raw osmium an operation, Mekanism's `from_raw_ore`).
  const f = at(8), g = at(10);
  await place(f, F('basic', 'purifying'), tank(O, 630), three('mekanism:raw_osmium'), energy(120000));
  await place(g, F('basic', 'injecting'), tank(HCL, 630), three('mekanism:raw_osmium', 3), energy(240000));
  const uses: number[] = [], inj: number[] = [];
  let [lastF, lastG] = [630, 630];
  let usedAt100 = 0, energyG = 0;
  for (let t = 1; t <= 200; t++) {
    await fixture.stepServer(1);
    const [nf, ng] = [await row(f), await row(g)];
    uses.push(lastF - nf.r[2]);
    inj.push(lastG - ng.r[2]);
    [lastF, lastG] = [nf.r[2], ng.r[2]];
    if (t === 100) usedAt100 = sum(nf.r[3]);
    if (t === 1) energyG = ng.r[1];
  }
  const [fEnd, gEnd] = [await row(f), await row(g)];
  log.purifying = { uses, total: sum(uses), usedAt100, end: fEnd.r, slots: fEnd.slots };
  log.injecting = { inj, total: sum(inj), energyG, end: gEnd.r, slots: gEnd.slots };
  assert(uses.every(u => u >= 0 && u <= 9) && inj.every(u => u >= 0 && u <= 9), 'each process draws 0 to 3 mB a tick');
  assert(uses.some(u => u === 0 || u === 1) && uses.some(u => u >= 4), `drawn at random: ${uses}`);
  assert.equal(usedAt100, sum(uses.slice(0, 100)), 'the use so far by process adds up to what the tank lost');
  assert(sum(uses) >= 420 && sum(uses) <= 630 && sum(inj) >= 420 && sum(inj) <= 630, `about 545 mB in 200 ticks: ${sum(uses)}, ${sum(inj)}`);
  assert.equal(energyG, 238800, 'the injecting factory: 400 J a process');
  for (const [end, made] of [[fEnd, 'alltheores:osmium_clump'], [gEnd, 'alltheores:osmium_shard']] as [any, string][]) {
    assert.deepEqual(end.r[0], [0, 0, 0]);
    assert.deepEqual(end.r[3], [0, 0, 0]);
    assert.deepEqual([1, 3, 5].map(s => end.slots[s]?.item), [made, made, made]);
  }
  assert.equal((await mek(`mek chemical ${f} west insert ${O} 5`)).rest.amount, 0, 'the purifying factory takes oxygen on its sides');
  assert.equal((await mek(`mek chemical ${f} west extract 5`)).got.amount, 0, 'and gives none');
  fixture.mark('the purifying and injecting factories draw each process\'s use at random');

  // H: a basic infusing factory: 10 mB of fungi with each netherrack on the 200th tick; I: 15 mB for three.
  const h = at(12), i = at(14);
  const rack = three('minecraft:netherrack');
  await place(h, F('basic', 'infusing'), tank(FUNGI, 3000), rack, energy(30000));
  await place(i, F('basic', 'infusing'), tank(FUNGI, 15), rack, energy(30000));
  await fixture.stepServer(1);
  const h1 = (await row(h)).r;
  await fixture.stepServer(199);
  const [h200, i200] = [await row(h), await row(i)];
  log.infusing = { h1, h200: h200.r, i200: i200.r, slots: i200.slots };
  assert.deepEqual(h1, [[1, 1, 1], 29850, 3000, [0, 0, 0]], '50 J a process, the fungi taken at the end');
  assert.deepEqual(h200.r.slice(0, 3), [[0, 0, 0], 0, 2970]);
  assert.deepEqual([1, 3, 5].map(s => h200.slots[s]?.item), ['minecraft:crimson_nylium', 'minecraft:crimson_nylium', 'minecraft:crimson_nylium']);
  assert.deepEqual(i200.r.slice(0, 3), [[0, 0, 0], 100, 5], 'the first took 10 mB; the others, short, start over');
  assert.deepEqual([count(i200.slots, 0), i200.slots[1]?.item, count(i200.slots, 2), count(i200.slots, 3)], [1, 'minecraft:crimson_nylium', 2, 0]);
  fixture.mark('the infusing factory takes 10 mB with each item; short of it a process starts over');

  // J: tier installers from each machine (and a factory one tier up).
  const machines: [string, string, string, string][] = [
    ['mekanism:osmium_compressor', OS, 'mekanism:dust_refined_obsidian', 'compressing'],
    ['mekanism:purification_chamber', O, 'mekanism:raw_osmium', 'purifying'],
    ['mekanism:chemical_injection_chamber', HCL, 'mekanism:raw_osmium', 'injecting'],
    ['mekanism:metallurgic_infuser', FUNGI, 'minecraft:netherrack', 'infusing'],
  ];
  const installed: any[] = [];
  for (const [n, [machine, chemical, input, kind]] of machines.entries()) {
    const pos = at(16 + 2 * n);
    // The advanced machines: input 0, chemical slot 1; the infuser: infusion slot 0, input 1.
    const [inSlot, chemSlot, chemItem] = kind === 'infusing' ? [1, 0, 'minecraft:crimson_fungus'] : [0, 1, kind === 'compressing' ? 'mekanism:ingot_osmium' : 'minecraft:flint'];
    // In slot order: a fresh block entity's item list takes a merged one in order.
    const held0: [number, string, number][] = [[inSlot, input, 3], [chemSlot, chemItem, 1]];
    await place(pos, machine, tank(chemical, 100), items(held0.sort((a, b) => a[0] - b[0])), energy(5000), kind === 'infusing' ? '{progress:50}' : '{progress:50,used_so_far:50L}');
    const held = (await row(pos)).p.slots;
    const made = await mek(`mek install ${pos} basic_tier_installer`);
    const p = (await row(pos)).p;
    installed.push({ held, made: made.block, tank: p.tank, used: p.used_so_far, progress: p.progress, slots: [p.slots[0], p.slots[6]], energy: p.energy });
    log.installed = installed;
    assert.equal(made.block, F('basic', kind), `${machine}: ${JSON.stringify(made)}`);
    assert.deepEqual([p.tank?.chemical, p.tank?.amount, p.tank?.capacity], [chemical, 100, kind === 'infusing' ? 3000 : 630], 'the tank comes along');
    assert.deepEqual(p.used_so_far, kind === 'infusing' ? [0, 0, 0] : [50, 0, 0], 'its use so far into process 0');
    assert.deepEqual(p.progress, [50, 0, 0]);
    assert.deepEqual([p.slots[0]?.item, count(p.slots, 0), p.slots[6]?.item, p.energy], [input, 3, chemItem, 5000], `the input to process 0, the chemical slot to the factory's (the machine held ${JSON.stringify(held)})`);
  }
  const up = await mek(`mek install ${at(16)} advanced_tier_installer`);
  const upProbe = (await row(at(16))).p;
  installed.push({ up: up.block, tank: upProbe.tank, used: upProbe.used_so_far });
  log.installed = installed;
  assert.equal(up.block, F('advanced', 'compressing'));
  assert.deepEqual([upProbe.tank?.amount, upProbe.tank?.capacity, upProbe.used_so_far], [100, 1050, [50, 0, 0, 0, 0]]);
  fixture.mark('tier installers turn the four machines into their factories with their tanks, and a factory one tier up');

  // K: the screen of a basic compressing factory: its slots, the chemical bar.
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${b}`), v => v.block === F('basic', 'compressing'), 'the factory reaches the client');
  await fixture.raw(`interact ${b} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the factory screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], 'three processes, the chemical slot, energy and the upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'chemical-factory.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its slots and its chemical bar');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK chemical-factories', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK chemical-factories', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
