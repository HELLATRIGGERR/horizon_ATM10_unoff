/** Mekanism's Nutritional Liquifier, as the `mekanism` native bundle makes it under ATM10's configs
 * (`machine-usage.toml` `nutritionalLiquifier` 200 J a tick, `machine-storage.toml` 40 000 J;
 * `TileEntityNutritionalLiquifier`: 100 ticks, a 10 000 mB paste tank, no recipe files: `getRecipe` builds one from the
 * food in the slot, `nutrition * 50` mB of `mekanism:nutritional_paste` and what eating it leaves, read from the
 * engine's food table, hook points API 436): an apple (4) makes 200 mB on the 100th tick, a mushroom stew (6) 300 mB and
 * its bowl; stone is no food. The paste fills a bucket in the fluid slot and ejects out of the right into a fluid tank.
 * Its screen shows its input, output, fluid, fluid output and energy slots and the upgrade's. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-nutritional-liquifier', 'server-thread', { devMods: [content, genContent] });
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
/** progress, Joules, paste mB, each slot's count (input, output, fluid slot, its output), from `/mek probe`. */
async function row(pos: string): Promise<{ r: number[]; p: any }> {
  const p = await mek(`mek probe ${pos}`);
  const slots = p.slots ?? [];
  return { r: [p.progress, p.energy, p.fluid?.amount ?? 0, ...[0, 1, 2, 3].map(i => slots[i]?.count ?? 0)], p };
}
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const paste = (mb: number) => `{fluid_tanks:[{tank:0b,stored:{id:"mekanism:nutritional_paste",amount:${mb}}}]}`;

const NL = 'mekanism:nutritional_liquifier';

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(20, 5, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(20, -2, 6)} minecraft:stone`);
  await fixture.hold();

  const [p1, p2, p3] = [at(0), at(4), at(8)];
  const place = async (pos: string, ...nbt: string[]) => {
    await fixture.command(`setblock ${pos} ${NL}[facing=north]`);
    for (const n of nbt) await fixture.command(`data merge block ${pos} ${n}`);
  };
  await place(p1, energy(30000), '{Items:[{Slot:0b,id:"minecraft:apple",count:3}]}');
  await place(p2, energy(30000), '{Items:[{Slot:0b,id:"minecraft:mushroom_stew",count:1}]}');
  await place(p3, energy(30000), '{Items:[{Slot:0b,id:"minecraft:stone",count:1}]}');
  const rows: Record<string, number[][]> = { p1: [], p2: [], p3: [] };
  for (let t = 1; t <= 101; t++) {
    await fixture.stepServer(1);
    if (t <= 2 || [99, 100, 101].includes(t)) {
      rows.p1.push([t, ...(await row(p1)).r]);
      rows.p2.push([t, ...(await row(p2)).r]);
      rows.p3.push([t, ...(await row(p3)).r]);
    }
  }
  log.rows = rows;
  const probe = (await row(p1)).p;
  log.probe = { required: probe.required, per_tick: probe.per_tick, capacity: probe.capacity, recipes: probe.recipes };
  assert.deepEqual([probe.required, probe.per_tick, probe.capacity], [100, 200, 40000]);
  assert(probe.recipes > 40, `the food table's foods: ${probe.recipes}`);
  // [tick, progress, Joules, paste mB, input, output, fluid slot, its output]
  assert.deepEqual(rows.p1[0].slice(0, 4), [1, 1, 29800, 0]);
  assert.deepEqual(rows.p1[3], [100, 0, 10000, 200, 2, 0, 0, 0]);
  assert.deepEqual(rows.p1[4].slice(0, 4), [101, 1, 9800, 200]);
  assert.deepEqual(rows.p2[3], [100, 0, 10000, 300, 0, 1, 0, 0]);
  assert.equal((await row(p2)).p.slots[1]?.item, 'minecraft:bowl');
  assert.equal((await row(p2)).p.fluid?.id, 'mekanism:nutritional_paste');
  assert.deepEqual(rows.p3[4].slice(0, 4), [101, 0, 30000, 0], 'stone is no food: no progress, no Joules');
  fixture.mark('an apple makes 200 mB of paste on the 100th tick at 200 J a tick, a mushroom stew 300 mB and its bowl; stone nothing');

  // The fluid slot: the tank fills a bucket, which goes to the slot below.
  const p4 = at(12);
  await place(p4, paste(1500), '{Items:[{Slot:2b,id:"minecraft:bucket",count:2}]}');
  await fixture.stepServer(1);
  const filled = (await row(p4)).p;
  log.bucket = [filled.fluid?.amount, filled.slots?.[2], filled.slots?.[3]];
  assert.equal(filled.fluid?.amount, 500, 'a bucket takes 1000 mB');
  assert.deepEqual([filled.slots?.[2]?.count, filled.slots?.[3]?.item, filled.slots?.[3]?.count], [1, 'mekanism:nutritional_paste_bucket', 1]);
  fixture.mark('the tank fills a bucket in the fluid slot into a paste bucket below');

  // Its paste ejects out of the right (west, facing north) into a fluid tank there.
  const p5 = at(16), west = at(15);
  await fixture.command(`setblock ${west} mekanism:basic_fluid_tank`);
  await place(p5, paste(700));
  await fixture.stepServer(2);
  const ejected = [(await row(p5)).p.fluid?.amount ?? 0, (await mek(`mek probe ${west}`)).amount ?? 0];
  log.ejected = ejected;
  assert.deepEqual(ejected, [0, 700], 'its paste leaves out of the right');
  fixture.mark('its paste ejects out of its right into a fluid tank');

  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${p2}`), v => v.block === NL, 'the machine reaches the client');
  await fixture.raw(`interact ${p2} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5, 6], 'its input, output, fluid, fluid output, energy and upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'liquifier.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its seven slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK nutritional-liquifier', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK nutritional-liquifier', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
