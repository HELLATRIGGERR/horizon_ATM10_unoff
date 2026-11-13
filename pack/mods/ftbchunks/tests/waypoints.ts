/** FTB Chunks' waypoints, death waypoint and corner minimap in a world, as the `ftbchunks` native bundle has them:
 * the minimap is on by default (no other minimap mod is built in) and drawn in the top right; `/ftbchunks waypoint
 * add` saves a waypoint in the client's `local/ftbchunks/data/<manager>/minecraft_overworld/waypoints.json`; on the
 * claim screen a right click on an unclaimed chunk adds one there, and on a marker hides or removes it; dying and
 * respawning adds the red `Death #<n>` waypoint where the player died. Screenshots of the minimap with a claim and
 * waypoints, and of the claim screen's markers. `pack/scripts/ftbchunks.ts world` sets `AE2_TEST_BINARY` and
 * `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const fixture = new WorldFixture('ftb-waypoints', 'server-thread');
const log: Record<string, unknown> = {};

async function probeLine(command: string, prefix: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith(prefix));
  assert(line, `${command}: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(prefix.length));
}
const client = () => probeLine('ftbchunksclient', 'FTBCC ');
const named = (c: any, name: string) => c.minimap.waypoints.find((w: any) => w.name === name);
const saved = (file: string) => JSON.parse(readFileSync(file, 'utf8')).waypoints as any[];

/** The first widget of `ui` whose label matches `re`. */
function widget(tree: any, re: RegExp): any {
  if (tree && typeof tree === 'object') {
    if (typeof tree.label === 'string' && re.test(tree.label) && tree.id !== undefined) return tree;
    for (const v of Object.values(tree)) { const w = widget(v, re); if (w) return w; }
  }
  return undefined;
}

try {
  await fixture.start();
  await fixture.command('gamemode creative');
  await fixture.command('time set noon');
  const status = await fixture.raw('status');
  const [px, py, pz] = status.player.position.map(Math.floor);

  let c = await until(client, (c: any) => c.manager && c.minimap?.shown && c.minimap.uploaded, 'the corner minimap shows', 30000);
  assert.equal(c.minimap.default, true, 'on by default: no other minimap mod is built in');
  const [mx, my, size] = c.minimap.rect;
  assert(mx > 200 && my < 40 && size >= 32, `top right, a tenth of the GUI width: ${c.minimap.rect}`);
  fixture.mark(`the minimap is on by default, top right at ${c.minimap.rect.map((v: number) => v.toFixed(0))}, ${c.minimap.tiles} tiles`);

  // A claim of the player's chunk and two waypoints, one beyond the map's edge.
  await fixture.command('ftbchunks claim');
  await until(client, (c: any) => c.general.claimed === 1, 'the claim reaches the client', 10000);
  await fixture.command('ftbchunks waypoint add Base ~6 ~ ~-5');
  await fixture.command('ftbchunks waypoint add Far ~60 ~ ~25 minecraft:overworld gold');
  c = await until(client, (c: any) => named(c, 'Base') && named(c, 'Far'), 'the client keeps both waypoints', 10000);
  const file = c.minimap.file;
  assert(file && existsSync(file), `the waypoints file: ${file}`);
  assert(file.replaceAll('\\', '/').endsWith(`data/${c.manager}/minecraft_overworld/waypoints.json`), file);
  const base = saved(file).find(w => w.name === 'Base');
  assert.deepEqual(base, { hidden: false, name: 'Base', x: px + 6, y: py, z: pz - 5, color: '#FFFFFF', type: 'default' });
  assert.equal(saved(file).find(w => w.name === 'Far').color, '#FFAA00');
  fixture.mark('/ftbchunks waypoint add saves the waypoint in FTB Chunks\' file, keys and all');

  await Bun.sleep(1500);
  const hudShot = join(fixture.run, 'ftbchunks-minimap.png');
  await fixture.raw(`screenshot ${hudShot}`);
  log.minimap = { rect: c.minimap.rect, shot: hudShot };

  // The claim screen: markers, and its right-click menu.
  await fixture.raw('key m tap');
  let open = await until(client, (c: any) => c.screen && c.filled && c.uploaded && c.cell > 0, 'the claim screen is open and filled', 20000)
    .catch(async () => { await probeLine('ftbchunksmap', 'FTBCC '); return until(client, (c: any) => c.screen && c.filled && c.uploaded, 'opened by /ftbchunksmap', 20000); });
  const [ox, oy] = open.map, cell = open.cell;
  const cx = px >> 4, cz = pz >> 4;
  /** The GUI point of block `bx, bz` (its middle). */
  const point = (bx: number, bz: number) => [ox + ((bx + 0.5) / 16 - (cx - 15)) * cell, oy + ((bz + 0.5) / 16 - (cz - 15)) * cell];
  const [ax, az] = [(cx - 3) * 16 + 8, (cz - 3) * 16 + 8];
  const [qx, qy] = point(ax, az);
  await fixture.raw(`click-at ${qx.toFixed(1)} ${qy.toFixed(1)} right`);
  c = await until(client, (c: any) => c.minimap.menu?.length === 1, 'a right click on unclaimed land opens the menu', 10000);
  await Bun.sleep(500);
  await fixture.raw(`screenshot ${join(fixture.run, 'ftbchunks-menu.png')}`);
  await fixture.raw(`click-at ${(qx + 6).toFixed(1)} ${(qy + 6).toFixed(1)}`);
  c = await until(client, (c: any) => c.minimap.waypoints.length === 3 && !c.minimap.menu, 'the menu adds a waypoint there', 10000);
  const added = c.minimap.waypoints[2];
  assert(Math.abs(added.x - ax) <= 1 && Math.abs(added.z - az) <= 1, `at the clicked block: ${JSON.stringify(added)}`);
  assert.equal(saved(file).length, 3, 'and saves it');
  fixture.mark(`the claim screen's menu adds a waypoint at the clicked block (${added.x}, ${added.z})`);
  await Bun.sleep(700);
  const mapShot = join(fixture.run, 'ftbchunks-map-waypoints.png');
  await fixture.raw(`screenshot ${mapShot}`);

  const [wx, wy] = point(added.x, added.z);
  await fixture.raw(`click-at ${wx.toFixed(1)} ${wy.toFixed(1)} right`);
  c = await until(client, (c: any) => c.minimap.menu?.length === 2, 'a right click on the marker opens its menu', 10000);
  await Bun.sleep(500);
  await fixture.raw(`screenshot ${join(fixture.run, 'ftbchunks-marker-menu.png')}`);
  await fixture.raw(`click-at ${(wx + 6).toFixed(1)} ${(wy + 6).toFixed(1)}`);
  c = await until(client, (c: any) => c.minimap.waypoints[2]?.hidden === true, 'the first item hides it', 10000);
  assert.equal(saved(file)[2].hidden, true);
  await fixture.raw(`click-at ${wx.toFixed(1)} ${wy.toFixed(1)} right`);
  await until(client, (c: any) => c.minimap.menu?.length === 2, 'its menu again', 10000);
  await fixture.raw(`click-at ${(wx + 6).toFixed(1)} ${(wy + 6 + 12).toFixed(1)}`);
  c = await until(client, (c: any) => c.minimap.waypoints.length === 2, 'the second item removes it', 10000);
  assert.equal(saved(file).length, 2);
  fixture.mark('a marker\'s menu hides it, then removes it; the file follows');
  await fixture.raw('key esc tap');
  await until(client, (c: any) => !c.screen, 'Escape closes the claim screen', 10000);

  // Death: the red death waypoint where the player died, after the respawn.
  const at = (await fixture.raw('status')).player.position.map(Math.floor);
  await fixture.command('kill @s');
  let respawned = false;
  for (let i = 0; i < 20 && !respawned; i++) {
    await Bun.sleep(500);
    const button = widget(await fixture.raw('ui'), /respawn/i);
    if (button) { await fixture.raw(`click ${button.id}`); respawned = true; }
  }
  assert(respawned, 'the death screen\'s respawn button');
  c = await until(client, (c: any) => c.minimap.waypoints.some((w: any) => w.type === 'death'), 'the death waypoint', 15000);
  const death = c.minimap.waypoints.find((w: any) => w.type === 'death');
  assert.match(death.name, /^Death #\d+$/);
  assert.equal(death.color, '#FF0000');
  assert.deepEqual([death.x, death.z], [at[0], at[2]], 'at the spot the player died');
  assert(saved(file).some(w => w.type === 'death' && !w.hidden), 'saved');
  fixture.mark(`dying adds ${death.name} (red) where the player died`);
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
