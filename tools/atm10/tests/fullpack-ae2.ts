/** AE2's basics in the full pack (docs/atm10-port.md, "The full pack"): a world with every carry beside AE2's content
 * and its addons, where a player
 *
 * - `cable`: in creative mode takes a fluix glass cable from the creative inventory (AE2's tab when its first screen
 *   shows it, else the search tab) and uses it on the top of a stone block: an `ae2:cable_bus` stands on that block;
 * - `survival-cable`: in survival, with cables from `/give`, does the same;
 * - `network`: places a creative energy cell, a controller, a cable and a drive by using their items on the floor,
 *   puts a 1k item cell in the drive through its screen, puts a terminal on the cable, opens it and stores iron ingots
 *   with a shift-click;
 * - `parts`: puts an import bus and an export bus on two faces of a cable and a stone facade on a third, by use.
 *
 * A step that fails says what it saw in `fullpack-ae2.json`; `network` also goes on past a cell that didn't reach the
 * drive (the cell goes in by command) to try the terminal, and then fails.
 *
 * Each placement by use also wants the player's arm to swing on this client (`state`'s `swings`), as vanilla's
 * `Minecraft.startUseItem` swings for a use that returns SUCCESS: AE2's cables, parts and facades as its blocks.
 *
 *   bun run tools/atm10/tests/fullpack-ae2.ts [step...]   (default: every step)
 *
 * - ATM10_CARRIES: the carries folder `final-data` wrote. PACK_START_ONLY / PACK_START_WITHOUT: carry folder names
 *   to keep / leave out, as for pack-start.ts (`PACK_START_ONLY=none`: no carry), to find what breaks a step.
 *   PACK_START_ADDONS=0: no AE2 addon.
 * - AE2_TEST_BINARY, AE2_RESOURCES, AE2_CONTENT_DIR, AE2_ADDON_CONTENT_ROOT as for AE2's world suites;
 *   FULLPACK_AE2_MODE=in-frame: the window ticks the world itself (default: on its server thread, as players run).
 * - `fullpack-ae2.json` in the fixture's run folder (printed) holds each step's result and what was seen when one
 *   failed; it is written after every step.
 *
 * One full-pack game at a time: hold the machine's full-pack lock. */
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../ae2/tests/world/harness';
import { addonContents } from '../../../ae2/scripts/common';

const carriesDir = resolve(process.env.ATM10_CARRIES ?? '');
const list = (name: string) => (process.env[name] ?? '').split(',').map(s => s.trim()).filter(Boolean);
const only = list('PACK_START_ONLY'), without = new Set(list('PACK_START_WITHOUT'));
const carries = only.includes('none') ? [] : readdirSync(carriesDir, { withFileTypes: true })
  .filter(d => d.isDirectory() && existsSync(join(carriesDir, d.name, 'horizon-mod.toml')))
  .map(d => d.name)
  .sort()
  .filter(id => (only.length ? only.includes(id) : true) && !without.has(id));
const addons = process.env.PACK_START_ADDONS === '0' ? [] : addonContents().filter(a => existsSync(a.path)).map(a => a.id);
process.env.AE2_TEST_STARTUP_MS = String(Math.max(Number(process.env.AE2_TEST_STARTUP_MS ?? 0), 1_800_000));
const steps = process.argv.slice(2).length ? process.argv.slice(2) : ['cable', 'survival-cable', 'network', 'parts'];

const mode = process.env.FULLPACK_AE2_MODE === 'in-frame' ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('fullpack-ae2', mode, { addons, devMods: carries.map(id => join(carriesDir, id)) });
const out = join(fixture.run, 'fullpack-ae2.json');
const results: Record<string, any> = { carries: carries.length, addons, steps: {} };
const save = () => writeFileSync(out, JSON.stringify(results, null, 2) + '\n');
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => line.includes('Test passed'));
/** The server's block at `pos` is `state`. */
const isBlock = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
/** What a failed step saw: the hand, the screen, the client's and the server's block, the game's last log lines. */
async function seen(pos?: string): Promise<Record<string, unknown>> {
  const view: Record<string, unknown> = {};
  try { const inv = await fixture.raw('inventory'); view.hotbar = inv.hotbar?.slice(0, 3); view.carried = inv.carried; } catch (e) { view.inventory = String(e); }
  try { const s = await fixture.raw('status'); view.creative = s.creative; view.selected = s.selected; view.position = s.player?.position; view.screen = s.screen; } catch (e) { view.status = String(e); }
  if (pos) {
    try { view.client_block = (await fixture.raw(`block ${pos}`)).block; } catch (e) { view.client_block = String(e); }
    try { view.server_air = await isBlock(pos, 'minecraft:air'); } catch (e) { view.server_air = String(e); }
  }
  const log = join(fixture.run, 'game-1.log');
  if (existsSync(log)) {
    view.log = readFileSync(log, 'utf8').split(/\r?\n/).filter(l => /ae2|cable|refus|cancel|denied|protect|claim|place/i.test(l)).slice(-25);
  }
  return view;
}
/** Closes what a step that failed left open, so the next starts in the world. */
async function closeScreen() {
  for (let i = 0; i < 3; i++) {
    const view = await fixture.raw('screen').catch(() => ({}));
    if (!view.screen || view.screen === 'playing') return;
    await fixture.raw('close').catch(() => {});
  }
}
async function step(name: string, run: (r: Record<string, any>) => Promise<void>) {
  if (!steps.includes(name)) return;
  const r: Record<string, any> = (results.steps[name] = { passed: false });
  try {
    await closeScreen();
    await run(r);
    const still = Object.entries(r.swung ?? {}).filter(([, n]) => !n).map(([label]) => label);
    if (still.length) throw new Error(`the arm did not swing for: ${still.join(', ')}`);
    r.passed = true; fixture.mark(`${name}: ${r.what ?? 'passed'}`);
  }
  catch (error) { r.error = String(error).slice(0, 2000); console.log(`FAIL ${name}: ${r.error}`); }
  save();
}
/** Puts `item` in hotbar slot `slot` (0-8) from the creative inventory, as a player does: the inventory, the mod's
 * tab that shows it (or the search tab and `search` typed), a click on it, a click on the hotbar slot. Returns
 * whether it went; `false` with what was seen when the item could not be found. */
async function fromCreativeTab(item: string, search: string, slot: number, r: Record<string, any>): Promise<boolean> {
  await fixture.raw('open-inventory');
  const creative = await until(() => fixture.raw('screen'), v => v.screen === 'creative' && v.creative_tabs?.length, 'creative inventory');
  const tabs = creative.creative_tabs.filter((t: any) => String(t.id).startsWith(item.split(':')[0] + ':') || t.id === 'minecraft:search');
  tabs.sort((a: any, b: any) => Number(a.id === 'minecraft:search') - Number(b.id === 'minecraft:search'));
  r.tabs = tabs.map((t: any) => t.id);
  for (const tab of tabs) {
    // NeoForge's page buttons, above the panel's left and right corners, when the mods' tabs fill more pages.
    let page = await fixture.raw('screen');
    for (let i = 0; page.creative_page !== tab.page && i < 20; i++) {
      const from = page.creative_page, next = from < tab.page;
      await fixture.raw(`click-at ${page.origin[0] + (next ? 185 : 10)} ${page.origin[1] - 40}`);
      page = await until(() => fixture.raw('screen'), v => v.creative_page !== from, `creative page ${tab.page + 1}`);
    }
    const shown = (page.creative_tabs ?? []).find((t: any) => t.id === tab.id) ?? tab;
    await fixture.raw(`click-at ${shown.rect[0] + shown.rect[2] / 2} ${shown.rect[1] + shown.rect[3] / 2}`);
    let view = await until(() => fixture.raw('screen'), v => v.creative_tab === tab.id, `the ${tab.id} tab`);
    if (tab.id === 'minecraft:search') {
      await fixture.raw(`type ${search}`);
      try { view = await until(() => fixture.raw('screen'), v => (v.slots ?? []).some((s: any) => s.item === item), `${item} found by "${search}"`); }
      catch { r.search_screen = JSON.stringify(await fixture.raw('screen')).slice(0, 2000); }
    }
    const found = (view.slots ?? []).find((s: any) => s.item === item);
    if (!found) continue;
    r.tab = tab.id; r.slot = found;
    await fixture.raw(found.rect ? `click-at ${found.rect[0] + found.rect[2] / 2} ${found.rect[1] + found.rect[3] / 2}` : `click-slot ${found.slot}`);
    await until(() => fixture.raw('inventory'), v => v.carried?.item === item, `${item} on the cursor`);
    await fixture.raw(`click-at ${view.origin[0] + 17 + slot * 18} ${view.origin[1] + 120}`);
    await until(() => fixture.raw('inventory'), v => !v.carried && v.hotbar?.[slot]?.item === item, `${item} in hotbar slot ${slot + 1}`);
    await fixture.raw('close');
    return true;
  }
  r.last_screen = JSON.stringify(await fixture.raw('screen')).slice(0, 3000);
  await fixture.raw('close');
  return false;
}
/** The arm swings this client has started (`LivingEntity.swing`). */
const swings = async () => Number((await fixture.raw('state')).swings ?? 0);
/** Uses the held item on `face` of the block at `target`, waits for `placed()` on the server, and keeps in
 * `r.swung[label]` how many times this client's arm swung for it. */
async function useOn(target: string, face: string, placed: () => Promise<boolean>, r: Record<string, any>, label: string,
  seenAt = target) {
  const before = await swings();
  await fixture.raw(`interact ${target} ${face}`);
  try { await until(placed, v => v, `${label} placed on ${face} of ${target}`, 10000); }
  catch (error) { r.seen = await seen(seenAt); throw error; }
  let after: number;
  try { after = await until(swings, n => n > before, `${label}: the arm swings`, 3000); } catch { after = await swings(); }
  (r.swung ??= {})[label] = after - before;
}
/** Uses the held item on the top of the floor block under `pos`, and waits for `state` at `pos` on the server. */
async function useOnFloor(pos: string, floor: string, state: string, r: Record<string, any>, label: string) {
  await useOn(floor, 'up', () => isBlock(pos, state), r, label, pos);
}
/** The parts this client draws on the cable bus at `pos`, as `face=id`. */
const partsAt = async (pos: string) => ((await fixture.raw(`block ${pos}`)).ae2?.parts ?? []).map((p: any) => `${p.face}=${p.id}`);

try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  console.log(`started with ${carries.length} carries and ${addons.length} addons; results ${out}`);
  const x = Math.floor(px) + 6, y = Math.min(Math.floor(py) + 2, 120), z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-4, -4], [-4, 8], [8, -4], [8, 8]] as const) {
    await until(async () => passed(await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)), v => v, 'fixture corner loaded', 60000);
  }
  await fixture.command(`fill ${at(-4, 0, -4)} ${at(8, 4, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-4, -1, -4)} ${at(8, -1, 8)} minecraft:stone`);
  await fixture.command('clear @s');

  await step('cable', async r => {
    await fixture.command('gamemode creative @s');
    await fixture.command(`tp @s ${x + 1.5} ${y} ${z - 2.5} 0 30`);
    r.creative_tab = await fromCreativeTab('ae2:fluix_glass_cable', 'fluix me glass', 0, r);
    if (!r.creative_tab) await fixture.replace('hotbar.0', 'ae2:fluix_glass_cable');
    await fixture.select(0, 'ae2:fluix_glass_cable');
    await useOnFloor(at(0), at(0, -1), 'ae2:cable_bus', r, 'the creative cable');
    r.what = `a fluix glass cable ${r.creative_tab ? `from the creative inventory (${r.tab})` : 'given (not found in the creative inventory)'} placed ae2:cable_bus in creative`;
  });

  await step('survival-cable', async r => {
    await fixture.command('clear @s');
    await fixture.command('gamemode survival @s');
    await fixture.command(`tp @s ${x + 1.5} ${y} ${z - 2.5} 0 30`);
    await fixture.command('give @s ae2:fluix_glass_cable 8');
    await until(() => fixture.raw('inventory'), v => (v.counts?.['ae2:fluix_glass_cable'] ?? 0) === 8, 'eight cables given');
    await fixture.select(0, 'ae2:fluix_glass_cable');
    await useOnFloor(at(2), at(2, -1), 'ae2:cable_bus', r, 'the survival cable');
    r.cables_left = (await fixture.raw('inventory')).counts?.['ae2:fluix_glass_cable'];
    r.what = `a given fluix glass cable placed ae2:cable_bus in survival (${r.cables_left} left)`;
  });

  await step('network', async r => {
    // A row four blocks south of the cables: energy cell, controller, cable, drive; the player north of it.
    await fixture.command('clear @s');
    await fixture.command('gamemode creative @s');
    await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 1.5} 0 30`);
    const row = (dx: number) => at(dx, 0, 4), floor = (dx: number) => at(dx, -1, 4);
    const placed: Record<string, boolean> = (r.placed = {});
    for (const [dx, item, state] of [[0, 'ae2:creative_energy_cell', 'ae2:creative_energy_cell'], [1, 'ae2:controller', 'ae2:controller'],
      [2, 'ae2:fluix_glass_cable', 'ae2:cable_bus'], [3, 'ae2:drive', 'ae2:drive']] as const) {
      await fixture.replace('hotbar.0', item); await fixture.select(0, item);
      await useOnFloor(row(dx), floor(dx), state, r, item);
      placed[item] = true;
    }
    // The drive's screen, and the cell shift-clicked into it from the hotbar.
    await fixture.replace('hotbar.1', 'ae2:item_storage_cell_1k'); await fixture.select(1, 'ae2:item_storage_cell_1k');
    await fixture.raw(`interact ${row(3)} north`);
    const drive = await until(() => fixture.raw('screen'), v => (v.slots ?? []).some((s: any) => s.item === 'ae2:item_storage_cell_1k'), 'the drive screen');
    r.drive_title = drive.title;
    const cell = drive.slots.filter((s: any) => s.item === 'ae2:item_storage_cell_1k').at(-1);
    await fixture.raw(`click-slot ${cell.slot} shift`);
    await until(() => fixture.raw('inventory'), v => !(v.counts?.['ae2:item_storage_cell_1k']), 'the cell leaves the hotbar');
    await fixture.raw('close');
    // The drive holds it, as this client draws the drive. When it doesn't, the cell goes in by command so the
    // terminal is still tried, and the step fails at its end.
    const problems: string[] = (r.problems = []);
    const driveCells = async () => ((await fixture.raw(`block ${row(3)}`)).ae2?.cells ?? []).filter(Boolean).map((c: any) => c.item);
    try { await until(driveCells, cells => cells.includes('ae2:item_storage_cell_1k'), 'the cell in the drive', 10000); }
    catch {
      problems.push(`the cell shift-clicked in the drive's screen left the hotbar but the drive shows ${JSON.stringify(await driveCells())}`);
      await fixture.command(`item replace block ${row(3)} container.0 with ae2:item_storage_cell_1k`);
    }
    const status = await until(() => fixture.ae(`grid status ${row(3)}`), s => s.online && s.powered, 'the network is online and powered', 30000);
    r.grid = { online: status.online, powered: status.powered };
    // The terminal on the cable's north face, opened, and iron stored with a shift-click.
    await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
    await useOn(row(2), 'north', async () => (await partsAt(row(2))).includes('north=ae2:terminal'), r, 'ae2:terminal');
    await fixture.replace('hotbar.0', 'minecraft:iron_ingot', 16); await fixture.select(0, 'minecraft:iron_ingot');
    const openTerminal = async () => {
      try {
        await fixture.raw(`interact ${row(2)} north`);
        return await until(() => fixture.raw('screen'), v => /Terminal/i.test(JSON.stringify(v.title ?? '')), 'the terminal opens', 15000);
      } catch (error) { r.seen = await seen(row(2)); throw error; }
    };
    const ironStored = async (wait: number) => {
      try {
        const s = await until(() => fixture.ae(`grid status ${row(3)}`),
          s => (s.items ?? []).some((i: any) => i.item === 'minecraft:iron_ingot' && i.count === 16), 'sixteen iron ingots stored', wait);
        return s.items;
      } catch { return undefined; }
    };
    // A shift-click on the iron in the player's slots, in creative, then (when nothing went in) in survival.
    for (const mode of ['creative', 'survival']) {
      await fixture.command(`gamemode ${mode} @s`);
      const terminal = await openTerminal();
      r.terminal_title = terminal.title;
      const iron = terminal.slots.find((s: any) => s.item === 'minecraft:iron_ingot' && s.slot < 36);
      await fixture.raw(`click-slot ${iron.slot} shift`);
      r.stored = await ironStored(mode === 'creative' ? 10000 : 30000);
      r[`after_${mode}_click`] = { stored: !!r.stored, player_iron: (await fixture.raw('inventory')).counts?.['minecraft:iron_ingot'] ?? 0 };
      await fixture.raw('close');
      if (r.stored) break;
      problems.push(`a shift-click on 16 iron ingots in the terminal stored nothing in ${mode}`);
    }
    if (!r.stored) {
      // What the network takes without the terminal: the held iron deposited into the drive's grid.
      await fixture.select(0, 'minecraft:iron_ingot');
      try { r.deposit = await fixture.ae(`grid deposit ${row(3)}`); } catch (error) { r.deposit = String(error).slice(0, 500); }
      r.after_deposit = await fixture.ae(`grid status ${row(3)}`).catch((error: unknown) => String(error).slice(0, 500));
      throw new Error(problems.join('; '));
    }
    if (problems.length) throw new Error(problems.join('; '));
    r.what = 'energy cell, controller, cable and drive placed by use, a cell put in the drive, the terminal opened and 16 iron stored';
  });

  await step('parts', async r => {
    // A cable on the cleared floor where the first cables stood, and the player square in front of each face it
    // uses, two blocks out on a block's centre (a thin cable shows a face only to an eye in front of it; `tp` to a
    // whole number stands on the centre): an import bus on its north face, an export bus on its west face, a stone
    // facade on its south face.
    await fixture.command('clear @s');
    await fixture.command('gamemode creative @s');
    await fixture.command(`fill ${at(-4, 0, -4)} ${at(8, 4, 2)} minecraft:air`);
    let standing = '';
    const stand = async (dx: number, dz: number) => {
      const to = `${x + 4.5 + dx} ${y} ${z + 0.5 + dz}`;
      if (to !== standing) await fixture.command(`tp @s ${to}`);
      standing = to;
    };
    await stand(0, -2);
    await fixture.replace('hotbar.0', 'ae2:fluix_glass_cable'); await fixture.select(0, 'ae2:fluix_glass_cable');
    await useOnFloor(at(4), at(4, -1), 'ae2:cable_bus', r, 'ae2:fluix_glass_cable');
    for (const [item, face, dx, dz] of [['ae2:import_bus', 'north', 0, -2], ['ae2:export_bus', 'west', -2, 0]] as const) {
      await stand(dx, dz);
      await fixture.replace('hotbar.0', item); await fixture.select(0, item);
      await useOn(at(4), face, async () => (await partsAt(at(4))).includes(`${face}=${item}`), r, item);
    }
    await stand(0, 2);
    await fixture.replace('hotbar.0', 'ae2:facade[ae2:facade_item="minecraft:stone"]'); await fixture.select(0, 'ae2:facade');
    const facades = async () => JSON.stringify((await fixture.command(`data get block ${at(4)} ae2_lab_config.facades`)).chat ?? []);
    await useOn(at(4), 'south', async () => /south/.test(await facades()) && /minecraft:stone/.test(await facades()), r, 'ae2:facade');
    r.parts = await partsAt(at(4));
    r.what = `import and export buses and a stone facade put on a cable by use (${r.parts.join(', ')}), the arm swinging for each`;
  });

  const failed = steps.filter(s => !results.steps[s]?.passed);
  results.status = failed.length ? 'failed' : 'passed';
  save();
  if (failed.length) throw new Error(`failed: ${failed.join(', ')} (${out})`);
  fixture.report('passed');
} catch (error) { results.status = 'failed'; results.error = String(error).slice(0, 2000); save(); fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
