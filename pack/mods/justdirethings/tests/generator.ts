/** Just Dire Things' Generator T1, as the `justdirethings` native bundle and the content mod made from the carry of
 * the player's jar (the generator on the furnace template) make it.
 *
 * - Its screen opens on a click: one fuel slot (the hopper menu's four others hidden), which takes coal and refuses
 *   dirt.
 * - The redstone button cycles the mode; in "high" with no signal, the coal stays in the slot. A redstone block beside
 *   it starts the burn: one coal burns 400 ticks (1,600 at the config's 4 times) at 60 FE a tick (15 FE a burn tick
 *   at 4 times), the slot drops to one, and the buffer climbs by 60 a tick.
 * - A picture of the screen, burning, goes to the run folder (`generator.png`).
 *
 * Reads the saved compound (`data get block`, Java's keys plus `energy`). Needs the staged native build
 * (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`); `pack/scripts/justdirethings.ts world` sets
 * both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-generator', 'server-thread', { devMods: [content] });

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => {
  const found = nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?[\\d.]+)`));
  return found ? Number(found[1]) : 0;
};
/** The count of the item in slot `slot` of the saved `Items`, 0 for none. */
function slotCount(nbt: string, slot: number): number {
  const items = nbt.slice(nbt.indexOf('Items: ['));
  for (const entry of items.match(/\{[^{}]*\}/g) ?? []) {
    if (new RegExp(`Slot: ?${slot}b`).test(entry)) return field(entry, 'count');
  }
  return 0;
}
const gametime = async () => Number(((await fixture.command('time query gametime')).chat ?? []).join(' ').match(/(\d+)/)?.[1]);
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 2} ${y} ${z - 2} ${x + 2} ${y + 3} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 2} ${x + 2} ${y - 1} ${z + 3} minecraft:stone`);
  const pos = `${x} ${y} ${z}`;
  // The fixture window is 960x540 at GUI scale 2: the 176x214 screen is centred in 480x270.
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];

  await fixture.replace('hotbar.0', 'minecraft:coal', 2);
  await fixture.replace('hotbar.1', 'minecraft:dirt', 1);
  await fixture.command(`setblock ${pos} justdirethings:generatort1`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5}`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:generatort1', 'the generator reaches the client');
  await fixture.raw(`interact ${pos} south`);
  let view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the generator screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `the fuel slot only: ${JSON.stringify(view.slots)}`);

  // Dirt is no fuel: the slot refuses it.
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:dirt', 'dirt on the cursor');
  await fixture.raw('click-slot 300');
  await Bun.sleep(300);
  view = await fixture.raw('screen');
  assert.equal(view.carried?.item, 'minecraft:dirt', 'the fuel slot refuses dirt');
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the dirt back in the hotbar');

  // The redstone button: ignored -> low -> high.
  const button = [ox + 104 + 8, oy + 58 + 8];
  await fixture.raw(`click-at ${button[0]} ${button[1]}`);
  await until(() => saved(pos), s => field(s, 'redstoneMode') === 1, 'the button sets "low"');
  // The button cycles from the mode the screen shows: let the menu's data reach the client first.
  await Bun.sleep(1000);
  await fixture.raw(`click-at ${button[0]} ${button[1]}`);
  await until(() => saved(pos), s => field(s, 'redstoneMode') === 2, 'the button sets "high"');
  fixture.mark('the screen shows one fuel slot, which refuses dirt; its redstone button cycles ignored, low, high');

  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'minecraft:coal', 'coal on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), v => !v.carried, 'coal in the fuel slot');
  await Bun.sleep(2000);
  let nbt = await saved(pos);
  assert.equal(slotCount(nbt, 0), 2, `"high" without a signal burns nothing: ${nbt}`);
  assert.equal(field(nbt, 'energy'), 0);
  fixture.mark('in "high" with no signal the coal stays in the slot');

  await fixture.command(`setblock ${x + 1} ${y} ${z} minecraft:redstone_block`);
  nbt = await until(() => saved(pos), s => slotCount(s, 0) === 1, 'a signal starts the burn');
  const t0 = await gametime(), e0 = field(nbt, 'energy');
  assert.equal(field(nbt, 'maxBurn'), 400, 'coal burns 1,600 / 4 ticks');
  assert.equal(field(nbt, 'fuelBurnMultiplier'), 1);
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'generator.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await Bun.sleep(2000);
  nbt = await saved(pos);
  const t1 = await gametime(), e1 = field(nbt, 'energy');
  const rate = (e1 - e0) / (t1 - t0);
  log.rate = { e0, e1, ticks: t1 - t0 };
  assert(Math.abs(rate - 60) <= 6, `60 FE a tick: ${e1 - e0} FE in ${t1 - t0} ticks`);
  assert.equal((e1 % 60), 0, 'whole ticks of 60 FE');
  assert(field(nbt, 'feRemaining') === 24000 - e1, `the coal's 24,000 FE: ${field(nbt, 'feRemaining')} left at ${e1}`);
  fixture.mark(`a signal starts the burn: one coal, 400 ticks, 60 FE a tick (${e1 - e0} FE in ${t1 - t0} ticks)`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS generator', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS generator', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
