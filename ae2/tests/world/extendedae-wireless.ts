/** ExtendedAE slice 4, the Wireless Connector and Hub: the Wireless Setup Kit binds to one block and pairs it with
 * another; the link joins their grids as a cable would, carrying power and the controller's channels 900 blocks
 * away, and costs d ln d AE/t at each end; the kit will not pair past 1,000 blocks; a hub takes eight links and
 * refuses a ninth; the kit forgets its block when sneak-used; the hub's upgrade kit turns a linked connector into a
 * hub that keeps the link; links come back after a restart. Needs content/extendedae
 * (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-wireless', mode, { addons: ['extendedae'] });
const CONNECTOR = 'extendedae:wireless_connect', HUB = 'extendedae:wireless_hub', KIT = 'extendedae:wireless_tool';
const HUB_KIT = 'extendedae:wireless_connector_upgrade';
const chat = (reply: any) => (reply.chat ?? []).join('\n');
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  // Three sites on one row, chunk-aligned: A, B 900 blocks east of A, C 1,001 blocks east of A.
  const x = Math.floor(px / 16) * 16 + 40, y = Math.ceil(py) + 5, z = Math.floor(pz / 16) * 16 + 8;
  const sites = { a: x, b: x + 900, c: x + 1001 };
  const at = (sx: number, dx = 0, dy = 0, dz = 0) => `${sx + dx} ${y + dy} ${z + dz}`;
  const loaded = async (sx: number) => {
    try { return chat(await fixture.command(`execute if loaded ${at(sx)}`)).includes('Test passed'); } catch { return false; }
  };
  const visit = async (sx: number) => {
    await fixture.command(`tp @s ${sx + 0.5} ${y + 2} ${z - 3.5}`);
    await until(() => fixture.raw(`block ${sx} ${y} ${z}`), block => block.loaded, `the chunk at ${sx}`, 60000);
  };
  for (const sx of Object.values(sites)) {
    // Each site's chunk stays loaded while the player is away (the far end of a link must be there).
    for (const dz of [0, 16]) await fixture.command(`forceload add ${sx} ${z + dz}`);
    await until(() => loaded(sx), Boolean, `forced chunk ${sx}`, 90000);
    await visit(sx);
    await fixture.command(`fill ${at(sx, -3, 0, -2)} ${at(sx, 30, 4, 20)} minecraft:air`);
    await fixture.command(`fill ${at(sx, -3, -1, -2)} ${at(sx, 30, -1, 20)} minecraft:stone`);
  }
  await fixture.command('clear @s');
  const status = (pos: string) => fixture.ae(`grid status ${pos}`);
  const online = (pos: string, label: string) => until(() => status(pos), v => v.online && v.powered, label, 30000);
  const record = async (pos: string) => chat(await fixture.command(`data get block ${pos}`));
  const freqOf = async (pos: string, key = 'freq') => BigInt((await record(pos)).match(new RegExp(`\\b${key}: (-?\\d+)L`))?.[1] ?? 0);
  const selected = async () => chat(await fixture.command('data get entity @s SelectedItem'));
  const holdKit = async () => {
    await fixture.replace('hotbar.0', KIT); await fixture.select(0, KIT);
  };
  /** Stands on the floor a block and a half north of block `pos`, within reach of it. */
  const near = async (pos: string) => {
    const [bx, , bz] = pos.split(' ').map(Number);
    await fixture.command(`tp @s ${bx + 0.5} ${y} ${bz - 1.5}`);
  };
  const use = async (pos: string) => { await near(pos); await fixture.raw(`interact ${pos} north`); };
  /** Binds the held kit at `first` and pairs it with `second`, visiting each. */
  const pair = async (firstSite: number, first: string, secondSite: number, second: string) => {
    await visit(firstSite);
    await use(first);
    await until(selected, text => text.includes('extendedae:wireless_locator'), 'the kit bound');
    if (secondSite !== firstSite) await visit(secondSite);
    await use(second);
    await until(selected, text => !text.includes('extendedae:wireless_locator'), 'the kit used up its binding');
  };

  // Grid A: power, a controller, connector A. Site B: connector B among five drives, no power of its own.
  const cell = at(sites.a, -2), controller = at(sites.a, -1), connA = at(sites.a);
  await visit(sites.a);
  await fixture.command(`setblock ${cell} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${controller} ae2:controller`);
  await fixture.command(`setblock ${connA} ${CONNECTOR}`);
  await online(controller, 'the controller');
  await visit(sites.b);
  const connB = at(sites.b, 0, 1);
  const drivesB = [at(sites.b), at(sites.b, 0, 2), at(sites.b, 1, 1), at(sites.b, -1, 1), at(sites.b, 0, 1, 1)];
  await fixture.command(`setblock ${connB} ${CONNECTOR}`);
  for (const pos of drivesB) await fixture.command(`setblock ${pos} ae2:drive`);
  await until(() => status(drivesB[0]), v => !v.powered, 'site B has no power yet');
  assert.match(await fixture.raw(`getblock ${connB}`).then(v => v.block), /connected=false/);

  await holdKit();
  await pair(sites.a, connA, sites.b, connB);
  const f = await freqOf(connA);
  assert.notEqual(f, 0n, 'connector A has a frequency');
  assert.equal(await freqOf(connB), f, 'both connectors have the same frequency');
  for (const pos of drivesB) await online(pos, `drive ${pos} online over the link`);
  const grid = await status(controller);
  assert(grid.channels >= drivesB.length, `the controller's channels reach site B: ${JSON.stringify(grid)}`);
  assert((await status(drivesB[0])).channel, 'a drive 900 blocks away has a channel');
  await until(() => fixture.raw(`getblock ${connB}`), v => /connected=true/.test(v.block), 'connector B shows its link');
  fixture.mark('the kit pairs two connectors 900 blocks apart; their link carries power and five channels as a cable');

  // updatePowerUsage: d ln d at each end, 900 ln 900 = 6,122 AE/t each.
  const idle = (await fixture.ae(`power ${cell} north`)).idle as number;
  const end = 900 * Math.log(900);
  assert(idle >= 2 * end && idle < 2 * end + 30, `the grid's idle draw ${idle} holds two link ends of ${end}`);
  fixture.mark(`each end of a 900-block link draws 900 ln 900 = ${end.toFixed(1)} AE/t`);

  // Site C is 1,001 blocks from A: the kit bound at C refuses connector A, and C stays unpowered.
  await visit(sites.c);
  const connC = at(sites.c, 0, 1), driveC = at(sites.c);
  await fixture.command(`setblock ${connC} ${CONNECTOR}`);
  await fixture.command(`setblock ${driveC} ae2:drive`);
  await use(connC);
  await until(selected, text => text.includes('extendedae:wireless_locator'), 'the kit bound at C');
  await visit(sites.a);
  await use(connA);
  await fixture.raw('wait 10');
  assert(chat(await fixture.command('data get entity @s SelectedItem')).includes('extendedae:wireless_locator'), 'out of range: the kit keeps its binding');
  assert.equal(await freqOf(connA), f, 'connector A keeps its link');
  assert.equal(await freqOf(connC), 0n, 'connector C got no frequency');
  assert(!(await status(driveC)).powered, 'nothing reaches site C');
  // Sneak-used in the air, the kit forgets C.
  await fixture.raw('key shift down');
  try { await fixture.raw('use-item'); } finally { await fixture.raw('key shift up'); }
  await until(selected, text => !text.includes('extendedae:wireless_locator'), 'the kit cleared');
  fixture.mark('the kit will not pair 1,001 blocks apart, and forgets its block when sneak-used');

  // A hub with eight connectors around it, each with a drive; a ninth finds no port.
  const hub = at(sites.a, 0, 0, 8);
  await fixture.command(`setblock ${at(sites.a, -1, 0, 8)} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${hub} ${HUB}`);
  const spokes = Array.from({ length: 9 }, (_, i) => at(sites.a, 3 + 3 * i, 1, 12));
  for (const [i, pos] of spokes.entries()) {
    await fixture.command(`setblock ${pos} ${CONNECTOR}`);
    await fixture.command(`setblock ${at(sites.a, 3 + 3 * i, 0, 12)} ae2:drive`);
  }
  for (const pos of spokes.slice(0, 8)) await pair(sites.a, pos, sites.a, hub);
  const ports = await Promise.all(Array.from({ length: 8 }, (_, i) => freqOf(hub, `freq${i}`)));
  assert(ports.every(p => p !== 0n) && new Set(ports).size === 8, `eight ports, eight frequencies: ${ports}`);
  for (const [i, pos] of spokes.slice(0, 8).entries()) {
    assert.equal(await freqOf(pos), ports[i], `spoke ${i} on port ${i}`);
    await online(at(sites.a, 3 + 3 * i, 0, 12), `spoke ${i}'s drive online through the hub`);
  }
  await use(spokes[8]);
  await until(selected, text => text.includes('extendedae:wireless_locator'), 'the kit bound at the ninth');
  await use(hub);
  await fixture.raw('wait 10');
  assert.equal(await freqOf(spokes[8]), 0n, 'the ninth connector found no port');
  assert(!(await status(at(sites.a, 3 + 3 * 8, 0, 12))).powered, 'nothing reaches the ninth');
  await until(() => fixture.raw(`getblock ${hub}`), v => /connected=true/.test(v.block), 'the hub shows its links');
  fixture.mark('a hub takes eight links, each on its own port, and refuses a ninth');

  // The hub's kit on connector B (sneaking): a hub in its place, the link on its first port.
  await visit(sites.b);
  await fixture.replace('hotbar.1', HUB_KIT); await fixture.select(1, HUB_KIT);
  await near(connB);
  await fixture.sneakInteract(`${connB} north`);
  await until(() => fixture.raw(`getblock ${connB}`), v => v.block.startsWith(HUB), 'connector B is a hub');
  assert.equal(await freqOf(connB, 'freq0'), f, 'its first port keeps the link');
  for (const pos of drivesB) await online(pos, `drive ${pos} still online`);
  fixture.mark('the upgrade kit turns a linked connector into a hub that keeps its link');

  // A restart: the frequencies are saved, the links come back.
  await fixture.restart();
  for (const sx of Object.values(sites)) for (const dz of [0, 16]) {
    try { await fixture.command(`forceload add ${sx} ${z + dz}`); } catch {}
  }
  await visit(sites.b);
  await visit(sites.a);
  for (const pos of drivesB) await online(pos, `drive ${pos} online after the restart`);
  await online(at(sites.a, 3 + 3 * 7, 0, 12), 'the hub\'s last spoke after the restart');
  fixture.mark('the links come back after a restart');

  // Broken, a connector drops itself.
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((entity: any) => entity.item?.item === item).reduce((sum: number, entity: any) => sum + entity.item.count, 0);
  await fixture.command(`tp @s ${x + 27.5} ${y + 2} ${z + 9.5}`);
  await fixture.command(`setblock ${spokes[8]} minecraft:air destroy`);
  await until(() => ground(CONNECTOR), n => n === 1, 'the connector on the ground', 15000);
  fixture.mark('a broken connector drops itself');
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
