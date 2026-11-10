/** Ender Drives slice 1: Ender Item Storage Cells in AE2 Drives on two networks that are not connected share one
 * storage by scope and frequency: what one network puts in, the other lists and takes out; another frequency or a
 * private disk is another storage; an input-only disk gives nothing, an output-only one takes nothing; a 1k disk holds
 * one type; a disk the server config turns off is FULL and works not; a second disk of one storage in the same network
 * sleeps (FULL, no drain, its items not counted twice); a disk drains 100 + 0.015 * items^0.8 AE/t; the storage
 * comes back after a restart. Needs content/enderdrives (ae2/scripts/prepare-addon.ts enderdrives) and a client built
 * with ae2-enderdrives. */
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('enderdrives-disks', mode, { addons: ['enderdrives'] });
// The server config: the 64k disk turned off.
mkdirSync(join(fixture.game, 'config'), { recursive: true });
writeFileSync(join(fixture.game, 'config/enderdrives-server.toml'), '[general]\nenable_ender_disk_64k = false\n');
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const disk = (tier: string, data = '') => `enderdrives:ender_disk_${tier}${data ? `[minecraft:custom_data={${data}}]` : ''}`;
const drain = (items: number) => 100 + 0.015 * Math.max(1, items) ** 0.8;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const prepare = async () => {
    for (const [dx, dz] of [[-3, -3], [-3, 4], [12, -3], [12, 4]] as const) {
      await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
    }
  };
  await prepare();
  await fixture.command(`fill ${at(-3, 0, -3)} ${at(12, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(12, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s');
  // Two networks four blocks apart: an energy cell and a drive each.
  const driveA = at(1), driveB = at(7);
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [driveA, 'ae2:drive'], [at(6), 'ae2:creative_energy_cell'], [driveB, 'ae2:drive']] as const)
    await fixture.command(`setblock ${pos} ${id}`);
  for (const drive of [driveA, driveB]) await until(() => fixture.ae(`grid status ${drive}`), v => v.online && v.powered, 'drive online', 30000);
  const status = (drive: string) => fixture.ae(`grid status ${drive}`);
  const idle = async (drive: string) => (await fixture.ae(`power ${drive} east`)).idle as number;
  const leds = async (drive: string) => ((await fixture.raw(`block ${drive}`)).ae2?.cells ?? []).map((c: any) => c?.state ?? 0);
  const store = (scope: string, frequency: number) => fixture.ae(`enderdrives store ${scope} ${frequency}`);
  const deposit = async (drive: string, item: string, n: number) => {
    await fixture.replace('hotbar.0', item, n); await fixture.select(0, item);
    return (await fixture.ae(`grid deposit ${drive}`)).inserted as number;
  };
  const withdraw = async (drive: string, item: string, n: number) => {
    const view = await status(drive);
    const index = view.items.findIndex((e: any) => e.item === item);
    assert(index >= 0, `${item} listed`);
    await fixture.replace('hotbar.0', 'minecraft:air');
    return (await fixture.ae(`grid withdraw ${drive} ${index} ${n}`)).extracted as number;
  };
  const place = (drive: string, slot: number, item: string) => fixture.command(`item replace block ${drive} container.${slot} with ${item}`);
  const emptyA = await idle(driveA);

  // Frequency 0, global, in both drives: one storage.
  await place(driveA, 0, disk('4k'));
  await place(driveB, 0, disk('4k'));
  assert.equal(await deposit(driveA, 'minecraft:cobblestone', 64), 64);
  await until(() => status(driveB), v => count(v, 'minecraft:cobblestone') === 64, 'the other network lists the cobblestone', 30000);
  assert.equal(await withdraw(driveB, 'minecraft:cobblestone', 14), 14);
  await until(() => status(driveA), v => count(v, 'minecraft:cobblestone') === 50, 'the first network lists what is left', 30000);
  assert.deepEqual((await store('global', 0)).items, [{ item: 'minecraft:cobblestone', count: 50 }]);
  fixture.mark('two disks of frequency 0 on two unconnected networks share one storage');
  await until(() => leds(driveA), s => s[0] === 2, 'one type of 31: NOT_EMPTY', 30000);
  await until(() => idle(driveA), n => Math.abs(n - emptyA - drain(50)) < 1e-6, `a disk drains ${drain(50)} AE/t`, 30000);
  fixture.mark(`its LED shows one type of 31 (NOT_EMPTY) and it drains 100 + 0.015 * 50^0.8 = ${drain(50).toFixed(3)} AE/t`);

  // The held disk set by the command (the frequency screen's packet): frequency 7, then private.
  await fixture.replace('hotbar.0', disk('4k')); await fixture.select(0, 'enderdrives:ender_disk_4k');
  let info = await fixture.ae('enderdrives set 7');
  assert.equal(info.frequency, 7); assert.equal(info.scope, 'global'); assert.equal(info.prefix, 'global');
  assert.match((await fixture.command('data get entity @s SelectedItem')).chat.join('\n'), /ender_freq: ?7/);
  info = await fixture.ae('enderdrives set 7 private output');
  const owner: string = info.owner;
  assert.match(owner, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  assert.equal(info.prefix, `player_${owner}`); assert.equal(info.mode, 2);
  fixture.mark('/ae2 enderdrives set gives the held disk a frequency, and a private scope its player as owner');

  // Another frequency is another storage.
  await place(driveB, 0, disk('4k', 'ender_freq:7'));
  await until(() => status(driveB), v => count(v, 'minecraft:cobblestone') === 0, 'frequency 7 lists nothing of frequency 0', 30000);
  assert.equal(await deposit(driveB, 'minecraft:dirt', 10), 10);
  assert.equal(count(await status(driveA), 'minecraft:dirt'), 0);
  assert.deepEqual((await store('global', 7)).items, [{ item: 'minecraft:dirt', count: 10 }]);
  fixture.mark('another frequency is another storage');
  // A private disk of frequency 7 is its owner's storage, not the global one.
  await place(driveB, 0, disk('4k', `ender_freq:7,ender_scope:1,ender_owner:"${owner}"`));
  await until(() => status(driveB), v => count(v, 'minecraft:dirt') === 0, 'the private frequency 7 lists nothing of the global one', 30000);
  assert.equal(await deposit(driveB, 'minecraft:gravel', 3), 3);
  assert.deepEqual((await store(`player_${owner}`, 7)).items, [{ item: 'minecraft:gravel', count: 3 }]);
  assert.deepEqual((await store('global', 7)).items, [{ item: 'minecraft:dirt', count: 10 }]);
  fixture.mark('a private disk reaches its owner\'s storage of the frequency');

  // Transfer modes: input only gives nothing, output only takes nothing.
  await place(driveB, 0, disk('4k', 'ender_transfer_mode:1'));
  await until(() => status(driveB), v => count(v, 'minecraft:cobblestone') === 50, 'an input-only disk lists its storage', 30000);
  assert.equal(await withdraw(driveB, 'minecraft:cobblestone', 5), 0, 'an input-only disk gives nothing');
  assert.equal(await deposit(driveB, 'minecraft:cobblestone', 5), 5);
  await place(driveB, 0, disk('4k', 'ender_transfer_mode:2'));
  await until(() => status(driveB), v => count(v, 'minecraft:cobblestone') === 55, 'the output-only disk lists 55', 30000);
  assert.equal(await deposit(driveB, 'minecraft:cobblestone', 5), 0, 'an output-only disk takes nothing');
  assert.equal(await withdraw(driveB, 'minecraft:cobblestone', 5), 5);
  fixture.mark('an input-only disk gives nothing and an output-only one takes nothing');

  // A 1k disk holds one type; a disk the config turns off is FULL and works not.
  await place(driveB, 0, disk('1k', 'ender_freq:3'));
  await until(() => leds(driveB), s => s[0] === 1, 'an empty frequency: EMPTY', 30000);
  assert.equal(await deposit(driveB, 'minecraft:sand', 4), 4);
  assert.equal(await deposit(driveB, 'minecraft:red_sand', 4), 0, 'a second type past a 1k disk\'s limit of one');
  await until(() => leds(driveB), s => s[0] === 4, 'one type of one: FULL', 30000);
  await place(driveB, 0, disk('64k'));
  await until(() => leds(driveB), s => s[0] === 4, 'a disk turned off: FULL', 30000);
  assert.equal(count(await status(driveB), 'minecraft:cobblestone'), 0, 'a disk turned off lists nothing');
  assert.equal(await deposit(driveB, 'minecraft:cobblestone', 1), 0, 'and takes nothing');
  fixture.mark('a 1k disk holds one type (FULL then); a disk the config turns off is FULL, lists and takes nothing');

  // A second disk of one storage on a network sleeps: FULL, no drain, its items not counted twice.
  const oneDisk = await idle(driveA);
  await place(driveA, 1, disk('16k'));
  await until(() => leds(driveA), s => s[0] === 2 && s[1] === 4, 'the second disk sleeps: FULL', 30000);
  await Bun.sleep(1000);
  assert.equal(count(await status(driveA), 'minecraft:cobblestone'), 50, 'counted once');
  assert(Math.abs((await idle(driveA)) - oneDisk) < 1e-6, 'the sleeping disk draws nothing');
  assert.equal(await deposit(driveA, 'minecraft:cobblestone', 10), 10);
  await until(() => status(driveA), v => count(v, 'minecraft:cobblestone') === 60, 'the active disk takes it', 30000);
  // The first disk out: the second one wakes.
  await place(driveA, 0, 'minecraft:air');
  await until(() => leds(driveA), s => s[1] === 2, 'the second disk wakes', 30000);
  assert.equal(count(await status(driveA), 'minecraft:cobblestone'), 60);
  fixture.mark('a second disk of one storage on a network sleeps (FULL, no drain, not counted twice) and wakes when the first goes');

  // The storage is the world's: it comes back after a restart.
  await fixture.restart();
  await prepare();
  await until(() => status(driveA), v => count(v, 'minecraft:cobblestone') === 60, 'frequency 0 after the restart', 60000);
  assert.deepEqual((await store('global', 7)).items, [{ item: 'minecraft:dirt', count: 10 }]);
  assert.deepEqual((await store(`player_${owner}`, 7)).items, [{ item: 'minecraft:gravel', count: 3 }]);
  assert.deepEqual((await store('global', 3)).items, [{ item: 'minecraft:sand', count: 4 }]);
  assert(existsSync(join(fixture.game, 'saves', 'AE2 Fixture', 'data/enderdrives/horizon_enderdrives.dat')), 'the storage is in the world\'s data');
  fixture.mark('every frequency\'s storage comes back after a restart, from the world\'s data/enderdrives');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
