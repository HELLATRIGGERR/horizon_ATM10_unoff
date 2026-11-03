/** Mekanism's Metallurgic Infuser, as the `mekanism` native bundle and its content mod make it, under ATM10's
 * `machine-usage.toml` (50 J a tick) and `machine-storage.toml` (20 000 J). The numbers are those a NeoForge 1.21.1
 * server with Mekanism 10.7.19 gave for the same steps:
 *
 * - Crimson fungi in its infusion slot turn into fungi (`mekanism:chemical_conversion`), 10 mB a tick, one item a
 *   tick; its tank holds one infuse type and 1000 mB.
 * - Netherrack in its input with fungi in the tank and 20 000 J: crimson nylium after 200 ticks of 50 J, and the 10
 *   mB of fungi it takes are used at the end, not a tick at a time; with 5 mB it doesn't start.
 * - Its tank survives a save and restart; the dump button empties it.
 * - Its screen: the infusion, input, output and energy slots and the upgrade component's two (screenshot
 *   `infuser-main.png` in the run folder).
 *
 * The tank (`chemical_tanks`), Joules, progress and items are read from the saved compound (`data get block`). */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-infuser', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-usage.toml'))) {
  for (const file of ['general.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n');
  writeFileSync(join(config, 'machine-usage.toml'), 'metallurgicInfuser = 50\n');
  writeFileSync(join(config, 'machine-storage.toml'), 'metallurgicInfuser = 20000\n');
}

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const energyIn = (nbt: string) => Number(nbt.match(/energy_containers: ?\[\{[^\]]*stored: (\d+)L/)?.[1] ?? 0);
const progressIn = (nbt: string) => Number(nbt.match(/progress: ?(\d+)/)?.[1] ?? -1);
/** The tank's chemical and mB (`chemical_tanks: [{tank: 0b, stored: {id, amount}}]`). */
function tankIn(nbt: string): [string, number] {
  const entry = nbt.match(/chemical_tanks: ?\[(.*?)\]/)?.[1] ?? '';
  return [entry.match(/id: ?"([^"]+)"/)?.[1] ?? '', Number(entry.match(/amount: ?(\d+)L/)?.[1] ?? 0)];
}
function slotIn(nbt: string, slot: number, item: string): number {
  const list = nbt.match(/Items: ?\[(.*?)\]\s*(,|\})/)?.[1] ?? '';
  for (const entry of list.match(/\{[^{}]*\}/g) ?? []) {
    if (new RegExp(`Slot: ?${slot}b`).test(entry) && entry.includes(`"${item}"`)) return Number(entry.match(/count: ?(\d+)/)?.[1] ?? 1);
  }
  return 0;
}
const items = (list: [number, string, number][]) => `{Items:[${list.map(([slot, id, count]) => `{Slot:${slot}b,id:"${id}",count:${count}}`).join(',')}]}`;
const energy = (joules: number) => `{energy_containers:[{container:0b,stored:${joules}L}]}`;
const tank = (id: string, amount: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${id}",amount:${amount}L}}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(16, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(16, -2, 8)} minecraft:stone`);

  // A: crimson fungi into fungi, 10 mB a tick.
  await fixture.hold();
  const a = at(0);
  await fixture.command(`setblock ${a} mekanism:metallurgic_infuser`);
  await fixture.command(`data merge block ${a} ${items([[0, 'minecraft:crimson_fungus', 3]])}`);
  const filled: [string, number][] = [];
  for (let t = 0; t < 3; t++) {
    await fixture.stepServer(1);
    filled.push(tankIn(await saved(a)));
  }
  log.filled = filled;
  assert.deepEqual(filled, [['mekanism:fungi', 10], ['mekanism:fungi', 20], ['mekanism:fungi', 30]], 'one fungus a tick, 10 mB each');
  assert.equal(slotIn(await saved(a), 0, 'minecraft:crimson_fungus'), 0, 'all three used');
  fixture.mark('crimson fungi become fungi, 10 mB a tick');

  // Netherrack and 20 000 J: crimson nylium after 200 ticks, the fungi used at the end.
  await fixture.command(`data merge block ${a} ${energy(20000)}`);
  await fixture.command(`data merge block ${a} ${items([[1, 'minecraft:netherrack', 2]])}`);
  await fixture.stepServer(199);
  const almost = await saved(a);
  assert.equal(progressIn(almost), 199);
  assert.deepEqual(tankIn(almost), ['mekanism:fungi', 30], 'nothing taken from the tank yet');
  assert.equal(energyIn(almost), 10050);
  await fixture.stepServer(1);
  const done = await saved(a);
  log.done = done;
  assert.equal(slotIn(done, 2, 'minecraft:crimson_nylium'), 1, `crimson nylium on the 200th tick: ${done}`);
  assert.deepEqual(tankIn(done), ['mekanism:fungi', 20], '10 mB taken at the end');
  assert.equal(energyIn(done), 10000);
  assert.equal(slotIn(done, 1, 'minecraft:netherrack'), 1);
  fixture.mark('netherrack and 10 mB of fungi make crimson nylium in 200 ticks of 50 J');

  // B: 5 mB is not enough.
  const b = at(4);
  await fixture.command(`setblock ${b} mekanism:metallurgic_infuser`);
  await fixture.command(`data merge block ${b} ${energy(20000)}`);
  await fixture.command(`data merge block ${b} ${tank('mekanism:fungi', 5)}`);
  await fixture.command(`data merge block ${b} ${items([[1, 'minecraft:netherrack', 1]])}`);
  await fixture.stepServer(10);
  const short = await saved(b);
  assert.equal(progressIn(short), 0, `5 mB doesn't start it: ${short}`);
  assert.equal(energyIn(short), 20000, 'and uses no energy');
  fixture.mark('with 5 mB of fungi it doesn\'t start');

  // Save and restart: A's tank survives (its energy taken away first, so it stays put).
  await fixture.command(`data merge block ${a} ${energy(0)}`);
  await fixture.stepServer(1);
  const before = tankIn(await saved(a));
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  const after = tankIn(await saved(a));
  log.restart = { before, after };
  assert.deepEqual(after, before);
  await fixture.hold(false);
  fixture.mark('the tank survives a save and restart');

  // Its screen; the dump button empties the tank.
  await fixture.command(`data merge block ${b} ${tank('mekanism:fungi', 600)}`);
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${b}`), v => v.block === 'mekanism:metallurgic_infuser', 'the infuser reaches the client');
  await fixture.raw(`interact ${b} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the infuser screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1, 2, 3, 4, 5], 'infusion, input, output, energy and the two upgrade slots');
  assert.equal(view.title?.[0]?.text, 'Metallurgic Infuser');
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const [ox, oy] = [Math.trunc((960 / scale - 176) / 2), Math.trunc((540 / scale - 166) / 2)];
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'infuser-main.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw(`click-at ${ox + 26} ${oy + 64}`);
  await until(() => saved(b), s => tankIn(s)[1] === 0, 'the dump button empties the tank');
  await fixture.raw('close');
  fixture.mark('its screen shows six slots; the dump button empties the tank');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK infuser', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK infuser', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
