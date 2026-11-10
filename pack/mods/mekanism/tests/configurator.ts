/** Mekanism's Configurator, as the `mekanism` native bundle makes it (`ItemConfigurator` of Mekanism 10.7.19, read
 * from its code: the Configurator has no energy in 10.7). A player holds it and uses it on blocks, sneaking where
 * Mekanism needs a sneaking use (a block with a screen opens it on a plain one):
 *
 * - Configurating items or energy, a sneaking use bumps the used side of a machine's item or energy configuration to
 *   its next type; on a mechanical pipe it bumps the side's connection type (normal, push, pull, none), on a
 *   logistical transporter's connected side its colour (none, black, blue...), and on a side with no connection that
 *   connection; on a fluid tank it turns giving down on and off.
 * - Rotate: a sneaking use turns a machine to face away from the side used; a plain one turns it clockwise.
 * - Empty: a sneaking use takes a machine's items out into the world.
 * - Wrench: a sneaking use takes a cube up into the player's inventory, with its energy. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-configurator', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
async function data(pos: string, path = ''): Promise<string> {
  const reply = await fixture.command(`data get block ${pos} ${path}`.trim());
  return (reply.chat ?? []).find((t: string) => t.includes('has the following block data')) ?? '';
}
async function is(pos: string, state: string): Promise<boolean> {
  const reply = await fixture.command(`execute if block ${pos} ${state}`);
  return (reply.chat ?? []).some((t: string) => /passed/i.test(t));
}
const ints = (line: string) => (line.match(/\[I; ?([-\d, ]*)\]/)?.[1] ?? '').split(',').map(s => Number(s.trim())).filter(n => !Number.isNaN(n));

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py), z = Math.floor(pz) + 3;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(20, 3, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(20, -2, 6)} minecraft:stone`);
  // Clearing the area can drop its grass's seeds: they'd pass for the empty mode's items below.
  const near = `@e[type=minecraft:item,x=${x - 4},y=${y - 3},z=${z - 4},dx=26,dy=8,dz=12]`;
  await until(async () => {
    await fixture.command('kill @e[type=minecraft:item]');
    return (await fixture.command(`execute if entity ${near}`)).chat?.join(' ') ?? '';
  }, line => !/passed/i.test(line), 'no items left in the cleared area');
  await fixture.replace('hotbar.0', 'mekanism:configurator');
  await fixture.select(0, 'mekanism:configurator');
  /** Stands the player east of the block `dx` along, facing its east side. */
  const eastOf = async (dx: number) => fixture.command(`tp @s ${x + dx + 2.5} ${y} ${z + 0.5}`);
  const mode = async (name: string) => {
    const reply = await mek(`mek configurator ${name}`);
    assert(!reply.error, JSON.stringify(reply));
  };

  // Configurating items, then energy: a sneaking use on a machine's east side.
  const machine = at(0);
  await fixture.command(`setblock ${machine} mekanism:enrichment_chamber[facing=north]`);
  await until(() => data(machine, 'component_config'), line => line.includes('config3'), 'the machine\'s side config');
  const before = { items: ints(await data(machine, 'component_config.config3')), energy: ints(await data(machine, 'component_config.config0')) };
  await eastOf(0);
  await mode('items');
  await fixture.sneakInteract(`${machine} east`);
  const items = await until(async () => ints(await data(machine, 'component_config.config3')), now => now.join() !== before.items.join(), 'the item side bumped');
  await mode('energy');
  await fixture.sneakInteract(`${machine} east`);
  const energy = await until(async () => ints(await data(machine, 'component_config.config0')), now => now.join() !== before.energy.join(), 'the energy side bumped');
  log.sides = { before, items, energy };
  const changed = (a: number[], b: number[]) => a.map((v, i) => (v !== b[i] ? i : -1)).filter(i => i >= 0);
  assert.equal(changed(before.items, items).length, 1, 'one item side');
  assert.deepEqual(changed(before.energy, energy), changed(before.items, items), 'the same side, the one used');
  fixture.mark('configurating, a sneaking use bumps the used side of a machine\'s item and energy configuration');

  // Rotate: sneaking, away from the side used; plain, clockwise.
  await mode('rotate');
  await fixture.sneakInteract(`${machine} east`);
  await until(() => is(machine, 'mekanism:enrichment_chamber[facing=west]'), ok => ok, 'facing away from the east side');
  await fixture.raw(`interact ${machine} east`);
  await until(() => is(machine, 'mekanism:enrichment_chamber[facing=north]'), ok => ok, 'a plain use: clockwise, west to north');
  fixture.mark('rotate: a sneaking use faces the machine away from the side used, a plain one turns it clockwise');

  // Empty: a sneaking use takes the machine's items out.
  await fixture.command(`data merge block ${machine} {Items:[{Slot:0b,id:"minecraft:iron_ore",count:5}]}`);
  await mode('empty');
  await fixture.sneakInteract(`${machine} east`);
  await until(() => data(machine, 'Items'), line => !line.includes('iron_ore'), 'the machine emptied');
  const dropped = await until(async () => (await fixture.command(`execute if entity @e[type=minecraft:item,x=${x - 2},y=${y - 2},z=${z - 2},dx=6,dy=4,dz=4]`)).chat?.join(' ') ?? '', line => /passed/i.test(line), 'its items in the world');
  log.dropped = dropped;
  await fixture.command('kill @e[type=minecraft:item]');
  fixture.mark('empty: a sneaking use takes a machine\'s items out into the world');

  // A pipe's side, a transporter's colour and side, a fluid tank.
  const pipe = at(4), transporter = at(8), tank = at(12);
  await fixture.command(`setblock ${pipe} mekanism:basic_mechanical_pipe`);
  await fixture.command(`setblock ${transporter} mekanism:basic_logistical_transporter`);
  await fixture.command(`setblock ${tank} mekanism:basic_fluid_tank`);
  await mode('fluids');
  await eastOf(4);
  const sides: string[] = [];
  for (let i = 0; i < 2; i++) {
    const was = (await mek(`mek probe ${pipe}`)).connections[5];
    await fixture.sneakInteract(`${pipe} east`);
    sides.push((await until(() => mek(`mek probe ${pipe}`), p => p.connections[5] !== was, 'the pipe side bumped')).connections[5]);
  }
  assert.deepEqual(sides, ['push', 'pull']);
  await eastOf(8);
  const colours: (string | null)[] = [];
  for (let i = 0; i < 2; i++) {
    const was = (await mek(`mek probe ${transporter}`)).color;
    await fixture.sneakInteract(`${transporter} east`);
    colours.push((await until(() => mek(`mek probe ${transporter}`), p => p.color !== was, 'the colour bumped')).color);
  }
  assert.deepEqual(colours, ['Black', 'DarkBlue']);
  await mek(`mek connection ${transporter} east none`);
  await fixture.sneakInteract(`${transporter} east`);
  const reconnected = await until(() => mek(`mek probe ${transporter}`), p => p.connections[5] === 'normal', 'a cut side connects again');
  assert.equal(reconnected.color, 'DarkBlue', 'a side with no connection bumps the connection, not the colour');
  await eastOf(12);
  await fixture.sneakInteract(`${tank} east`);
  await until(() => mek(`mek probe ${tank}`), p => p.active === true, 'the tank on');
  await fixture.sneakInteract(`${tank} east`);
  await until(() => mek(`mek probe ${tank}`), p => p.active === false, 'and off');
  log.transmitters = { sides, colours };
  fixture.mark('a pipe\'s side goes push, pull; a transporter\'s colour black, blue, a cut side connects; a fluid tank turns on and off');

  // Wrench: a sneaking use takes a cube up into the inventory, with its energy.
  const cube = at(16);
  await fixture.command(`setblock ${cube} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${cube} {energy_containers:[{container:0b,stored:1000L}]}`);
  await mode('wrench');
  await eastOf(16);
  await fixture.sneakInteract(`${cube} east`);
  await until(() => is(cube, 'minecraft:air'), ok => ok, 'the cube taken up');
  const inventory = await until(async () => JSON.stringify(await fixture.raw('inventory')), text => text.includes('basic_energy_cube'), 'the cube in the inventory');
  const held = (await fixture.command('data get entity @s Inventory')).chat?.join(' ') ?? '';
  log.wrench = { inventory: inventory.slice(0, 400), held: held.slice(0, 600) };
  assert(/basic_energy_cube/.test(held) && /1000/.test(held), `with its energy: ${held}`);
  fixture.mark('wrench: a sneaking use takes a cube up into the player\'s inventory with its energy');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK configurator', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK configurator', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
