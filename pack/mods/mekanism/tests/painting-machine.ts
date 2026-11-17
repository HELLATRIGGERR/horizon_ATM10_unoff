/** Mekanism's Painting Machine, as the `mekanism` native bundle makes it under ATM10's configs
 * (`machine-usage.toml` `paintingMachine` 100 J a tick, 40 000 J; `TileEntityPaintingMachine`: 200 ticks, a 15 000 mB
 * pigment tank, an item and 256 mB of a pigment for an item, taken on the last tick): a dye base and red pigment make a
 * red dye, white wool a red wool (`neoforge:difference` ingredients); pigment goes in on all sides, nothing comes out of
 * the tank. Its screen shows its pigment, input, output and energy slots and the upgrade's. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-painting-machine', 'server-thread', { devMods: [content, genContent] });
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

const PM = 'mekanism:painting_machine';

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
  await fixture.command(`setblock ${p1} ${PM}[facing=north]`);
  await fixture.command(`data merge block ${p1} {energy_containers:[{container:0b,stored:30000L}],chemical_tanks:[${tank(0, 'mekanism:red', 1000)}],Items:[{Slot:1b,id:"mekanism:dye_base",count:3}]}`);
  await fixture.command(`setblock ${p2} ${PM}[facing=north]`);
  await fixture.command(`data merge block ${p2} {energy_containers:[{container:0b,stored:30000L}],chemical_tanks:[${tank(0, 'mekanism:red', 1000)}],Items:[{Slot:1b,id:"minecraft:white_wool",count:2}]}`);
  const rows: Record<string, number[][]> = { p1: [], p2: [] };
  for (let t = 1; t <= 201; t++) {
    await fixture.stepServer(1);
    if (t <= 2 || [199, 200, 201].includes(t)) {
      rows.p1.push([t, ...(await row(p1)).r]);
      rows.p2.push([t, ...(await row(p2)).r]);
    }
  }
  log.rows = rows;
  const probe = (await row(p1)).p;
  log.probe = { required: probe.required, per_tick: probe.per_tick, capacity: probe.capacity, recipes: probe.recipes };
  assert.deepEqual([probe.required, probe.per_tick, probe.capacity], [200, 100, 40000]);
  assert.equal(probe.recipes, 176, 'the painting recipes are read (11 sets of the 16 colours)');
  // [tick, progress, Joules, pigment mB, pigment slot, input, output]
  assert.deepEqual(rows.p1[0].slice(0, 4), [1, 1, 29900, 1000]);
  assert.deepEqual(rows.p1[3], [200, 0, 10000, 744, 0, 2, 1]);
  assert.deepEqual(rows.p2[3], [200, 0, 10000, 744, 0, 1, 1]);
  assert.equal((await row(p1)).p.slots[2]?.item, 'minecraft:red_dye');
  assert.equal((await row(p2)).p.slots[2]?.item, 'minecraft:red_wool');
  fixture.mark('a dye base and 256 mB of red pigment make a red dye on the 200th tick; so do white wool, red wool');

  assert.equal((await mek(`mek chemical ${p1} east insert mekanism:red 5`)).rest.amount, 0, 'pigment in on every side');
  assert.equal((await mek(`mek chemical ${p1} west extract 5`)).got?.amount ?? 0, 0, 'nothing comes out');
  fixture.mark('the pigment goes in, nothing comes out');

  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${p1}`), v => v.block === PM, 'the machine reaches the client');
  await fixture.raw(`interact ${p1} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5], 'its pigment, input, output, energy and upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'painting.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its six slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK painting-machine', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK painting-machine', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
