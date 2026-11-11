/** Applied Flux slice 2: the Flux Accessor, the portable FE cells and FE cell disassembly.
 * - A Flux Accessor part on grid A's cable bus pushes the FE of grid A's FE cell into grid B's Energy Acceptor (the
 *   part's face keeps the grids apart): grid B stores what arrives as AE, grid A's FE goes down by as much.
 * - A Flux Accessor block on grid A answers its neighbours: grid D's Storage Bus on it lists grid A's FE.
 * - A 1k portable FE cell (a quarter of the 1k cell's bytes, 2^28 FE) is filled from grid A by an IO Port in FILL
 *   mode and emptied back by one in EMPTY mode; it charges in a Charger as AE2's portable cells do (80 AE a tick).
 * - Sneak-use takes an empty FE cell apart by Applied Flux's disassembly recipe; a filled one stays whole.
 * The FE machine is AE2's own Energy Acceptor: Mekanism's machines need its native hooks, which this fixture does
 * not load. Needs content/appflux (ae2/scripts/prepare-addon.ts appflux) and a client built with ae2-appflux. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('appflux-accessor', mode, { addons: ['appflux'] });
const fe = (view: any) => view.keys?.find((key: any) => key.type === 'appflux:flux' && key.id === 'appflux:fe')?.amount ?? 0;
const PORTABLE = 'appflux:fe_1k_portable_cell', PORTABLE_MAX = 2 ** 28, START = 500_000_000;
const text = (reply: any) => JSON.stringify(reply.chat ?? reply);
const card = (operation: string, fullness: string) => `ae2:memory_card[ae2:exported_settings_source={translate:"block.ae2.io_port"},ae2:exported_settings={operation_mode:"${operation}",fullness_mode:"${fullness}",redstone_controlled:"IGNORE"}]`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 8, y = Math.ceil(py) + 4, z = Math.floor(pz) + 8;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -5)} ${at(6, 3, 5)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -2, -5)} ${at(6, -1, 5)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 3.5}`);

  // Grid A: power, a drive with a 1k FE cell of 500,000,000 FE, a cable bus. Grid B: an acceptor and an energy cell.
  const drive = at(1), cable = at(2), acceptor = at(3), battery = at(3, 0, 1);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`); await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`setblock ${cable} ae2:cable_bus`);
  await fixture.command(`item replace block ${drive} container.0 with appflux:fe_1k_cell[appflux:fe_energy=${START}L]`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, s => s.online && s.powered && fe(s) === START, 'grid A lists its FE', 30000);
  // The part on the east face, against grid B's acceptor (the part keeps the grids apart): EnergyHandler.send
  // each side's tick.
  await fixture.replace('hotbar.0', 'appflux:part_flux_accessor'); await fixture.select(0, 'appflux:part_flux_accessor');
  assert.equal((await fixture.ae(`grid part ${cable} east`)).attached, true);
  const nodes = (await status()).nodes;
  // A new energy cell is empty.
  const before = 0;
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`); await fixture.command(`setblock ${battery} ae2:energy_cell`);
  const other = () => fixture.ae(`grid status ${battery}`);
  await until(other, s => s.nodes === 2, 'grid B, the acceptor and its cell', 30000);
  const charged = await until(other, s => s.energy > before + 1000, 'the accessor part powers grid B\'s acceptor', 60000);
  assert.equal((await status()).nodes, nodes, 'grid B is a grid of its own');
  await until(status, s => fe(s) < START, 'grid A\'s FE goes out', 30000);
  // Grid B fills (an energy cell holds 200,000 AE, 400,000 FE); grid A gave what B took.
  const full = await until(other, s => s.energy >= 199_000, 'grid B\'s energy cell fills', 60000);
  await Bun.sleep(1500);
  const given = START - fe(await status());
  assert(given >= 2 * (full.energy - before) && given < 2 * (full.energy - before) + 100_000, `grid A gave what B stored: ${given} FE for ${full.energy - before} AE`);
  fixture.mark(`a Flux Accessor part pushes the FE cell's FE into another grid's Energy Acceptor (${charged.energy} AE early, ${given} FE given)`);

  // The block, north of the cable bus (grid A); grid D's Storage Bus on it.
  const accessor = at(2, 0, -1), dcable = at(2, 0, -2);
  await fixture.command(`setblock ${accessor} appflux:flux_accessor`);
  await until(status, s => s.nodes === nodes + 1, 'the accessor joins grid A', 30000);
  await fixture.command(`setblock ${at(2, 0, -3)} ae2:creative_energy_cell`); await fixture.command(`setblock ${dcable} ae2:cable_bus`);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 1.5}`);
  await fixture.replace('hotbar.0', 'ae2:storage_bus'); await fixture.select(0, 'ae2:storage_bus');
  assert.equal((await fixture.ae(`grid part ${dcable} south`)).attached, true);
  const dstatus = () => fixture.ae(`grid status ${dcable}`);
  const listed = await until(dstatus, s => fe(s) > 0, 'grid D lists grid A\'s FE through the accessor', 30000);
  assert.equal(fe(listed), fe(await status()), 'NetworkFEPower.getEnergyStored: grid A\'s FE');
  assert.notEqual((await dstatus()).nodes, (await status()).nodes, 'grid D is a grid of its own');
  fixture.mark('a Flux Accessor block gives its grid\'s FE to a neighbour\'s Storage Bus');

  // The portable cell: an IO Port of grid A fills it (FILL, until full), another empties it (EMPTY, until empty).
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 3.5}`);
  const fill = at(1, 0, 1), empty = at(0, 0, 1);
  await fixture.command(`setblock ${fill} ae2:io_port`);
  await fixture.replace('hotbar.0', card('FILL', 'FULL')); await fixture.select(0, 'ae2:memory_card');
  await fixture.raw(`interact ${fill} south`);
  await until(() => fixture.command(`data get block ${fill}`), r => /fill/i.test(text(r)), 'the IO Port set to FILL');
  const stored = fe(await status());
  await fixture.command(`item replace block ${fill} container.0 with ${PORTABLE}`);
  const filled = await until(() => fixture.command(`data get block ${fill}`), r => /Slot:\s*6b/.test(text(r)) && text(r).includes(`${PORTABLE_MAX}L`), 'the IO Port fills the portable cell and moves it out', 60000);
  assert.match(text(filled), new RegExp(PORTABLE), 'the portable FE cell in the output');
  await until(status, s => fe(s) === stored - PORTABLE_MAX, 'grid A gave 2^28 FE', 30000);
  fixture.mark('an IO Port fills a 1k portable FE cell from the grid: 268,435,456 FE, a quarter of a 1k cell');
  await fixture.command(`setblock ${empty} ae2:io_port`);
  await fixture.command(`item replace block ${empty} container.0 with ${PORTABLE}[appflux:fe_energy=${PORTABLE_MAX}L]`);
  await until(() => fixture.command(`data get block ${empty}`), r => /Slot:\s*6b/.test(text(r)) && !text(r).includes('appflux:fe_energy'), 'the IO Port empties the portable cell', 60000);
  await until(status, s => fe(s) === stored, 'grid A has its FE back', 30000);
  fixture.mark('an IO Port empties the portable FE cell into the grid; the empty cell drops its component');

  // A Charger on grid A: 80 AE a tick into the portable cell's battery, up to 20,000 AE.
  const charger = at(1, 0, -1);
  await fixture.command(`setblock ${charger} ae2:charger[facing=north]`);
  await fixture.command(`item replace block ${charger} container.0 with ${PORTABLE}`);
  const energy = async () => { const t = text(await fixture.command(`data get block ${charger}`)); const m = t.match(/ae2:stored_energy\\?"?:\s*([0-9.E+-]+)d/); return m ? Number(m[1]) : 0; };
  await until(energy, e => e > 0, 'the Charger charges the portable FE cell', 30000);
  await fixture.hold(); const e0 = await energy(); await fixture.stepServer(20); const gained = await energy() - e0; await fixture.hold(false);
  assert(gained > 0 && gained <= 80 * 20 && gained % 80 === 0, `in steps of 80 AE a tick: ${gained}`);
  fixture.mark(`a Charger charges a portable FE cell as AE2's portable cells (${gained} AE in 20 ticks)`);

  // Disassembly: an empty 1k FE cell gives its housing and core back; one holding FE is kept whole.
  await fixture.command('clear @s');
  const sneakUse = () => fixture.sneakUseItem();
  await fixture.raw('look 0 -80');
  await fixture.replace('hotbar.0', 'appflux:fe_1k_cell[appflux:fe_energy=5L]'); await fixture.select(0, 'appflux:fe_1k_cell');
  await sneakUse();
  await Bun.sleep(500);
  assert.equal((await fixture.raw('inventory')).counts?.['appflux:fe_1k_cell'], 1, 'a cell holding FE stays whole');
  await fixture.replace('hotbar.0', 'appflux:fe_1k_cell'); await fixture.select(0, 'appflux:fe_1k_cell');
  await sneakUse();
  const parts = await until(() => fixture.raw('inventory'), s => !s.counts?.['appflux:fe_1k_cell'] && s.counts?.['appflux:core_1k'] === 1, 'the empty cell taken apart');
  assert.equal(parts.counts?.['appflux:fe_cell_housing'], 1);
  await fixture.replace('hotbar.0', PORTABLE); await fixture.select(0, PORTABLE);
  await sneakUse();
  const portableParts = await until(() => fixture.raw('inventory'), s => !s.counts?.[PORTABLE] && s.counts?.['ae2:energy_cell'] === 1, 'the empty portable cell taken apart');
  for (const part of ['appflux:fe_cell_housing', 'ae2:chest', 'appflux:core_1k']) assert(portableParts.counts?.[part] >= 1, part);
  fixture.mark('sneak-use takes an empty FE cell and an empty portable FE cell apart by Applied Flux\'s recipes');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; } finally { await fixture.close(); }
