/** A cube placed by a player, and the item it drops.
 *
 * - A cube faces the player who places it (`AttributeStateFacing`): placed from the east, its front (output) is east.
 * - A cube item's custom data (`custom_data.mekanism`: Mekanism's `energy_containers`) is the placed cube's energy.
 * - Broken with a pickaxe in survival, a cube holding energy drops an item that keeps it; an untouched cube drops a
 *   plain item, which stacks with new ones.
 * Needs what `energy-cube.ts` needs; `pack/scripts/mekanism.ts world` runs every test. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mekanism-cube-items', 'server-thread', { devMods: [content] });

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
/** The item entity nearest `pos`: `data get` of `path` on it, as chat prints it. */
async function dropped(pos: string, path: string): Promise<string> {
  const reply = await fixture.command(`execute positioned ${pos} run data get entity @e[type=minecraft:item,limit=1,sort=nearest,distance=..3] ${path}`);
  return (reply.chat ?? []).join(' ');
}

/** Turns the player and waits until the server has the yaw: a placement reads the placer's rotation there. */
async function look(yaw: number): Promise<void> {
  await fixture.raw(`look ${yaw} 30`);
  await until(() => fixture.command('data get entity @s Rotation[0]'), r => {
    const m = (r.chat ?? []).join(' ').match(/(-?[\d.]+)f/);
    return !!m && Math.abs(((Number(m[1]) - yaw) % 360 + 540) % 360 - 180) < 1;
  }, `the server sees yaw ${yaw}`);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 3} ${y} ${z - 6} ${x + 4} ${y + 3} ${z + 2} minecraft:air`);
  await fixture.command(`fill ${x - 3} ${y - 1} ${z - 6} ${x + 4} ${y - 1} ${z + 2} minecraft:stone`);

  // Placed from the east: the cube's front faces east, at the player.
  const east = `${x} ${y} ${z}`;
  await fixture.replace('hotbar.0', 'mekanism:basic_energy_cube');
  await fixture.select(0, 'mekanism:basic_energy_cube');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 0.5}`);
  await look(90);
  await fixture.raw(`interact ${x} ${y - 1} ${z} up`);
  const placed = await until(() => saved(east).catch(() => ''), s => s.includes('mekanism:basic_energy_cube'), 'the cube is placed');
  log.placed_east = placed;
  assert(/facing: ?["']east["']/.test(placed), `facing east: ${placed}`);
  const state = await until(() => fixture.raw(`block ${east}`), v => String(v.state).includes('east=output'), 'the east face shows output');
  assert(String(state.state).includes('north=input'), state.state);
  fixture.mark('a cube placed from the east faces east (east=output, north=input)');

  // An item holding 1 MJ places a cube holding 1 MJ.
  const west = `${x} ${y} ${z - 3}`;
  const charged = 'mekanism:basic_energy_cube[minecraft:custom_data={mekanism:{energy_containers:[{container:0b,stored:1000000L}]}}]';
  await fixture.replace('hotbar.1', charged);
  await fixture.select(1, 'mekanism:basic_energy_cube');
  await fixture.command(`tp @s ${x - 1.5} ${y} ${z - 2.5}`);
  await look(-90);
  await fixture.raw(`interact ${x} ${y - 1} ${z - 3} up`);
  const full = await until(() => saved(west).catch(() => ''), s => s.includes('mekanism:basic_energy_cube'), 'the charged cube is placed');
  log.placed_west = full;
  assert(/stored: ?1000000L/.test(full), `1 MJ from the item: ${full}`);
  assert(/facing: ?["']west["']/.test(full), full);
  fixture.mark('a cube item holding 1 MJ places a cube holding 1 MJ (facing west)');

  // Broken in survival with a pickaxe: the charged cube's drop keeps its energy; the untouched one's is plain.
  await fixture.command('gamemode survival');
  await fixture.replace('hotbar.2', 'minecraft:diamond_pickaxe');
  await fixture.select(2, 'minecraft:diamond_pickaxe');
  // Three blocks off: at two, a drop that pops half a block towards the player is picked up.
  await fixture.command(`tp @s ${x - 2.5} ${y} ${z - 2.5}`);
  await fixture.raw(`break ${west}`);
  const kept = await until(() => dropped(west, 'Item'), s => s.includes('basic_energy_cube'), 'the charged cube drops');
  log.drop_charged = kept;
  assert(/stored: ?1000000L/.test(kept), `the drop keeps 1 MJ: ${kept}`);
  await fixture.command(`kill @e[type=minecraft:item,distance=..32]`);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 0.5}`);
  await fixture.raw(`break ${east}`);
  const plain = await until(() => dropped(east, 'Item'), s => s.includes('basic_energy_cube'), 'the untouched cube drops');
  log.drop_plain = plain;
  assert(!plain.includes('custom_data'), `an untouched cube drops a plain item: ${plain}`);
  fixture.mark('a broken cube\'s drop keeps its energy; an untouched one drops a plain item');
  await fixture.command('gamemode creative');
  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK items', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK items', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
