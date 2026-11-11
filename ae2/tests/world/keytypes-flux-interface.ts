/** An addon's key type in AE2's generic slots and in items, with Applied Flux's FE (`appflux:flux`) as the type: a
 * right click with a charged AE2 energy cell on an ME Interface's config slot marks its FE (`EMPTY_ITEM` through
 * Applied Flux's `FEContainerItemStrategy`), the interface stocks that FE from the grid and keeps it through a restart
 * (the type's id in `#t`); a shift click on the ME Terminal's FE entry fills a carried energy cell from the grid
 * (`FILL_ENTIRE_ITEM`) and a right click cannot drain it (AE2's `PoweredItemCapabilities` gives no FE back); a Pattern
 * Provider with an Induction Card takes FE pushed by another grid's Flux Accessor into its return slots
 * (`FEGenericStackInvStorage`) and returns it to its grid, where the crafting CPU waiting for a pattern's FE output
 * finishes its job. Needs content/appflux (ae2/scripts/prepare-addon.ts appflux) and a client built with ae2-appflux. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('keytypes-flux-interface', mode, { addons: ['appflux'] });
const fe = (view: any) => view.keys?.find((key: any) => key.type === 'appflux:flux' && key.id === 'appflux:fe')?.amount ?? 0;
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
const text = (reply: any) => reply.chat?.join('\n') ?? '';
const START = 1_000_000, MARKED = 200_000, CELL = 400_000, OUTPUT = 100_000, PUSHED = 500_000;
const pattern = `ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"minecraft:diamond","#":1L}],sparseOutputs:[{"#t":"appflux:flux",type:"FE","#":${OUTPUT}L}]}]`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 8, y = Math.ceil(py) + 4, z = Math.floor(pz) + 8;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -4)} ${at(8, 4, 5)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -2, -4)} ${at(8, -1, 5)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');

  // Grid A: power, a drive with a 1k FE cell of 1,000,000 FE and an item cell holding a diamond, a cable bus with the
  // ME Terminal north, a crafting storage above it, an ME Interface north of the power and a Pattern Provider east.
  const drive = at(1), cable = at(2), cpu = at(2, 1), iface = at(0, 0, -1), provider = at(3), chest = at(3, 0, 1);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`); await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`setblock ${cable} ae2:cable_bus`); await fixture.command(`setblock ${cpu} ae2:64k_crafting_storage`);
  await fixture.command(`setblock ${iface} ae2:interface`); await fixture.command(`setblock ${provider} ae2:pattern_provider`);
  await fixture.command(`setblock ${chest} minecraft:chest`);
  await fixture.command(`item replace block ${drive} container.0 with appflux:fe_1k_cell[appflux:fe_energy=${START}L]`);
  await fixture.command(`item replace block ${drive} container.1 with ae2:item_storage_cell_1k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:diamond","#":1L}]]`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, s => s.online && s.powered && fe(s) === START, 'grid A lists its FE', 30000);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${cable} north`)).attached, true, 'the ME Terminal on the north face');

  // The interface's config slot from a charged energy cell: 100,000 AE is 200,000 FE.
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.1', 'ae2:energy_cell[ae2:stored_energy=100000.0d]'); await fixture.replace('hotbar.8', 'minecraft:stick');
  await fixture.select(8, 'minecraft:stick'); await fixture.command(`tp @s ${x + 0.5} ${y} ${z - 2.5}`); await fixture.raw(`interact ${iface} north`);
  await until(() => fixture.raw('screen'), view => view.screen === 'container' && view.ae2_screen?.style === 'interface.json', 'interface menu');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), view => view.carried?.item === 'ae2:energy_cell', 'the charged cell on the cursor');
  await fixture.raw('click-slot 309 right');
  await until(() => fixture.raw('screen'), view => slot(view, 309)?.item === 'appflux:fe' && String(slot(view, 309)?.components ?? '').includes('200,000'), 'an FE config of 200,000 FE');
  assert.equal((await fixture.raw('inventory')).carried?.item, 'ae2:energy_cell', 'the emptying action leaves the cell on the cursor');
  await until(() => fixture.raw('screen'), view => slot(view, 300)?.item === 'appflux:fe', 'the interface stocks the FE');
  await fixture.raw('click-slot 1'); await fixture.raw('close');
  const stock = async () => text(await fixture.command(`data get block ${iface} ae2_lab_config.ae2_interface_fluids`));
  const held = await stock();
  assert.match(held, /appflux:flux/); assert.match(held, /appflux:fe/); assert.match(held, new RegExp(`${MARKED}L`));
  assert.equal(fe(await status()), START - MARKED, 'the grid gave the interface its FE');
  fixture.mark('an energy cell marks FE in an ME Interface\'s config slot, and the interface stocks it with its type in #t');
  await fixture.restart();
  await until(status, s => s.online && s.powered && fe(s) === START - MARKED, 'grid A after the restart', 60000);
  assert.match(await stock(), new RegExp(`appflux:flux[\\s\\S]*${MARKED}L|${MARKED}L[\\s\\S]*appflux:flux`), 'the FE stock survives the restart');
  fixture.mark('the interface\'s FE stock survives save and load');

  // The terminal fills an empty energy cell from the grid (200,000 AE, 400,000 FE) and cannot drain it.
  await fixture.command('gamemode survival @s'); await fixture.replace('hotbar.1', 'ae2:energy_cell'); await fixture.select(8, 'minecraft:stick');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`); await fixture.raw(`interact ${cable} north`);
  const entry = (view: any) => view.slots?.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === 'appflux:fe');
  const terminal = await until(() => fixture.raw('screen'), view => entry(view), 'the terminal lists the FE');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), view => view.carried?.item === 'ae2:energy_cell', 'the empty cell on the cursor');
  await fixture.raw(`click-slot ${entry(terminal).slot} shift`);
  await until(status, s => fe(s) === START - MARKED - CELL, 'the terminal fills the cell from the grid', 30000);
  const filled = await fixture.raw('inventory');
  assert.equal(filled.carried?.item, 'ae2:energy_cell'); assert.match(String(filled.carried?.components ?? ''), /200000/, 'the cell holds 200,000 AE');
  await fixture.raw(`click-slot ${entry(terminal).slot} right`); await fixture.raw('inventory');
  assert.equal(fe(await status()), START - MARKED - CELL, 'a right click drains no FE from an AE2 powered item');
  assert.equal((await fixture.raw('inventory')).carried?.item, 'ae2:energy_cell', 'the cell stays on the cursor');
  await fixture.raw('click-slot 1'); await fixture.raw('close');
  fixture.mark('the ME Terminal fills an energy cell with the grid\'s FE through Applied Flux\'s container strategy');

  // The provider takes a pattern of a diamond into 100,000 FE.
  await fixture.replace('hotbar.2', pattern); await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 1.5}`); await fixture.raw(`interact ${provider} north`);
  const menu = await until(() => fixture.raw('screen'), view => /Pattern Provider/i.test(JSON.stringify(view.title ?? '')) && view.slots?.some((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern'), 'provider menu');
  await fixture.raw(`click-slot ${menu.slots.find((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), view => !view.counts?.['ae2:processing_pattern'], 'pattern installed'); await fixture.raw('close');

  // The terminal requests 1,000 FE: the CPU sends the diamond to the chest and waits for the pattern's FE.
  const before = fe(await status());
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`); await fixture.raw(`interact ${cable} north`);
  const listed = await until(() => fixture.raw('screen'), view => entry(view), 'the FE entry');
  await fixture.raw(`click-slot ${entry(listed).slot} middle`);
  await until(() => fixture.raw('screen'), view => JSON.stringify(view.title ?? '').includes('AE2 Craft Amount'), 'craft amount'); await fixture.raw('click-slot 302');
  const confirm = await until(() => fixture.raw('screen'), view => JSON.stringify(view.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(view)), 'craft confirmation', 90000);
  assert.doesNotMatch(JSON.stringify(confirm), /Missing materials|craft planning:/);
  await fixture.raw('click-slot 302'); await fixture.raw('close');
  await until(() => fixture.command(`data get block ${chest} Items`), reply => text(reply).includes('minecraft:diamond'), 'the provider sends the diamond', 60000);
  const job = async () => text(await fixture.command(`data get block ${cpu} ae2_lab_config`));
  assert.match(await job(), /ae2_recursive_job/, 'the CPU waits for the FE');
  assert.equal(fe(await status()), before, 'no FE yet');

  // Grid B: its Flux Accessor part faces the provider. Without an Induction Card the provider has no FE storage
  // (AFUtil.shouldTryCast); with one (Applied Flux's own upgrade slot of the provider) the accessor pushes 500,000 FE
  // into its return slots, which give it to grid A: the CPU takes the pattern's 100,000 FE first and the job ends.
  const bcable = at(4), bdrive = at(5);
  await fixture.command(`setblock ${bcable} ae2:cable_bus`); await fixture.command(`setblock ${bdrive} ae2:drive`); await fixture.command(`setblock ${at(6)} ae2:creative_energy_cell`);
  await fixture.command(`item replace block ${bdrive} container.0 with appflux:fe_1k_cell[appflux:fe_energy=${PUSHED}L]`);
  await fixture.command('gamemode creative @s'); await fixture.replace('hotbar.0', 'appflux:part_flux_accessor'); await fixture.select(0, 'appflux:part_flux_accessor');
  assert.equal((await fixture.ae(`grid part ${bcable} west`)).attached, true, 'the accessor part faces the provider');
  await until(() => fixture.ae(`grid status ${bdrive}`), s => s.online && fe(s) === PUSHED, 'grid B lists its FE', 30000);
  await Bun.sleep(3000);
  assert.equal(fe(await fixture.ae(`grid status ${bdrive}`)), PUSHED, 'no FE into a provider without an Induction Card');
  assert.equal(fe(await status()), before);
  await fixture.command(`data merge block ${provider} {ae2_lab_config:{appflux_upgrades:[{Slot:0b,id:"appflux:induction_card",count:1}]}}`);
  await until(status, s => fe(s) === before + PUSHED, 'grid A receives grid B\'s FE through the provider', 60000);
  const other = await fixture.ae(`grid status ${bdrive}`);
  assert.equal(fe(other), 0, 'grid B gave all its FE'); assert.notEqual(other.nodes, (await status()).nodes, 'the grids stay apart');
  await until(job, reply => !/ae2_recursive_job/.test(reply), 'the FE output ends the CPU\'s job', 60000);
  assert.doesNotMatch(text(await fixture.command(`data get block ${provider} ae2_lab_config`)), /ae2_provider_return_fluids:\[\{/, 'the return slots are empty');
  fixture.mark('a Pattern Provider with an Induction Card takes pushed FE and returns it to the waiting CPU');

  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; } finally { await fixture.close(); }
