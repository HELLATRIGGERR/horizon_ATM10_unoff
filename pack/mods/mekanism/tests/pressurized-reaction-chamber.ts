/** Mekanism's Pressurized Reaction Chamber, as the `mekanism` native bundle makes it under ATM10's configs
 * (`machine-usage.toml` `pressurizedReactionBase` 5 J a tick, `machine-storage.toml` 2000 J). The numbers are those a
 * NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (local
 * `mekmach3-work/lab/m1.py`, rows in `m1.json`):
 *
 * - Substrate from 2 bio fuel, 10 mB of water and 100 mB of hydrogen (100 ticks): 5 J a tick, progress 1, 2, 3 ...;
 *   on the 100th tick the progress is back at 0, 2 bio fuel, 10 mB of water and 100 mB of hydrogen gone, one
 *   substrate and 100 mB of ethene made; the next one starts on the 101st.
 * - A saved progress of 60 goes on (61 after a tick) and finishes on the 40th tick; with 300 J it stops at 20 of the
 *   next with its Joules gone, the progress kept.
 * - Hydrogen goes in on its input sides, oxygen (no reaction with bio fuel takes it) doesn't; ethene comes out of the
 *   right; its screen shows its three slots and the upgrade's. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-prc', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml')), 'ATM10_DIR with its Mekanism configs');
mkdirSync(config, { recursive: true });
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const PRC = 'mekanism:pressurized_reaction_chamber';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** progress, Joules, the input and output tanks' and the fluid tank's mB, the input's and output's counts. */
async function row(pos: string): Promise<{ r: number[]; p: any }> {
  const p = await mek(`mek probe ${pos}`);
  const slots = p.slots ?? [];
  return { r: [p.progress, p.energy, p.tanks?.[0]?.amount ?? 0, p.tanks?.[1]?.amount ?? 0, p.fluid?.amount ?? 0, slots[0]?.count ?? 0, slots[1]?.count ?? 0], p };
}
const nbt = (joules: number, extra = '') =>
  `{energy_containers:[{container:0b,stored:${joules}L}],fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:1000}}],` +
  `chemical_tanks:[{tank:0b,stored:{id:"mekanism:hydrogen",amount:1000L}}],Items:[{Slot:0b,id:"mekanism:bio_fuel",count:4}]${extra}}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(12, 5, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(12, -2, 6)} minecraft:stone`);
  await fixture.hold();

  const [p1, p2] = [at(0), at(3)];
  for (const [pos, data] of [[p1, nbt(2000)], [p2, nbt(300, ',progress:60')]]) {
    await fixture.command(`setblock ${pos} ${PRC}[facing=north]`);
    await fixture.command(`data merge block ${pos} ${data}`);
  }
  const rows: Record<string, number[][]> = { p1: [], p2: [] };
  for (let t = 1; t <= 101; t++) {
    await fixture.stepServer(1);
    if (t <= 3 || [40, 41, 99, 100, 101].includes(t)) {
      rows.p1.push([t, ...(await row(p1)).r]);
      rows.p2.push([t, ...(await row(p2)).r]);
    }
  }
  log.rows = rows;
  const probe = (await row(p1)).p;
  log.probe = { recipe: probe.recipe, required: probe.required, per_tick: probe.per_tick, capacity: probe.capacity, recipes: probe.recipes };
  assert.deepEqual(rows.p1.slice(0, 3), [[1, 1, 1995, 1000, 0, 1000, 4, 0], [2, 2, 1990, 1000, 0, 1000, 4, 0], [3, 3, 1985, 1000, 0, 1000, 4, 0]]);
  assert.deepEqual([probe.required, probe.per_tick, probe.capacity], [100, 5, 2000]);
  assert(probe.recipes > 0, 'the reaction recipes are read');
  fixture.mark('5 J a tick, 100 ticks for substrate');
  assert.deepEqual(rows.p1.slice(5), [[99, 99, 1505, 1000, 0, 1000, 4, 0], [100, 0, 1500, 900, 100, 990, 2, 1], [101, 1, 1495, 900, 100, 990, 2, 1]]);
  fixture.mark('on the 100th tick: 2 bio fuel, 10 mB of water and 100 mB of hydrogen into a substrate and 100 mB of ethene');
  assert.deepEqual(rows.p2[0].slice(0, 3), [1, 61, 295]);
  assert.deepEqual(rows.p2.find(r => r[0] === 40), [40, 0, 100, 900, 100, 990, 2, 1]);
  assert.deepEqual(rows.p2.find(r => r[0] === 99)?.slice(0, 3), [99, 20, 0]);
  fixture.mark('a saved 60 finishes on the 40th tick; out of Joules the progress stays');

  assert.equal((await mek(`mek chemical ${p1} east insert mekanism:hydrogen 5`)).rest.amount, 0);
  assert.equal((await mek(`mek chemical ${p1} east insert mekanism:oxygen 5`)).rest.amount, 5, 'no reaction with bio fuel takes oxygen');
  assert.equal((await mek(`mek chemical ${p1} west insert mekanism:hydrogen 5`)).rest.amount, 5, 'the right takes nothing in');
  assert.equal((await mek(`mek chemical ${p1} west extract 5`)).got.amount, 5, 'ethene out of the right (west, facing north)');
  fixture.mark('hydrogen in on its input sides, ethene out of the right');

  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${p1}`), v => v.block === PRC, 'the chamber reaches the client');
  await fixture.raw(`interact ${p1} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4], 'its input, output, energy and upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'prc.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its five slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK prc', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK prc', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
