/** Advanced AE slice 3: the Quantum Computer. With All the Mods 10's config (12 threads, factors of 6) a box of Quantum
 * Structure holding no core, or a unit on its shell, does not form; with a core, a Data Entangler and an accelerator
 * inside it forms one CPU of 256 MB x 6 and 24 co-processors (AdvCraftingCPUCalculator, AdvCraftingCPUCluster); a core
 * alone forms too. On a grid with two processing patterns it takes two jobs at once, each of its plan's bytes; its
 * screen lists them and its free block and sets the selection mode of them all. Both jobs and the mode survive a
 * restart; the outputs coming back end them. A job waiting with its inputs, the computer broken, gives them back to the
 * grid (breakCluster). Needs content/advanced_ae (ae2/scripts/prepare-addon.ts advancedae) and a client built with
 * ae2-advancedae. */
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('advancedae-quantum-computer', mode, { addons: ['advanced_ae'] });
const dir = join(fixture.game, 'config'); mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'advanced_ae-common.toml'), '["quantum computer"]\nquantumComputerMaxSize = 7\nquantumComputerAcceleratorThreads = 12\n'
  + 'quantumComputerMaxMultiThreaders = 1\nquantumComputermaxDataEntanglers = 1\nquantumComputerMultiThreaderMultiplication = 6\n'
  + 'quantumComputerDataEntanglerMultiplication = 6\n');
const MIB = 1024 * 1024;
const STRUCTURE = 'advanced_ae:quantum_structure';
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const pattern = (input: string, output: string) =>
  `ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"${input}","#":1L}],sparseOutputs:[{"#t":"ae2:i",id:"${output}","#":1L}]}]`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-7, -4], [-7, 6], [6, -4], [6, 6]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-7, 0, -4)} ${at(6, 4, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-7, -1, -4)} ${at(6, -1, 6)} minecraft:stone`);
  await fixture.command('clear @s');
  const block = (pos: string) => fixture.raw(`block ${pos}`);
  const quantum = (pos: string) => fixture.ae(`advae quantum ${pos}`);
  const shell = at(-1), top = at(-3, 2);

  // A 5 x 3 x 3 box of Quantum Structure west of the grid: structure inside does not form, nor do units with no core.
  await fixture.command(`fill ${at(-5, 0, -1)} ${at(-1, 2, 1)} ${STRUCTURE}`);
  await fixture.raw('wait 5');
  assert.equal((await quantum(shell)).formed, false, 'structure inside');
  for (const dx of [-4, -3, -2]) await fixture.command(`setblock ${at(dx, 1)} advanced_ae:quantum_unit`);
  await fixture.raw('wait 5');
  assert.equal((await quantum(shell)).formed, false, 'no core');
  // A core, the Data Entangler and an accelerator inside.
  await fixture.command(`setblock ${at(-4, 1)} advanced_ae:quantum_core`);
  await fixture.command(`setblock ${at(-3, 1)} advanced_ae:data_entangler`);
  await fixture.command(`setblock ${at(-2, 1)} advanced_ae:quantum_accelerator`);
  const formed = await until(() => quantum(shell), q => q.formed, 'the computer forms', 15000);
  assert.deepEqual([formed.blocks, formed.storage, formed.coprocessors, formed.core], [45, 256 * MIB * 6, 24, [x - 4, y + 1, z]]);
  assert.match((await block(shell)).state, /formed=true/);
  assert.match((await block(shell)).state, /multiblocked=true/);
  fixture.mark('a core, an entangler and an accelerator in a shell of structure form 1,610,612,736 bytes and 24 co-processors');
  await fixture.command(`setblock ${top} advanced_ae:quantum_unit`);
  await until(() => quantum(shell), q => !q.formed, 'a unit on the shell breaks it', 15000);
  await until(() => block(shell), b => /formed=false/.test(b.state), 'the shell unformed', 15000);
  await fixture.command(`setblock ${top} ${STRUCTURE}`);
  await until(() => quantum(shell), q => q.formed, 'formed again', 15000);
  // A core alone.
  const lone = at(-3, 0, 5);
  await fixture.command(`setblock ${lone} advanced_ae:quantum_core`);
  const alone = await until(() => quantum(lone), q => q.formed, 'a core alone forms', 15000);
  assert.deepEqual([alone.blocks, alone.storage, alone.coprocessors], [1, 256 * MIB, 12]);
  assert.match((await block(lone)).state, /multiblocked=false/);
  await fixture.command(`setblock ${lone} minecraft:air`);
  fixture.mark('a unit on the shell or no core does not form; a core alone does');

  // The grid: power, a drive of 4 iron and 4 copper, a terminal, a provider of two processing patterns over a chest.
  const power = at(0), drive = at(1), terminal = at(2), provider = at(3), chest = at(3, 0, 1);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [provider, 'ae2:pattern_provider'], [chest, 'minecraft:chest']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:iron_ingot","#":4L},{"#t":"ae2:i",id:"minecraft:copper_ingot","#":4L}]]`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  await fixture.command('item replace entity @s hotbar.0 with minecraft:air');
  const grid = () => until(() => fixture.ae(`grid status ${drive}`), v => v.online && v.powered, 'powered drive', 30000);
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:iron_ingot') === 4, 'the cell mounted', 30000);
  await until(() => quantum(shell), q => q.online, 'the computer on the grid', 30000);
  await fixture.command(`item replace entity @s hotbar.1 with ${pattern('minecraft:iron_ingot', 'minecraft:gold_ingot')}`);
  await fixture.command(`item replace entity @s hotbar.2 with ${pattern('minecraft:copper_ingot', 'minecraft:diamond')}`);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${provider} north`);
  for (let n = 1; n <= 2; n++) {
    const view = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern'), 'the provider\'s screen', 15000);
    await fixture.raw(`click-slot ${view.slots.find((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern').slot} shift`);
    await until(() => fixture.raw('screen'), v => v.slots.filter((s: any) => s.slot >= 300 && s.slot < 309 && s.item === 'ae2:processing_pattern').length === n, `pattern ${n} in the provider`);
  }
  await fixture.raw('close');

  // Two crafts from the terminal, each on the computer: both run at once.
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  const craft = async (item: string, amount: number) => {
    await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
    await fixture.raw(`interact ${terminal} north`);
    const entry = (v: any) => v.slots?.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === item);
    await until(() => fixture.raw('screen'), entry, `${item} craftable`, 30000);
    await until(async () => {
      const v = await fixture.raw('screen');
      if (!JSON.stringify(v.title ?? '').includes('AE2 Craft Amount') && entry(v)) await fixture.raw(`click-slot ${entry(v).slot} middle`);
      return v;
    }, v => JSON.stringify(v.title ?? '').includes('AE2 Craft Amount'), 'craft amount');
    if (amount !== 1) { await fixture.raw('ui-key right'); await fixture.raw('ui-key backspace'); await fixture.raw(`type ${amount}`); await fixture.raw('wait 5'); }
    await fixture.raw('click-slot 302');
    const confirm = await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(v)), 'craft confirm');
    assert.doesNotMatch(JSON.stringify(confirm), /Missing materials|craft planning:/);
    await fixture.raw('click-slot 302');
    await fixture.raw('close');
  };
  await craft('minecraft:gold_ingot', 1);
  await until(() => quantum(shell), q => q.jobs.length === 1, 'the first job on the computer', 30000);
  await craft('minecraft:diamond', 1);
  const busy = await until(() => quantum(shell), q => q.jobs.length === 2, 'two jobs at once', 30000);
  assert(busy.jobs.some((j: any) => /gold_ingot/.test(j.item)) && busy.jobs.some((j: any) => /diamond/.test(j.item)), JSON.stringify(busy.jobs));
  assert.deepEqual(busy.jobs[0].at, [x - 4, y + 1, z], 'the first job on the core');
  const used = busy.jobs.reduce((n: number, j: any) => n + j.bytes, 0);
  assert(busy.jobs.every((j: any) => j.bytes > 0 && j.bytes < 1024), 'each job holds its plan\'s bytes');
  assert.equal(busy.remaining, busy.storage - used);
  const pushed = await until(() => fixture.raw(`container ${chest}`), c => ['minecraft:iron_ingot', 'minecraft:copper_ingot'].every(id => (c.slots ?? []).some((s: any) => s?.item === id)), 'both inputs pushed', 30000);
  assert(pushed);
  fixture.mark(`two jobs at once on the computer, ${busy.jobs.map((j: any) => j.bytes).join(' and ')} bytes, the rest left for more`);

  // Its screen: the two jobs and the free block, the selection mode for them all.
  await fixture.command(`tp @s ${x - 2.5} ${y} ${z - 2.5}`);
  await fixture.raw(`interact ${at(-3, 1, -1)} north`);
  const screen = await until(() => fixture.raw('screen'), v => v.ae2_craft?.cpus?.length === 3, 'the computer\'s screen', 15000).then(v => v.ae2_craft);
  assert.equal(screen.style, 'crafting_status.json');
  assert(screen.buttons.Mode && !screen.buttons.StatusBack, 'the mode in place of back');
  assert.deepEqual(screen.cpus.map((c: any) => c.storage).sort((a: number, b: number) => a - b), [...busy.jobs.map((j: any) => j.bytes), busy.remaining].sort((a, b) => a - b));
  assert.equal(screen.cpus.filter((c: any) => c.job !== null).length, 2);
  assert(screen.cpus.every((c: any) => c.coprocessors === 24));
  const [mx, my, mw, mh] = screen.buttons.Mode.rect;
  await fixture.raw(`click-at ${mx + mw / 2} ${my + mh / 2}`);
  await until(() => quantum(shell), q => q.mode === 'PLAYER_ONLY', 'the selection mode on the core', 15000);
  await fixture.raw('close');
  fixture.mark('the screen lists both jobs and the free block, and its mode button sets PLAYER_ONLY for the computer');

  await fixture.restart();
  const kept = await until(() => quantum(shell), q => q.formed && q.jobs.length === 2, 'the jobs after a restart', 30000);
  assert.equal(kept.mode, 'PLAYER_ONLY');
  assert.equal(kept.remaining, busy.remaining);
  fixture.mark('both jobs and the mode survive a restart');

  // The outputs coming back end both jobs.
  await grid();
  for (const item of ['minecraft:gold_ingot', 'minecraft:diamond']) {
    await fixture.replace('hotbar.0', item); await fixture.select(0, item);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 1);
  }
  const done = await until(() => quantum(shell), q => q.jobs.length === 0, 'both jobs done', 30000);
  assert.equal(done.remaining, done.storage);
  const after = await grid();
  assert.deepEqual([count(after, 'minecraft:gold_ingot'), count(after, 'minecraft:diamond'), count(after, 'minecraft:iron_ingot')], [1, 1, 3]);
  fixture.mark('the outputs end both jobs and land in the grid');

  // Nothing to push into: a job keeps its two iron; the computer broken gives them back.
  await fixture.command(`setblock ${chest} minecraft:air`);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await craft('minecraft:gold_ingot', 2);
  await until(() => quantum(shell), q => q.jobs.length === 1, 'the waiting job', 30000);
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:iron_ingot') === 1, 'its iron taken', 30000);
  await fixture.command(`setblock ${top} minecraft:air destroy`);
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:iron_ingot') === 3, 'the iron back in the grid', 30000);
  assert.equal((await quantum(shell)).formed, false);
  fixture.mark('broken, the computer cancels its job and gives its two iron back to the grid');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
