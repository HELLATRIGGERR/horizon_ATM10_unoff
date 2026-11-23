/** Just Dire Things' Paradox Machine, as the `justdirethings` native bundle and the content mod made from the carry of
 * the player's jar (the machine on the furnace template) make it.
 *
 * - Its screen opens on a click with no slot of its own; the radius button on X widens the area (the row in front of
 *   it) to three blocks, and the snapshot button keeps the ores there (the tag `justdirethings:paradox_allow`; stone
 *   is left out) and the pig standing in it.
 * - The ores mined and the pig killed, the machine holds Forge Energy and time fluid (set in its saved compound). On its redstone pulse (its default mode) it restores the two ores and the pig (same UUID) after 30 ticks,
 *   for 750,000 FE and 150 mB, and holds 0.75 paradox energy.
 * - A picture of the screen goes to the run folder (`paradox.png`).
 *
 * Reads the saved compound (`data get block`, Java's keys plus `energy` and `timeFluid`). Needs the staged native
 * build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`); `pack/scripts/justdirethings.ts
 * world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-paradox', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => /Test passed/i.test(line));
const is = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
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
const pig = async () => passed(await fixture.command('execute if entity @e[type=minecraft:pig,name=Paradox]'));

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 3;
  await fixture.command(`fill ${x - 3} ${y} ${z - 3} ${x + 3} ${y + 3} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 3} ${x + 3} ${y - 1} ${z + 3} minecraft:stone`);
  await fixture.command('gamerule spawn_mobs false');
  const pos = `${x} ${y} ${z}`;
  const iron = `${x - 1} ${y} ${z - 1}`, coal = `${x + 1} ${y} ${z - 1}`;
  // The fixture window is 960x540 at GUI scale 2: the 256x214 screen is centred in 480x270.
  const [ox, oy] = [Math.trunc((480 - 256) / 2), Math.trunc((270 - 214) / 2)];
  const at = (gx: number, gy: number) => `click-at ${ox + gx} ${oy + gy}`;

  await fixture.command(`setblock ${pos} justdirethings:paradoxmachine[facing=north]`);
  await fixture.command(`setblock ${iron} minecraft:iron_ore`);
  await fixture.command(`setblock ${coal} minecraft:coal_ore`);
  await fixture.command(`setblock ${x - 1} ${y} ${z - 2} minecraft:stone`);
  await fixture.command(`summon minecraft:pig ${x + 0.5} ${y} ${z - 0.5} {NoAI:1b,PersistenceRequired:1b,CustomName:"Paradox"}`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${pos}`), v => v.block === 'justdirethings:paradoxmachine', 'the machine reaches the client');
  await fixture.raw(`interact ${pos} south`);
  let view = await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the paradox machine screen opens');
  const own = ((view.slots ?? []) as any[]).filter(s => s.slot >= 300 && s.slot < 400);
  assert.deepEqual(own, [], `no slot of its own: ${JSON.stringify(view.slots)}`);
  let nbt = await until(() => saved(pos), s => s.includes('redstoneMode'), 'the machine has ticked');
  assert.equal(field(nbt, 'redstoneMode'), 3, 'pulse by default');
  assert.equal(field(nbt, 'zOffset'), -1, 'the area is the block in front (north)');

  // The X radius's plus, twice: one block either side.
  await fixture.raw(at(40 + 25 + 30 + 6, 20 + 12 + 6));
  await until(() => saved(pos), s => field(s, 'xRadiusDouble') === 0.5, 'radius X 0.5');
  await Bun.sleep(500);
  await fixture.raw(at(40 + 25 + 30 + 6, 20 + 12 + 6));
  await until(() => saved(pos), s => field(s, 'xRadiusDouble') === 1, 'radius X 1.0');
  await fixture.raw(at(40 + 116 + 8, 20 + 62 + 8));
  nbt = await until(() => saved(pos), s => s.includes('minecraft:iron_ore') && s.includes('minecraft:pig'), 'the snapshot is taken');
  log.snapshot = nbt.slice(nbt.indexOf('snapshotData'), nbt.indexOf('snapshotData') + 400);
  assert(nbt.includes('minecraft:iron_ore') && nbt.includes('minecraft:coal_ore'), 'the ores are kept');
  assert(!nbt.includes('"minecraft:stone"'), 'stone is not in the tag');
  fixture.mark('the screen has no slot of its own; its X radius button widens the area; the snapshot keeps the two ores and the pig');

  await fixture.raw('key escape tap');
  await until(() => fixture.raw('screen'), v => v.screen !== 'container', 'the screen closes');
  await fixture.command(`setblock ${iron} minecraft:air`);
  await fixture.command(`setblock ${coal} minecraft:air`);
  await fixture.command('kill @e[type=minecraft:pig]');
  await until(() => pig(), v => !v, 'the pig is gone');
  await fixture.command('kill @e[type=minecraft:item]');

  // The bucket's own route (`ParadoxMachine.useItemOn`) needs `use_item_on`, which the engine gates on a trait mod's Part: see NOTES.
  await fixture.command(`data merge block ${pos} {energy:10000000, timeFluid:1000}`);
  nbt = await until(() => saved(pos), s => field(s, 'timeFluid') === 1000, 'the tank holds the time fluid');
  assert.equal(field(nbt, 'energy'), 10000000);
  await Bun.sleep(2000);
  assert(await is(iron, 'minecraft:air'), 'no pulse, no run');
  fixture.mark('the machine holds energy and time fluid and waits for its pulse');

  await fixture.command(`setblock ${x + 1} ${y} ${z} minecraft:redstone_block`);
  await until(() => is(iron, 'minecraft:iron_ore'), v => v, 'the run restores the iron ore', 20000);
  assert(await is(coal, 'minecraft:coal_ore'), 'and the coal ore');
  await until(() => pig(), v => v, 'and the pig');
  nbt = await until(() => saved(pos), s => field(s, 'isRunning') === 0, 'the run ends');
  log.after = { energy: field(nbt, 'energy'), fluid: field(nbt, 'timeFluid'), paradox: field(nbt, 'paradoxEnergy') };
  assert.equal(field(nbt, 'timeFluid'), 850, '150 mB for two blocks and a mob');
  assert.equal(field(nbt, 'energy'), 10000000 - 750000, '750,000 FE');
  assert.equal(field(nbt, 'paradoxEnergy'), 0.75, 'a quarter of paradox energy each');
  assert(await is(`${x - 1} ${y} ${z - 2}`, 'minecraft:stone'));
  fixture.mark(`a pulse restores the two ores and the pig for 750,000 FE and 150 mB; paradox energy 0.75`);

  await fixture.command('clear @s');
  await fixture.command('gamemode creative @s');
  await fixture.raw(`interact ${pos} south`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the screen opens again');
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'paradox.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;

  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS paradox', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS paradox', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
