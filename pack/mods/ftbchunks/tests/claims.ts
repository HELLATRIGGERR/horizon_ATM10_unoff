/** FTB Chunks' server side in a world, as the `ftbchunks` native bundle runs it: a claim refuses a non-ally's
 * breaking and placing while a member's go through; force-loaded claims keep a furnace smelting with the player
 * 400 blocks away, and drop their tickets when nobody of the team is on (FTB Chunks' defaults); the limits (500
 * claims, 25 force-loaded chunks); `/ftbchunks info`; and the claims, force-loads and tickets after a restart, from
 * `<world>/ftbchunks/<team>.snbt`. Lines are the bundle's fallback wording (no FTB lang file in the test game).
 *
 * The second player is FTB Teams' dev fake player (`/ftbteams_add_fake_player Bob`): known, offline. The party P is
 * Bob's after the real player hands it over and leaves, so the real player is a non-ally until force-added back.
 * `pack/scripts/ftbchunks.ts world` sets `AE2_TEST_BINARY` and `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const fixture = new WorldFixture('ftb-claims', 'server-thread');
const log: Record<string, unknown> = {};

async function probeLine(command: string, prefix: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith(prefix));
  assert(line, `${command}: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(prefix.length));
}
/** The server's claims (`/ftbchunks_probe`) and teams (`/ftbteams_probe`). */
const chunks = () => probeLine('ftbchunks_probe', 'FTBC ');
const teams = () => probeLine('ftbteams_probe', 'FTBT ');
/** The lines of `after` past its overlap with the end of `before` (two reads of the chat log). */
function fresh(before: string[], after: string[]): string[] {
  let k = Math.min(before.length, after.length);
  for (; k > 0; k--) if (before.slice(before.length - k).every((t, i) => t === after[i])) break;
  return after.slice(k);
}
/** Runs a command and waits for each of `lines` among the chat lines that came after it. */
async function say(command: string, ...lines: Array<string | RegExp>): Promise<void> {
  const before: string[] = (await fixture.raw('chat-log 50')).chat ?? [];
  await fixture.command(command);
  for (const line of lines) {
    const hit = (t: string) => typeof line === 'string' ? t === line || t.endsWith(line) : line.test(t);
    await until(() => fixture.raw('chat-log 50'), (r: any) => fresh(before, r.chat ?? []).some(hit), `"${line}" after /${command}`, 10000);
  }
  fixture.mark(`/${command}: ${lines.join(' | ') || 'ran'}`);
}
const blockAt = async (p: number[]) => JSON.stringify(await fixture.raw(`block ${p.join(' ')}`));
/** Tries an agent action the server should refuse: it may end in its timeout, or the client may have guessed the
 * change, which the server's block update puts back; `settled` is then waited for. */
async function refused(action: string, settled: () => Promise<boolean>, what: string): Promise<void> {
  let outcome = 'the action returned';
  try { await fixture.raw(action); } catch (error) { outcome = `the action ended: ${String(error).slice(0, 80)}`; }
  await Bun.sleep(1500);
  await until(settled, Boolean, what, 10000);
  fixture.mark(`${what} (${outcome})`);
}
/** Iron smelted so far by the furnace at `p`, read on the server (its chunk must be loaded). */
async function ingots(p: number[]): Promise<number> {
  const reply = await fixture.raw(`command data get block ${p.join(' ')} Items`);
  const text = (reply.chat ?? []).join('\n');
  assert(!/not loaded/i.test(text), `the furnace's chunk is loaded: ${text}`);
  const entry = text.match(/\{[^{}]*iron_ingot[^{}]*\}/)?.[0];
  return Number(entry?.match(/count:\s*(\d+)/)?.[1] ?? 0);
}
const team = (p: any, id: string) => p.teams.find((t: any) => t.id === id);

try {
  await fixture.start();
  await fixture.command('gamemode creative');
  const me = (await teams()).players.find((x: any) => x.name === 'AE2Fixture');
  await fixture.command('ftbteams_add_fake_player Bob');
  await say('ftbteams party create', 'AE2Fixture is now in the party');
  const party = (await teams()).teams.find((t: any) => t.type === 'party');
  await say(`ftbteams force-add ${party.short_name} Bob`, 'Bob is now in the party');
  await say('ftbteams party transfer_ownership Bob', 'Bob now owns the team');
  await say('ftbteams party leave', 'AE2Fixture left the party');

  // The arena: a platform over the player's chunk, claimed for Bob's party.
  const status = await fixture.raw('status');
  const [px, py, pz] = status.player.position.map(Math.floor);
  const x0 = px & ~15, z0 = pz & ~15, y = py + 2;
  await fixture.command(`fill ${x0} ${y - 1} ${z0} ${x0 + 15} ${y - 1} ${z0 + 15} minecraft:stone`);
  await fixture.command(`fill ${x0} ${y} ${z0} ${x0 + 15} ${y + 3} ${z0 + 15} minecraft:air`);
  const dirt = [x0 + 8, y, z0 + 6], spot = [x0 + 9, y, z0 + 6], furnace = [x0 + 4, y, z0 + 12];
  await fixture.command(`setblock ${dirt.join(' ')} minecraft:dirt`);
  await fixture.command(`tp @s ${x0 + 8.5} ${y} ${z0 + 3.5}`);
  await fixture.command('give @s minecraft:cobblestone 16');
  await say(`ftbchunks admin claim_as ${party.short_name} 0 ${x0 + 8} ${y} ${z0 + 8}`, '1 chunk(s) claimed');
  await say('ftbchunks info', `Chunk [${x0 >> 4}, ${z0 >> 4}] minecraft:overworld`, /^Held by: /, 'Kept loaded: false');

  // A non-ally may neither break nor place there.
  await refused(`break ${dirt.join(' ')} --timeout 3`, async () => (await blockAt(dirt)).includes('minecraft:dirt'), 'a non-ally cannot break in the claim');
  await refused(`place ${spot.join(' ')} --item minecraft:cobblestone`, async () => !(await blockAt(spot)).includes('cobblestone'), 'a non-ally cannot place in the claim');

  // A member may.
  await say(`ftbteams force-add ${party.short_name} AE2Fixture`, 'AE2Fixture is now in the party');
  await fixture.raw(`break ${dirt.join(' ')}`);
  await until(() => blockAt(dirt), (b: string) => !b.includes('minecraft:dirt'), 'a member breaks the dirt');
  await fixture.raw(`place ${spot.join(' ')} --item minecraft:cobblestone`);
  await until(() => blockAt(spot), (b: string) => b.includes('cobblestone'), 'a member places cobblestone');
  fixture.mark('a member breaks and places in the claim');

  // Limits: 500 claims and 25 force-loads a player; the party has the largest of its members'.
  await say('ftbchunks claim 176', '499 chunk(s) claimed');
  await say(`ftbchunks admin claim_as ${party.short_name} 0 ${x0 + 3000} ${y} ${z0 + 3000}`, '0 chunk(s) claimed');
  await say('ftbchunks load 48', '25 chunk(s) now kept loaded');
  await say('ftbchunks load 64', '0 chunk(s) now kept loaded');
  let c = await chunks();
  assert.deepEqual([team(c, party.id).claimed, team(c, party.id).force_loaded, team(c, party.id).max_claims], [500, 25, 500]);
  c = await until(chunks, (v: any) => v.held.length === 25, 'the 25 tickets are held');
  log.claimed = 500;

  // A force-loaded furnace smelts with nobody near.
  await fixture.command(`setblock ${furnace.join(' ')} minecraft:furnace[facing=north]`);
  await fixture.command(`item replace block ${furnace.join(' ')} container.0 with minecraft:raw_iron 64`);
  await fixture.command(`item replace block ${furnace.join(' ')} container.1 with minecraft:coal 8`);
  await fixture.raw(`command tp @s ${x0 + 408.5} ${y + 40} ${z0 + 8.5}`);
  await until(() => fixture.raw('status'), (s: any) => Math.abs(s.player.position[0] - (x0 + 408.5)) < 1, 'the player is 400 blocks away');
  const first = await ingots(furnace);
  const second = await until(() => ingots(furnace), (n: number) => n >= first + 2, 'the furnace keeps smelting 400 blocks away', 60000);
  log.smelted = [first, second];
  fixture.mark(`a force-loaded furnace smelts with nobody near: ${first} -> ${second} ingots`);

  // After a restart: the claims, force-loads and tickets, from the team's file.
  const dir = (await chunks()).dir;
  await fixture.restart();
  const file = join(dir, 'ftbchunks', `${party.id}.snbt`);
  assert(existsSync(file), file);
  const text = readFileSync(file, 'utf8');
  assert(text.includes('"minecraft:overworld"') && text.includes('force_loaded') && text.includes('member_data'), text.slice(0, 400));
  c = await until(chunks, (v: any) => v.held.length === 25, 'the tickets are held again after the restart');
  assert.deepEqual([team(c, party.id).claimed, team(c, party.id).force_loaded], [500, 25]);
  const third = await ingots(furnace);
  await until(() => ingots(furnace), (n: number) => n >= third + 1, 'the furnace smelts on after the restart', 60000);
  fixture.mark('after a restart: 500 claims, 25 force-loads, the tickets and the furnace');

  // With nobody of the party on (Bob is offline), the tickets go, as FTB Chunks' defaults say.
  await say('ftbteams party leave', 'AE2Fixture left the party');
  c = await until(chunks, (v: any) => v.held.length === 0, 'no tickets with nobody of the party on');
  fixture.mark('no member on: the force-loaded chunks are let go');
  await fixture.command(`tp @s ${x0 + 8.5} ${y} ${z0 + 3.5}`);
  await until(() => blockAt(spot), (b: string) => b.includes('cobblestone'), 'the client sees the cobblestone again');
  await refused(`break ${spot.join(' ')} --timeout 3`, async () => (await blockAt(spot)).includes('cobblestone'), 'the claim still refuses a non-ally after the restart');
  await say('ftbchunks admin bypass_protection', 'Claims no longer stop you');
  await fixture.raw(`break ${spot.join(' ')}`);
  await until(() => blockAt(spot), (b: string) => !b.includes('cobblestone'), 'an admin with the bypass breaks it');
  await say('ftbchunks admin bypass_protection', 'Claims stop you again');
  await say(`ftbchunks unclaim_all ${party.short_name}`, '500 chunk(s) given up');
  assert.equal((await chunks()).total_claims, 0);
  log.party = party.id;
  log.me = me?.id;
  fixture.assertHidden(await fixture.raw('status'));
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
