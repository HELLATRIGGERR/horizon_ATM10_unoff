/** Mekanism's Dynamic Tank, as the `mekanism` native bundle makes it under ATM10's configs. The numbers are those a
 * NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave for the same builds over RCON (local
 * `mekm2-work/oracle`, x6: a comparator on a valve tells formed, the manager's saved data the contents):
 *
 * - Forms: a hollow box of dynamic tank casings with valves in its sides, from 3x3x3 to 18x18x18 (19 long or
 *   3x2x3 doesn't); it holds `volume * 350 000` mB of a fluid (9 450 000 for 3x3x3, 2 041 200 000 for 18x18x18)
 *   and `volume * 16 000 000` of a chemical, the volume being the whole box's.
 * - Refuses: a hole in a side or an edge, stone in a side, glass or a valve on an edge, dirt inside, a casing
 *   touching it from outside; glass in a side or beside it is fine. The Configurator's plain use on one that
 *   doesn't form says why when there is a reason (`Couldn't create frame, invalid block at ...`).
 * - The valve fills and drains: an active creative tank above the top valve fills it in a tick; an ultimate pipe
 *   pulling from a side valve takes 32 000 a tick. A chemical goes in through a valve while no fluid is in, and a
 *   fluid isn't taken while a chemical is.
 * - Contents survive: a casing broken leaves the cache holding what it held; put back, the tank forms under a new
 *   id with all of it; a restart keeps it too (the level's saved data).
 * - The slots: an empty bucket in the input slot comes out of the output slot full, 1000 mB less in the tank. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-dynamic-tank', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const CASING = 'mekanism:dynamic_tank', VALVE = 'mekanism:dynamic_valve', GLASS = 'mekanism:structural_glass';
const WATER = '{active_state:1b,fluid_tanks:[{tank:0b,stored:{id:"minecraft:water",amount:2147483647}}]}';
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const tankOf = async (pos: string) => (await mek(`mek probe ${pos}`)).multiblocks?.[0];

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 1, 70), z = Math.floor(pz) - 10;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(40, 20, 18)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, 19)} ${at(40, 20, 40)} minecraft:air`);
  await fixture.hold();
  /** A hollow box of casings from `a` (dx, dy, dz) of size (sx, sy, sz), a valve in its top's middle and one in its
   * west side's middle. */
  const box = async (a: number[], s: number[]) => {
    await fixture.command(`fill ${at(a[0], a[1], a[2])} ${at(a[0] + s[0] - 1, a[1] + s[1] - 1, a[2] + s[2] - 1)} ${CASING} hollow`);
    const top = at(a[0] + (s[0] >> 1), a[1] + s[1] - 1, a[2] + (s[2] >> 1));
    const west = at(a[0], a[1] + (s[1] >> 1), a[2] + (s[2] >> 1));
    await fixture.command(`setblock ${top} ${VALVE}`);
    await fixture.command(`setblock ${west} ${VALVE}`);
    return { top, west };
  };

  // Forms at 3x3x3: its size by volume.
  const a = await box([0, 0, 0], [3, 3, 3]);
  await fixture.stepServer(2);
  const formed = await tankOf(a.top);
  log.formed = formed;
  assert(formed, `formed: ${JSON.stringify(await mek(`mek probe ${a.top}`))}`);
  assert.deepEqual(formed.size, [3, 3, 3]);
  assert.equal(formed.volume, 27);
  assert.equal(formed.fluid_capacity, 9_450_000, '27 * 350 000');
  assert.equal(formed.chemical_capacity, 432_000_000, '27 * 16 000 000');
  assert.deepEqual(formed.valves.map((v: any) => v.side).sort(), ['up', 'west']);
  fixture.mark('a 3x3x3 forms: 9 450 000 mB of fluid, 432 000 000 of chemical, valves looking up and west');

  // The valve fills: an active creative water tank above the top valve; then a pipe drains the west one.
  const above = at(1, 3, 1);
  await fixture.command(`setblock ${above} mekanism:creative_fluid_tank[active=true]`);
  await fixture.command(`data merge block ${above} ${WATER}`);
  await fixture.stepServer(2);
  await fixture.command(`setblock ${above} minecraft:air`);
  const full = await tankOf(a.top);
  assert.equal(full.fluid, 'minecraft:water');
  assert.equal(full.fluid_amount, 9_450_000, 'filled to its volume in a tick');
  const pipe = at(-1, 1, 1), sink = at(-2, 1, 1);
  await fixture.command(`setblock ${sink} mekanism:ultimate_fluid_tank`);
  await fixture.command(`setblock ${pipe} mekanism:ultimate_mechanical_pipe`);
  await mek(`mek connection ${pipe} east pull`);
  await fixture.stepServer(1);
  const before = (await tankOf(a.top)).fluid_amount;
  await fixture.stepServer(3);
  const drained = before - (await tankOf(a.top)).fluid_amount;
  log.drained = drained;
  assert.equal(drained, 3 * 32_000, 'an ultimate pipe pulls 32 000 a tick out of a valve');
  await fixture.command(`setblock ${pipe} minecraft:air`);
  fixture.mark(`the valves fill and drain: 9 450 000 mB in a tick from a creative tank, ${drained} out in 3 ticks`);

  // The slots: an empty bucket into the input slot comes out of the output full.
  const held = (await tankOf(a.top)).fluid_amount;
  await fixture.command(`data merge block ${a.west} {Items:[{Slot:0b,id:"minecraft:bucket",count:1}]}`);
  await fixture.stepServer(3);
  const slots = await tankOf(a.top);
  log.slots = slots.slots;
  assert.equal(slots.slots[0], null);
  assert.equal(slots.slots[1]?.item, 'minecraft:water_bucket');
  assert.equal(held - slots.fluid_amount, 1000);
  fixture.mark('an empty bucket in the input slot fills and goes to the output slot');

  // Break a corner: no tank, its cache keeps the water; put back: formed under a new id with all of it.
  const keep = slots.fluid_amount, oldId = slots.id;
  await fixture.command(`setblock ${at(0, 0, 0)} minecraft:air`);
  await fixture.stepServer(2);
  assert.equal(await tankOf(a.top), undefined, 'broken: no tank');
  const caches = await mek('mek multiblock caches');
  assert((caches.dynamicTank ?? []).some((c: any) => c.id === oldId && c.fluids[0].amount === keep), `the cache keeps ${keep}: ${JSON.stringify(caches)}`);
  await fixture.command(`setblock ${at(0, 0, 0)} ${CASING}`);
  await fixture.stepServer(2);
  const back = await tankOf(a.top);
  assert.equal(back?.fluid_amount, keep, 'formed again with what it held');
  assert.notEqual(back.id, oldId, 'under a new id');
  fixture.mark(`a corner broken and put back: ${keep} mB come back under a new id`);

  // Shapes that don't form, and some that do.
  const shapes: Record<string, boolean> = {};
  const trial = async (name: string, a0: number[], size: number[], edits: [number[], string][], expect: boolean) => {
    const t = await box(a0, size);
    for (const [d, block] of edits) await fixture.command(`setblock ${at(a0[0] + d[0], a0[1] + d[1], a0[2] + d[2])} ${block}`);
    await fixture.stepServer(2);
    shapes[name] = !!(await tankOf(t.top));
    assert.equal(shapes[name], expect, `${name}: ${JSON.stringify(await mek(`mek probe ${t.top}`))}`);
    return t;
  };
  await trial('hole_side', [6, 0, 0], [3, 3, 3], [[[1, 1, 0], 'minecraft:air']], false);
  await trial('hole_edge', [11, 0, 0], [3, 3, 3], [[[0, 2, 1], 'minecraft:air']], false);
  await trial('stone_side', [16, 0, 0], [3, 3, 3], [[[1, 1, 0], 'minecraft:stone']], false);
  const edge = await trial('glass_edge', [21, 0, 0], [3, 3, 3], [[[1, 0, 0], GLASS]], false);
  await trial('valve_edge', [26, 0, 0], [3, 3, 3], [[[1, 0, 0], VALVE]], false);
  await trial('dirt_inside', [31, 0, 0], [3, 3, 3], [[[1, 1, 1], 'minecraft:dirt']], false);
  await trial('flat_3x2x3', [0, 0, 6], [3, 2, 3], [], false);
  await trial('glass_side', [6, 0, 6], [3, 3, 3], [[[1, 1, 0], GLASS]], true);
  await trial('glass_beside', [11, 0, 6], [3, 3, 3], [[[3, 1, 1], GLASS]], true);
  await trial('casing_beside', [16, 0, 6], [3, 3, 3], [[[3, 1, 1], CASING]], false);
  const deep = await trial('deep_5', [21, 0, 6], [5, 5, 5], [], true);
  // A block deep inside goes unseen (no block of the tank is beside it); beside a wall it breaks the tank.
  await fixture.command(`setblock ${at(23, 2, 8)} minecraft:dirt`);
  await fixture.stepServer(2);
  shapes.deep_dirt = !!(await tankOf(deep.top));
  assert.equal(shapes.deep_dirt, true, 'dirt in the middle of a 5x5x5 goes unseen');
  await fixture.command(`setblock ${at(22, 2, 8)} minecraft:dirt`);
  await fixture.stepServer(2);
  shapes.wall_dirt = !!(await tankOf(deep.top));
  assert.equal(shapes.wall_dirt, false, 'dirt beside a wall breaks it');
  log.shapes = shapes;
  fixture.mark(`shapes as the oracle's: ${JSON.stringify(shapes)}`);

  // The Configurator's plain use on the one with glass on an edge says why.
  await fixture.hold(false);
  await fixture.replace('hotbar.0', 'mekanism:configurator');
  await fixture.select(0, 'mekanism:configurator');
  await fixture.command(`tp @s ${x + 22.5} ${y + 1} ${z - 2.5}`);
  await fixture.raw(`interact ${at(22, 1, 0)} north`);
  const said = await until(async () => ((await fixture.raw('chat-log 20')).chat ?? []).join('\n'), text => /Couldn't create frame, invalid block at \(/.test(text), 'the reason');
  log.configurator = said.split('\n').find((l: string) => l.includes("Couldn't create frame"));
  assert(log.configurator && String(log.configurator).includes(`(${x + 22}, ${y}, ${z})`), String(log.configurator));
  void edge;
  await fixture.hold();
  fixture.mark(`the Configurator on a tank with glass on an edge: ${log.configurator}`);

  // The biggest: 18x18x18 forms; 19 long doesn't.
  const big = await box([0, 0, 14], [18, 18, 18]);
  const long = await box([20, 0, 14], [19, 3, 3]);
  await fixture.stepServer(2);
  const bigTank = await tankOf(big.top);
  assert.equal(bigTank?.fluid_capacity, 2_041_200_000, '5832 * 350 000');
  assert.equal(await tankOf(long.top), undefined, '19 long');
  fixture.mark('18x18x18 forms with 2 041 200 000 mB; 19x3x3 doesn\'t');

  // A chemical through a valve: a basic chemical tank putting hydrogen out of every side, beside a fresh tank.
  const c = await box([20, 0, 20], [3, 3, 3]);
  await fixture.stepServer(2);
  const ct = at(19, 1, 21);
  await fixture.command(`setblock ${ct} mekanism:basic_chemical_tank`);
  await fixture.command(`data merge block ${ct} {chemical_tanks:[{tank:0b,stored:{id:"mekanism:hydrogen",amount:50000L}}],component_config:{config2:[I;4,4,4,4,4,4],eject2:1b}}`);
  await fixture.stepServer(3);
  const gas = await tankOf(c.top);
  log.chemical = { chemical: gas.chemical, amount: gas.chemical_amount };
  assert.equal(gas.chemical, 'mekanism:hydrogen');
  assert(gas.chemical_amount >= 2000 && gas.chemical_amount % 1000 === 0, `1000 a tick: ${gas.chemical_amount}`);
  await fixture.command(`setblock ${at(21, 3, 21)} mekanism:creative_fluid_tank[active=true]`);
  await fixture.command(`data merge block ${at(21, 3, 21)} ${WATER}`);
  await fixture.stepServer(2);
  await fixture.command(`setblock ${at(21, 3, 21)} minecraft:air`);
  assert.equal((await tankOf(c.top)).fluid_amount, 0, 'no water while a chemical is in');
  fixture.mark(`a chemical goes in through a valve (${gas.chemical_amount} mB hydrogen), and water isn't taken then`);

  // A restart keeps it all.
  await fixture.hold(false);
  await fixture.restart();
  const after = await until(() => tankOf(a.top), t => !!t, 'formed again after the restart');
  assert.equal(after.fluid_amount, keep, 'the water after a restart');
  const gasAfter = await until(() => tankOf(c.top), t => !!t, 'the chemical tank formed again');
  assert.equal(gasAfter.chemical_amount > 0, true);
  fixture.mark(`a restart keeps ${after.fluid_amount} mB of water and ${gasAfter.chemical_amount} of hydrogen`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK dynamic-tank', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK dynamic-tank', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
