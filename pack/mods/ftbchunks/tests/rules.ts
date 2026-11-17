/** FTB Chunks' team properties and the rules they drive, in a world, as the `ftbchunks` native bundle runs them:
 * `/ftbteams party settings ftbchunks:*` (the open property registry on FTB Teams: values, defaults, refusals, the
 * team's file); TNT that leaves a claim's blocks alone until the team allows explosions; with the world config's
 * `pvp_mode` NEVER a player's hit stays out of a claim; with `flowing_fluid_protection` water does not flow into a
 * claim; and, after restarts that age the team's file, the expiry of a force-load, then (idle days off by default)
 * of the force-loads and of the claims of a team nobody is on for. Lines are the bundle's fallback wording.
 *
 * The second player is FTB Teams' dev fake player (`/ftbteams_add_fake_player Bob`): known, offline. The party P
 * is Bob's once the real player hands it over and leaves. `pack/scripts/ftbchunks.ts world` sets `AE2_TEST_BINARY`
 * and `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const fixture = new WorldFixture('ftb-rules', 'server-thread');
const log: Record<string, unknown> = {};

async function probeLine(command: string, prefix: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith(prefix));
  assert(line, `${command}: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(prefix.length));
}
const chunks = () => probeLine('ftbchunks_probe', 'FTBC ');
const teams = () => probeLine('ftbteams_probe', 'FTBT ');
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
const health = async (): Promise<number> => {
  const reply = await fixture.raw('command data get entity @s Health');
  const value = (reply.chat ?? []).join('\n').match(/(-?[\d.]+)f\s*$/m)?.[1];
  assert(value !== undefined, `health: ${JSON.stringify(reply.chat)}`);
  return Number(value);
};
const team = (p: any, id: string) => p.teams.find((t: any) => t.id === id);
/** Writes the world's server config (FTB Chunks reads it when the world opens). */
function writeConfig(world: string, body: string): void {
  mkdirSync(join(world, 'serverconfig'), { recursive: true });
  writeFileSync(join(world, 'serverconfig', 'ftbchunks-world.snbt'), `{\n${body}\n}\n`);
}
/** Stops the game, lets `change` edit the world's files, starts it again. */
async function offline(change: () => void): Promise<void> {
  await fixture.raw('save');
  await fixture.stop();
  change();
  await fixture.start();
}

try {
  await fixture.start();
  await fixture.command('gamemode creative');
  await fixture.command('ftbteams_add_fake_player Bob');
  await say('ftbteams party create', 'AE2Fixture is now in the party');
  const party = (await teams()).teams.find((t: any) => t.type === 'party');
  const world = (await chunks()).dir as string;
  assert((await chunks()).properties === 12, 'FTB Chunks registered its 12 properties on FTB Teams');

  // --- The properties: shown with the config's defaults, set, refused when they do not read, kept in the team's file.
  const set = (id: string, value = '') => `ftbteams party settings ftbchunks:${id}${value ? ' ' + value : ''}`;
  await say(set('allow_explosions'), 'Explosions break blocks=false');
  await say(set('allow_mob_griefing'), 'Mobs may change blocks=false');
  await say(set('allow_pvp'), 'Players may fight=true');
  await say(set('claim_visibility'), 'Who sees the claims=public');
  await say(set('block_edit_mode'), 'Who may break and place=allies');
  await say(set('block_interact_mode'), 'Who may use blocks=allies');
  await say(set('allow_explosions', 'maybe'), 'Could not read the value maybe');
  await say(set('block_edit_mode', 'everyone'), 'Could not read the value everyone');
  await say('ftbteams party settings ftbchunks:nothing 1', 'No property has that id');
  await say(set('allow_pvp', 'false'), 'Players may fight is now false');
  await say(set('allow_pvp', 'true'), 'Players may fight is now true');
  await say(set('block_interact_mode', 'private'), 'Who may use blocks is now private');
  await say(set('block_interact_mode'), 'Who may use blocks=private');
  await say('ftbteams party settings ftbteams:color #3366ff', 'Colour is now #3366ff');
  fixture.mark('the ftbchunks properties are set and read like FTB Teams\' own');

  // --- Explosions: TNT leaves a claim's blocks alone until the team allows explosions.
  const status = await fixture.raw('status');
  const [px, py, pz] = status.player.position.map(Math.floor);
  const x0 = px & ~15, z0 = pz & ~15, y = py + 2;
  await fixture.command(`fill ${x0} ${y - 1} ${z0 - 16} ${x0 + 31} ${y - 1} ${z0 + 31} minecraft:stone`);
  await fixture.command(`fill ${x0} ${y} ${z0 - 16} ${x0 + 31} ${y + 6} ${z0 + 31} minecraft:air`);
  await fixture.command(`tp @s ${x0 + 2.5} ${y} ${z0 + 2.5}`);
  await say(`ftbchunks admin claim_as ${party.short_name} 0 ${x0 + 8} ${y} ${z0 + 8}`, '1 chunk(s) claimed');
  const cluster = [x0 + 8, y, z0 + 8];
  const dirtCluster = () => fixture.command(`fill ${x0 + 7} ${y} ${z0 + 7} ${x0 + 9} ${y} ${z0 + 9} minecraft:dirt`);
  const tnt = async () => {
    await fixture.command(`summon minecraft:tnt ${x0 + 8.5} ${y + 1} ${z0 + 8.5} {fuse:10}`);
    await until(async () => (await fixture.raw('command execute if entity @e[type=minecraft:tnt]')).chat ?? [], (c: string[]) => c.join(' ').includes('Test failed'), 'the TNT went off', 15000);
  };
  await dirtCluster();
  await tnt();
  assert((await blockAt(cluster)).includes('minecraft:dirt'), 'the claim\'s dirt survives the TNT');
  fixture.mark('TNT in a claim whose team forbids explosions: the blocks stay');
  await say(set('allow_explosions', 'true'), 'Explosions break blocks is now true');
  await tnt();
  await until(() => blockAt(cluster), (b: string) => !b.includes('minecraft:dirt'), 'the team allows explosions: the dirt goes');
  fixture.mark('allow_explosions true: the same TNT blows the blocks');
  await say(set('allow_explosions', 'false'), 'Explosions break blocks is now false');

  // --- Restart with the config: PvP never, fire and fluids stay in their claims.
  await offline(() => writeConfig(world, '\tpvp_mode: "NEVER"\n\tfire_spread_protection: true\n\tflowing_fluid_protection: true'));
  await fixture.command('gamemode survival');
  let c = await chunks();
  assert.deepEqual([c.config.pvp_mode, c.config.fire_spread_protection, c.config.flowing_fluid_protection], ['Never', true, true]);
  assert.equal(c.properties, 12, 'the properties are registered again after the restart');
  await say(set('allow_explosions'), 'Explosions break blocks=false');
  fixture.mark('the world config is read: pvp_mode NEVER, fire and fluid protection');

  // PvP: there is no second real player, so the dev command asks what a hit from here on a player standing at the
  // given block would meet (`attack`: a melee blow, `shot`: a hit that names the shooter, here the player itself).
  const pvp = async (x: number, z: number) => probeLine(`ftbchunks_probe_pvp ${x} ${z}`, 'FTBP ');
  const standAt = async (x: number, z: number) => {
    await fixture.command(`tp @s ${x} ${y} ${z}`);
    await until(() => fixture.raw('status'), (s: any) => Math.abs(s.player.position[0] - x) < 1, 'the player has moved');
    await Bun.sleep(1500);
  };
  await standAt(x0 + 3.5, z0 + 3.5);
  assert.deepEqual(await pvp(x0 + 24, z0 + 4), { attack: true, shot: true }, 'in a claim: refused whoever the victim is');
  await standAt(x0 + 24.5, z0 + 2.5);
  assert.deepEqual(await pvp(x0 + 40, z0 + 4), { attack: false, shot: false }, 'both outside the claims: it lands');
  assert.equal((await pvp(x0 + 8, z0 + 8)).attack, true, 'a victim inside a claim: refused');
  fixture.mark('pvp_mode never: a blow in, into or out of a claim is refused; on unclaimed land it lands');

  // Fluids: water beside the claim fills the wild chunk and stops at the claim's border.
  const border = [x0 + 15, y, z0 + 8];
  await fixture.command(`setblock ${x0 + 16} ${y} ${z0 + 8} minecraft:water`);
  await fixture.command(`setblock ${x0 + 24} ${y} ${z0 + 4} minecraft:water`);
  await until(() => blockAt([x0 + 20, y, z0 + 4]), (b: string) => b.includes('water'), 'water flows over unclaimed land', 30000);
  await until(() => blockAt([x0 + 18, y, z0 + 8]), (b: string) => b.includes('water'), 'the water beside the claim spreads on its own side', 30000);
  await Bun.sleep(2000);
  assert(!(await blockAt(border)).includes('water'), `the water did not cross into the claim: ${await blockAt(border)}`);
  fixture.mark('flowing_fluid_protection: water flows in the wild and stops at the claim');

  // --- Expiry. The party goes to Bob and the player leaves, so nobody of it is on; two chunks are kept loaded.
  await fixture.command('setblock ' + `${x0 + 16} ${y} ${z0 + 8} minecraft:air`);
  await say(`ftbchunks admin claim_as ${party.short_name} 0 ${x0 + 24} ${y} ${z0 + 4}`, '1 chunk(s) claimed');
  await say(`ftbchunks admin claim_as ${party.short_name} 0 ${x0 + 40} ${y} ${z0 + 4}`, '1 chunk(s) claimed');
  await say('ftbchunks load 16', /chunk\(s\) now kept loaded/);
  c = await chunks();
  assert.equal(team(c, party.id).claimed, 3);
  assert.equal(team(c, party.id).force_loaded >= 2, true, JSON.stringify(team(c, party.id)));
  await say(`ftbteams force-add ${party.short_name} Bob`, 'Bob is now in the party');
  await say('ftbteams party transfer_ownership Bob', 'Bob now owns the team');
  await say('ftbteams party leave', 'AE2Fixture left the party');
  const loaded = c.claims.filter((k: any) => k.force_loaded).map((k: any) => k.x);
  assert(loaded.length >= 2, `kept loaded: ${JSON.stringify(c.claims)}`);
  const file = join(world, 'ftbchunks', `${party.id}.snbt`);
  const age = (expireX: number | null, days: number) => () => {
    let text = readFileSync(file, 'utf8');
    text = text.replace(/last_login_time:\s*\d+L?/, `last_login_time: ${Date.now() - days * 86_400_000}L`);
    if (expireX !== null) {
      const entry = new RegExp(`(x:\\s*${expireX}\\s+z:\\s*-?\\d+\\s+time:\\s*\\d+L?\\s+force_loaded:\\s*\\d+L?)`);
      text = text.replace(entry, '$1\n\t\t\t\texpiry_time: 1000L');
      assert(text.includes('expiry_time: 1000L'), `the chunk ${expireX} got an expiry time: ${text.slice(0, 600)}`);
    }
    writeFileSync(file, text);
  };
  // 1: one force-load's expiry time passed, the team has been away 400 days; idle days are off by default.
  await offline(age(loaded[0], 400));
  c = await until(chunks, (v: any) => v.sweeps === true, 'the claims are looked at after the world opens', 60000);
  const [first, second] = [c.claims.find((k: any) => k.x === loaded[0]), c.claims.find((k: any) => k.x === loaded[1])];
  assert.equal(first.force_loaded, false, 'the chunk whose time ran out is no longer kept loaded');
  assert.equal(second.force_loaded, true, 'the other stays');
  assert.equal(team(c, party.id).claimed, 3, 'idle days are off: the claims stay however long the team was away');
  assert.equal(c.config.max_idle_days_before_unclaim, 0);
  fixture.mark('an expired force-load is let go; 400 idle days change nothing while the config says 0');
  // 2: idle past max_idle_days_before_unforce.
  writeConfig(world, '\tmax_idle_days_before_unforce: 1.0');
  await offline(age(null, 3));
  c = await until(chunks, (v: any) => v.sweeps === true, 'the sweep after the second restart', 60000);
  assert.equal(c.config.max_idle_days_before_unforce, 1);
  assert.equal(team(c, party.id).force_loaded, 0, 'idle past unforce days: no force-load is left');
  assert.equal(team(c, party.id).claimed, 3, 'the claims stay');
  fixture.mark('max_idle_days_before_unforce 1, away 3 days: the force-loads go, the claims stay');
  await until(chunks, (v: any) => v.held.length === 0, 'no tickets are held');
  // 3: idle past max_idle_days_before_unclaim.
  writeConfig(world, '\tmax_idle_days_before_unclaim: 2.0');
  await offline(age(null, 3));
  c = await until(chunks, (v: any) => v.sweeps === true, 'the sweep after the third restart', 60000);
  assert.equal(c.total_claims, 0, 'idle past unclaim days: the claims are given up');
  assert(readFileSync(file, 'utf8').includes('chunks'), 'the team file is still there');
  fixture.mark('max_idle_days_before_unclaim 2, away 3 days: all claims given up');
  fixture.assertHidden(await fixture.raw('status'));
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTB numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
