/** MEGA Cells slice 2: MEGA's crafting CPU blocks. A 1M MEGA Crafting Storage from MEGA's own recipe; a
 * MEGA unit upgraded in place with MEGA's components only, and back; a CPU of MEGA's blocks alone forms with
 * 1,048,576 bytes and 4 co-processors, takes a job a 1K CPU refuses, runs it and shows it on its MEGA Crafting
 * Monitor; MEGA's and AE2's blocks form one CPU together; all through a restart, and a broken block drops
 * itself. Needs content/megacells (ae2/scripts/prepare-addon.ts megacells) and a client built with ae2-megacells. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('megacells-crafting-cpu', mode, { addons: ['megacells'] });
const count = (status: any, item: string) => status.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const shots = process.env.AE2_TEST_SHOTS;
const shot = async (name: string) => { if (shots) await fixture.raw(`screenshot ${join(shots, name + '.png')}`); };
const entities = async (id: string) => ((await fixture.raw('entities 48 item')).entities ?? []).filter((e: any) => e.item?.item === id);
const MIB = 1024 * 1024;
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 32, y = Math.ceil(py) + 5, z = Math.floor(pz) + 32;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-4, -3], [-4, 4], [8, -3], [8, 4]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command('clear @s');
  await fixture.command(`fill ${at(-4, 0, -3)} ${at(8, 5, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-4, -1, -3)} ${at(8, -1, 4)} minecraft:stone`);
  const block = async (pos: string) => fixture.raw(`block ${pos}`);
  const blockIs = (pos: string, id: string, label: string) => until(() => block(pos), b => b.block === id, label, 15000);

  // MEGA's shapeless recipe: a MEGA Crafting Unit and a 1M component make a 1M MEGA Crafting Storage.
  await fixture.command('gamemode survival @s');
  await until(() => fixture.raw('status'), s => s.creative === false, 'survival inventory');
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 1.5}`);
  await fixture.replace('hotbar.0', 'megacells:mega_crafting_unit'); await fixture.replace('hotbar.1', 'megacells:cell_component_1m');
  await fixture.raw('open-inventory');
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 100), 'inventory crafting grid');
  for (const [from, to] of [[0, 100], [1, 101]]) {
    await fixture.raw(`click-slot ${from}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${to}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === 'megacells:1m_crafting_storage'), 'the recipe offers a 1M MEGA Crafting Storage');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  const crafted = await until(() => fixture.raw('inventory'), s => s.counts?.['megacells:1m_crafting_storage'] === 1, 'the crafted storage is in the inventory');
  assert(!crafted.counts?.['megacells:mega_crafting_unit'] && !crafted.counts?.['megacells:cell_component_1m'], 'both ingredients were used');
  fixture.mark('MEGA\'s own recipe crafts a 1M MEGA Crafting Storage from a MEGA Crafting Unit and a 1M component');

  // In place (CraftingUnitTransformRecipe with MEGA's mixin): a MEGA unit takes MEGA's components, AE2's
  // does not; an alternate use gives the component back and leaves MEGA's unit.
  await fixture.command('clear @s');
  const megaUnit = at(6, 0, 3), ae2Unit = at(6, 0, -1);
  await fixture.command(`setblock ${megaUnit} megacells:mega_crafting_unit`); await fixture.command(`setblock ${ae2Unit} ae2:crafting_unit`);
  await fixture.replace('hotbar.0', 'megacells:cell_component_4m', 2); await fixture.select(0, 'megacells:cell_component_4m');
  await fixture.raw(`interact ${ae2Unit} south`);
  await fixture.raw(`interact ${megaUnit} north`);
  await blockIs(megaUnit, 'megacells:4m_crafting_storage', 'the MEGA unit becomes a 4M MEGA Crafting Storage');
  // The use on AE2's unit came first: it took no component and left the unit as it was.
  await until(() => fixture.raw('inventory'), s => s.counts?.['megacells:cell_component_4m'] === 1, 'one 4M component used');
  assert.equal((await block(ae2Unit)).block, 'ae2:crafting_unit', 'AE2\'s unit takes no MEGA component');
  await fixture.replace('hotbar.0', 'minecraft:air'); await fixture.raw('hotbar 1');
  await fixture.command('clear @s megacells:cell_component_4m');
  await until(() => fixture.raw('inventory'), s => !s.counts?.['megacells:cell_component_4m'], 'empty hands');
  await fixture.sneakInteract(`${megaUnit} north`);
  await blockIs(megaUnit, 'megacells:mega_crafting_unit', 'the alternate use leaves MEGA\'s own unit');
  await until(() => fixture.raw('inventory'), s => s.counts?.['megacells:cell_component_4m'] === 1, 'the 4M component comes back');
  await fixture.replace('hotbar.0', 'ae2:engineering_processor'); await fixture.select(0, 'ae2:engineering_processor');
  await fixture.raw(`interact ${megaUnit} north`);
  await blockIs(megaUnit, 'megacells:mega_crafting_accelerator', 'an engineering processor makes MEGA\'s co-processor of MEGA\'s unit');
  await fixture.replace('hotbar.0', 'ae2:engineering_processor'); await fixture.select(0, 'ae2:engineering_processor');
  await fixture.raw(`interact ${ae2Unit} south`);
  await blockIs(ae2Unit, 'ae2:crafting_accelerator', 'and AE2\'s of AE2\'s');
  await fixture.command(`setblock ${megaUnit} minecraft:air`); await fixture.command(`setblock ${ae2Unit} minecraft:air`);
  fixture.mark('a MEGA unit changes in place into MEGA\'s blocks only, and back into MEGA\'s unit');

  // A network with a plank pattern before three Molecular Assemblers, and a CPU of MEGA's blocks alone:
  // unit, 1M storage, co-processor and monitor in a 2x2 face over the drive and the terminal's cable.
  await fixture.command('gamemode creative @s'); await fixture.command('clear @s');
  const drive = at(1), terminal = at(2), provider = at(3);
  // The provider's south face stays free for the player's click.
  const assemblers = [at(3, 0, -1), at(4), at(3, 1)];
  const unit = at(1, 1), storage = at(2, 1), accelerator = at(1, 2), monitor = at(2, 2);
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [provider, 'ae2:pattern_provider'],
    ...assemblers.map(pos => [pos, 'ae2:molecular_assembler']),
    [unit, 'megacells:mega_crafting_unit'], [storage, 'megacells:1m_crafting_storage'], [accelerator, 'megacells:mega_crafting_accelerator'],
    [monitor, 'megacells:mega_crafting_monitor[facing=north]']] as const) await fixture.command(`setblock ${pos} ${id}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_1k`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  const ready = () => until(() => fixture.ae(`grid status ${drive}`), s => s.online && s.powered, 'powered drive', 30000);
  await ready();
  for (const pos of assemblers) await until(() => fixture.ae(`grid status ${pos}`), s => s.online && s.powered, 'assembler online', 30000);
  const formation = (pos: string) => until(() => fixture.ae(`cpu-formation ${pos}`), s => !!s.cluster?.published, `formed CPU at ${pos}`, 30000);
  let mega = await formation(storage);
  assert.equal(mega.cluster.members.length, 4);
  assert.deepEqual([mega.cluster.storage, mega.cluster.coprocessors], [MIB, 4], 'MEGA\'s storage and co-processor');
  for (const pos of [unit, storage, accelerator, monitor]) {
    const b = await until(() => block(pos), b => /formed=true/.test(b.state) && /powered=true/.test(b.state), `formed and powered ${pos}`, 30000);
    assert.match(b.block, /^megacells:/);
  }
  // CraftingBlockEntity.getConnections: the storage's model joins the unit to its west and the monitor above.
  const joined = await until(() => block(storage), b => (b.ae2?.connections ?? []).length === 2, 'formed storage connections', 30000);
  assert.deepEqual([...joined.ae2.connections].sort(), ['up', 'west']);
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z - 2.5}`); await fixture.raw(`look-at block ${monitor}`); await fixture.raw('wait 10'); await shot('mega-cpu-formed');
  fixture.mark('MEGA\'s unit, 1M storage, co-processor and monitor form a CPU of 1,048,576 bytes and 4 co-processors');

  // A plank pattern in the provider and 192 logs in the drive.
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  const encoded = await fixture.ae('crafting encode minecraft:oak_log - - - - - - - -');
  assert.deepEqual([encoded.encoded, encoded.output, encoded.count], [true, 'minecraft:oak_planks', 4]);
  await until(() => fixture.raw('inventory'), view => view.hotbar[0]?.item === 'ae2:crafting_pattern', 'physical plank pattern');
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5}`); await fixture.raw(`interact ${provider} south`);
  const menu = await until(() => fixture.raw('screen'), view => view.slots?.some((slot: any) => slot.slot < 36 && slot.item === 'ae2:crafting_pattern'), 'provider menu', 5000);
  await fixture.raw(`click-slot ${menu.slots.find((slot: any) => slot.slot < 36 && slot.item === 'ae2:crafting_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), view => !view.counts?.['ae2:crafting_pattern'], 'pattern installed'); await fixture.raw('close');
  for (let i = 0; i < 3; i++) {
    await fixture.replace('hotbar.0', 'minecraft:oak_log', 64); await fixture.select(0, 'minecraft:oak_log');
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 64);
  }
  assert.equal(count(await ready(), 'minecraft:oak_log'), 192);

  // 2 nodes * 8 + 768 planks + 192 logs + 192 pushes = 1168 bytes: more than a 1K CPU holds (crafting-cpu-capacity).
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${terminal} north`);
  const view = await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot >= 300 && slot.slot < 345 && slot.item === 'minecraft:oak_planks'), 'craftable planks');
  await fixture.raw(`click-slot ${view.slots.find((slot: any) => slot.slot >= 300 && slot.slot < 345 && slot.item === 'minecraft:oak_planks').slot}`);
  await until(() => fixture.raw('screen'), s => JSON.stringify(s.title ?? '').includes('AE2 Craft Amount'), 'craft amount');
  await fixture.raw('ui-key right'); await fixture.raw('ui-key backspace'); await fixture.raw('type 768'); await fixture.raw('wait 5'); await fixture.raw('click-slot 302');
  const confirm = await until(() => fixture.raw('screen'), s => JSON.stringify(s.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(s)), 'craft confirmation');
  assert.match(JSON.stringify(confirm), /1168 bytes/); assert.doesNotMatch(JSON.stringify(confirm), /Missing materials/);
  // The CPU button picks the one CPU there is: the confirmation then states its storage and co-processors.
  const plan = await until(() => fixture.raw('screen'), v => v.ae2_craft?.buttons?.SelectCpu?.active, 'CPU selection button').then(v => v.ae2_craft);
  await fixture.raw(`click-at ${centre(plan.buttons.SelectCpu.rect)}`);
  const picked = await until(() => fixture.raw('screen'), v => v.ae2_craft?.button_label === 'Crafting CPU: CPU #1', 'the MEGA CPU selected').then(v => v.ae2_craft);
  assert.equal(picked.plan[1], 'Storage: 1048576 : Co Processors: 4', 'the confirmation names the MEGA CPU');
  await fixture.raw('click-slot 302'); await fixture.raw('close');
  fixture.mark('the craft confirmation offers the MEGA CPU, 1,048,576 bytes and 4 co-processors, for a 1,168-byte job');

  // The MEGA Crafting Monitor shows the job while it runs (CraftingMonitorRenderer).
  const shown = await until(() => block(monitor), b => b.ae2?.crafting_job?.item === 'minecraft:oak_planks' && b.ae2.crafting_job.amount > 0, 'the MEGA monitor shows the planks', 30000);
  assert(shown.ae2.crafting_job.amount <= 768);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`); await fixture.raw(`look-at block ${monitor}`); await fixture.raw('wait 5'); await shot('mega-cpu-monitor');
  fixture.mark(`the MEGA Crafting Monitor shows the job: ${shown.ae2.crafting_job.amount} planks to go`);
  const done = await until(() => fixture.ae(`grid status ${drive}`), s => count(s, 'minecraft:oak_planks') === 768 && count(s, 'minecraft:oak_log') === 0, 'the job runs on the MEGA CPU', 180000);
  assert.equal(count(done, 'minecraft:oak_planks'), 768);
  await until(() => block(monitor), b => !b.ae2?.crafting_job, 'the monitor clears when the job is done', 30000);
  fixture.mark('the 1,168-byte job runs on the MEGA CPU: 768 planks from 192 logs');

  // MEGA's and AE2's blocks in one CPU (CraftingCPUCalculator takes any CraftingBlockEntity).
  const mixed = [[at(-1), 'ae2:1k_crafting_storage'], [at(-1, 1), 'megacells:1m_crafting_storage'], [at(-2), 'ae2:crafting_accelerator'], [at(-2, 1), 'megacells:mega_crafting_accelerator']] as const;
  for (const [pos, id] of mixed) await fixture.command(`setblock ${pos} ${id}`);
  const both = await formation(at(-1));
  assert.equal(both.cluster.members.length, 4);
  assert.deepEqual([both.cluster.storage, both.cluster.coprocessors], [1024 + MIB, 1 + 4], 'AE2\'s and MEGA\'s bytes and threads add up');
  for (const [pos] of mixed) await until(() => block(pos), b => /formed=true/.test(b.state), `formed ${pos}`, 30000);
  await fixture.command(`tp @s ${x - 0.5} ${y} ${z - 2.5}`); await fixture.raw(`look-at block ${at(-1, 1)}`); await fixture.raw('wait 10'); await shot('mixed-cpu-formed');
  fixture.mark('AE2\'s 1K storage and co-processor and MEGA\'s 1M storage and co-processor form one CPU of 1,049,600 bytes and 5 co-processors');

  await fixture.restart();
  mega = await formation(storage);
  assert.deepEqual([mega.cluster.storage, mega.cluster.coprocessors], [MIB, 4]);
  const after = await formation(at(-1));
  assert.deepEqual([after.cluster.storage, after.cluster.coprocessors], [1024 + MIB, 5]);
  assert.equal(count(await ready(), 'minecraft:oak_planks'), 768);
  assert.match((await block(monitor)).block, /^megacells:mega_crafting_monitor$/);
  fixture.mark('both CPUs form again after a restart with the same bytes and co-processors');

  // Broken (Level.destroyBlock), each MEGA block drops itself by its loot table, and its CPU comes apart.
  for (const [pos, id] of [[monitor, 'megacells:mega_crafting_monitor'], [at(-1, 1), 'megacells:1m_crafting_storage']] as const) {
    await fixture.command(`setblock ${pos} minecraft:air destroy`);
    await until(() => entities(id), list => list.length === 1 && list[0].item.count === 1, `${id} drops itself`, 15000);
  }
  await until(() => block(storage), b => /formed=false/.test(b.state), 'the MEGA CPU without its monitor is no cuboid', 30000);
  fixture.mark('a broken MEGA crafting block drops itself and its CPU comes apart');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
