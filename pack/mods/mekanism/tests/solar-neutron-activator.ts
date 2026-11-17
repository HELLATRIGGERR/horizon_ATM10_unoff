/** Mekanism's Solar Neutron Activator, as the `mekanism` native bundle makes it under ATM10's `general.toml`
 * (`maxSolarNeutronActivatorRate` 256). Measured on a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's
 * configs over RCON (local `mekmach3-work/lab/m1.py`), in plains (0.8, downfall 0.4: a peak of 256 * 0.88):
 * at noon in clear weather 225 operations a tick (225, 450, 675 mB of polonium over three ticks), at time 12000 158,
 * at night none, at noon after 200 ticks of rain 33 (a fifth of the dimmed sun). An input short of a second
 * operation runs one and keeps the rest. It takes no Joules and no upgrades. Mekanism's own recipe takes 10 mB of
 * waste an operation, ATM10's 5. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-sna', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/general.toml')), 'ATM10_DIR with its Mekanism configs');
mkdirSync(config, { recursive: true });
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const SNA = 'mekanism:solar_neutron_activator', WASTE = 'mekanism:nuclear_waste';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const tanks = async (pos: string) => {
  const p = await mek(`mek probe ${pos}`);
  return [p.tanks?.[0]?.amount ?? 0, p.tanks?.[1]?.amount ?? 0];
};

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(20, 30, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(20, -2, 6)} minecraft:stone`);
  await fixture.command(`fillbiome ${at(-3, -2, -3)} ${at(20, 8, 6)} minecraft:plains`);
  await fixture.command('time set 6000');
  await fixture.command('weather clear');
  await fixture.hold();
  const place = async (pos: string, mb: number) => {
    await fixture.command(`setblock ${pos} ${SNA}[facing=north]`);
    await fixture.command(`data merge block ${pos} {chemical_tanks:[{tank:0b,stored:{id:"${WASTE}",amount:${mb}L}}]}`);
  };

  const [s1, s2] = [at(0), at(3)];
  await place(s1, 10000);
  await place(s2, 7);
  const rows: number[][] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    rows.push([...(await tanks(s1)), ...(await tanks(s2))]);
  }
  log.rows = rows;
  const probe = await mek(`mek probe ${s1}`);
  log.probe = { recipe: probe.recipe, capacity: probe.capacity, energy_used: probe.energy_used, recipes: probe.recipes, active: probe.active };
  const waste = (10000 - rows[0][0]) / 225;
  assert(waste === 5 || waste === 10, `waste an operation: ${JSON.stringify(rows)}`);
  assert.deepEqual(rows.map(r => r[1]), [225, 450, 675], 'noon, clear, plains: 225 operations a tick');
  assert.deepEqual(rows.map(r => [r[2], r[3]]), [[7 - waste, 1], [7 - waste, 1], [7 - waste, 1]], 'short of a second operation: one');
  assert.equal(probe.capacity, 0, 'no Joules');
  fixture.mark(`225 operations a tick at noon (${waste} mB of waste each)`);

  await fixture.command('time set 12000');
  const s4 = at(6);
  await place(s4, 10000);
  await fixture.stepServer(1);
  const dusk = (await tanks(s4))[1];
  await fixture.command('time set 18000');
  const s3 = at(9);
  await place(s3, 10000);
  await fixture.stepServer(2);
  const night = (await tanks(s3))[1];
  await fixture.command('time set 6000');
  await fixture.command('weather rain');
  await fixture.stepServer(200);
  const s5 = at(12);
  await place(s5, 10000);
  await fixture.stepServer(1);
  const rain = (await tanks(s5))[1];
  await fixture.command('weather clear');
  log.sky = { dusk, night, rain };
  assert.deepEqual([dusk, night, rain], [158, 0, 33], 'time 12000, night, rain at noon');
  fixture.mark('158 at time 12000, none at night, 33 in rain');

  assert.equal((await mek(`mek chemical ${s1} west insert ${WASTE} 5`)).rest.amount, 0);
  assert.equal((await mek(`mek chemical ${s1} north extract 5`)).got.amount, 5, 'polonium out in front (north)');
  fixture.mark('waste in on its input sides, polonium out in front');

  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${s1}`), v => v.block === SNA, 'the activator reaches the client');
  await fixture.raw(`interact ${s1} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1], 'its fill and drain slots, no energy or upgrade slot');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'sna.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its two slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK sna', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK sna', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
