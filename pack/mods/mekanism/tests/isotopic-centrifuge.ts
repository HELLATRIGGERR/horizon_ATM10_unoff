/** Mekanism's Isotopic Centrifuge, as the `mekanism` native bundle makes it under ATM10's configs
 * (`machine-usage.toml` `isotopicCentrifuge` 200 J an operation, `machine-storage.toml` 80 000 J). Measured on a
 * NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs over RCON (local `mekmach3-work/lab/m1.py`):
 * nuclear waste with 1000 J makes 1 mB of plutonium a tick for 200 J (one operation without speed upgrades), 1, 2, 3
 * mB over three ticks with 800, 600, 400 J left. Mekanism's own recipe takes 10 mB of waste an operation, ATM10's 5.
 * Waste goes in on its input sides, plutonium comes out in front, a bounding block stands above it while it is
 * there; its screen shows its fill, drain and energy slots and the upgrade's. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-centrifuge', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
const atm10 = process.env.ATM10_DIR;
assert(atm10 && existsSync(join(atm10, 'config/Mekanism/machine-storage.toml')), 'ATM10_DIR with its Mekanism configs');
mkdirSync(config, { recursive: true });
for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) {
  copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const IC = 'mekanism:isotopic_centrifuge', WASTE = 'mekanism:nuclear_waste';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(8, 5, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(8, -2, 6)} minecraft:stone`);
  await fixture.hold();

  const c1 = at(0);
  await fixture.command(`setblock ${c1} ${IC}[facing=north]`);
  await fixture.command(`data merge block ${c1} {energy_containers:[{container:0b,stored:1000L}],chemical_tanks:[{tank:0b,stored:{id:"${WASTE}",amount:1000L}}]}`);
  const rows: number[][] = [];
  for (let t = 1; t <= 3; t++) {
    await fixture.stepServer(1);
    const p = await mek(`mek probe ${c1}`);
    rows.push([p.energy, p.tanks?.[0]?.amount ?? 0, p.tanks?.[1]?.amount ?? 0, p.energy_used]);
  }
  log.rows = rows;
  const probe0 = await mek(`mek probe ${c1}`);
  log.probe = { recipe: probe0.recipe, per_tick: probe0.per_tick, capacity: probe0.capacity, recipes: probe0.recipes };
  const waste = 1000 - rows[0][1];
  assert(waste === 5 || waste === 10, `waste an operation: ${waste}`);
  assert.deepEqual(rows, [[800, 1000 - waste, 1, 200], [600, 1000 - 2 * waste, 2, 200], [400, 1000 - 3 * waste, 3, 200]]);
  assert.deepEqual([probe0.per_tick, probe0.capacity], [200, 80000]);
  assert(probe0.recipes > 0, 'the centrifuging recipes are read');
  fixture.mark(`1 mB of plutonium a tick for 200 J, ${waste} mB of waste an operation`);

  assert.equal((await mek(`mek chemical ${c1} west insert ${WASTE} 5`)).rest.amount, 0);
  assert.equal((await mek(`mek chemical ${c1} north extract 1`)).got.amount, 1, 'plutonium out in front (north)');
  assert.equal((await mek(`mek chemical ${c1} west insert mekanism:oxygen 5`)).rest.amount, 5, 'no recipe takes oxygen');
  fixture.mark('waste in on its input sides, plutonium out in front');

  // Placed from its item, a bounding block goes above it, and leaves with it.
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  const c2 = at(4, -1);
  await fixture.command(`item replace entity @s weapon.mainhand with ${IC}`);
  await until(() => fixture.raw(`block ${at(4, -2)}`), v => v.block === 'minecraft:stone', 'the floor reaches the client');
  await fixture.raw(`interact ${at(4, -2)} up`);
  await until(() => fixture.raw(`block ${c2}`), v => v.block === IC, 'the centrifuge is placed');
  await until(() => fixture.raw(`block ${at(4)}`), v => v.block === 'mekanism:bounding_block', 'its bounding block above');
  await fixture.command(`setblock ${c2} minecraft:air`);
  await until(() => fixture.raw(`block ${at(4)}`), v => v.block === 'minecraft:air', 'the bounding block goes with it');
  fixture.mark('a bounding block above it while it stands');

  await until(() => fixture.raw(`block ${c1}`), v => v.block === IC, 'the centrifuge reaches the client');
  await fixture.raw(`interact ${c1} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'its screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4], 'its fill, drain, energy and upgrade slots');
  await Bun.sleep(1500);
  await fixture.raw(`screenshot ${join(fixture.run, 'centrifuge.png')}`);
  await fixture.raw('close');
  fixture.mark('its screen shows its five slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK centrifuge', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK centrifuge', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
