/** ExtendedAE's Oversize and Vacuum Interfaces. The Config Modifier set to MAX (its mode button) makes each config slot of an Oversize
 * Interface its oversize maximum (oversize_interface_multiplier 16: 1,024 cobblestone, 256 ender pearls, 64 buckets of
 * water), where AE2's interface stops at 64, 16 and 4 buckets; the Oversize Interface then stocks 1,024 and 256 from the
 * grid, block and part. The Vacuum Interface takes dropped items in its 5x5x5 work area into the grid and leaves those
 * outside it; with a filter only what it names; moved by its offset; all through a restart. Needs content/extendedae
 * (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-interfaces', mode, { addons: ['extendedae'] });
const cell = (id: string, n: number) => `ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"${id}","#":${n}L}]]`;
const chat = (r: any) => (r.chat ?? []).join('\n');
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-4, -4], [-4, 14], [12, -4], [12, 14]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-4, 0, -4)} ${at(12, 6, 14)} minecraft:air`);
  await fixture.command(`fill ${at(-4, -1, -4)} ${at(12, -1, 14)} minecraft:stone`);
  await fixture.command('clear @s');
  /** Lets `ticks` game ticks pass on a running server. */
  const wait = async (ticks: number) => { const t0 = (await fixture.raw('simulation')).game_time; await until(() => fixture.raw('simulation'), s => s.game_time >= t0 + ticks, `${ticks} ticks`, 30000); };
  const stored = async (pos: string, item: string) => ((await fixture.ae(`grid status ${pos}`)).items ?? []).find((e: any) => e.item === item)?.count ?? 0;
  const record = async (pos: string, path: string) => chat(await fixture.command(`data get block ${pos} ae2_lab_config.${path}`));
  const counts = (text: string) => Object.fromEntries(text.split('}').map(entry => [entry.match(/id:\s*"([^"]+)"/)?.[1], Number(entry.match(/count:\s*(\d+)/)?.[1])]).filter(([id]) => id));
  /** Uses the held item on block `pos`'s north face, standing right in front of it. */
  const use = async (pos: string, face = 'north') => {
    const [bx, by, bz] = pos.split(' ').map(Number);
    await fixture.command(`tp @s ${bx + 0.5} ${by} ${bz - 1.5}`);
    await fixture.raw(`interact ${pos} ${face}`);
  };

  // One grid: power, a drive with cobblestone, ender pearls and water; an AE2 interface, an Oversize Interface
  // and a cable with the Oversize Interface part, in a row.
  const drive = at(1), ae2 = at(3), oversize = at(5), host = at(7);
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [at(2), 'ae2:cable_bus'], [ae2, 'ae2:interface'], [at(4), 'ae2:cable_bus'], [oversize, 'extendedae:oversize_interface'], [at(6), 'ae2:cable_bus'], [host, 'ae2:cable_bus']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ${cell('minecraft:cobblestone', 8000)}`);
  await fixture.command(`item replace block ${drive} container.1 with ${cell('minecraft:ender_pearl', 2000)}`);
  await until(() => stored(drive, 'minecraft:cobblestone'), n => n === 8000, 'the grid online', 30000);
  const config = '[{Slot:0b,id:"minecraft:cobblestone",count:1},{Slot:1b,id:"minecraft:ender_pearl",count:1}]';
  const fluid = '[{Slot:2b,fluid:"minecraft:water",components:[B;],amount:1000L}]';
  for (const pos of [ae2, oversize]) {
    await fixture.command(`data modify block ${pos} ae2_lab_config.ae2_interface_config set value ${config}`);
    await fixture.command(`data modify block ${pos} ae2_lab_config.ae2_interface_fluid_config set value ${fluid}`);
  }
  // The Config Modifier set to MAX: its mode button twice from MUL (ADD, SUB, MUL, DIV, MAX, ...).
  await fixture.replace('hotbar.0', 'extendedae:config_modifier'); await fixture.select(0, 'extendedae:config_modifier');
  await fixture.raw('look 0 -90'); await fixture.raw('use-item');
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'config_modifier.json', 'Config Modifier screen', 15000);
  for (let i = 0; i < 2; i++) {
    const before = view.ae2_screen.data?.[0];
    await fixture.pressSetting(0);
    view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.data?.[0] !== before, 'the mode changed');
  }
  assert.equal(view.ae2_screen.data[0], 4, 'MAX');
  await fixture.raw('close');
  await use(ae2);
  await until(() => record(ae2, 'ae2_interface_config'), t => counts(t)['minecraft:cobblestone'] === 64, 'AE2\'s interface: 64', 15000);
  assert.equal(counts(await record(ae2, 'ae2_interface_config'))['minecraft:ender_pearl'], 16);
  assert.match(await record(ae2, 'ae2_interface_fluid_config'), /amount:\s*4000L/, 'four buckets');
  await use(oversize);
  await until(() => record(oversize, 'ae2_interface_config'), t => counts(t)['minecraft:cobblestone'] === 1024, 'the Oversize Interface: 1,024', 15000);
  assert.equal(counts(await record(oversize, 'ae2_interface_config'))['minecraft:ender_pearl'], 256, 'a stack of 16, times 16');
  assert.match(await record(oversize, 'ae2_interface_fluid_config'), /amount:\s*64000L/, 'four buckets times 16');
  fixture.mark('the Config Modifier at MAX gives the Oversize Interface 16 times AE2\'s capacity, by key type');
  // It stocks them: 1,024 + 64 cobblestone and 256 + 16 pearls leave the grid.
  await until(() => stored(drive, 'minecraft:cobblestone'), n => n === 8000 - 1024 - 64, 'both interfaces stocked cobblestone', 30000);
  await until(() => stored(drive, 'minecraft:ender_pearl'), n => n === 2000 - 256 - 16, 'and pearls', 30000);
  fixture.mark('the Oversize Interface stocks 1,024 cobblestone and 256 ender pearls in one slot each');

  // The part: the same capacity on a cable.
  await fixture.replace('hotbar.1', 'extendedae:oversize_interface_part'); await fixture.select(1, 'extendedae:oversize_interface_part');
  await fixture.command(`tp @s ${x + 7.5} ${y} ${z - 1.5}`);
  assert.equal((await fixture.ae(`grid part ${host} north`)).attached, true, 'the part attached');
  await until(() => stored(drive, 'minecraft:cobblestone'), n => n === 8000 - 1024 - 64, 'settled', 15000);
  fixture.mark('the Oversize Interface part goes on a cable');

  // The Vacuum Interface: a grid of its own south, an empty cell.
  const vacuum = at(1, 1, 9), drive2 = at(2, 1, 9);
  await fixture.command(`setblock ${at(0, 1, 9)} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${drive2} ae2:drive`);
  await fixture.command(`setblock ${vacuum} extendedae:vacuum_interface`);
  await fixture.command(`item replace block ${drive2} container.0 with ae2:item_storage_cell_4k`);
  await fixture.command(`tp @s ${x + 10.5} ${y} ${z - 3.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 32 minecraft:item')).entities.filter((e: any) => e.item?.item === item).reduce((n: number, e: any) => n + e.item.count, 0);
  const drop = (dx: number, dz: number, id: string, n: number) => fixture.command(`summon minecraft:item ${x + 1.5 + dx} ${y + 0.2} ${z + 9.5 + dz} {Item:{id:"${id}",count:${n}},PickupDelay:32767s}`);
  await until(() => fixture.ae(`grid status ${drive2}`), v => Array.isArray(v.items), 'the vacuum grid online', 30000);
  await drop(1, 1, 'minecraft:cobblestone', 10);
  await drop(4, 0, 'minecraft:dirt', 5);
  await until(() => stored(drive2, 'minecraft:cobblestone'), n => n === 10, 'it takes the cobblestone in its area', 30000);
  await wait(40);
  assert.equal(await ground('minecraft:dirt'), 5, 'not the dirt three blocks past it');
  fixture.mark('the Vacuum Interface takes dropped items in its 5x5x5 area into the grid');
  // A filter: diamonds only.
  await fixture.ae(`eae config ${vacuum} block 0 minecraft:diamond`);
  await drop(0, 1, 'minecraft:gravel', 3);
  await drop(1, 0, 'minecraft:diamond', 2);
  await until(() => stored(drive2, 'minecraft:diamond'), n => n === 2, 'it takes the diamonds', 30000);
  await wait(40);
  assert.equal(await ground('minecraft:gravel'), 3, 'and leaves the gravel');
  assert.equal(await stored(drive2, 'minecraft:gravel'), 0);
  fixture.mark('with a filter the Vacuum Interface takes only what it names');
  // Moved four blocks east, three wide: it reaches the dirt.
  const area = await fixture.ae(`eae area ${vacuum} 3 3 3 4 0 0`);
  assert.deepEqual(area.offset, [4, 0, 0]);
  await fixture.ae(`eae config ${vacuum} block 0 none`);
  await until(() => stored(drive2, 'minecraft:dirt'), n => n === 5, 'the offset area takes the dirt', 30000);
  assert.equal(await ground('minecraft:gravel'), 3, 'the gravel is outside it now');
  fixture.mark('the Vacuum Interface\'s offset moves its work area');

  // Through a restart: the capacities and the area stay.
  await fixture.restart();
  await until(() => stored(drive, 'minecraft:ender_pearl'), n => n > 0, 'the grid after restart', 30000);
  assert.equal(counts(await record(oversize, 'ae2_interface_config'))['minecraft:cobblestone'], 1024);
  assert.deepEqual((await fixture.ae(`eae show ${vacuum} block`)).offset, [4, 0, 0]);
  fixture.mark('the Oversize and Vacuum Interfaces keep their settings through a restart');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
