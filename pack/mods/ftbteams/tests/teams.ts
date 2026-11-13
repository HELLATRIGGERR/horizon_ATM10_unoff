/** FTB Teams' core in a world, as the `ftbteams` native bundle runs it: the player's personal team from their first
 * login, a party (create, settings, invite, allies, force-add, ownership, leave, force-disband), server teams, team
 * chat and its redirect, `/ftbteams info` and `list`, answering with the fallback lines (vanilla keys with the game's own); the files under `<world>/ftbteams/`
 * across a restart; and the teams the client was sent (`/ftbteamsclient`, the bundle's client probe).
 *
 * The second player is the dev command's fake one (`/ftbteams_add_fake_player Bob`, as in Java's development
 * environment): known, offline, with a personal team. The rules between online players are the crate's unit tests.
 * `pack/scripts/ftbchunks.ts world` sets `AE2_TEST_BINARY` and `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const fixture = new WorldFixture('ftb-teams', 'server-thread');
const log: Record<string, unknown> = {};

/** The server's teams (`/ftbteams_probe`). */
async function probe(): Promise<any> {
  const reply = await fixture.command('ftbteams_probe');
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith('FTBT {'));
  assert(line, `probe: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(5));
}
/** What the client was told (`/ftbteamsclient`, typed in chat: the client answers it itself). */
async function client(): Promise<any> {
  const reply = await fixture.raw('chat /ftbteamsclient');
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith('FTBTC {'));
  assert(line, `client probe: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(6));
}
/** Runs `/ftbteams ...` and waits for each of `lines` in the chat (feedback and team messages alike). */
async function teams(words: string, ...lines: Array<string | RegExp>): Promise<void> {
  const before = (await fixture.raw('chat-log 50')).chat?.length ?? 0;
  await fixture.command(`ftbteams ${words}`);
  for (const line of lines) {
    await until(() => fixture.raw('chat-log 50'), (r: any) => r.chat?.some((t: string) => typeof line === 'string' ? t === line || t.endsWith(line) : line.test(t)), `"${line}" after /ftbteams ${words} (${before} lines before)`, 10000);
  }
  fixture.mark(`/ftbteams ${words}: ${lines.join(' | ') || 'ran'}`);
}
const team = (p: any, name: string) => p.teams.find((t: any) => t.display_name === name);
/** The game's own line for the vanilla key `command.unknown.command`, which a refused usage answers with. */
const UNKNOWN = /^Unknown or incomplete command/;

try {
  await fixture.start();
  let p = await probe();
  const me = p.players.find((x: any) => x.name === 'AE2Fixture');
  assert(me?.online, `the player is known and online: ${JSON.stringify(p.players)}`);
  const mine = p.teams.find((t: any) => t.id === me.id);
  assert.equal(mine.type, 'player');
  assert.equal(mine.display_name, 'AE2Fixture');
  assert.deepEqual(mine.ranks, { [me.id]: 'owner' });
  assert.match(mine.color, /^#[0-9A-F]{6}$/);
  fixture.mark(`first login: personal team ${mine.short_name}, colour ${mine.color}`);
  let c = await client();
  assert.equal(c.self, me.id);
  assert.equal(c.manager, p.manager);
  assert.equal(c.teams.find((t: any) => t.id === me.id)?.properties['ftbteams:description'], '', 'the own team in full');
  fixture.mark('the client got the full sync at login');

  await teams('info', '==AE2Fixture [player] ==', `Full id: ${me.id}`, `Short id: ${mine.short_name}`);
  await teams('party leave', UNKNOWN);
  await teams('party create', "<Server @AE2Fixture's Team>AE2Fixture is now in the party");
  p = await probe();
  let party = team(p, "AE2Fixture's Team");
  assert.equal(party.type, 'party');
  assert.equal(party.owner, me.id);
  assert.deepEqual(party.ranks, { [me.id]: 'owner' });
  assert.deepEqual(p.teams.find((t: any) => t.id === me.id).ranks, {}, 'a party member\'s own team holds no rank');
  await teams('party create Again', UNKNOWN);

  await teams('party settings ftbteams:display_name Builders', 'Team name is now Builders');
  await teams('party settings ftbteams:display_name ab', 'Could not read the value ab');
  await teams('party settings ftbteams:color #FF8800', 'Colour is now #FF8800');
  await teams('party settings ftbteams:color', 'Colour=#FF8800');
  await teams('party settings ftbteams:lives_remaining 2', 'Lives left is read-only');
  await teams('party settings ftbteams:nope 1', 'No property has that id');
  c = await until(client, (v: any) => v.self === party.id && v.teams.find((t: any) => t.id === party.id)?.properties['ftbteams:display_name'] === 'Builders', 'the client sees the renamed party as its team');
  assert.equal(c.teams.find((t: any) => t.id === party.id).properties['ftbteams:color'], '#FF8800');
  fixture.mark('the client follows the settings');

  await fixture.command('ftbteams_add_fake_player Bob');
  p = await probe();
  const bob = p.players.find((x: any) => x.name === 'Bob');
  assert(bob && !bob.online && bob.team === bob.id, `Bob is known, offline, in his own team: ${JSON.stringify(p.players)}`);
  party = team(p, 'Builders');
  await teams('party invite Bob', '<AE2Fixture @Builders>Bob has been invited');
  await teams('party allies list', 'Allied players:', 'Bob');
  assert.equal(team(await probe(), 'Builders').ranks[bob.id], 'invited');
  await teams(`force-add ${party.short_name} Bob`, '<Server @Builders>Bob is now in the party');
  await teams('party transfer_ownership Bob', '<AE2Fixture @Builders>Bob now owns the team');
  p = await probe();
  party = team(p, 'Builders');
  assert.equal(party.owner, bob.id);
  assert.deepEqual(party.ranks, { [me.id]: 'officer', [bob.id]: 'owner' });
  await teams('party settings ftbteams:description x', UNKNOWN);
  await teams('list', `Teams here: AE2Fixture,Bob,Builders`);
  c = await client();
  assert.equal(c.teams.find((t: any) => t.id === party.id).ranks[bob.id], 'owner', 'the client sees the new owner');
  await teams('msg hello', '<AE2Fixture @Builders>hello');

  // Files, and the same teams after a restart.
  const dir = p.dir;
  await fixture.raw('save');
  const file = join(dir, 'ftbteams/party', `${party.id}.snbt`);
  await until(async () => existsSync(file) && readFileSync(file, 'utf8'), (text: any) => typeof text === 'string' && text.includes('officer'), 'the party file has the new ranks');
  const text = readFileSync(file, 'utf8');
  assert.match(text, /^\{\n\tid: "[0-9a-f-]{36}"\n\ttype: "party"\n\towner: "/, text);
  assert(text.includes(`"ftbteams:display_name": "Builders"`) && text.includes(`"ftbteams:color": "#FF8800"`), text);
  assert(existsSync(join(dir, 'ftbteams/ftbteams.snbt')) && existsSync(join(dir, 'ftbteams/player', `${me.id}.snbt`)));
  fixture.mark('files: ftbteams.snbt, player/, party/ as FTB Library writes them');
  await fixture.restart();
  p = await probe();
  assert.equal(p.manager, (await client()).manager);
  const again = team(p, 'Builders');
  assert.deepEqual({ id: again.id, owner: again.owner, ranks: again.ranks, color: again.color, history: again.history },
    { id: party.id, owner: bob.id, ranks: party.ranks, color: '#FF8800', history: party.history + 1 });
  assert.equal(p.players.find((x: any) => x.name === 'AE2Fixture').team, party.id, 'the party is rebuilt from its ranks');
  c = await until(client, (v: any) => v.self === party.id, 'the client\'s team after the restart');
  fixture.mark('after a restart: the party, its owner, ranks, colour and history');

  await teams('party leave', '<Server @Builders>AE2Fixture left the party');
  p = await probe();
  assert.equal(p.players.find((x: any) => x.name === 'AE2Fixture').team, me.id);
  await teams(`force-disband ${party.short_name}`, `Party 'Builders' (${party.id}) was disbanded by force`);
  p = await probe();
  assert.equal(team(p, 'Builders'), undefined);
  await fixture.raw('save');
  await until(async () => existsSync(join(dir, 'ftbteams/deleted', `${party.id}.snbt`)), Boolean, 'the disbanded party\'s file moves to deleted/');
  assert(!existsSync(file));
  c = await until(client, (v: any) => !v.teams.some((t: any) => t.id === party.id), 'the client forgets the party');

  await teams('server create ab', 'The name needs at least 3 characters');
  await teams('server create Staff', "Server team 'Staff' created");
  const staff = team(await probe(), 'Staff');
  assert.equal(staff.type, 'server');
  assert.equal(staff.color, '#FFFFFF', 'server teams stay white');
  await teams(`server delete ${staff.short_name}`, `Server team '${staff.short_name}' removed`);
  await teams('redirect_chat', 'Your chat now goes to the team');
  assert.deepEqual((await probe()).chat_redirected, [me.id]);
  await teams('redirect_chat', 'Your chat now goes to everyone');
  log.party = party.id;
  fixture.assertHidden(await fixture.raw('status'));
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
