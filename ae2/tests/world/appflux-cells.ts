/** Applied Flux slice 1: an FE cell in an ME Drive takes FE from an FE block through an Import Bus, is listed as
 * `appflux:flux` keys, keeps them in `appflux:fe_energy` through a restart, gives them back to another FE block
 * through an Export Bus, and a Storage Bus lists an FE block's own store. The FE blocks are AE2's: the debug energy
 * generator (an endless FE source) and a second grid's Energy Acceptor (its energy cell counts what arrives).
 * The import strategy is Applied Flux's `misc.enable`, off in All the Mods 10 (it ships no appflux-common.toml), so
 * the fixture writes one that turns it on. Needs content/appflux (ae2/scripts/prepare-addon.ts appflux) and a client
 * built with ae2-appflux. */
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('appflux-cells', mode, { addons: ['appflux'] });
mkdirSync(join(fixture.game, 'config'), { recursive: true });
writeFileSync(join(fixture.game, 'config/appflux-common.toml'), '[misc]\nenable = true\n');
const fe = (view: any) => view.keys?.find((key: any) => key.type === 'appflux:flux' && key.id === 'appflux:fe')?.amount ?? 0;
const CELL_1K = 2 ** 30;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 8, y = Math.ceil(py) + 4, z = Math.floor(pz) + 8;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -3)} ${at(6, 3, 5)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -2, -3)} ${at(6, -1, 5)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);

  // Grid A: power, a drive with an empty 1k FE cell, a cable bus. Grid B: an acceptor and an energy cell.
  const drive = at(1), cable = at(2), source = at(3), acceptor = at(2, 0, 1), battery = at(2, 0, 2);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`); await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`setblock ${cable} ae2:cable_bus`);
  await fixture.command(`item replace block ${drive} container.0 with appflux:fe_1k_cell`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, s => s.online && s.powered, 'powered drive', 30000);
  assert.equal(fe(await status()), 0, 'an empty FE cell lists nothing');

  // Import Bus on the debug generator: FE into the cell, 16,384 FE an operation.
  await fixture.command(`setblock ${source} ae2:debug_energy_gen`);
  await fixture.replace('hotbar.0', 'ae2:import_bus'); await fixture.select(0, 'ae2:import_bus');
  assert.equal((await fixture.ae(`grid part ${cable} east`)).attached, true);
  const filled = await until(status, s => fe(s) >= 4 * 16384, 'the import bus moves FE into the cell', 60000);
  await fixture.command(`setblock ${source} minecraft:air`);
  await Bun.sleep(1500);
  const imported = fe(await status());
  assert(imported >= fe(filled) && imported % 16384 === 0, `whole operations of 16,384 FE: ${imported}`);
  assert(imported < CELL_1K);
  await Bun.sleep(1500);
  assert.equal(fe(await status()), imported, 'the import stops with its source');
  const saved = (await fixture.command(`data get block ${drive}`)).chat?.join('\n') ?? '';
  assert.match(saved, /appflux:fe_1k_cell/); assert.match(saved, new RegExp(`"?appflux:fe_energy"?: ?${imported}L`));
  fixture.mark('an Import Bus moves FE from an FE block into a 1k FE cell, kept in appflux:fe_energy');

  // The ME Terminal lists it.
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${cable} north`)).attached, true);
  await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  await fixture.raw(`interact ${cable} north`);
  const listed = (view: any) => view.slots?.some((slot: any) => slot.slot >= 300 && slot.slot < 354 && slot.item === 'appflux:fe');
  await until(() => fixture.raw('screen'), listed, 'the ME Terminal lists the stored FE', 30000);
  await fixture.raw('close');
  fixture.mark('an ME Terminal lists the FE the cell holds');

  await fixture.restart();
  await until(status, s => s.online && s.powered && fe(s) === imported, 'the FE cell keeps its FE through a restart', 60000);
  fixture.mark('the cell keeps its FE through save and load');

  // Export Bus, filtered to FE, into grid B's Energy Acceptor (the bus on that face keeps the grids apart): the
  // cell's FE goes to the other grid.
  await fixture.replace('hotbar.0', 'ae2:export_bus'); await fixture.select(0, 'ae2:export_bus');
  assert.equal((await fixture.ae(`grid part ${cable} south`)).attached, true);
  await fixture.ae(`bus filter_key ${cable} south 0 appflux:flux|appflux:fe`);
  const nodes = (await status()).nodes;
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`); await fixture.command(`setblock ${battery} ae2:energy_cell`);
  const other = () => fixture.ae(`grid status ${battery}`);
  await until(other, s => s.online && s.nodes === 2, 'grid B, the acceptor and its cell', 30000);
  assert.equal((await status()).nodes, nodes, 'grid B is a grid of its own');
  const drained = await until(status, s => fe(s) < imported, 'the export bus moves FE out of the cell', 60000);
  assert(fe(drained) % 16384 === 0, `whole operations: ${fe(drained)}`);
  await until(other, s => s.energy > 0, 'grid B stores the FE it was given', 30000);
  await until(status, s => fe(s) === 0, 'the export bus empties the cell', 60000);
  const empty = (await fixture.command(`data get block ${drive}`)).chat?.join('\n') ?? '';
  assert.doesNotMatch(empty, /appflux:fe_energy/, 'an empty FE cell drops its component');
  fixture.mark('an Export Bus filtered to FE gives the FE of the cell to another FE block, until the cell is empty');

  // Storage Bus on a debug generator: the grid lists the generator's FE beside the cell's.
  await fixture.command(`setblock ${at(2, 1)} ae2:debug_energy_gen`);
  await fixture.replace('hotbar.0', 'ae2:storage_bus'); await fixture.select(0, 'ae2:storage_bus');
  assert.equal((await fixture.ae(`grid part ${cable} up`)).attached, true);
  await until(status, s => fe(s) >= 2147483647, 'the storage bus lists the generator\'s FE', 30000);
  fixture.mark('a Storage Bus mounts an FE block as FE storage');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; } finally { await fixture.close(); }
