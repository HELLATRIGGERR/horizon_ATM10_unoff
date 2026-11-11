/** Advanced AE slice 6: the Quantum Crafter. On a grid of a dense energy cell and a drive of 10 oak logs, a crafting
 * pattern of planks shift-clicked into its screen (quantum_crafter.json) does nothing until its radio enables it; then
 * it crafts from the grid on its own (QuantumCrafterEntity.tickingRequest) and sends the planks into the grid (ME
 * export), keeping the 4 logs its job keeps and, with a limit of 32, stopping at 32 planks in the grid. With ME export
 * off the toolbar shows the six output sides, and the front sends the planks to a chest there. Four speed cards and a
 * redstone card go in; with HIGH_SIGNAL it waits for a signal. Its pattern, job settings, radio, cards and settings
 * survive a restart; broken, it drops its pattern and cards. Needs content/advanced_ae (ae2/scripts/prepare-addon.ts
 * advancedae) and a client built with ae2-advancedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('advancedae-quantum-crafter', mode, { addons: ['advanced_ae'] });
const CRAFTER = 'advanced_ae:quantum_crafter', LOG = 'minecraft:oak_log', PLANKS = 'minecraft:oak_planks';
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slot = (view: any, n: number) => view.slots?.find((s: any) => s.slot === n);
const entries = (view: any) => view.ae2_screen.toolbar.map((b: any) => b.entry).filter((e: any) => e !== undefined);
const RADIOS = [41, 42, 43, 44, 45, 46, 47, 48, 49];
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -5], [-3, 6], [8, -5], [8, 6]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -5)} ${at(8, 4, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -5)} ${at(8, -1, 6)} minecraft:stone`);
  await fixture.command('clear @s');
  const contents = async (pos: string) => (await fixture.command(`data get block ${pos}`)).chat?.join('\n') ?? '';
  const container = async (pos: string, item: string) => ((await fixture.raw(`container ${pos}`)).slots ?? []).filter((s: any) => s?.item === item).reduce((n: number, s: any) => n + s.count, 0);

  // The grid: a dense energy cell, a drive of 10 logs, the crafter (facing north) and a chest at its front.
  const power = at(0), drive = at(1), crafter = at(2), chest = at(2, 0, -1);
  for (const [pos, id] of [[power, 'ae2:dense_energy_cell'], [drive, 'ae2:drive'], [crafter, CRAFTER], [chest, 'minecraft:chest']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`data merge block ${power} {ae2_lab_energy:1000000.0d}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"${LOG}","#":10L}]]`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, v => v.online && v.powered && count(v, LOG) === 10, 'the grid with 10 logs', 30000);
  await until(() => fixture.ae(`grid status ${crafter}`), v => v.online && v.powered, 'the crafter on the grid', 30000);
  assert.equal((await fixture.ae(`power ${crafter} up`)).fe?.capacity, 16000, 'its own buffer of 8,000 AE');

  // A pattern of four planks from a log, into the crafter's screen by a shift-click.
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  const encoded = await fixture.ae(`crafting encode ${LOG} - - - - - - - -`);
  assert.deepEqual([encoded.encoded, encoded.output, encoded.count], [true, PLANKS, 4]);
  await until(() => fixture.raw('inventory'), v => v.hotbar[0]?.item === 'ae2:crafting_pattern', 'the pattern encoded');
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${crafter} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Quantum Crafter screen', 15000);
  assert.equal(view.ae2_screen.style, 'quantum_crafter.json');
  assert.deepEqual(entries(view), [33, 34, ...RADIOS], 'the terminal and ME export toggles, then the nine radios');
  assert(fixture.settingIs(view, 34, 'YES'), 'ME export on');
  assert(fixture.settingIs(view, 41, 'NO'), 'the first pattern disabled');
  await fixture.raw('click-slot 0 shift');
  view = await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'ae2:crafting_pattern', 'the pattern in its first slot', 15000);
  const job = await until(() => fixture.ae(`advae crafter ${crafter}`), v => v.patterns[0] !== null, 'the job of the pattern', 15000);
  assert.deepEqual([job.patterns[0].output, job.patterns[0].count, job.patterns[0].inputs[0].multiplier], [PLANKS, 4, 1]);
  await new Promise(resolve => setTimeout(resolve, 1500));
  assert.equal(count(await status(), LOG), 10, 'a disabled pattern is never crafted');
  fixture.mark('a crafting pattern goes into the Quantum Crafter\'s screen and waits while disabled');

  // Enabled, keeping 4 logs: six crafts of the grid's logs, the planks sent into the grid.
  await fixture.ae(`advae crafter ${crafter} keep 0 0 4`);
  await fixture.pressSetting(41);
  await until(status, v => count(v, PLANKS) === 24, '24 planks in the grid', 30000);
  assert.equal(count(await status(), LOG), 4, 'the 4 logs kept');
  assert(((await status()).energy ?? 1000000) < 1000000, 'the crafts took power');
  fixture.mark('enabled, it crafts from the grid on its own into the grid, keeping the stock its job keeps');

  // A limit of 32 planks: two crafts more, then it stops with logs left.
  await fixture.ae(`advae crafter ${crafter} keep 0 0 0`);
  await fixture.ae(`advae crafter ${crafter} limit 0 32`);
  await until(status, v => count(v, PLANKS) === 32, '32 planks', 30000);
  await new Promise(resolve => setTimeout(resolve, 1500));
  assert.deepEqual([count(await status(), PLANKS), count(await status(), LOG)], [32, 2], 'the limit holds');
  fixture.mark('the output limit stops it at 32 planks in the grid');

  // ME export off: the six sides on the toolbar; the front sends the planks into the chest.
  await fixture.pressSetting(34);
  view = await until(() => fixture.raw('screen'), v => entries(v).includes(35), 'the sides shown', 15000);
  assert.deepEqual(entries(view), [33, 34, 35, 36, 37, 38, 39, 40, ...RADIOS]);
  await fixture.ae(`advae crafter ${crafter} limit 0 0`);
  await fixture.pressSetting(35);
  await until(() => container(chest, PLANKS), n => n === 8, 'the last two crafts in the chest', 30000);
  assert.equal(count(await status(), LOG), 0);
  fixture.mark('without ME export the planks go to the chest on its front side');

  // Four speed cards and a redstone card; HIGH_SIGNAL waits for a signal.
  await fixture.replace('hotbar.1', 'ae2:speed_card', 4); await fixture.replace('hotbar.2', 'ae2:redstone_card');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), v => v.carried?.item === 'ae2:speed_card', 'cards held');
  for (const entry of [327, 328, 329, 330]) await fixture.raw(`click-slot ${entry}`);
  await fixture.raw('click-slot 2'); await until(() => fixture.raw('inventory'), v => v.carried?.item === 'ae2:redstone_card', 'the redstone card held');
  await fixture.raw('click-slot 331');
  view = await until(() => fixture.raw('screen'), v => [327, 328, 329, 330].every(n => slot(v, n)?.item === 'ae2:speed_card') && slot(v, 331)?.item === 'ae2:redstone_card', 'five cards in', 15000);
  view = await until(() => fixture.raw('screen'), v => entries(v)[0] === 32, 'the redstone mode with its card', 15000);
  assert.deepEqual(entries(view).slice(0, 3), [32, 33, 34]);
  await fixture.pressSetting(32); await fixture.pressSetting(32);
  await until(() => fixture.raw('screen'), v => fixture.settingIs(v, 32, 'HIGH_SIGNAL'), 'HIGH_SIGNAL', 15000);
  await fixture.pressSetting(34);
  await fixture.command(`item replace block ${drive} container.1 with ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"${LOG}","#":64L}]]`);
  await until(status, v => count(v, LOG) === 64, '64 logs more', 30000);
  await new Promise(resolve => setTimeout(resolve, 1500));
  assert.equal(count(await status(), LOG), 64, 'no signal, no crafts');
  await fixture.command(`setblock ${at(2, 1)} minecraft:redstone_block`);
  await until(status, v => count(v, LOG) === 0 && count(v, PLANKS) === 32 + 256, 'with a signal, all 64 crafted', 30000);
  fixture.mark('speed and redstone cards go in, and HIGH_SIGNAL crafts only with a signal');

  // Through a restart: the pattern, its job settings and radio, the cards and settings stay.
  await fixture.ae(`advae crafter ${crafter} keep 0 0 3`);
  await fixture.restart();
  await until(() => fixture.ae(`grid status ${crafter}`), v => v.online && v.powered, 'the crafter back on the grid', 60000);
  const kept = await contents(crafter);
  assert.match(kept, /ae2:crafting_pattern/);
  assert.match(kept, /aae_enabled_1: 1b/);
  assert.match(kept, /listMinInput: \[L; 3L\]/);
  assert.match(kept, /redstone_controlled: "HIGH_SIGNAL"/);
  assert.match(kept, /me_export: "YES"/);
  assert.equal((kept.match(/ae2:speed_card/g) ?? []).length, 4, 'the four speed cards');
  const after = await fixture.ae(`advae crafter ${crafter}`);
  assert.equal(after.patterns[0].inputs[0].keep, 3);
  fixture.mark('the pattern, its job settings, radio, cards and settings survive a restart');

  // Broken, it drops its pattern and cards.
  await fixture.command('clear @s');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 3.5}`);
  const ground = async (item: string) => (await fixture.raw('entities 24 minecraft:item')).entities.filter((e: any) => e.item?.item === item).reduce((n: number, e: any) => n + e.item.count, 0);
  await fixture.command(`setblock ${crafter} minecraft:air destroy`);
  await until(() => ground('ae2:speed_card'), n => n === 4, 'the four speed cards on the ground', 30000);
  assert.equal(await ground('ae2:crafting_pattern'), 1, 'the pattern');
  assert.equal(await ground('ae2:redstone_card'), 1, 'the redstone card');
  assert.equal(await ground(CRAFTER), 1, 'the block itself');
  fixture.mark('broken, the Quantum Crafter drops its pattern and cards');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
