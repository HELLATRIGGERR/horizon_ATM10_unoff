/** FTB Chunks' map cache and large map in a world, as the `ftbchunks` native bundle has them: the client keeps every
 * chunk it has seen in region files under `local/ftbchunks/data/<manager>/minecraft_overworld/` (FTB Chunks' region
 * names, our own extension), so a chunk shows on the maps after it unloads, and after a restart; M opens the large map,
 * which a left drag pans and the wheel zooms, with the claims, waypoints and the player on it; C opens the claim screen
 * from it; the inventory's sidebar button opens the map; walking into a claim and out of it puts FTB Chunks' notice
 * over the hotbar. Screenshots of the large map and the inventory's buttons. `pack/scripts/ftbchunks.ts world` sets
 * `AE2_TEST_BINARY` and `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { existsSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const fixture = new WorldFixture('ftb-region-map', 'server-thread');
const log: Record<string, unknown> = {};

async function probeLine(command: string, prefix: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith(prefix));
  assert(line, `${command}: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(prefix.length));
}
// A client command's chat line can reach the reply a frame late, so the reply may still hold the line of the command
// before: each probe asks again until the line it wants (its own key) comes.
async function probeKey(command: string, key: string): Promise<any> {
  let last: any;
  for (let i = 0; i < 30; i++) {
    last = await probeLine(command, 'FTBCC ');
    if (key in last) return last;
    await Bun.sleep(100);
  }
  throw new Error(`${command}: no reply with "${key}": ${JSON.stringify(last)}`);
}
const client = () => probeKey('ftbchunksclient', 'large');
const has = (cx: number, cz: number) => probeKey(`ftbchunksbigmap has ${cx} ${cz}`, 'live');
const actionbar = async () => (await fixture.raw('messages')).actionbar ?? '';
const shot = async (name: string) => {
  await Bun.sleep(1200);
  const file = join(fixture.run, name);
  await fixture.raw(`screenshot ${file}`);
  return file;
};

try {
  await fixture.start();
  await fixture.command('gamemode creative');
  await fixture.command('time set noon');
  const [px, , pz] = (await fixture.raw('status')).player.position.map(Math.floor);
  const [cx, cz] = [px >> 4, pz >> 4];
  let c = await until(client, (c: any) => c.manager && c.large.cache.folder && c.large.cache.tiles >= 100, 'the cache fills round the player', 60000);
  fixture.mark(`the region cache fills from the loaded chunks: ${c.large.cache.tiles} tiles in ${c.large.cache.regions} regions`);

  // A claim of the player's chunk; out of it and back in: the notice over the hotbar.
  await fixture.command('ftbchunks claim');
  await until(client, (c: any) => c.general.claimed === 1, 'the claim reaches the client', 10000);
  await fixture.command('tp @s ~40 ~ ~');
  const out = await until(actionbar, (a: string) => a.length > 0, 'the notice on leaving the claim', 10000);
  await fixture.command(`tp @s ${px + 0.5} ~ ${pz + 0.5}`);
  const into = await until(actionbar, (a: string) => a.length > 0 && a !== out, 'the notice on entering the claim', 10000);
  log.notice = { out, into };
  const noticeShot = await shot('ftbchunks-notice.png');
  fixture.mark(`walking out of the claim shows "${out}", back in "${into}"`);

  // Far away: the spawn's chunks unload, the cache keeps them, and its files are written.
  await fixture.command('ftbchunks waypoint add Home ~ ~ ~');
  await fixture.command('tp @s ~1500 ~ ~');
  await until(() => has(cx, cz), (h: any) => !h.live, 'the spawn chunk unloads', 60000);
  const kept = await has(cx, cz);
  assert(kept.cached, `the spawn chunk is still in the cache: ${JSON.stringify(kept)}`);
  await probeLine('ftbchunksbigmap save', 'FTBCC ');
  const file = join(kept.folder, kept.file);
  assert(existsSync(file) && statSync(file).size > 1024, `the region file: ${file}`);
  fixture.mark(`the unloaded spawn chunk stays cached, in ${kept.file}`);

  // After a restart the file gives it back.
  await fixture.restart();
  await until(client, (c: any) => c.manager, 'login_data after the restart', 30000);
  const back = await until(() => has(cx, cz), (h: any) => h.cached, 'the spawn chunk from the file', 20000);
  assert.equal(back.live, false, 'not loaded: the picture comes from the file');
  c = await client();
  assert(c.large.cache.read >= 1, `region files read: ${JSON.stringify(c.large.cache)}`);
  fixture.mark('after a restart the cache reads the spawn chunk back from its region file');

  // The large map: M opens it on the player; a drag pans, the wheel zooms.
  await fixture.raw('key m tap');
  c = await until(client, (c: any) => c.large.open && c.large.uploads > 0, 'M opens the large map', 15000);
  const [x, y, w, h] = c.large.rect;
  const mid = [Math.round(x + w / 2), Math.round(y + h / 2)];
  const before = c.large.center;
  await fixture.raw(`drag-at ${mid[0]} ${mid[1]} ${mid[0] - 40} ${mid[1]} ${mid[0] - 80} ${mid[1] + 20}`);
  c = await until(client, (c: any) => Math.abs(c.large.center[0] - before[0]) > 40, 'a drag pans the map', 10000);
  const zoom = c.large.zoom;
  assert(Math.abs(c.large.center[0] - before[0] - 80 / zoom) < 2 && Math.abs(c.large.center[1] - before[1] + 20 / zoom) < 2,
    `the map moves with the pointer: ${before} -> ${c.large.center} at zoom ${zoom}`);
  await fixture.raw(`scroll-at ${mid[0]} ${mid[1]} -3`);
  c = await until(client, (c: any) => c.large.zoom < zoom, 'the wheel zooms out', 10000);
  assert.equal(c.large.zoom, zoom / 8, 'three notches: an eighth');
  // Back over the spawn, 1500 blocks west: the cached terrain, the claim and the Home waypoint.
  await fixture.command('ftbchunksbigmap center ' + `${px} ${pz}`);
  c = await until(client, (c: any) => c.large.known > 700, 'the cached spawn on the map', 10000);
  log.large = c.large;
  const mapShot = await shot('ftbchunks-largemap.png');
  fixture.mark(`the large map pans and zooms (zoom ${zoom} -> ${c.large.zoom}); the cached spawn shows: ${c.large.known} pixels of known terrain`);
  // Two notches in, centred on the spawn again: the cached terrain's detail, the claim and the Home marker.
  await fixture.raw(`scroll-at ${mid[0]} ${mid[1]} 2`);
  await until(client, (c: any) => c.large.zoom > zoom / 8, 'the wheel zooms in', 10000);
  await fixture.command('ftbchunksbigmap center ' + `${px} ${pz}`);
  const closeShot = await shot('ftbchunks-largemap-close.png');

  // C opens the claim screen from it.
  await fixture.raw('key c tap');
  c = await until(client, (c: any) => c.screen && !c.large.open, 'C opens the claim screen', 10000);
  await fixture.raw('key esc tap');
  await until(client, (c: any) => !c.screen, 'Escape closes it', 10000);

  // The claim screen nine chunks from the claim: the spawn's chunks are out of the loaded area and come from the
  // cache (no dark disc round the player), the claim on them in the team's colour.
  await fixture.command(`tp @s ${px + 150.5} ~ ${pz + 0.5}`);
  await until(() => has((px + 150) >> 4, cz), (h: any) => h.live, 'the chunks round the new spot load', 60000);
  await fixture.command('ftbchunksmap');
  c = await until(client, (c: any) => c.screen && c.filled, 'the claim screen opens', 10000);
  const claimShot = await shot('ftbchunks-claimscreen.png');
  fixture.mark(`the claim screen shows the cached spawn beyond the loaded chunks (${c.tiles} tiles)`);
  await fixture.raw('key esc tap');
  await until(client, (c: any) => !c.screen, 'Escape closes the claim screen', 10000);

  // The inventory's sidebar button opens the large map (the inventory closes first, API 432).
  await fixture.command('gamemode survival'); // the sidebar is on the survival inventory, not the creative one
  await fixture.raw('key e tap');
  await Bun.sleep(800);
  const sideShot = await shot('ftbchunks-sidebar.png');
  const [bx, by, bw, bh] = c.large.sidebar[1];
  await fixture.raw(`click-at ${bx + bw / 2} ${by + bh / 2}`);
  c = await until(client, (c: any) => c.large.open, 'the sidebar\'s map button opens the large map', 10000);
  fixture.mark('the inventory\'s sidebar map button opens the large map');
  log.shots = { noticeShot, mapShot, closeShot, claimShot, sideShot };
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
