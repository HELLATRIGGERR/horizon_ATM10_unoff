/** Mekanism's Pigment Mixer, as the `mekanism` native bundle makes it under ATM10's configs
 * (`machine-usage.toml` `pigmentMixer` 200 J a tick, 80 000 J; `TileEntityPigmentMixer`: two 1000 mB input tanks and a
 * 2000 mB output, `2^speed` operations a tick): aqua and yellow make lime (1 + 1 for 2) an operation a tick; eight
 * operations a tick under three speed upgrades; the left side takes into the left tank, nothing but a pigment goes
 * in, the front gives out. Its screen shows its three pigment item slots, its energy slot and the upgrade's. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-pigment-mixer', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml')), 'ATM10_DIR with its Mekanism configs');
mkdirSync(config, { recursive: true });
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** progress, Joules, each tank's mB, each slot's count, from `/mek probe`. */
async function row(pos: string): Promise<{ r: number[]; p: any }> {
  const p = await mek(`mek probe ${pos}`);
  const slots = p.slots ?? [];
  return { r: [p.progress, p.energy, ...(p.tanks ?? []).map((t: any) => t.amount ?? 0), ...[0, 1, 2].map(i => slots[i]?.count ?? 0)], p };
}
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const tank = (i: number, id: string, mb: number) => `{tank:${i}b,stored:{id:"${id}",amount:${mb}L}}`;

const MX = 'mekanism:pigment_mixer';

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(12, 5, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(12, -2, 6)} minecraft:stone`);
  await fixture.hold();

  const [p1, p2] = [at(0), at(4)];
  const tanks = `chemical_tanks:[${tank(0, 'mekanism:aqua', 100)},${tank(1, 'mekanism:yellow', 100)}]`;
  await fixture.command(`setblock ${p1} ${MX}[facing=north]`);
  await fixture.command(`data merge block ${p1} {${tanks},energy_containers:[{container:0b,stored:80000L}]}`);
  await fixture.command(`setblock ${p2} ${MX}[facing=north]`);
  await fixture.command(`data merge block ${p2} {${tanks},energy_containers:[{container:0b,stored:80000L}],component_upgrade:{upgrades:[{type:0,amount:3}]}}`);
  const rows: Record<string, number[][]> = { p1: [], p2: [] };
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    rows.p1.push([t, ...(await row(p1)).r]);
    rows.p2.push([t, ...(await row(p2)).r]);
  }
  log.rows = rows;
  const probe = (await row(p1)).p;
  log.probe = { per_tick: probe.per_tick, capacity: probe.capacity, recipes: probe.recipes };
  assert.deepEqual([probe.per_tick, probe.capacity], [200, 80000]);
  assert(probe.recipes >= 16, 'the pigment mixing recipes are read');
  // [tick, progress, Joules, left, right, output, ...]
  assert.deepEqual(rows.p1.map(r => r.slice(1, 6)), [[0, 79800, 99, 99, 2], [0, 79600, 98, 98, 4], [0, 79400, 97, 97, 6]]);
  assert.deepEqual(rows.p2[0].slice(1, 6), [0, 67200, 92, 92, 16], 'eight operations a tick at eight times the Joules under three speed upgrades');
  fixture.mark('aqua and yellow make 2 mB of lime an operation, 200 J each, 2^speed a tick');

  const q = at(0, 0, 3);
  await fixture.command(`setblock ${q} ${MX}[facing=north]`);
  assert.equal((await mek(`mek chemical ${q} east insert mekanism:aqua 5`)).rest.amount, 0, 'aqua in on the left');
  assert.equal((await mek(`mek chemical ${q} east insert mekanism:hydrogen 5`)).rest.amount, 5, 'no recipe takes hydrogen');
  assert.notEqual((await mek(`mek chemical ${q} south insert mekanism:yellow 5`)).rest?.amount, 0, 'the back takes nothing');
  await fixture.command(`data merge block ${q} {chemical_tanks:[${tank(2, 'mekanism:lime', 50)}]}`);
  assert.equal((await mek(`mek chemical ${q} north extract 5`)).got.amount, 5, 'lime out of the front');
  fixture.mark('the left takes pigments in, the front gives lime out');

  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${p1}`), v => v.block === MX, 'the mixer reaches the client');
  await fixture.raw(`interact ${p1} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5], 'its three pigment item slots, energy and upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'mixer.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its six slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK pigment-mixer', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK pigment-mixer', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
