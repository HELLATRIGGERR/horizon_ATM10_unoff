/** ME Requester: a requester keeps 100 sticks in stock through a pattern provider and a molecular
 * assembler, with ME Requester's statuses, its one-job batches, Missing Ingredients, a restart, its
 * screen and the block's active look. Needs content/merequester (ae2/scripts/prepare-addon.ts
 * merequester) and a client built with ae2-merequester. */
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('merequester-stock', mode, { addons: ['merequester'] });
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.ceil(py) + 3, z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const drive = at(1), host = at(2), requester = at(3);
  const status = () => fixture.ae(`requester status ${requester}`);
  const grid = () => fixture.ae(`grid status ${drive}`);
  const count = async (item: string) => ((await grid()).items ?? []).find((e: any) => e.item === item)?.count ?? 0;
  const row = async () => (await status()).rows[0];

  await fixture.command(`fill ${at(-3, -1, -4)} ${at(6, -1, 4)} minecraft:stone`);
  await fixture.command(`fill ${at(-3, 0, -4)} ${at(6, 4, 4)} minecraft:air`);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [host, 'ae2:cable_bus'], [at(2, 1), 'ae2:1k_crafting_storage'],
    [at(1, 0, -1), 'ae2:cable_bus'], [requester, 'merequester:requester']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k`);
  await fixture.replace('hotbar.0', 'ae2:cable_pattern_provider'); await fixture.select(0, 'ae2:cable_pattern_provider');
  assert.equal((await fixture.ae(`grid part ${host} north`)).attached, true);
  // Sticks: two planks in a column.
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  assert.equal((await fixture.ae('crafting encode minecraft:oak_planks - - minecraft:oak_planks - - - - -')).encoded, true);
  await until(() => fixture.raw('inventory'), s => s.held?.item === 'ae2:crafting_pattern', 'encoded stick pattern');
  await fixture.raw(`interact ${host} north`);
  const menu = await until(() => fixture.raw('screen'), s => s.slots?.some((v: any) => v.item === 'ae2:crafting_pattern' && v.slot < 36), 'provider menu');
  await fixture.raw(`click-slot ${menu.slots.find((v: any) => v.item === 'ae2:crafting_pattern' && v.slot < 36).slot} shift`);
  await until(() => fixture.raw('inventory'), s => !s.counts?.['ae2:crafting_pattern'], 'pattern installed');
  await fixture.raw('close');
  // The assembler goes in front of the provider once the pattern is in.
  await fixture.command(`setblock ${at(2, 0, -1)} ae2:molecular_assembler`);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5}`);
  await until(grid, s => s.online && s.powered, 'grid boots', 30000);
  for (const [item, n] of [['minecraft:oak_planks', 32], ['minecraft:stick', 50]] as const) {
    await fixture.replace('hotbar.0', item, n); await fixture.select(0, item);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, n);
  }
  let s = await until(status, v => v.online && v.powered, 'the requester is on the grid with its channel', 30000);
  assert.deepEqual(s.config, { requests: 5, idle_energy: 5, require_channel: true }, 'ATM10 settings');
  assert.equal(s.rows.length, 5);
  assert.deepEqual([s.rows[0].state, s.rows[0].amount, s.rows[0].batch, s.rows[0].status], [true, 0, 1, 'idle']);
  fixture.mark('an ME Requester joins the grid with five empty rows');

  // Keep 100 sticks, batch 64: 50 are there, so one job of 64 runs.
  s = await fixture.ae(`requester set ${requester} 0 minecraft:stick 100 64`);
  assert.deepEqual([s.rows[0].key.id, s.rows[0].amount, s.rows[0].batch], ['minecraft:stick', 100, 64]);
  await until(row, r => r.status === 'link', 'the row shows Crafting', 30000);
  await until(() => fixture.raw(`block ${requester}`), b => b.state.includes('active=true'), 'the block looks active while crafting');
  assert.equal((await row()).locked, true, 'a crafting row is locked');
  s = await until(status, v => v.rows[0].status === 'idle' && v.rows[0].known === 114, 'the row idles at 114 sticks', 60000);
  assert.equal(s.jobs, 1, 'batch size 64 makes one job');
  assert.equal(await count('minecraft:stick'), 114);
  assert.equal(await count('minecraft:oak_planks'), 0);
  await until(() => fixture.raw(`block ${requester}`), b => b.state.includes('active=false'), 'the block looks idle again');
  fixture.mark('keeping 100 sticks from 50: one job of 64, Crafting then Idling at 114');

  // Take 50: 64 < 100, but no planks are left.
  const index = (await grid()).items.findIndex((e: any) => e.item === 'minecraft:stick');
  await fixture.ae(`grid withdraw ${drive} ${index} 50`);
  await fixture.command('clear @s minecraft:stick');
  await until(row, r => r.status === 'missing', 'Missing Ingredients without planks', 60000);
  assert.equal((await status()).jobs, 1, 'no job without ingredients');
  fixture.mark('Missing Ingredients when the planks run out');

  await fixture.restart();
  await until(grid, v => v.online && v.powered, 'grid boots after restart', 60000);
  s = await until(status, v => v.rows[0].status === 'missing', 'still missing after a restart', 60000);
  assert.deepEqual([s.rows[0].key.id, s.rows[0].amount, s.rows[0].batch], ['minecraft:stick', 100, 64], 'the row survives a restart');
  await fixture.replace('hotbar.0', 'minecraft:oak_planks', 32); await fixture.select(0, 'minecraft:oak_planks');
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 32);
  s = await until(status, v => v.rows[0].status === 'idle' && v.rows[0].known === 128, 'crafting resumes with planks', 60000);
  assert.equal(s.jobs, 1);
  fixture.mark('after a restart the row stays Missing, then crafts once planks arrive: 128 sticks');

  // A job that runs through a restart: without the assembler the provider has nowhere to push.
  const assembler = at(2, 0, -1), cpu = at(2, 1);
  const deposit = async (item: string, n: number) => {
    await fixture.replace('hotbar.0', item, n); await fixture.select(0, item);
    // A block just taken from the grid: it forms again before it takes anything.
    await until(grid, v => v.online && v.powered, 'the grid back online', 30000);
    assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, n);
  };
  await fixture.command(`setblock ${assembler} minecraft:air`);
  await deposit('minecraft:oak_planks', 8);
  await fixture.ae(`requester set ${requester} 0 minecraft:stick 200 8`);
  await until(row, r => r.status === 'link', 'a job waits for the assembler', 30000);
  await fixture.restart();
  await until(grid, v => v.online && v.powered, 'grid boots after the second restart', 60000);
  s = await until(status, v => v.rows[0].status === 'link', 'the row keeps its job through a restart', 60000);
  assert.equal(s.rows[0].locked, true, 'a crafting row stays locked');
  await fixture.replace('hotbar.0', 'minecraft:stick'); await fixture.select(0, 'minecraft:stick');
  await fixture.raw(`interact ${requester} south`);
  let screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.lines?.[0]?.status === 'link', 'the screen shows Crafting', 30000);
  const shotCrafting = join(fixture.run, 'merequester-screen-crafting.png');
  await fixture.raw(`screenshot ${shotCrafting}`);
  // A locked row takes no edits from its ghost slot.
  await fixture.raw('click-slot 300 shift');
  await Bun.sleep(300);
  assert.equal((await row()).key?.id, 'minecraft:stick', 'a locked row keeps its key');
  await fixture.raw('close');
  await fixture.command(`setblock ${assembler} ae2:molecular_assembler`);
  // The job ends; the next one takes the other planks, then none are left.
  s = await until(status, v => v.rows[0].status === 'missing' && v.rows[0].known === 144, 'both jobs end at 144 sticks', 60000);
  assert.equal(await count('minecraft:oak_planks'), 0);
  fixture.mark(`a job survives a restart and finishes: 144 sticks; screenshot ${shotCrafting}`);

  // No CPU: the crafting storage goes away, and comes back.
  await fixture.command(`setblock ${cpu} minecraft:air`);
  await deposit('minecraft:oak_planks', 4);
  await until(row, r => r.status === 'cpu', 'No CPU without a crafting storage', 60000);
  assert.equal(await count('minecraft:oak_planks'), 4, 'nothing is taken without a CPU');
  await fixture.command(`setblock ${cpu} ae2:1k_crafting_storage`);
  await until(status, v => v.rows[0].status === 'missing' && v.rows[0].known === 152, 'crafting goes on with a CPU', 60000);
  fixture.mark('No CPU until a crafting storage is back: 152 sticks');

  // A CPU that breaks during the job cancels it (`link.isCanceled`) and gives the planks back.
  await fixture.command(`setblock ${assembler} minecraft:air`);
  await deposit('minecraft:oak_planks', 4);
  await until(row, r => r.status === 'link', 'a second job waits for the assembler', 60000);
  await fixture.command(`setblock ${cpu} minecraft:air`);
  s = await until(status, v => v.rows[0].status === 'cpu' && v.rows[0].link === null, 'the broken CPU ends the job', 60000);
  assert.equal(s.rows[0].buffer, 0);
  await fixture.command(`setblock ${cpu} ae2:1k_crafting_storage`);
  await fixture.command(`setblock ${assembler} ae2:molecular_assembler`);
  await until(status, v => v.rows[0].status === 'missing' && v.rows[0].known === 160, 'the planks come back and craft', 60000);
  fixture.mark('a broken CPU cancels the job and loses nothing: 160 sticks');

  await fixture.ae(`requester set ${requester} 0 minecraft:stick 100 64`);
  await until(row, r => r.status === 'idle', 'the row idles at 160 of 100');

  // The screen: rows, a click on the toggle, a screenshot.
  await fixture.ae(`requester set ${requester} 1 fluid:minecraft:water 5000 1000`);
  await fixture.replace('hotbar.0', 'minecraft:stick'); await fixture.select(0, 'minecraft:stick');
  await fixture.raw(`interact ${requester} south`);
  screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.lines?.length === 5, 'the requester screen opens', 30000);
  const view = screen.addon_screen;
  assert.equal(view.kind, 'merequester:requester');
  assert.deepEqual(view.size, [195, 20 + 99 + view.rows * 19 + 1]);
  assert.deepEqual([view.lines[0].amount, view.lines[0].batch, view.lines[0].status, view.lines[0].key], [100, 64, 'idle', true]);
  assert.deepEqual([view.lines[1].fluid, view.lines[1].amount, view.lines[1].batch], [true, 5000, 1000]);
  assert(screen.slots.some((v: any) => v.slot === 300 && v.item === 'minecraft:stick'), 'the ghost slot shows the stick');
  const shot = join(fixture.run, 'merequester-screen.png');
  await fixture.raw(`screenshot ${shot}`);
  const box = view.lines[2].state_box, [ox, oy] = view.origin;
  await fixture.raw(`click-at ${ox + box[0] + 7} ${oy + box[1] + 7}`);
  await until(status, v => v.rows[2].state === false, 'a click on the toggle switches row 2 off');
  screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.lines?.[2]?.on === false, 'the screen shows it off');
  // The amount field: a click focuses it, Enter submits what was typed.
  const amount = view.lines[0].amount_field;
  await fixture.raw(`click-at ${ox + amount[0] + 5} ${oy + amount[1] + 5}`);
  await until(() => fixture.raw('screen'), v => v.addon_screen?.focused?.text === '100', 'the amount field takes the focus');
  await fixture.raw('type 0');
  const shotField = join(fixture.run, 'merequester-screen-field.png');
  await fixture.raw(`screenshot ${shotField}`);
  await fixture.raw('ui-key enter');
  await until(status, v => v.rows[0].amount === 1000, 'Enter submits the amount');
  await fixture.ae(`requester set ${requester} 0 minecraft:stick 100 64`);
  await until(status, v => v.rows[0].status === 'idle' && v.rows[0].amount === 100, 'the row idles again');
  // A shift-click on a ghost slot clears its row (SHIFT_CLICK).
  await fixture.raw('click-slot 301 shift');
  await until(status, v => v.rows[1].key === null, 'shift-click clears the fluid row');
  const shot2 = join(fixture.run, 'merequester-screen-2.png');
  await fixture.raw(`screenshot ${shot2}`);
  await fixture.raw('close');
  fixture.mark(`the requester screen shows its rows; screenshots ${shot} and ${shot2}`);

  // A Memory Card with a requester's rows pastes them on another requester.
  const second = at(4);
  await fixture.command(`setblock ${second} merequester:requester`);
  await until(() => fixture.ae(`requester status ${second}`), v => v.online, 'a second requester on the grid', 30000);
  // Its front (north, the block's default) has no grid connection, its back does (`getExposedSides`).
  const front = at(4, 0, -1), back = at(4, 0, 1);
  await fixture.command(`setblock ${front} ae2:cable_bus`); await fixture.command(`setblock ${back} ae2:cable_bus`);
  await until(() => fixture.ae(`grid status ${back}`), v => v.online && v.powered, 'a cable on the back joins the grid', 30000);
  await Bun.sleep(1000);
  assert.equal((await fixture.ae(`grid status ${front}`)).powered, false, 'a cable on the front stays out');
  await fixture.command(`setblock ${front} minecraft:air`); await fixture.command(`setblock ${back} minecraft:air`);
  const card = 'ae2:memory_card[ae2:exported_settings_source={translate:"block.merequester.requester"},merequester:exported_requests=[{state:1b,key:{"#t":"ae2:i",id:"minecraft:stick"},amount:7L,batch:3L,client_status:"idle"}]]';
  await fixture.replace('hotbar.0', card); await fixture.select(0, 'ae2:memory_card');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${second} south`);
  s = await until(() => fixture.ae(`requester status ${second}`), v => v.rows[0].key?.id === 'minecraft:stick', 'the card pastes its rows');
  assert.deepEqual([s.rows[0].amount, s.rows[0].batch, s.rows[1].key], [7, 3, null]);
  fixture.mark('a Memory Card pastes a requester\'s rows');

  // The Requester Terminal lists both requesters under their one name.
  await fixture.replace('hotbar.0', 'merequester:requester_terminal'); await fixture.select(0, 'merequester:requester_terminal');
  assert.equal((await fixture.ae(`grid part ${host} south`)).attached, true);
  await fixture.replace('hotbar.0', 'minecraft:stick'); await fixture.select(0, 'minecraft:stick');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${host} south`);
  screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.kind === 'merequester:requester_terminal' && v.addon_screen.lines.length === 11, 'the terminal lists two requesters', 30000);
  assert.deepEqual([screen.addon_screen.lines[0].header, screen.addon_screen.lines[0].count], ['ME Requester', 2]);
  assert.equal(screen.addon_screen.lines[1].amount, 100);
  assert.equal(screen.addon_screen.lines[6].amount, 7, 'sorted by position');
  const shot3 = join(fixture.run, 'merequester-terminal.png');
  await fixture.raw(`screenshot ${shot3}`);
  // The search looks in names and keys; nothing found says so.
  assert.equal(screen.addon_screen.search_focused, true, 'the search has the focus');
  await fixture.raw('type nothing like this');
  screen = await until(() => fixture.raw('screen'), v => v.addon_screen?.lines?.length === 0, 'the search finds nothing');
  const shot5 = join(fixture.run, 'merequester-terminal-empty.png');
  await fixture.raw(`screenshot ${shot5}`);
  const [sx, sy] = screen.addon_screen.origin;
  await fixture.raw(`click-at ${sx + 110} ${sy + 8} right`);
  await until(() => fixture.raw('screen'), v => v.addon_screen?.lines?.length === 11, 'a right click clears the search');
  await fixture.raw('type stick');
  await until(() => fixture.raw('screen'), v => v.addon_screen?.lines?.length === 11, 'a key name matches');
  await fixture.raw('close');
  const shot4 = join(fixture.run, 'merequester-world.png');
  await fixture.raw(`screenshot ${shot4}`);
  fixture.mark(`the Requester Terminal groups the grid's requesters; screenshots ${shot3}, ${shot5} and ${shot4}`);
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; } finally { await fixture.close(); }
