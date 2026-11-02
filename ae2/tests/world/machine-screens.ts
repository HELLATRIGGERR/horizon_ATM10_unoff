/** AE2's machine screens open on their menus with their styles, the priority screen edits a Drive's priority, and the crafting screens take a job from the amount through the plan to the status. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('machine-screens', mode);
const centre = ([x, y, w, h]: number[]) => `${x + w / 2} ${y + h / 2}`;
// Each machine, the style its screen loads, and its x offset in the row.
const machines = [
  ['inscriber', 'inscriber.json'], ['drive', 'drive.json'], ['vibration_chamber', 'vibration_chamber.json'],
  ['io_port', 'io_port.json'], ['condenser', 'condenser.json'], ['molecular_assembler', 'molecular_assembler.json'],
  ['pattern_provider', 'pattern_provider.json'], ['interface', 'interface.json'], ['quantum_link', 'qnb.json'],
  ['spatial_io_port', 'spatial_io_port.json'], ['wireless_access_point', 'wireless_access_point.json'],
] as const;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const width = machines.length * 2;
  for (const [dx, dz] of [[-2, -3], [-2, 3], [width, -3], [width, 3]] as const) {
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), state => state.loaded, 'chunk loaded', 30000);
  }
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + width} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + width} ${y - 1} ${z + 3} minecraft:stone`);
  for (const [i, [id]] of machines.entries()) await fixture.command(`setblock ${x + 2 * i} ${y} ${z} ae2:${id}`);
  await fixture.command('clear @s');
  await fixture.replace('hotbar.1', 'ae2:wireless_booster', 2);
  await fixture.replace('hotbar.8', 'minecraft:stick');
  const open = async (i: number) => {
    await fixture.select(8, 'minecraft:stick');
    await fixture.command(`tp @s ${x + 2 * i + 0.5} ${y} ${z + 2.5}`);
    await fixture.raw(`interact ${x + 2 * i} ${y} ${z} south`);
    return until(() => fixture.raw('screen'), view => view.ae2_screen, `${machines[i][0]} screen`, 15000);
  };
  for (const [i, [id, style]] of machines.entries()) {
    const view = await open(i);
    assert.equal(view.ae2_screen.style, style, id);
    assert.equal(view.ae2_screen.toolbar?.[0]?.button, 'help', `${id} help button`);
    if (id === 'wireless_access_point') {
      // Two boosters: 16 + 2^1.5 blocks (tenths) and 8 + 2^(1 + 2/64) AE/t (hundredths).
      await fixture.raw('click-slot 1');
      await fixture.raw('click-slot 300');
      const boosted = await until(() => fixture.raw('screen'), v => v.ae2_screen?.data?.[0] === 188, 'range with two boosters', 15000);
      assert.equal(boosted.ae2_screen.data[4], 1004);
    }
    await fixture.raw('close');
  }
  fixture.mark('every AE2 machine opens its own screen style with the help button; the WAP shows its boosters\' range and drain');

  // The power unit button: FE on the WAP's toolbar, back to AE.
  const wap = await open(machines.findIndex(([id]) => id === 'wireless_access_point'));
  const power = wap.ae2_screen.toolbar.find((b: any) => b.button === 'power');
  assert.equal(power.value, 0);
  await fixture.raw(`click-at ${centre(power.rect)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen.toolbar.find((b: any) => b.button === 'power').value === 1, 'FE', 15000);
  await fixture.raw(`click-at ${centre(power.rect)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen.toolbar.find((b: any) => b.button === 'power').value === 0, 'AE again', 15000);
  await fixture.raw('close');

  // The ME Chest: its cell screen from the sides, the terminal from the top.
  const chest = `${x - 1} ${y} ${z - 2}`;
  await fixture.command(`setblock ${chest} ae2:chest`);
  await fixture.command(`setblock ${x - 2} ${y} ${z - 2} ae2:creative_energy_cell`);
  await fixture.replace('hotbar.0', 'ae2:item_storage_cell_1k');
  await fixture.select(8, 'minecraft:stick');
  await fixture.command(`tp @s ${x - 0.5} ${y} ${z + 0.5}`);
  await fixture.raw(`interact ${chest} south`);
  const chestView = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'me_chest.json', 'ME Chest cell screen', 15000);
  assert.ok(chestView.ae2_screen.priority, 'the chest has its priority tab');
  await fixture.raw('click-slot 0 shift');
  await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot === 300 && s.item === 'ae2:item_storage_cell_1k'), 'cell in the chest', 15000);
  await fixture.raw('close');
  await fixture.raw(`interact ${chest} up`);
  await until(() => fixture.raw('screen'), v => v.ae2_terminal, 'terminal from the top', 15000);
  await fixture.raw('close');
  await fixture.command(`setblock ${chest} minecraft:air`);
  await fixture.command(`setblock ${x - 2} ${y} ${z - 2} minecraft:air`);
  fixture.mark('the power unit button switches and the ME Chest opens its cell screen from the sides and the terminal from the top');

  // The Cell Workbench: all filters at once, its toolbar, a filter past the first page.
  const bench = `${x - 1} ${y} ${z + 2}`;
  await fixture.command(`setblock ${bench} ae2:cell_workbench`);
  await fixture.replace('hotbar.0', 'ae2:item_storage_cell_1k');
  await fixture.replace('hotbar.1', 'minecraft:diamond');
  await fixture.select(8, 'minecraft:stick');
  await fixture.raw(`interact ${bench} north`);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'cell_workbench.json' && v.ae2_workbench, 'workbench screen', 15000);
  await fixture.raw('click-slot 0 shift');
  let work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[0].open, 'cell in the workbench', 15000)).ae2_workbench;
  assert.equal(work.config.length, 63);
  assert.equal(work.upgrades, 4);
  assert.deepEqual(work.tools.map((t: any) => t.title), ['gui.tooltips.ae2.PartitionStorage', 'gui.tooltips.ae2.Clear', 'gui.ae2.CopyMode']);
  await fixture.raw('click-slot 1');
  await fixture.raw(`click-at ${centre(work.config[40].rect)}`);
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config[40].item === 'minecraft:diamond', 'filter 40 by place', 15000)).ae2_workbench;
  await fixture.raw(`click-at ${centre(work.tools[2].rect)}`);
  work = (await until(() => fixture.raw('screen'), v => v.ae2_workbench?.keep === true, 'copy mode', 15000)).ae2_workbench;
  await fixture.raw(`click-at ${centre(work.tools[1].rect)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_workbench?.config.every((c: any) => !c.item), 'filters cleared', 15000);
  await fixture.raw('click-slot 1');
  await fixture.raw('close');
  await fixture.command(`setblock ${bench} minecraft:air`);
  fixture.mark('the Cell Workbench shows its 63 filters at once, sets one past the first page, and has its own toolbar buttons');

  // The Drive's priority tab opens the priority screen; its buttons, field and back tab work.
  const drive = machines.findIndex(([id]) => id === 'drive');
  const driveView = await open(drive);
  await fixture.raw(`click-at ${centre(driveView.ae2_screen.priority)}`);
  let priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority, 'priority screen', 15000)).ae2_priority;
  assert.equal(priority.text, '0');
  await fixture.raw(`click-at ${centre(priority.buttons.find((b: any) => b.entry === 2).rect)}`);
  await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === '100', 'plus 100', 15000);
  await fixture.raw(`click-at ${centre(priority.buttons.find((b: any) => b.entry === 4).rect)}`);
  priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === '99', 'minus 1', 15000)).ae2_priority;
  await fixture.raw(`click-at ${centre(priority.field)}`);
  for (let n = 0; n < priority.text.length; n++) await fixture.raw('type \\b');
  await fixture.raw('type 12x');
  priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === '12x', 'typed text', 15000)).ae2_priority;
  assert.equal(priority.valid, false);
  for (let n = 0; n < 3; n++) await fixture.raw('type \\b');
  await fixture.raw('type 42');
  await until(() => fixture.raw('screen'), v => v.ae2_priority?.text === '42' && v.ae2_priority.valid, 'typed 42', 15000);
  await fixture.raw(`click-at ${centre(priority.back)}`);
  const back = await until(() => fixture.raw('screen'), v => v.ae2_screen && !v.ae2_priority, 'back to the drive', 15000);
  assert.equal(back.ae2_screen.style, 'drive.json');
  await fixture.raw('close');
  await fixture.restart();
  const reopened = await open(drive);
  await fixture.raw(`click-at ${centre(reopened.ae2_screen.priority)}`);
  priority = (await until(() => fixture.raw('screen'), v => v.ae2_priority, 'priority screen again', 15000)).ae2_priority;
  assert.equal(priority.text, '42');
  await fixture.raw('close');
  fixture.mark('the priority screen adds and takes away, flags text that is no number, goes back, and the Drive keeps 42 across restart');

  // The crafting screens, on a small grid with one pattern and two CPUs.
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + 12 + dz}`;
  const craft = (test: (c: any) => boolean, label: string, ms = 30000) => until(() => fixture.raw('screen'), v => v.ae2_craft && test(v.ae2_craft), label, ms).then(v => v.ae2_craft);
  const press = (c: any, button: string, right = false) => fixture.raw(`click-at ${centre(c.buttons[button].rect)}${right ? ' right' : ''}`);
  const backAtTerminal = (label: string) => until(() => fixture.raw('screen'), v => v.ae2_terminal && !v.ae2_craft, label, 15000);
  const terminal = at(2), cpuBlock = at(2, 1), provider = at(3);
  await until(() => fixture.raw(`block ${at(6)}`), s => s.loaded, 'grid chunk loaded', 30000);
  await fixture.command('clear @s');
  await fixture.command(`fill ${at(-2, 0, -3)} ${at(6, 5, 3)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -1, -3)} ${at(6, -1, 3)} minecraft:stone`);
  for (const [pos, id] of [[at(0), 'creative_energy_cell'], [at(1), 'drive'], [terminal, 'cable_bus'], [cpuBlock, '64k_crafting_storage'], [at(1, 1), 'cable_bus'], [at(0, 1), '1k_crafting_storage'], [provider, 'pattern_provider'], [at(3, 0, -1), 'molecular_assembler']] as const) {
    await fixture.command(`setblock ${pos} ae2:${id}`);
  }
  await fixture.command(`item replace block ${at(1)} container.0 with ae2:item_storage_cell_4k`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  await fixture.ae(`grid part ${terminal} north`);
  await until(() => fixture.ae(`grid status ${at(1)}`), s => s.online && s.powered, 'grid powered', 30000);
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  await fixture.ae('crafting encode minecraft:oak_log - - - - - - - -');
  await until(() => fixture.raw('inventory'), v => v.hotbar[0]?.item === 'ae2:crafting_pattern', 'encoded pattern');
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 14.5}`); await fixture.raw(`interact ${provider} south`);
  const providerMenu = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot < 36 && s.item === 'ae2:crafting_pattern'), 'provider menu');
  await fixture.raw(`click-slot ${providerMenu.slots.find((s: any) => s.slot < 36 && s.item === 'ae2:crafting_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), v => !v.counts?.['ae2:crafting_pattern'], 'pattern installed'); await fixture.raw('close');
  for (let i = 0; i < 2; i++) {
    await fixture.replace('hotbar.0', 'minecraft:oak_log', 64); await fixture.select(0, 'minecraft:oak_log');
    await fixture.ae(`grid deposit ${at(1)}`);
  }
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  const openAmount = async () => {
    await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 9.5}`);
    if (!(await fixture.raw('screen')).ae2_terminal) await fixture.raw(`interact ${terminal} north`);
    const view = await until(() => fixture.raw('screen'), s => s.slots?.some((v: any) => v.slot >= 300 && v.slot < 345 && v.item === 'minecraft:oak_planks'), 'craftable in the terminal', 30000);
    await fixture.raw(`click-slot ${view.slots.find((v: any) => v.slot >= 300 && v.slot < 345 && v.item === 'minecraft:oak_planks').slot}`);
    return craft(c => c.style === 'craft_amount.json', 'amount screen');
  };

  // The amount: steps, the wheel, text that is no amount, and back to the terminal.
  let amount = await openAmount();
  assert.equal(amount.text, '1');
  assert.equal(amount.buttons.Next.active, true);
  await press(amount, 'Number(2)');
  await craft(c => c.text === '100', 'plus 100');
  await press(amount, 'Number(4)');
  await craft(c => c.text === '99', 'minus 1');
  await fixture.raw(`scroll-at ${centre(amount.field)} 1`);
  await craft(c => c.text === '100', 'wheel up');
  await fixture.raw(`click-at ${centre(amount.field)}`);
  await fixture.raw('type x');
  await craft(c => c.text === '100x' && !c.buttons.Next.active, 'text that is no amount');
  await fixture.raw('type \\b');
  await craft(c => c.text === '100' && c.buttons.Next.active, 'amount again');
  await press(amount, 'Back');
  await backAtTerminal('amount back to the terminal');

  // The plan: its table, the CPU selection both ways, and cancelling back to the terminal.
  amount = await openAmount();
  await press(amount, 'Number(2)');
  await craft(c => c.text === '100', 'plus 100 again');
  await press(amount, 'Next');
  let plan = await craft(c => c.style === 'craft_confirm.json' && c.buttons.Start.active, 'finished plan');
  assert.match(plan.plan[0], /^Crafting Plan - [\d,]+ Bytes Used$/);
  assert.deepEqual(plan.rows.map((r: any) => [r.item, r.amounts]), [['minecraft:oak_planks', [0, 0, 100]], ['minecraft:oak_log', [25, 0, 0]]]);
  assert.equal(plan.button_label, 'Crafting CPU: Automatic');
  await press(plan, 'SelectCpu');
  plan = await craft(c => c.button_label === 'Crafting CPU: CPU #1', 'first CPU selected');
  assert.match(plan.plan[1], /^Storage: \d+ : Co Processors: 0$/);
  await press(plan, 'SelectCpu', true);
  plan = await craft(c => c.button_label === 'Crafting CPU: Automatic', 'back to automatic');
  await press(plan, 'Cancel');
  await backAtTerminal('plan cancelled to the terminal');

  // Start on Enter, a job big enough to look at, back to the terminal.
  amount = await openAmount();
  await fixture.raw('ui-key right'); await fixture.raw('ui-key backspace'); await fixture.raw('type 500');
  await craft(c => c.text === '500', 'typed amount');
  await fixture.raw('ui-key enter');
  await craft(c => c.style === 'craft_confirm.json' && c.buttons.Start.active, 'plan on enter');
  await fixture.raw('ui-key enter');
  const atTerminal = await backAtTerminal('started on enter');

  // The status: the CPU list, suspend, another CPU, cancel, the back tab.
  await fixture.raw(`click-at ${centre(atTerminal.ae2_terminal.widgets.find((w: any) => w.widget === 'CraftingStatus').rect)}`);
  let status = await craft(c => c.style === 'crafting_status.json' && c.cpus?.length === 2 && c.rows?.length > 0, 'status with its job');
  assert.equal(status.selected !== null && status.cpus[status.selected].job, 500);
  assert.equal(status.buttons.Suspend.active, true);
  await press(status, 'Suspend');
  status = await craft(c => c.suspended, 'job suspended');
  const busy = status.selected, idle = 1 - busy;
  await fixture.raw(`click-at ${centre(status.cpus[idle].rect)}`);
  status = await craft(c => c.selected === idle && c.rows.length === 0, 'the idle CPU');
  assert.equal(status.buttons.Suspend.active, false);
  await fixture.raw(`click-at ${centre(status.cpus[busy].rect)}`);
  status = await craft(c => c.selected === busy && c.rows.length > 0 && c.suspended, 'the busy CPU again');
  await press(status, 'Suspend');
  status = await craft(c => !c.suspended, 'job resumed');
  await press(status, 'CancelJob');
  status = await craft(c => c.rows.length === 0, 'job cancelled');
  await press(status, 'StatusBack');
  await backAtTerminal('status back to the terminal');
  await fixture.raw('close');

  // A CPU's own screen: no list, and its selection mode on the toolbar.
  await fixture.command(`tp @s ${x + 2.5} ${y + 1} ${z + 14.5}`);
  await fixture.raw(`interact ${cpuBlock} south`);
  let own = await craft(c => c.style === 'crafting_cpu.json', 'a CPU\'s own status');
  assert.equal(own.cpus.length, 1);
  assert.equal(own.mode, 'ANY');
  await press(own, 'Mode');
  own = await craft(c => c.mode === 'PLAYER_ONLY', 'players only');
  await press(own, 'Mode');
  own = await craft(c => c.mode === 'MACHINE_ONLY', 'machines only');
  await press(own, 'Mode');
  await craft(c => c.mode === 'ANY', 'any again');
  await fixture.raw('close');
  fixture.mark('the crafting screens step an amount, list a plan, cycle its CPU, start on Enter, and suspend, cancel and pick CPUs on the status');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
