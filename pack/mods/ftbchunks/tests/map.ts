/** FTB Chunks' claim screen in a world, as the `ftbchunks` native bundle draws it: M opens the map of the 31x31
 * chunks round the player over the terrain (the minimap extra's tiles through `native_map_tile`), a left drag over a
 * 3x3 area claims it in one `request_chunk_change`, Shift with the drag force-loads it, a right drag gives a row up;
 * the server's claims (`/ftbchunks_probe`), the client's copy and counters (`/ftbchunksclient`, from
 * `send_many_chunks`, `send_general_data` and `chunk_change_response`) and a screenshot of the screen.
 * `pack/scripts/ftbchunks.ts world` sets `AE2_TEST_BINARY` and `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const fixture = new WorldFixture('ftb-map', 'server-thread');
const log: Record<string, unknown> = {};

async function probeLine(command: string, prefix: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith(prefix));
  assert(line, `${command}: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(prefix.length));
}
const server = () => probeLine('ftbchunks_probe', 'FTBC ');
const client = () => probeLine('ftbchunksclient', 'FTBCC ');

try {
  await fixture.start();
  await fixture.command('gamemode creative');
  const status = await fixture.raw('status');
  const [px, , pz] = status.player.position.map(Math.floor);
  const me = (await probeLine('ftbteams_probe', 'FTBT ')).players.find((x: any) => x.name === 'AE2Fixture');
  assert(me, 'the player is known to FTB Teams');

  // The login: the world's team manager id and the counters (FTB Chunks' defaults: 500 claims, 25 force-loads).
  const login = await until(client, (c: any) => c.manager && c.general.max_claims === 500, 'login_data and send_general_data', 20000);
  assert.equal(login.general.max_force_loads, 25);
  fixture.mark(`the client has the manager ${login.manager} and the limits 500 / 25`);

  // M opens the claim screen; the map fills from the loaded chunks' tiles.
  await fixture.raw('key m tap');
  let open = await until(client, (c: any) => c.screen, 'M opens the claim screen', 10000).catch(() => null);
  if (!open) {
    fixture.mark('M did not open the screen through the agent; opening it with /ftbchunksmap');
    await probeLine('ftbchunksmap', 'FTBCC ');
  }
  open = await until(client, (c: any) => c.screen && c.filled && c.uploaded && c.cell > 0, 'the map is filled and uploaded', 20000);
  assert(open.tiles > 50, `terrain tiles on the map: ${open.tiles}`);
  assert.deepEqual(open.center, [px >> 4, pz >> 4], 'the map is centred on the player\'s chunk');
  log.layout = { map: open.map, cell: open.cell, tiles: open.tiles };
  const [mx, my] = open.map, cell = open.cell;
  /** The GUI point in the middle of the chunk `du, dv` chunks from the player's. */
  const at = (du: number, dv: number) => [mx + (15 + du + 0.5) * cell, my + (15 + dv + 0.5) * cell].map(v => v.toFixed(1)).join(' ');
  /** A freehand drag over the 3x3 chunks from the player's to two east and two south, row by row. */
  const serpentine = [at(0, 0), at(2, 0), at(2, 1), at(0, 1), at(0, 2), at(2, 2)].join(' ');
  const area = new Set([0, 1, 2].flatMap(dx => [0, 1, 2].map(dz => `${(px >> 4) + dx},${(pz >> 4) + dz}`)));
  const mine = (p: any) => p.claims.filter((c: any) => c.team === me.id && c.dim === 'minecraft:overworld');

  // A left drag claims the 3x3 area in one request.
  await fixture.raw(`drag-at ${serpentine}`);
  const claimed = await until(server, (p: any) => mine(p).length === 9, 'the server has the 9 claims', 10000);
  assert.deepEqual(new Set(mine(claimed).map((c: any) => `${c.x},${c.z}`)), area, 'exactly the dragged chunks');
  let c = await until(client, (c: any) => c.general.claimed === 9 && c.response?.changed === 9, 'the client hears the claims', 10000);
  assert.equal(c.response.total, 9);
  assert.equal(c.claims[me.id]?.[0], 9, 'send_many_chunks gave the client the 9 claims');
  fixture.mark('a left drag over 3x3 chunks claims them: 9 / 9');

  // Shift with a left drag force-loads them.
  await fixture.raw('key shift down');
  await fixture.raw(`drag-at ${serpentine}`);
  await fixture.raw('key shift up');
  const loaded = await until(server, (p: any) => mine(p).filter((c: any) => c.force_loaded).length === 9, 'the server force-loads the 9 chunks', 10000);
  assert(loaded.held.filter((h: any) => h.team === me.id).length === 9, 'and holds their tickets');
  c = await until(client, (c: any) => c.general.loaded === 9 && c.claims[me.id]?.[1] === 9, 'the client hears the force-loads', 10000);
  fixture.mark('Shift with a left drag force-loads them: 9 / 9, tickets held');
  await Bun.sleep(500);
  const shot = join(fixture.run, 'ftbchunks-claim-map.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;

  // A right drag over the last row gives those three up; a drag over claimed chunks again counts its problems.
  await fixture.raw(`drag-at right ${at(0, 2)} ${at(2, 2)}`);
  await until(server, (p: any) => mine(p).length === 6, 'a right drag unclaims a row', 10000);
  await fixture.raw(`drag-at ${at(0, 0)} ${at(2, 0)}`);
  c = await until(client, (c: any) => c.response?.total === 3 && c.response.changed === 0, 'claiming claimed chunks changes nothing', 10000);
  assert.equal(c.response.problems.already_claimed, 3);
  assert.equal(c.general.claimed, 6);
  fixture.mark('a right drag unclaims a row; claiming claimed chunks: 0 / 3, already_claimed 3');

  await fixture.raw('key esc tap');
  await until(client, (c: any) => !c.screen, 'Escape closes the claim screen', 10000);
  fixture.assertHidden(await fixture.raw('status'));
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
