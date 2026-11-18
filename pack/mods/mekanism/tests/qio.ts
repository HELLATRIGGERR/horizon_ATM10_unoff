/** Mekanism's QIO, slice 1 (pack/mods/mekanism/NOTES.md, "QIO"), as the `mekanism` native bundle makes it: a QIO
 * Drive Array holding a base drive (16 000 items, 128 types: `QIODriveTier.BASE`) on frequency "Test", a QIO
 * Dashboard put on the same frequency from its screen (the frequency tab, a typed name, Set: a `mekanism:qio`
 * mod message). Shift-clicks put the player's stacks into the frequency (`quickMoveStack`); the viewer shows them,
 * its search narrows them; a shift-click on a viewer cell takes a stack into the inventory, a click takes one to
 * the cursor and a click on the viewer puts it back. The items live on the drive (its custom data), so a restart
 * keeps them: the array registers its drive again and the dashboard shows the same list. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-qio', 'server-thread', { devMods: [content, genContent] });

async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const shot = (name: string) => fixture.raw(`screenshot ${join(fixture.run, name).replaceAll('\\', '/')}`);
const counts = (p: any) => Object.fromEntries((p.items ?? []).map((i: any) => [i.id, i.count]));
/** The player's inventory slots holding `item`, by the screen's slot ids. */
async function slotsOf(item: string): Promise<number[]> {
  const view = await fixture.raw('screen');
  return (view.slots ?? []).filter((s: any) => s.slot < 36 && s.item === item).map((s: any) => s.slot);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.min(Math.floor(py), 78), z = Math.floor(pz) + 3;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(6, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(6, -1, 4)} minecraft:stone`);
  const array = at(0), dash = at(3);
  await fixture.command(`setblock ${array} mekanism:qio_drive_array[facing=south]`);
  await fixture.command(`data merge block ${array} {Items:[{Slot:0b,id:"mekanism:qio_drive_base",count:1}]}`);
  await fixture.command(`setblock ${dash} mekanism:qio_dashboard[facing=south]`);
  assert.equal((await mek(`mek qio ${array} freq Test`)).set, true);
  await fixture.step(5);
  const a0 = await mek(`mek probe ${array}`);
  log.array = a0;
  assert.deepEqual([a0.registered, a0.drives, a0.capacity, a0.type_capacity, a0.total], [1, 1, 16000, 128, 0], 'the base drive joins "Test"');

  // The dashboard's frequency from its screen.
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const [ox, oy] = [Math.trunc((960 / scale - 176) / 2), Math.trunc((540 / scale - 222) / 2)];
  // `click-at` takes no shift (`click-slot` does), so a shift-click holds the key around the click.
  const click = async (dx: number, dy: number, extra = '') => {
    if (extra === 'shift') await fixture.raw('key shift down');
    try { return await fixture.raw(`click-at ${ox + dx} ${oy + dy}${extra === 'right' ? ' right' : ''}`); }
    finally { if (extra === 'shift') await fixture.raw('key shift up'); }
  };
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  await until(() => fixture.raw(`block ${dash}`), v => v.block === 'mekanism:qio_dashboard', 'the dashboard reaches the client');
  const open = async () => {
    await fixture.raw(`interact ${dash} south`);
    await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the dashboard screen opens');
    await Bun.sleep(300);
  };
  await open();
  await click(-13, 19); // the frequency tab
  await click(40, 36); // the name field
  await fixture.raw('type Test');
  await click(157, 36); // Set
  const d0 = await until(() => mek(`mek probe ${dash}`), p => p.frequency?.name === 'Test', 'the typed frequency reaches the dashboard', 15000);
  assert.equal(d0.frequency.public, true);
  await click(-13, 19); // back to the viewer
  fixture.mark('the drive array holds a base drive on "Test"; the dashboard joins it from its frequency tab');

  // Stacks in by shift-clicks.
  await fixture.command('clear @s');
  await fixture.command('give @s minecraft:cobblestone 64');
  await fixture.command('give @s minecraft:iron_ingot 20');
  await fixture.command('give @s minecraft:diamond 5');
  await Bun.sleep(500);
  for (const item of ['minecraft:cobblestone', 'minecraft:iron_ingot', 'minecraft:diamond']) {
    const [slot] = await slotsOf(item);
    assert(slot !== undefined, `${item} in the inventory`);
    await fixture.raw(`click-slot ${slot} shift`);
  }
  const d1 = await until(() => mek(`mek probe ${dash}`), p => p.total === 89, 'the three stacks reach the frequency');
  log.inserted = d1;
  assert.deepEqual(counts(d1), { 'minecraft:cobblestone': 64, 'minecraft:iron_ingot': 20, 'minecraft:diamond': 5 });
  assert.deepEqual([d1.types, d1.capacity], [3, 16000]);
  const a1 = await mek(`mek probe ${array}`);
  assert.deepEqual(a1.slots, [{ slot: 0, item: 'mekanism:qio_drive_base', count: 89, types: 3 }], 'the drive holds them');
  assert.deepEqual(await slotsOf('minecraft:cobblestone'), [], 'they left the inventory');
  await until(() => mek(`mek probe ${dash}`), p => p.viewer?.types === 3, 'the viewer data lists three types');
  await Bun.sleep(800);
  await shot('qio-dashboard.png');
  fixture.mark('shift-clicks put 64 cobblestone, 20 iron and 5 diamonds into the frequency, onto the drive');

  // The search narrows the viewer to iron; a shift-click on its first cell takes the 20 into the inventory.
  await click(60, 34);
  await fixture.raw('type iron');
  await Bun.sleep(500);
  await shot('qio-search.png');
  await click(16, 51, 'shift');
  const d2 = await until(() => mek(`mek probe ${dash}`), p => p.total === 69, 'the iron leaves the frequency');
  assert.deepEqual(counts(d2), { 'minecraft:cobblestone': 64, 'minecraft:diamond': 5 });
  await until(() => slotsOf('minecraft:iron_ingot'), v => v.length === 1, 'the iron is in the inventory');
  fixture.mark('the search shows iron alone; a shift-click on it takes all 20 into the inventory');

  // A click takes a stack of cobblestone to the cursor, a click on the viewer puts it back.
  await click(60, 34, 'right'); // clears the search
  await Bun.sleep(300);
  await click(16, 51);
  const d3 = await until(() => mek(`mek probe ${dash}`), p => p.total === 5, 'a stack of cobblestone to the cursor');
  assert.deepEqual(counts(d3), { 'minecraft:diamond': 5 });
  await click(70, 90);
  const d4 = await until(() => mek(`mek probe ${dash}`), p => p.total === 69, 'the cursor stack goes back');
  log.back = counts(d4);
  fixture.mark('a click takes 64 cobblestone to the cursor and a click on the viewer puts them back');
  await fixture.raw('close');

  // The items are the drive's: a restart keeps them.
  await fixture.restart();
  await fixture.step(5);
  const a5 = await until(() => mek(`mek probe ${array}`), p => p.registered === 1, 'the array registers its drive again');
  const d5 = await mek(`mek probe ${dash}`);
  log.restart = { array: a5.slots, dash: counts(d5) };
  assert.equal(d5.frequency?.name, 'Test');
  assert.deepEqual(counts(d5), { 'minecraft:cobblestone': 64, 'minecraft:diamond': 5 });
  assert.deepEqual(a5.slots, [{ slot: 0, item: 'mekanism:qio_drive_base', count: 69, types: 2 }]);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  await until(() => fixture.raw(`block ${dash}`), v => v.block === 'mekanism:qio_dashboard', 'the dashboard reaches the client again');
  await open();
  await until(() => mek(`mek probe ${dash}`), p => p.viewer?.types === 2, 'the viewer lists two types after the restart');
  await Bun.sleep(800);
  await shot('qio-restart.png');
  await fixture.raw('close');
  fixture.mark('after a restart the drive still holds 64 cobblestone and 5 diamonds and the dashboard shows them');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK qio', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK qio', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
