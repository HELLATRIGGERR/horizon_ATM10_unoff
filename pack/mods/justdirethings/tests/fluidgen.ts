/** Just Dire Things' Fluid Generator T1, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar
 * (the machine on the furnace template) make it.
 *
 * - Its screen opens on a click with one slot; a water bucket in it stays (the tank takes refined fuel only).
 * - A bucket of tier 2 refined fuel in its place: the tank takes it, the bucket comes back empty, and the generator burns
 *   a millibucket a tick for 450 FE each into its buffer.
 * - The redstone button set to "high" with no signal: it burns nothing more.
 * - A picture of its screen goes to the run folder (`fluidgen.png`).
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-fluidgen', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => Number(nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?\\d+)`))?.[1] ?? NaN);
const tank = (nbt: string) => {
  const at = nbt.indexOf('Fluid:');
  if (at < 0) return { amount: 0, id: '' };
  const part = nbt.slice(at, at + 120);
  return { amount: Number(part.match(/amount: ?(\d+)/)?.[1] ?? 0), id: part.match(/id: ?"([^"]+)"/)?.[1] ?? '' };
};
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 3} ${y} ${z - 6} ${x + 3} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 6} ${x + 3} ${y - 1} ${z + 4} minecraft:stone`);
  await fixture.command('gamemode survival @s');
  await fixture.command('gamerule spawn_mobs false');
  const pos = `${x} ${y} ${z}`, front = `${x} ${y} ${z - 1}`;
  const [ox, oy] = [Math.trunc((480 - 176) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;
  const open = async (id: string) => {
    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === id, 'the machine reaches the client');
  };
  const screen = async () => {
    await fixture.raw(`interact ${pos} south`);
    return until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the machine screen opens');
  };
  const put = async (item: string, count = 1) => {
    await fixture.replace('hotbar.0', item, count);
    await fixture.raw('click-slot 0');
    await until(() => fixture.raw('screen'), v => v.carried?.item === item, `${item} on the cursor`);
    await fixture.raw('click-slot 300');
    await until(() => fixture.raw('screen'), v => !v.carried || v.carried.item !== item, `${item} in the slot`);
  };
  const shoot = async (file: string) => {
    await Bun.sleep(1000);
    const shot = join(fixture.run, file);
    await fixture.raw(`screenshot ${shot}`);
    log[file] = shot;
  };

  await fixture.command(`setblock ${pos} justdirethings:generatorfluidt1[facing=north]`);
  await open('justdirethings:generatorfluidt1');
  let view = await screen();
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0], `one slot of its own: ${JSON.stringify(view.slots)}`);
  await put('minecraft:water_bucket');
  await Bun.sleep(2500);
  let nbt = await saved(pos);
  assert.equal(tank(nbt).amount, 0, 'water is no fuel');
  assert.equal(field(nbt, 'energy'), 0);
  await fixture.replace('hotbar.0', 'justdirethings:refined_t2_fluid_bucket', 1);
  await fixture.raw('click-slot 0');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'justdirethings:refined_t2_fluid_bucket', 'the fuel bucket on the cursor');
  await fixture.raw('click-slot 300');
  nbt = await until(() => saved(pos), s => field(s, 'energy') > 0, 'the generator makes energy');
  assert.equal(tank(nbt).id, 'justdirethings:refined_t2_fluid_source');
  assert(tank(nbt).amount < 1000, 'it burns the fuel');
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'minecraft:bucket', 'the bucket comes back empty');
  fixture.mark('water stays out; the tier 2 fuel bucket fills the tank and burns for energy');
  assert.equal(field(nbt, 'energy') % 450, 0, 'a millibucket of tier 2 fuel is 450 FE');
  await shoot('fluidgen.png');
  const mode = at(104 + 8, 58 + 8);
  await fixture.raw(mode);
  await until(() => saved(pos), s => field(s, 'redstoneMode') === 1, 'low');
  await Bun.sleep(600);
  await fixture.raw(mode);
  await until(() => saved(pos), s => field(s, 'redstoneMode') === 2, 'high');
  const frozen = await saved(pos);
  await Bun.sleep(2500);
  const later = await saved(pos);
  assert.equal(field(later, 'energy'), field(frozen, 'energy'), 'high with no signal: nothing more burns');
  assert.equal(tank(later).amount, tank(frozen).amount);
  fixture.mark('the redstone button on "high" with no signal stops it');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS fluidgen', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS fluidgen', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
