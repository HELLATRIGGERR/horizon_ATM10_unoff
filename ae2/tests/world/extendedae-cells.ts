/** ExtendedAE slice 6: an ME Infinity Cobblestone Cell in a drive gives cobblestone without end and takes in
 * cobblestone, voided, and nothing else; the Infinity Water Cell does so for water; a cell of All the Mods 10's
 * own (`kubejs:dirt_cell`, its record read from the pack's KubeJS startup script) for dirt; each drains 8 AE/t.
 * An ME Void Cell takes what its partition lets in (the grid's preferred storage for it) and keeps none of it,
 * an inverter card turns its partition into a blacklist, an unpartitioned one takes nothing; it drains 1 AE/t.
 * Needs content/extendedae (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

// A stand-in for the pack's KubeJS items: All the Mods 10's `kubejs:dirt_cell`, a plain item to the engine.
const pack = mkdtempSync(join(tmpdir(), 'eae-pack-'));
writeFileSync(join(pack, 'horizon-mod.toml'), 'id = "kubejs"\nname = "KubeJS items"\nversion = "1"\napi = "0.2"\ncapabilities = ["registries"]\n');
mkdirSync(join(pack, 'data/kubejs/horizon/item'), { recursive: true });
writeFileSync(join(pack, 'data/kubejs/horizon/item/dirt_cell.json'), JSON.stringify({ template: 'minecraft:stick', max_stack_size: 1 }));
const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-cells', mode, { addons: ['extendedae'], devMods: [pack] });
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const INT_MAX = 2147483647;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -5], [-3, 7], [12, -5], [12, 7]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -5)} ${at(12, 5, 7)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -5)} ${at(12, -1, 7)} minecraft:stone`);
  await fixture.command('clear @s');
  const power = at(0), drive = at(1);
  await fixture.command(`setblock ${power} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${drive} ae2:drive`);
  await until(() => fixture.ae(`grid status ${drive}`), v => v.online && v.powered, 'drive online', 30000);
  const idle = async () => (await fixture.ae(`power ${drive} east`)).idle as number;
  const empty = await idle();
  const status = () => fixture.ae(`grid status ${drive}`);
  const deposit = async (item: string, n: number) => {
    await fixture.replace('hotbar.0', item, n); await fixture.select(0, item);
    return (await fixture.ae(`grid deposit ${drive}`)).inserted as number;
  };
  const withdraw = async (item: string, n: number) => {
    const view = await status();
    const index = view.items.findIndex((e: any) => e.item === item);
    assert(index >= 0, `${item} listed`);
    await fixture.replace('hotbar.0', 'minecraft:air');
    return (await fixture.ae(`grid withdraw ${drive} ${index} ${n}`)).extracted as number;
  };

  // The Infinity Cobblestone Cell: Integer.MAX_VALUE of cobblestone, whatever is taken out or put in.
  await fixture.command(`item replace block ${drive} container.0 with extendedae:infinity_cobblestone_cell`);
  await until(status, v => count(v, 'minecraft:cobblestone') === INT_MAX, 'endless cobblestone listed', 30000);
  assert.equal(await withdraw('minecraft:cobblestone', 64), 64);
  assert.equal(count(await status(), 'minecraft:cobblestone'), INT_MAX, 'none the less after a stack out');
  await fixture.command('clear @s');
  assert.equal(await deposit('minecraft:cobblestone', 32), 32, 'cobblestone goes in, voided');
  assert.equal(count(await status(), 'minecraft:cobblestone'), INT_MAX);
  assert.equal(await deposit('minecraft:dirt', 16), 0, 'nothing else goes in');
  await until(idle, n => Math.abs(n - empty - 8) < 1e-6, 'it drains 8 AE/t', 15000);
  fixture.mark('an Infinity Cobblestone Cell gives cobblestone without end, voids cobblestone, takes nothing else, at 8 AE/t');

  // The Infinity Water Cell: 2,147,483,647 buckets of water.
  await fixture.command(`item replace block ${drive} container.1 with extendedae:infinity_water_cell`);
  const water = async () => (await fixture.ae(`fluid grid status ${drive}`)).entries?.find((e: any) => e.fluid === 'minecraft:water')?.amount_mb ?? 0;
  await until(water, n => n === INT_MAX * 1000, 'endless water listed', 30000);
  await until(idle, n => Math.abs(n - empty - 16) < 1e-6, 'two cells drain 16 AE/t', 15000);
  fixture.mark('an Infinity Water Cell gives water without end');

  // All the Mods 10's own: kubejs:dirt_cell holds dirt.
  await fixture.command(`item replace block ${drive} container.2 with kubejs:dirt_cell`);
  await until(status, v => count(v, 'minecraft:dirt') === INT_MAX, 'endless dirt from the pack\'s cell', 30000);
  assert.equal(await withdraw('minecraft:dirt', 10), 10);
  assert.equal(count(await status(), 'minecraft:dirt'), INT_MAX);
  fixture.mark('the pack\'s KubeJS infinity cell holds the record its script names');
  for (const n of [0, 1, 2]) await fixture.command(`item replace block ${drive} container.${n} with minecraft:air`);
  await until(status, v => count(v, 'minecraft:cobblestone') === 0 && count(v, 'minecraft:dirt') === 0, 'the cells out', 15000);

  // The Void Cell: partitioned for cobblestone, the grid's preferred storage for it; keeps nothing.
  const config = (id: string) => `ae2:storage_cell_config_inv=[{"#t":"ae2:i",id:"${id}","#":0L}]`;
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  await fixture.command(`item replace block ${drive} container.1 with extendedae:void_cell[${config('minecraft:cobblestone')}]`);
  await until(idle, n => Math.abs(n - empty - 0.5 - 1) < 1e-6, 'the 1k cell 0.5 and the void cell 1 AE/t', 15000);
  await fixture.command('clear @s');
  assert.equal(await deposit('minecraft:cobblestone', 64), 64);
  assert.equal(count(await status(), 'minecraft:cobblestone'), 0, 'voided, not stored in the 1k cell');
  assert.equal(await deposit('minecraft:dirt', 10), 10, 'what it lets pass goes to the 1k cell');
  assert.equal(count(await status(), 'minecraft:dirt'), 10);
  fixture.mark('a Void Cell is the preferred storage for its partition and deletes what it takes');

  // An inverter card: everything but cobblestone.
  const upgrades = '[{slot:0,item:{id:"ae2:inverter_card",count:1}}]';
  await fixture.command(`item replace block ${drive} container.1 with extendedae:void_cell[${config('minecraft:cobblestone')},ae2:upgrades=${upgrades}]`);
  await until(() => fixture.command(`data get block ${drive}`), r => (r.chat ?? []).join('').includes('inverter_card'), 'the inverted cell in');
  await fixture.command('clear @s');
  assert.equal(await deposit('minecraft:sand', 20), 20);
  assert.equal(count(await status(), 'minecraft:sand'), 0, 'sand voided');
  assert.equal(await deposit('minecraft:cobblestone', 5), 5);
  assert.equal(count(await status(), 'minecraft:cobblestone'), 5, 'cobblestone kept in the 1k cell');
  fixture.mark('with an inverter card its partition is a blacklist');

  // Unpartitioned, it takes nothing.
  await fixture.command(`item replace block ${drive} container.0 with minecraft:air`);
  await fixture.command(`item replace block ${drive} container.1 with extendedae:void_cell`);
  await until(status, v => count(v, 'minecraft:dirt') === 0, 'only the void cell left', 15000);
  await fixture.command('clear @s');
  assert.equal(await deposit('minecraft:cobblestone', 8), 0);
  fixture.mark('an unpartitioned Void Cell takes nothing');

  // Its screen sets its mode; in matter ball mode 256 cobblestone make a matter ball, which comes back out.
  await fixture.command(`item replace block ${drive} container.1 with minecraft:air`);
  await fixture.replace('hotbar.0', `extendedae:void_cell[${config('minecraft:cobblestone')}]`); await fixture.select(0, 'extendedae:void_cell');
  await fixture.raw('look 0 -80'); await fixture.raw('use-item');
  const screen = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'void_cell.json', 'Void Cell screen', 15000);
  for (const n of [0, 1, 2]) assert(screen.ae2_screen.toolbar?.some((b: any) => b.entry === n), `mode button ${n}`);
  assert.equal(screen.ae2_screen.data?.[0] ?? 0, 0, 'trash at first');
  await fixture.pressSetting(1);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.data?.[0] === 1, 'matter ball mode shown');
  await until(async () => (await fixture.command('data get entity @s SelectedItem')).chat?.join('') ?? '', text => text.includes('MATTER_BALLS'), 'matter ball mode on the cell');
  await fixture.raw('close');
  await fixture.command(`item replace block ${drive} container.1 from entity @s hotbar.0`);
  await until(() => fixture.command(`data get block ${drive}`), r => (r.chat ?? []).join('').includes('void_cell'), 'the cell in the drive');
  await fixture.command('clear @s');
  assert.equal(await deposit('minecraft:cobblestone', 64), 64);
  for (let i = 0; i < 7; i++) assert.equal(await deposit('minecraft:cobblestone', 64), 64);
  await until(status, v => count(v, 'ae2:matter_ball') === 2, 'two matter balls of 512 cobblestone', 15000);
  assert.equal(await withdraw('ae2:matter_ball', 2), 2);
  assert.equal(count(await status(), 'ae2:matter_ball'), 0);
  fixture.mark('its screen sets its mode, and in matter ball mode what it voids becomes matter balls');

  // Through a restart: the cells mount again.
  await fixture.command(`item replace block ${drive} container.2 with extendedae:infinity_cobblestone_cell`);
  await fixture.restart();
  await until(status, v => v.online && count(v, 'minecraft:cobblestone') === INT_MAX, 'endless cobblestone after a restart', 60000);
  fixture.mark('the cells survive a restart');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); rmSync(pack, { recursive: true, force: true }); }
