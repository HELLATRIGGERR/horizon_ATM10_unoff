/** ExtendedAE slice 5: each special bus, emitter and plane does its job. Export buses (one grid): the Tag Export Bus
 * sends what its expressions pass (planks or logs, not oak logs), the Mod Export Bus an ae2 item only, the Precise Export
 * Bus its slot's amount in whole batches (6 of 8 operations for 3), the Threshold Export Bus only what is above its
 * amount; the Active Formation Plane places a block of its filter. Storage buses (another grid): the Tag and Mod Storage
 * Buses show the grid only what their lists name, the Threshold Level Emitter keeps its band, the Smart Annihilation
 * Plane breaks only blocks whose drops its filter passes; on a third grid the Precise Storage Bus fills its barrel to
 * its amount and no further. Settings by `/ae2 eae` (the mod's screens' text and number fields); through a restart.
 * Needs content/extendedae (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-buses-2', mode, { addons: ['extendedae'] });
const cell = (id: string, n: number, size = '4k') => `ae2:item_storage_cell_${size}[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"${id}","#":${n}L}]]`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-2, -3], [-2, 12], [12, -3], [12, 12]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-2, 0, -3)} ${at(12, 4, 12)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -1, -3)} ${at(12, -1, 12)} minecraft:stone`);
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z - 2.5}`);
  const stored = async (pos: string, item: string) => ((await fixture.ae(`grid status ${pos}`)).items ?? []).find((e: any) => e.item === item)?.count ?? 0;
  const inside = async (pos: string, item: string) => ((await fixture.raw(`container ${pos}`)).slots ?? [])
    .filter((slot: any) => slot?.item === item).reduce((n: number, slot: any) => n + slot.count, 0);
  const attach = async (pos: string, id: string) => {
    await fixture.replace('hotbar.0', id); await fixture.select(0, id);
    assert.equal((await fixture.ae(`grid part ${pos} south`)).attached, true, `${id} attached`);
  };
  const filter = async (pos: string, item: string, slot = 0) => {
    await fixture.replace('hotbar.0', item); await fixture.select(0, item);
    await fixture.ae(`bus filter ${pos} south ${slot}`);
  };
  const eae = (words: string) => fixture.ae(`eae ${words}`);
  /** Lets `ticks` game ticks pass on a running server. */
  const wait = async (ticks: number) => { const t0 = (await fixture.raw('simulation')).game_time; await until(() => fixture.raw('simulation'), s => s.game_time >= t0 + ticks, `${ticks} ticks`, 30000); };

  // Grid A: power, a drive, five cables east of it, a barrel south of each.
  const driveA = at(1);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${driveA} ae2:drive`);
  for (let dx = 2; dx <= 6; dx++) await fixture.command(`setblock ${at(dx)} ae2:cable_bus`);
  for (const [slot, [id, n]] of ([['minecraft:oak_planks', 1000], ['minecraft:oak_log', 1000], ['minecraft:sand', 7000], ['ae2:certus_quartz_crystal', 500], ['minecraft:cobblestone', 1000]] as const).entries()) {
    await fixture.command(`item replace block ${driveA} container.${slot} with ${cell(id, n)}`);
  }
  await until(() => stored(driveA, 'minecraft:sand'), n => n === 7000, 'grid A online', 30000);
  await attach(at(2), 'extendedae:tag_export_bus');
  await attach(at(3), 'extendedae:mod_export_bus');
  await attach(at(4), 'extendedae:precise_export_bus');
  await attach(at(5), 'extendedae:threshold_export_bus');
  assert.equal((await fixture.ae(`bus status ${at(2)} south`)).id, 'ae2:export_bus', 'a tag export bus is AE2\'s export bus');
  await eae(`tag ${at(2)} south white minecraft:planks | minecraft:logs`);
  await eae(`tag ${at(2)} south black minecraft:oak_log`);
  await eae(`mod ${at(3)} south ae2`);
  await filter(at(4), 'minecraft:oak_planks');
  await eae(`amount ${at(4)} south 0 3`);
  await filter(at(5), 'minecraft:sand');
  await eae(`amount ${at(5)} south 0 6990`);
  await eae(`mode ${at(5)} south greater`);
  await fixture.hold();
  for (let dx = 2; dx <= 5; dx++) await fixture.command(`setblock ${at(dx, 0, 1)} minecraft:barrel`);
  // The precise bus, tick by tick: 1 operation times the bus speed (8), in batches of 3.
  let precise = 0;
  for (let i = 0; i < 200 && precise === 0; i++) { await fixture.stepServer(1); precise = await inside(at(4, 0, 1), 'minecraft:oak_planks'); }
  assert.equal(precise, 6, 'the Precise Export Bus sends two batches of 3 of its 8');
  await fixture.hold(false);
  await until(() => inside(at(2, 0, 1), 'minecraft:oak_planks'), n => n >= 16, 'the Tag Export Bus sends planks', 30000);
  assert.equal(await inside(at(2, 0, 1), 'minecraft:oak_log'), 0, 'oak logs are blacklisted');
  assert.equal(await inside(at(2, 0, 1), 'minecraft:cobblestone'), 0, 'cobblestone is in neither list');
  fixture.mark('the Tag Export Bus sends what its white expression passes and its black one does not');
  await until(() => inside(at(3, 0, 1), 'ae2:certus_quartz_crystal'), n => n >= 16, 'the Mod Export Bus sends ae2 items', 30000);
  for (const other of ['minecraft:oak_planks', 'minecraft:sand', 'minecraft:cobblestone']) assert.equal(await inside(at(3, 0, 1), other), 0, `no ${other}`);
  fixture.mark('the Mod Export Bus sends only the listed mod\'s items');
  assert.equal((await inside(at(4, 0, 1), 'minecraft:oak_planks')) % 3, 0, 'always whole batches');
  fixture.mark('the Precise Export Bus sends its amount in whole batches');
  await until(() => inside(at(5, 0, 1), 'minecraft:sand'), n => n === 10, 'the Threshold Export Bus sends what is above 6,990', 30000);
  await wait(40);
  assert.equal(await inside(at(5, 0, 1), 'minecraft:sand'), 10, 'and no more');
  assert.equal(await stored(driveA, 'minecraft:sand'), 6990);
  // LOWER: at or under its amount it sends, with no bound.
  await eae(`amount ${at(5)} south 0 7000`);
  await eae(`mode ${at(5)} south lower`);
  await until(() => inside(at(5, 0, 1), 'minecraft:sand'), n => n > 10, 'LOWER sends under its amount', 30000);
  fixture.mark('the Threshold Export Bus sends only what is above its amount, or while the grid is at or under it');

  // The Active Formation Plane: its filter's block in front of it, again once taken away.
  await attach(at(6), 'extendedae:active_formation_plane');
  await filter(at(6), 'minecraft:oak_log');
  await until(() => fixture.raw(`getblock ${at(6, 0, 1)}`), v => String(v.block).startsWith('minecraft:oak_log'), 'the plane places an oak log', 30000);
  const logs = await stored(driveA, 'minecraft:oak_log');
  await fixture.command(`setblock ${at(6, 0, 1)} minecraft:air`);
  await until(() => fixture.raw(`getblock ${at(6, 0, 1)}`), v => String(v.block).startsWith('minecraft:oak_log'), 'and again', 30000);
  assert.equal(await stored(driveA, 'minecraft:oak_log'), logs - 1, 'one log from the grid');
  fixture.mark('the Active Formation Plane places blocks of its filter from the grid');

  // Grid B: power, a drive with 1,000 cobblestone, four cables; barrels with mixed items south.
  const driveB = at(1, 0, 4);
  await fixture.command(`setblock ${at(0, 0, 4)} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${driveB} ae2:drive`);
  for (let dx = 2; dx <= 5; dx++) await fixture.command(`setblock ${at(dx, 0, 4)} ae2:cable_bus`);
  await fixture.command(`item replace block ${driveB} container.0 with ${cell('minecraft:cobblestone', 1000)}`);
  await until(() => stored(driveB, 'minecraft:cobblestone'), n => n === 1000, 'grid B online', 30000);
  await attach(at(2, 0, 4), 'extendedae:tag_storage_bus');
  await attach(at(3, 0, 4), 'extendedae:mod_storage_bus');
  await eae(`tag ${at(2, 0, 4)} south white minecraft:logs`);
  await eae(`mod ${at(3, 0, 4)} south ae2`);
  await fixture.command(`setblock ${at(2, 0, 5)} minecraft:barrel`);
  await fixture.command(`item replace block ${at(2, 0, 5)} container.0 with minecraft:oak_log 5`);
  await fixture.command(`item replace block ${at(2, 0, 5)} container.1 with minecraft:cobblestone 7`);
  await fixture.command(`setblock ${at(3, 0, 5)} minecraft:barrel`);
  await fixture.command(`item replace block ${at(3, 0, 5)} container.0 with ae2:certus_quartz_crystal 3`);
  await fixture.command(`item replace block ${at(3, 0, 5)} container.1 with minecraft:stone 4`);
  await until(() => stored(driveB, 'minecraft:oak_log'), n => n === 5, 'the Tag Storage Bus shows its logs', 30000);
  assert.equal(await stored(driveB, 'minecraft:cobblestone'), 1000, 'and not its cobblestone');
  fixture.mark('the Tag Storage Bus shows the grid only what its expression passes');
  await until(() => stored(driveB, 'ae2:certus_quartz_crystal'), n => n === 3, 'the Mod Storage Bus shows its ae2 item', 30000);
  assert.equal(await stored(driveB, 'minecraft:stone'), 0, 'and not its stone');
  await eae(`mod ${at(3, 0, 4)} south -`);
  await until(() => stored(driveB, 'ae2:certus_quartz_crystal'), n => n === 0, 'an empty list names no mod', 30000);
  fixture.mark('the Mod Storage Bus shows the grid only the listed mods\' items');

  // The Threshold Level Emitter on cobblestone, band 995 to 1,005.
  await attach(at(4, 0, 4), 'extendedae:threshold_level_emitter');
  await fixture.replace('hotbar.0', 'minecraft:cobblestone'); await fixture.select(0, 'minecraft:cobblestone');
  await fixture.ae(`emitter filter ${at(4, 0, 4)} south`);
  await eae(`band ${at(4, 0, 4)} south 1005 995`);
  const emitting = async () => (await eae(`show ${at(4, 0, 4)} south`)).active;
  const cobble = async (n: number) => {
    await fixture.command(`item replace block ${driveB} container.0 with ${cell('minecraft:cobblestone', n)}`);
    await until(() => stored(driveB, 'minecraft:cobblestone'), m => m === n, `${n} cobblestone`, 30000);
    await wait(10);
  };
  // Its state is saved and kept when the band changes: on since its threshold was 0, it stays on at 1,000.
  await wait(10);
  assert.equal(await emitting(), true, 'on from before, 1,000 over the lower bound');
  await cobble(990);
  await until(emitting, on => on === false, 'off under 995', 15000);
  await cobble(1000);
  assert.equal(await emitting(), false, 'still off at 1,000, under the upper bound');
  await cobble(1010);
  await until(emitting, on => on === true, 'on at 1,010', 15000);
  await cobble(1000);
  assert.equal(await emitting(), true, 'still on at 1,000');
  fixture.mark('the Threshold Level Emitter turns on at its upper bound and off under its lower one');

  // The Smart Annihilation Plane, filtered to cobblestone: it breaks cobblestone, not dirt.
  await attach(at(5, 0, 4), 'extendedae:smart_annihilation_plane');
  await eae(`config ${at(5, 0, 4)} south 0 minecraft:cobblestone`);
  await fixture.command(`setblock ${at(5, 0, 5)} minecraft:dirt`);
  await wait(60);
  assert.match(String((await fixture.raw(`getblock ${at(5, 0, 5)}`)).block), /minecraft:dirt/, 'dirt stays');
  const before = await stored(driveB, 'minecraft:cobblestone');
  await fixture.command(`setblock ${at(5, 0, 5)} minecraft:cobblestone`);
  await until(() => fixture.raw(`getblock ${at(5, 0, 5)}`), v => v.block === 'minecraft:air', 'cobblestone broken', 30000);
  assert.equal(await stored(driveB, 'minecraft:cobblestone'), before + 1, 'into the grid');
  fixture.mark('the Smart Annihilation Plane takes only what its filter passes');

  // Grid C: a Precise Storage Bus on an empty barrel, 10 cobblestone; an Import Bus from a barrel of 64.
  await fixture.command(`setblock ${at(0, 0, 8)} ae2:creative_energy_cell`);
  for (let dx = 1; dx <= 2; dx++) await fixture.command(`setblock ${at(dx, 0, 8)} ae2:cable_bus`);
  await attach(at(1, 0, 8), 'extendedae:precise_storage_bus');
  await filter(at(1, 0, 8), 'minecraft:cobblestone');
  await eae(`amount ${at(1, 0, 8)} south 0 10`);
  await fixture.command(`setblock ${at(1, 0, 9)} minecraft:barrel`);
  await attach(at(2, 0, 8), 'ae2:import_bus');
  await fixture.command(`setblock ${at(2, 0, 9)} minecraft:barrel`);
  await fixture.command(`item replace block ${at(2, 0, 9)} container.0 with minecraft:cobblestone 64`);
  await until(() => inside(at(1, 0, 9), 'minecraft:cobblestone'), n => n === 10, 'the precise bus fills to 10', 30000);
  await wait(60);
  assert.equal(await inside(at(1, 0, 9), 'minecraft:cobblestone'), 10, 'and no further');
  assert.equal(await inside(at(2, 0, 9), 'minecraft:cobblestone'), 54, 'the rest stays');
  fixture.mark('the Precise Storage Bus keeps its target at its amount');

  // Through a restart: the expressions, lists, amounts and band stay.
  await fixture.restart();
  await until(() => stored(driveA, 'minecraft:sand'), n => n > 0, 'grid A after restart', 30000);
  const tag = await eae(`show ${at(2)} south`);
  assert.equal(tag.white, 'minecraft:planks | minecraft:logs');
  assert.equal(tag.black, 'minecraft:oak_log');
  assert.equal((await eae(`show ${at(3)} south`)).mod, 'ae2');
  assert.equal((await eae(`show ${at(4)} south`)).amounts[0], 3);
  const band = await eae(`show ${at(4, 0, 4)} south`);
  assert.deepEqual([band.upper, band.lower], [1005, 995]);
  assert.equal(await inside(at(1, 0, 9), 'minecraft:cobblestone'), 10);
  fixture.mark('the special buses keep their settings through a restart');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
