/** Just Dire Things' Dropper T2, as the `justdirethings` native bundle and the content mod made from the carry of the player's jar (on the furnace template) make it.
 *
 * - Its screen opens with nine slots of its own and nine filter slots.
 * - With no energy it drops nothing; filled with Forge Energy (a `data merge` of its buffer) it drops one item of its first
 *   slot at the area's offset (the block in front), 25 FE a drop.
 * - With dirt in its filter it drops only the dirt of its slots, the cobblestone stays.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-dropper2', 'server-thread', { devMods: [content] });
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
  await fixture.command(`fill ${x - 3} ${y} ${z - 6} ${x + 9} ${y + 3} ${z + 4} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 6} ${x + 9} ${y - 1} ${z + 4} minecraft:stone`);
  await fixture.command('gamemode survival @s');
  await fixture.command('gamerule spawn_mobs false');
  const pos = `${x} ${y} ${z}`, front = `${x} ${y} ${z - 1}`;
  const open = async (id: string) => {
    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === id, 'the machine reaches the client');
  };
  const screen = async () => {
    await fixture.raw(`interact ${pos} south`);
    return until(() => fixture.raw('screen'), v => v.screen === 'container' && !!(slot(v, 300) || slot(v, 301)), 'the machine screen opens');
  };
  const put = async (item: string, count: number, n: number) => {
    await fixture.replace('hotbar.0', item, count);
    await fixture.raw('click-slot 0');
    await until(() => fixture.raw('screen'), v => v.carried?.item === item, `${item} on the cursor`);
    await fixture.raw(`click-slot ${300 + n}`);
    await until(() => fixture.raw('screen'), v => !v.carried || v.carried.item !== item, `${item} in the slot`);
  };
  const shoot = async (file: string) => {
    await Bun.sleep(1000);
    const shot = join(fixture.run, file);
    await fixture.raw(`screenshot ${shot}`);
    log[file] = shot;
  };
  const own = (view: any) => (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);

  const item = (id: string) => `@e[type=minecraft:item,nbt={Item:{id:"${id}"}}]`;
  const exists = async (selector: string) => passed(await fixture.command(`execute if entity ${selector}`));
  await fixture.command(`setblock ${pos} justdirethings:droppert2[facing=north]`);
  await open('justdirethings:droppert2');
  const view = await screen();
  assert.deepEqual(own(view), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17], `nine slots and nine filter slots: ${JSON.stringify(view.slots)}`);
  await put('minecraft:cobblestone', 3, 0);
  await put('minecraft:dirt', 3, 1);
  await shoot('dropper2.png');
  await Bun.sleep(3000);
  assert(!(await exists(item('minecraft:cobblestone'))), 'no energy, no drop');
  fixture.mark('the Dropper T2 shows nine slots and nine filter slots; without energy it drops nothing');
  await fixture.command(`data merge block ${pos} {energy:100000}`);
  await until(() => exists(item('minecraft:cobblestone')), v => v, 'with energy it drops the first slot', 30000);
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.count === 2, 'one cobblestone left the slot');
  const after = field(await saved(pos), 'energy');
  log.energy = after;
  assert(after <= 99975, `a drop takes 25 FE: ${after}`);
  fixture.mark(`with energy it drops one cobblestone (${after} FE left)`);
  await fixture.command('kill @e[type=minecraft:item]');
  await put('minecraft:dirt', 1, 9);
  await until(() => exists(item('minecraft:dirt')), v => v, 'with dirt in the filter it drops dirt', 30000);
  await until(() => fixture.raw('screen'), v => !slot(v, 301)?.item, 'the three dirt are dropped, one an operation', 90000);
  assert.equal((await fixture.raw('screen')).slots.find((s: any) => s.slot === 300)?.count, 2, 'the cobblestone stays');
  assert(!(await exists(item('minecraft:cobblestone'))), 'no cobblestone was dropped');
  fixture.mark('with dirt in its filter it drops the dirt and leaves the cobblestone');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS dropper2', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS dropper2', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
