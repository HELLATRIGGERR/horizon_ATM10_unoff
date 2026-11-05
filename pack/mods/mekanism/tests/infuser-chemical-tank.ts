/** The Metallurgic Infuser's infusion tank as a chemical tank (docs/mekanism-chemicals.md, slice 1): Mekanism 10.7's
 * `BasicChemicalTank` of 1000 mB behind its chemical side configuration (`setupIOConfig(CHEMICAL, tank, RIGHT)`:
 * the right side `OUTPUT`, the rest `INPUT`), as `TileEntityMetallurgicInfuser` builds it: any automation may take
 * out, another block may put in an infuse type a recipe takes with the item in the input slot
 * (`containsRecipeBA`), one chemical at a time, read and written in Mekanism's saved form.
 *
 * Facing north, the right side is west. Checked with `/mek chemical` (the handler as another block meets it) and
 * `data get block`: redstone goes in through the back and not the right, comes out through the right and not the
 * back; hydrogen, which no infusing recipe takes, stays out; with netherrack in the input slot only fungi goes in; a
 * saved chemical the registry lacks reads as an empty tank; a tag-based recipe takes the registry's tag members. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-infuser-tank', 'server-thread', { devMods: [content] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/machine-usage.toml'))) {
  for (const file of ['general.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n');
}

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
function tankIn(nbt: string): [string, number] {
  const entry = nbt.match(/chemical_tanks: ?\[(.*?)\]/)?.[1] ?? '';
  return [entry.match(/id: ?"([^"]+)"/)?.[1] ?? '', Number(entry.match(/amount: ?(\d+)L/)?.[1] ?? 0)];
}
const items = (list: [number, string, number][]) => `{Items:[${list.map(([slot, id, count]) => `{Slot:${slot}b,id:"${id}",count:${count}}`).join(',')}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(12, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(12, -2, 8)} minecraft:stone`);
  await fixture.hold();

  const a = at(0);
  await fixture.command(`setblock ${a} mekanism:metallurgic_infuser`);
  await fixture.stepServer(1);
  const ins = (face: string, chemical: string, amount: number, sim = false) => mek(`mek chemical ${a} ${face} insert ${chemical} ${amount}${sim ? ' simulate' : ''}`);
  const out = (face: string, amount: number) => mek(`mek chemical ${a} ${face} extract ${amount}`);

  // In through an input side, not through the right (west).
  assert.equal((await ins('south', 'mekanism:redstone', 100, true)).rest.amount, 0, 'simulated: all of it fits');
  assert.deepEqual(tankIn(await saved(a)), ['', 0], 'and nothing moved');
  assert.equal((await ins('west', 'mekanism:redstone', 100)).rest.amount, 100, 'the right side gives out only');
  assert.equal((await ins('south', 'mekanism:redstone', 1200)).rest.amount, 200, '1000 mB fit');
  assert.deepEqual(tankIn(await saved(a)), ['mekanism:redstone', 1000]);
  assert.equal((await ins('up', 'mekanism:fungi', 10)).rest.amount, 10, 'one chemical at a time');
  fixture.mark('an infuse type goes in through an input side, up to 1000 mB, one at a time; not through the right');

  // Out through the right, not through an input side or the null side.
  assert.deepEqual((await out('south', 10)).got, { id: null, amount: 0 });
  assert.deepEqual((await out('none', 10)).got, { id: null, amount: 0 });
  assert.deepEqual((await out('west', 300)).got, { id: 'mekanism:redstone', amount: 300 });
  assert.deepEqual(tankIn(await saved(a)), ['mekanism:redstone', 700]);
  const tanks = await mek(`mek chemical ${a} none tanks`);
  assert.deepEqual(tanks.tanks, [{ id: 'mekanism:redstone', amount: 700, capacity: 1000 }]);
  fixture.mark('it comes out through the right only; the null side reads it');

  // Hydrogen is no infuse type a recipe takes; with netherrack in the input only fungi go in.
  await out('west', 1000);
  assert.equal((await ins('south', 'mekanism:hydrogen', 10)).rest.amount, 10, 'no infusing recipe takes hydrogen');
  await fixture.command(`data merge block ${a} ${items([[1, 'minecraft:netherrack', 1]])}`);
  await fixture.stepServer(1);
  assert.equal((await ins('south', 'mekanism:redstone', 10)).rest.amount, 10, 'no recipe takes netherrack with redstone');
  assert.equal((await ins('south', 'mekanism:fungi', 10)).rest.amount, 0, 'fungi and netherrack make crimson nylium');
  fixture.mark('only what a recipe takes with the input goes in (containsRecipeBA)');

  // A saved chemical the registry lacks reads as an empty tank; a known one in Mekanism's form.
  const b = at(3);
  await fixture.command(`setblock ${b} mekanism:metallurgic_infuser`);
  await fixture.command(`data merge block ${b} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:no_such_chemical",amount:50L}}]}`);
  assert.equal((await mek(`mek chemical ${b} none tanks`)).tanks[0].amount, 0);
  await fixture.command(`data merge block ${b} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:carbon",amount:40}}]}`);
  assert.deepEqual((await mek(`mek chemical ${b} none tanks`)).tanks, [{ id: 'mekanism:carbon', amount: 40, capacity: 1000 }], 'an int amount reads too');
  // Infuser A: netherrack and the fungi put in through its side run the recipe that takes `#mekanism:fungi`.
  await fixture.command(`data merge block ${a} {energy_containers:[{container:0b,stored:20000L}]}`);
  await fixture.stepServer(1);
  const probe = await mek(`mek probe ${a}`);
  log.probe = probe;
  assert.equal(probe.recipe, 'mekanism:metallurgic_infusing/netherrack_to_crimson_nylium');
  assert.equal(probe.progress, 1);
  await fixture.hold(false);
  fixture.mark('an unknown saved chemical reads empty; the #mekanism:fungi recipe takes fungi put in by another block');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK infuser tank', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK infuser tank', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
