/** Flux Networks' plug and point on one network, as the `fluxnetworks` native bundle and its content mod make them.
 *
 * AE2's debug energy generator (a creative FE source: 8 FE a tick to the power of 1 + the generators beside it) feeds a
 * plug; a point 20 blocks away feeds an AE2 Energy Acceptor with an energy cell on the same grid (2 FE = 1 AE).
 * - `config/fluxnetworks-server.toml` is read: a test file's values, then ATM10's own after a restart;
 * - right-clicking a device opens the chest menu: create a network (a shift click and a double click don't make extra
 *   ones, the player's quota holds), connect, disconnect, all with real clicks;
 * - per tick: 8 FE (one generator) and 4096 FE (with three beside it) reach the grid, with no loss;
 * - a plug's limit, a point's limit, a second point's higher priority, surge and bypass change the amounts as they do
 *   in the mod's transfer cycle;
 * - the network, the settings and energy earned into a plug's buffer survive a save and a restart, and reach the grid
 *   exactly once; a `/data` write to a buffer acts;
 * - energy earned into a point's buffer stays in the item when a player breaks the point, and comes back with its
 *   network and settings when the player places the item again;
 * - a plug disconnected from the menu and a point or plug broken while energy flows stop the flow, and a broken plug's
 *   buffer goes with its item, not to the network.
 *
 * Not here: another player's access (one player only; the crate tests it).
 *
 * Energy in the devices is read from their saved compounds (`data get block`, the mod's `energy`), AE from the cell.
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`FLUX_CONTENT_DIR`);
 * `pack/scripts/fluxnetworks.ts world` sets both, and `ATM10_DIR` for ATM10's file. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.FLUX_CONTENT_DIR ?? join(root, '../.build/content/fluxnetworks'));
const fixture = new WorldFixture('flux-plug-point', 'server-thread', { devMods: [content] });
// Settings that differ from the defaults (800000, 5), so that seeing them proves the file was read.
const LIMIT = 700000, MAX_NETWORKS = 2;
const configFile = join(fixture.game, 'config/fluxnetworks-server.toml');
mkdirSync(join(fixture.game, 'config'), { recursive: true });
writeFileSync(configFile, `[energy]\ndefaultLimit = ${LIMIT}\n[networks]\nmaximumPerPlayer = ${MAX_NETWORKS}\n`);
const atm10Config = process.env.ATM10_DIR ? join(process.env.ATM10_DIR, 'config/fluxnetworks-server.toml') : '';

/** The block entity's compound on the server, as `data get block` prints it. */
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string): number => {
  const found = nbt.match(new RegExp(`\\b${name}: ?(-?\\d+)`));
  assert(found, `${name} in ${nbt}`);
  return Number(found[1]);
};
const buffer = async (pos: string) => field(await saved(pos), 'energy');
const networkOf = async (pos: string) => field(await saved(pos), 'network_id');
/** `/flux ...`, as the world's owner runs it; its answer comes back in the reply. */
async function flux(words: string): Promise<any> {
  const reply = await fixture.command(`flux ${words}`);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith('FLUX {'));
  assert(line, `/flux ${words}: ${JSON.stringify(reply.chat)}`);
  const value = JSON.parse(line.slice(5));
  assert(!value.error, line);
  return value;
}

/** A grid's AE, buffers included. */
async function ae(cell: string): Promise<number> {
  const s = await fixture.ae(`power ${cell} north`);
  return s.energy + (s.energy_service?.buffer ?? 0);
}
/** What each grid in `cells` gains in each of `ticks` steps of the held server, in FE. */
async function gains(cells: string[], ticks: number): Promise<number[][]> {
  const out: number[][] = cells.map(() => []);
  let before = await Promise.all(cells.map(ae));
  for (let i = 0; i < ticks; i++) {
    await fixture.stepServer(1);
    const now = await Promise.all(cells.map(ae));
    now.forEach((value, n) => out[n].push(Math.round((value - before[n]) * 2)));
    before = now;
  }
  return out;
}
/** Steps until the flow settles (a setting acts within a tick or two, and the plug's buffer drains), then checks the
 * rate in FE a tick of each grid in `cells` for 5 ticks. */
async function rate(what: string, cells: string[], expected: number[]): Promise<void> {
  await fixture.stepServer(4);
  const got = await gains(cells, 5);
  got.forEach((perTick, n) => assert.deepEqual(perTick, Array(5).fill(expected[n]), `${what}, grid ${n}: ${JSON.stringify(got)}`));
  log[what] = got.map(perTick => perTick[perTick.length - 1]);
  fixture.mark(`${what}: ${expected.join(' and ')} FE a tick`);
}

const CREATE = 345, DISCONNECT = 353, FIRST_NETWORK = 300, SECOND_NETWORK = 301, INFO = 349;
const slot = (view: any, id: number) => view.slots?.find((s: any) => s.slot === id && s.item);
const lore = (view: any, id: number) => String(slot(view, id)?.components);
/** Stands by a device and right clicks it: its chest menu. */
async function openMenu(pos: string, [x, y, z]: number[]): Promise<any> {
  await fixture.command(`tp @s ${x + 0.5} ${y - 1} ${z + 2.5}`);
  await fixture.raw(`interact ${pos} south`);
  return until(() => fixture.raw('screen'), view => view.screen === 'container' && !!slot(view, CREATE), `the menu of ${pos} opens`);
}
/** What the nearest dropped item holds. */
async function dropAt(pos: string): Promise<string> {
  const reply = await fixture.command(`execute positioned ${pos} run data get entity @e[type=minecraft:item,limit=1,sort=nearest,distance=..3] Item`);
  return (reply.chat ?? []).join(' ');
}
const keptEnergy = (text: string): number => {
  const found = text.match(/stored_energy"?: ?(-?\d+)L/);
  assert(found, `no stored energy in ${text}`);
  return Number(found[1]);
};

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.ceil(py) + 3, z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  // The generator west of the plug, the plug 20 blocks west of point A; each point has an acceptor east of it and a
  // cell east of that. Point B is 6 blocks south of A. Points C and D share a lone acceptor between them.
  const plug = at(0), generator = at(-1), pointA = at(20), acceptorA = at(21), cellA = at(22);
  const pointB = at(20, 0, 6), acceptorB = at(21, 0, 6), cellB = at(22, 0, 6), loose = at(0, 0, -4);
  const pointC = at(9), lone = at(10), pointD = at(11);
  /** Three more generators beside the first, none touching the plug: it gives 8^4 FE a tick. */
  const boost = async () => {
    for (const beside of [at(-2), at(-1, 1), at(-1, -1)]) await fixture.command(`setblock ${beside} ae2:debug_energy_gen`);
  };
  await fixture.command(`fill ${at(-3, -3, -6)} ${at(24, 3, 11)} minecraft:air`);
  // Floors to stand on by each device, for the menus and the hands.
  for (const [from, to] of [[at(-1, -2, 0), at(1, -2, 4)], [at(8, -2, 0), at(12, -2, 4)], [at(19, -2, 0), at(21, -2, 10)]]) await fixture.command(`fill ${from} ${to} minecraft:stone`);

  // The test file's settings, not the defaults: the file was read.
  const first = await flux('networks');
  log.settings = first;
  assert.deepEqual([first.from_file, first.default_limit, first.maximum_per_player], [true, LIMIT, MAX_NETWORKS]);
  fixture.mark(`the config file is read: limit ${LIMIT}, ${MAX_NETWORKS} networks a player`);

  // The blocks as declared: six sides unconnected; the item takes the device's (new) settings, as the mod's loot does.
  await fixture.command(`setblock ${loose} fluxnetworks:flux_plug`);
  const shown = await until(() => fixture.raw(`block ${loose}`), v => v.block === 'fluxnetworks:flux_plug', 'a placed plug reaches the client');
  for (const side of ['down', 'up', 'north', 'south', 'west', 'east']) assert(String(shown.state).includes(`${side}=false`), `${side} in ${shown.state}`);
  log.block_state = shown.state;
  await fixture.command(`setblock ${loose} minecraft:air destroy`);
  const dropped = await dropAt(loose);
  log.dropped_new_plug = dropped;
  assert(dropped.includes('fluxnetworks:flux_plug'), `drop: ${dropped}`);
  assert.equal(keptEnergy(dropped), 0);
  assert.match(dropped, /device_config"?: ?\{/);
  assert.match(dropped, new RegExp(`limit: ?${LIMIT}L`));
  await fixture.command('kill @e[type=minecraft:item,distance=..64]');
  fixture.mark('a placed plug shows no connections, and its dropped item carries its settings and energy');

  await fixture.command(`setblock ${plug} fluxnetworks:flux_plug`);
  await fixture.command(`setblock ${pointA} fluxnetworks:flux_point`);
  await fixture.command(`setblock ${pointB} fluxnetworks:flux_point`);
  await fixture.command(`setblock ${acceptorA} ae2:energy_acceptor`);
  await fixture.command(`setblock ${cellA} ae2:dense_energy_cell`);
  await fixture.command(`setblock ${acceptorB} ae2:energy_acceptor`);
  await fixture.command(`setblock ${cellB} ae2:dense_energy_cell`);
  const fresh = await saved(plug);
  log.new_plug_nbt = fresh;
  assert.equal(field(fresh, 'network_id'), -1, 'a new device is on no network');
  assert.equal(field(fresh, 'limit'), LIMIT, 'energy.defaultLimit');
  assert.equal(field(fresh, 'energy'), 0);
  await until(() => fixture.ae(`power ${cellA} north`), s => s.energy_service?.members > 0, 'acceptor A and its cell form a grid', 30000);
  await until(() => fixture.ae(`power ${cellB} north`), s => s.energy_service?.members > 0, 'acceptor B and its cell form a grid', 30000);

  // The menu: no networks yet. A shift click on Create means nothing.
  const plugPos = [x, y, z], pointPos = [x + 20, y, z];
  let menu = await openMenu(plug, plugPos);
  assert.match(JSON.stringify(menu.title), /Flux Plug/);
  assert.equal(slot(menu, FIRST_NETWORK), undefined, 'no networks yet');
  assert.equal(slot(menu, CREATE).item, 'minecraft:emerald');
  assert.equal(slot(menu, DISCONNECT).item, 'minecraft:barrier');
  assert.match(lore(menu, INFO), /Network: none/);
  await fixture.raw(`click-slot ${CREATE} shift`);
  await Bun.sleep(1500);
  assert.equal((await flux('networks')).networks.length, 0, 'a shift click on Create makes no network');
  // A double click is two plain clicks at once: one network.
  await Promise.all([fixture.raw(`click-slot ${CREATE}`), fixture.raw(`click-slot ${CREATE}`)]);
  menu = await until(() => fixture.raw('screen'), view => !!slot(view, FIRST_NETWORK), 'the new network is listed');
  await Bun.sleep(1500);
  assert.equal((await flux('networks')).networks.length, 1, 'a double click on Create makes one network');
  assert.match(lore(menu, FIRST_NETWORK), /AE2Fixture's Network #1/);
  log.menu_network = slot(menu, FIRST_NETWORK);
  // A later click makes the second, and the player's quota stops the third.
  await fixture.raw(`click-slot ${CREATE}`);
  await until(() => fixture.raw('screen'), view => !!slot(view, SECOND_NETWORK), 'the second network is listed');
  await Bun.sleep(500);
  await fixture.raw(`click-slot ${CREATE}`);
  await Bun.sleep(1500);
  assert.equal((await flux('networks')).networks.length, MAX_NETWORKS, 'networks.maximumPerPlayer holds');
  // Connect; a shift click on another network doesn't move the plug.
  await fixture.raw(`click-slot ${FIRST_NETWORK}`);
  menu = await until(() => fixture.raw('screen'), view => /Connected/.test(lore(view, FIRST_NETWORK)), 'the plug shows as connected');
  assert.match(lore(menu, INFO), new RegExp(`Transfer limit: ${LIMIT} FE/t`));
  await fixture.raw(`click-slot ${SECOND_NETWORK} shift`);
  await Bun.sleep(1500);
  menu = await fixture.raw('screen');
  assert.match(lore(menu, FIRST_NETWORK), /Connected/, 'a shift click connects nothing');
  try {
    const shot = join(fixture.run, 'menu.png');
    await Bun.sleep(500);
    await fixture.raw(`screenshot ${shot}`);
    log.menu_screenshot = shot;
  } catch (error) { log.menu_screenshot = `not taken: ${error}`; }
  await fixture.raw('close');
  const network = await networkOf(plug);
  assert(network > 0, `the plug is on network ${network}`);
  log.network_id = network;
  // The points join by the same click, from their own menus.
  for (const [pos, stand] of [[pointA, pointPos], [pointB, [x + 20, y, z + 6]]] as [string, number[]][]) {
    menu = await openMenu(pos, stand);
    assert.match(JSON.stringify(menu.title), /Flux Point/);
    assert(slot(menu, FIRST_NETWORK), 'the network is listed on the point');
    await fixture.raw(`click-slot ${FIRST_NETWORK}`);
    await until(() => fixture.raw('screen'), view => /Connected/.test(lore(view, FIRST_NETWORK)), 'the point shows as connected');
    await fixture.raw('close');
    assert.equal(await networkOf(pos), network, 'the point saves its network');
  }
  const listed = await flux('networks');
  log.networks = listed;
  assert.equal(listed.networks.length, MAX_NETWORKS);
  assert.equal(listed.networks[0].id, network);
  assert.equal(listed.networks[0].devices, 3);
  assert.equal(listed.networks[0].security, 'Private');
  fixture.mark(`the chest menu creates network ${network} (${listed.networks[0].name}), takes plain clicks only, and connects a plug and two points`);

  // A point shows its side with FE on its block, and answers a probe.
  await until(() => fixture.raw(`block ${pointA}`), v => String(v.state).includes('east=true'), 'point A connects to its acceptor', 20000);
  const probed = await flux(`probe ${pointA}`);
  log.probe_point = probed;
  assert.equal(probed.kind, 'fluxnetworks:flux_point');
  assert.equal(probed.network, network);
  assert.equal(probed.connections, 1 << 5, 'east, Direction 5');
  assert.equal(probed.members, 3);
  fixture.mark('point A shows and probes its connection to the acceptor');

  // Point B out of the way first: the plug and point A alone.
  await fixture.command(`data merge block ${pointB} {limit:0L}`);
  await fixture.command(`setblock ${generator} ae2:debug_energy_gen`);
  await until(() => fixture.raw(`block ${generator}`), v => v.block === 'ae2:debug_energy_gen', 'the generator reaches the client');
  await fixture.hold();

  await rate('the debug generator, 8 FE a tick, through plug and point', [cellA], [8]);
  await boost();
  await rate('a generator with three beside it', [cellA], [4096]);

  // Limits: the plug's bounds what it takes; the point's, what it gives. The buffers stay within a tick's worth.
  await fixture.command(`data merge block ${plug} {limit:500L}`);
  await rate('a plug limited to 500', [cellA], [500]);
  assert(await buffer(plug) <= 500, 'a limited plug holds no more than its limit');
  await fixture.command(`data merge block ${plug} {limit:${LIMIT}L}`);
  await fixture.command(`data merge block ${pointA} {limit:300L}`);
  await rate('a point limited to 300', [cellA], [300]);
  await fixture.command(`data merge block ${pointA} {limit:0L}`);
  await rate('a point limited to 0', [cellA], [0]);
  await fixture.command(`data merge block ${pointA} {limit:${LIMIT}L}`);

  // Two points against a plug of 1000: the higher priority first, surge over any priority, limits pass the rest on.
  await fixture.command(`data merge block ${plug} {limit:1000L}`);
  await fixture.command(`data merge block ${pointB} {limit:${LIMIT}L,priority:5}`);
  const both = [cellA, cellB];
  await rate('point B at priority 5 before A at 0', both, [0, 1000]);
  await fixture.command(`data merge block ${pointA} {surge_mode:1b}`);
  await rate('point A in surge before B at priority 5', both, [1000, 0]);
  await fixture.command(`data merge block ${pointA} {limit:300L}`);
  await rate('A in surge limited to 300, the rest to B', both, [300, 700]);
  await fixture.command(`data merge block ${pointA} {limit:300L,disable_limit:1b}`);
  await rate('A in surge with its limit bypassed', both, [1000, 0]);
  await fixture.command(`data merge block ${pointA} {limit:${LIMIT}L,disable_limit:0b,surge_mode:0b}`);
  await fixture.command(`data merge block ${pointB} {limit:0L,priority:0}`);
  await rate('point A alone again, the plug limited to 1000', both, [1000, 0]);
  await fixture.command(`data merge block ${plug} {limit:${LIMIT}L}`);
  await rate('the plug unlimited again', both, [4096, 0]);

  // Save and restart: the network, the settings and the flow.
  await fixture.command(`data merge block ${plug} {priority:7}`);
  await fixture.command(`data merge block ${pointA} {limit:1200L}`);
  await rate('point A limited to 1200 before the restart', both, [1200, 0]);
  await fixture.hold(false);
  await fixture.restart();
  assert.equal(await networkOf(plug), network, 'the plug keeps its network');
  assert.equal(await networkOf(pointA), network);
  const plugNbt = await saved(plug), pointNbt = await saved(pointA);
  log.after_restart_plug = plugNbt; log.after_restart_point = pointNbt;
  assert.equal(field(plugNbt, 'priority'), 7);
  assert.equal(field(pointNbt, 'limit'), 1200);
  await until(() => fixture.ae(`power ${cellA} north`), s => s.energy_service?.members > 0, 'the grid forms again', 30000);
  const again = await until(() => flux('networks'), v => v.networks[0]?.devices === 3, 'every device rejoins at its first tick', 20000);
  log.networks_after_restart = again;
  assert.equal(again.networks.length, MAX_NETWORKS, 'data/fluxnetworksdata.dat keeps the networks');
  assert.equal(again.networks[0].id, network);
  assert.equal(again.networks[0].name, listed.networks[0].name);
  await fixture.hold();
  await rate('after the restart, point A limited to 1200', both, [1200, 0]);
  fixture.mark(`network ${network} and the settings survive a save and a restart, and the energy flows again`);

  // Energy earned into a plug's buffer: the point stops after the plug took a tick's intake, which stays there.
  await fixture.command(`data merge block ${pointA} {limit:${LIMIT}L}`);
  await rate('flowing before a plug earns a buffer', both, [4096, 0]);
  await fixture.command(`data merge block ${pointA} {limit:0L}`);
  await fixture.stepServer(3);
  const earned = await buffer(plug);
  log.plug_earned = earned;
  assert(earned > 0 && earned <= 4096, `the plug holds what it took: ${earned}`);
  await fixture.stepServer(3);
  assert.equal(await buffer(plug), earned, 'a stopped network lets it take no more');
  await fixture.command(`setblock ${generator} minecraft:air`);
  await fixture.hold(false);

  // Over this restart, ATM10's own file in place of the test's: the same values as the mod's defaults, read from it.
  await fixture.raw('save');
  await fixture.stop();
  const fromAtm10 = !!atm10Config && existsSync(atm10Config);
  if (fromAtm10) copyFileSync(atm10Config, configFile);
  await fixture.start();
  assert.equal(await buffer(plug), earned, 'the earned buffer survives the restart');
  if (fromAtm10) {
    const settings = await flux('networks');
    log.settings_atm10 = settings;
    assert.deepEqual([settings.from_file, settings.default_limit, settings.maximum_per_player], [true, 800000, 5]);
    await fixture.command(`setblock ${loose} fluxnetworks:flux_plug`);
    assert.equal(field(await saved(loose), 'limit'), 800000, 'a new device takes ATM10\'s limit');
    await fixture.command(`setblock ${loose} minecraft:air`);
    fixture.mark('ATM10\'s fluxnetworks-server.toml is read: limit 800000, 5 networks a player');
  } else log.settings_atm10 = 'not tested: no ATM10_DIR';
  await until(() => fixture.ae(`power ${cellA} north`), s => s.energy_service?.members > 0, 'the grid forms again', 30000);
  await until(() => flux('networks'), v => v.networks[0]?.devices === 3, 'every device rejoins', 20000);
  await fixture.hold();
  // Once the point runs again, that energy goes through and no more.
  const beforeDrain = await ae(cellA);
  await fixture.command(`data merge block ${pointA} {limit:${LIMIT}L}`);
  await fixture.stepServer(6);
  const drained = Math.round((await ae(cellA) - beforeDrain) * 2);
  log.drained_fe = drained;
  assert.equal(drained, earned, 'all of the buffer reaches the grid, once');
  assert.equal(await buffer(plug), 0);
  assert.equal(await buffer(pointA), 0);
  fixture.mark(`${earned} FE earned by a plug survive a restart and reach the grid exactly once`);
  // A `/data` write to a buffer acts: the live device takes it.
  const beforeData = await ae(cellA);
  await fixture.command(`data merge block ${plug} {energy:500L}`);
  await fixture.stepServer(4);
  assert.equal(Math.round((await ae(cellA) - beforeData) * 2), 500, 'what /data put in the plug reaches the grid');
  assert.equal(await buffer(plug), 0);
  fixture.mark('a /data write to a buffer reaches the grid');
  await fixture.hold(false);

  // Energy earned into a point's buffer: two points on one acceptor both ask for its room, the first to send fills it,
  // and the other keeps what it was given.
  await fixture.command(`data merge block ${pointA} {limit:0L}`);
  await fixture.command(`setblock ${generator} ae2:debug_energy_gen`);
  await fixture.command(`setblock ${lone} ae2:energy_acceptor`);
  await until(() => fixture.ae(`power ${lone} north`), s => s.energy_service?.members > 0, 'the lone acceptor is a grid', 30000);
  await fixture.command(`setblock ${pointC} fluxnetworks:flux_point`);
  await fixture.command(`setblock ${pointD} fluxnetworks:flux_point`);
  await fixture.command(`data merge block ${pointC} {network_id:${network}}`);
  await fixture.command(`data merge block ${pointD} {network_id:${network}}`);
  const pointBuffers = async () => ({ c: await buffer(pointC), d: await buffer(pointD) });
  const held = await until(pointBuffers, v => v.c + v.d > 0, 'a point keeps what the acceptor had no more room for', 20000);
  log.point_buffers = held;
  // Both stop, so nothing moves from here on; the one with the buffer has settings the item must keep.
  await fixture.command(`data merge block ${pointC} {limit:0L}`);
  await fixture.command(`data merge block ${pointD} {limit:0L}`);
  await fixture.command(`setblock ${generator} minecraft:air`);
  await Bun.sleep(500);
  const stopped = await pointBuffers();
  log.point_buffers_stopped = stopped;
  const holder = stopped.c > 0 ? pointC : pointD, holderX = stopped.c > 0 ? x + 9 : x + 11;
  const pointEarned = Math.max(stopped.c, stopped.d);
  assert(pointEarned > 0, `a point holds a buffer it earned: ${JSON.stringify(stopped)}`);
  await fixture.command(`data merge block ${holder} {priority:3,surge_mode:1b,custom_name:"Bank"}`);
  await Bun.sleep(300);
  assert.equal(await buffer(holder), pointEarned);
  fixture.mark(`a point earns ${pointEarned} FE into its buffer`);

  // A player breaks it with a pickaxe and picks the item up; the item keeps all of it.
  const standAt = `${holderX + 0.5} ${y - 1} ${z + 2.5}`;
  await fixture.command(`tp @s ${standAt}`);
  await fixture.replace('hotbar.0', 'minecraft:diamond_pickaxe', 1);
  await fixture.raw('hotbar 1');
  await fixture.command('gamemode survival @s');
  await until(() => fixture.raw(`block ${holder}`), v => v.block === 'fluxnetworks:flux_point', 'the point reaches the client');
  await fixture.raw(`break ${holder}`);
  await until(() => fixture.raw(`block ${holder}`), v => v.block === 'minecraft:air', 'the player breaks the point');
  await fixture.command(`tp @s ${holderX + 0.5} ${y - 1} ${z + 0.5}`);
  const stacks = (v: any): any[] => [...(v.hotbar ?? []), ...(v.main ?? [])];
  const inHand = await until(() => fixture.raw('inventory'), v => stacks(v).some(s => s?.item === 'fluxnetworks:flux_point'), 'the player picks the item up');
  const picked = stacks(inHand).find(s => s?.item === 'fluxnetworks:flux_point');
  log.picked_point = picked;
  const kept = String(picked.components);
  assert.equal(keptEnergy(kept), pointEarned, `the item keeps the buffer: ${kept}`);
  assert.match(kept, new RegExp(`network_id: ?${network}\\b`));
  assert.match(kept, /priority: ?3\b/);
  fixture.mark(`broken by a player, the item keeps ${pointEarned} FE, the network and the settings`);

  // Placed again from the hand: the block has them back, and the player owns it.
  await fixture.command(`tp @s ${standAt}`);
  await fixture.raw(`hotbar ${picked.slot + 1}`);
  await fixture.raw(`place ${holder}`);
  await until(() => fixture.raw(`block ${holder}`), v => v.block === 'fluxnetworks:flux_point', 'the item places');
  const back = await saved(holder);
  log.placed_point_nbt = back;
  assert.equal(field(back, 'energy'), pointEarned, 'the buffer is back');
  assert.equal(field(back, 'network_id'), network);
  assert.equal(field(back, 'priority'), 3);
  assert.equal(field(back, 'surge_mode'), 1);
  assert.equal(field(back, 'limit'), 0);
  assert.match(back, /custom_name: ?"Bank"/);
  assert.doesNotMatch(back, /player_uuid: ?\[I; ?0, ?0, ?0, ?0\]/, 'the placer owns it');
  const live = await until(() => flux(`probe ${holder}`), v => v.buffer === pointEarned && v.network === network, 'the placed device joins its network with the buffer', 20000);
  log.placed_point_probe = live;
  await fixture.command('gamemode creative @s');
  assert.equal((await flux('networks')).networks[0].devices, 5);
  fixture.mark('placed from the hand, the point has its buffer, network and settings back, and the player owns it');

  // Disconnecting from the menu takes the plug out while energy flows.
  await fixture.command(`data merge block ${pointA} {limit:${LIMIT}L}`);
  await fixture.command(`setblock ${generator} ae2:debug_energy_gen`);
  await fixture.hold();
  await rate('flowing before the plug is disconnected', both, [4096, 0]);
  await fixture.hold(false);
  menu = await openMenu(plug, plugPos);
  await fixture.raw(`click-slot ${DISCONNECT}`);
  await until(() => fixture.raw('screen'), view => /Network: none/.test(lore(view, INFO)), 'the menu shows no network');
  await fixture.raw('close');
  assert.equal(await networkOf(plug), -1, 'the plug saves no network');
  await fixture.hold();
  await rate('the plug disconnected from the menu', both, [0, 0]);
  await fixture.hold(false);
  menu = await openMenu(plug, plugPos);
  await fixture.raw(`click-slot ${FIRST_NETWORK}`);
  await until(() => fixture.raw('screen'), view => /Connected/.test(lore(view, FIRST_NETWORK)), 'the plug joins again');
  await fixture.raw('close');
  await fixture.hold();
  await rate('the plug connected again', both, [4096, 0]);

  // A point broken while energy flows: no more reaches its grid.
  await fixture.command(`setblock ${pointA} minecraft:air destroy`);
  await rate('point A broken while energy flows', [cellA], [0]);
  const pointDrop = await dropAt(pointA);
  log.dropped_point = pointDrop;
  assert.match(pointDrop, /fluxnetworks:flux_point/);
  assert.match(pointDrop, new RegExp(`network_id: ?${network}\\b`));

  // A plug broken with a buffer: the item has it, the network gets none of it.
  await fixture.command(`data merge block ${pointB} {limit:${LIMIT}L}`);
  await rate('point B takes over', [cellB], [4096]);
  await fixture.command(`data merge block ${pointB} {limit:0L}`);
  await fixture.stepServer(3);
  const plugHeld = await buffer(plug);
  assert(plugHeld > 0, 'the plug holds a tick\'s intake');
  await fixture.command(`setblock ${generator} minecraft:air`);
  await fixture.command(`setblock ${plug} minecraft:air destroy`);
  const plugDrop = await dropAt(plug);
  log.dropped_plug = plugDrop;
  assert.equal(keptEnergy(plugDrop), plugHeld, 'the item takes the buffer');
  await fixture.command(`data merge block ${pointB} {limit:${LIMIT}L}`);
  await rate('the buffer of a broken plug reaches no point', [cellB], [0]);
  await fixture.command('kill @e[type=minecraft:item,distance=..64]');
  await fixture.hold(false);
  const after = await until(() => flux('networks'), v => v.networks[0].devices === 3, 'the broken plug and point A leave the network (B, C and D stay)', 20000);
  log.networks_at_end = after;

  // A picture of the blocks to look at.
  try {
    await fixture.command(`tp @s ${x + 17} ${y} ${z - 3} -45 10`);
    await Bun.sleep(11000);
    const shot = join(fixture.run, 'devices.png');
    await fixture.raw(`screenshot ${shot}`);
    log.screenshot = shot;
  } catch (error) { log.screenshot = `not taken: ${error}`; }
  fixture.assertHidden(await fixture.raw('status'));
  console.log('FLUX numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FLUX numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
