/** Mekanism's Pigment Extractor, as the `mekanism` native bundle makes it under ATM10's configs
 * (`machine-usage.toml` `pigmentExtractor` 200 J a tick, `machine-storage.toml` 40 000 J; `TileEntityPigmentExtractor`:
 * 100 ticks, a 20 000 mB pigment tank, the pigment out of the right): a red dye makes 256 mB of red pigment on the
 * 100th tick; nothing goes in on any side, the pigment comes out of the right (west, facing north).
 * Its screen shows its input, pigment item and energy slots and the upgrade's. */

import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-pigment-extractor', 'server-thread', { devMods: [content, genContent] });
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

const EX = 'mekanism:pigment_extractor';

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(12, 5, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(12, -2, 6)} minecraft:stone`);
  await fixture.hold();

  const p1 = at(0);
  await fixture.command(`setblock ${p1} ${EX}[facing=north]`);
  await fixture.command(`data merge block ${p1} {energy_containers:[{container:0b,stored:30000L}],Items:[{Slot:0b,id:"minecraft:red_dye",count:3}]}`);
  const rows: number[][] = [];
  for (let t = 1; t <= 101; t++) {
    await fixture.stepServer(1);
    if (t <= 2 || [99, 100, 101].includes(t)) rows.push([t, ...(await row(p1)).r]);
  }
  log.rows = rows;
  const probe = (await row(p1)).p;
  log.probe = { required: probe.required, per_tick: probe.per_tick, capacity: probe.capacity, recipes: probe.recipes };
  assert.deepEqual([probe.required, probe.per_tick, probe.capacity], [100, 200, 40000]);
  assert(probe.recipes >= 150, 'the pigment extracting recipes are read');
  // [tick, progress, Joules, pigment mB, input, drain, energy slot]
  assert.deepEqual(rows[0].slice(0, 3), [1, 1, 29800]);
  assert.deepEqual(rows[2], [99, 99, 10200, 0, 3, 0, 0]);
  assert.deepEqual(rows[3], [100, 0, 10000, 256, 2, 0, 0], 'one red dye made 256 mB');
  assert.deepEqual(rows[4].slice(0, 4), [101, 1, 9800, 256]);
  fixture.mark('a red dye is 256 mB of red pigment over 100 ticks at 200 J a tick');

  assert.notEqual((await mek(`mek chemical ${p1} east insert mekanism:red 5`)).rest?.amount, 0, 'no input side');
  assert.equal((await mek(`mek chemical ${p1} west extract 5`)).got.amount, 5, 'the pigment out of the right (west, facing north)');
  assert.equal((await mek(`mek chemical ${p1} north extract 5`)).got?.amount ?? 0, 0, 'nothing out of the front');
  fixture.mark('the pigment comes out of the right only');

  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${p1}`), v => v.block === EX, 'the extractor reaches the client');
  await fixture.raw(`interact ${p1} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4], 'its input, pigment item, energy and upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'extractor.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its five slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK pigment-extractor', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK pigment-extractor', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
