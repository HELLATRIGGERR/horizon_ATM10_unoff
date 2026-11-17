/** Applied Mekanistics (docs/appmek.md, slice 1): AE2's buses move Mekanism's chemicals through the chemical handler
 * of hook points API 435, as Applied Mekanistics 1.6.3's strategies do (AE2's fluid strategies over
 * `IChemicalHandler`), with the `mekanism` native bundle and the `appmek` addon built in:
 *
 * - a Storage Bus on a Basic Chemical Tank mounts it: what an Import Bus takes from another tank goes into it, and
 *   the grid lists it (`appmek:chemical`, `mekanism:hydrogen`, in mB);
 * - an Export Bus filtered to hydrogen takes it back out through the Storage Bus into the other tank;
 * - an Import Bus moves the hydrogen into a 1k ME Chemical Storage Cell; the ME Terminal lists it; it stays through a
 *   save and restart;
 * - an Export Bus gives it back to the first tank and the cell is empty.
 *
 * A tank's sides take in only by default (`FRONT_OUT_EJECT`), so neither bus could take anything out, as in Mekanism:
 * both tanks are set to input and output on every side, eject off. Amounts by `/mek probe`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-appmek-chemicals', 'server-thread', { addons: ['appmek'], devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const H = 'mekanism:hydrogen', MB = 2000;
const TANK = 'mekanism:basic_chemical_tank', CELL = 'appmek:chemical_storage_cell_1k';
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const amount = async (pos: string) => (await mek(`mek probe ${pos}`)).amount ?? 0;
const held = (mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${H}",amount:${mb}L}}]}`;
const hydrogen = (view: any) => view.keys?.find((key: any) => key.type === 'appmek:chemical' && key.id === H)?.amount ?? 0;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.min(Math.ceil(py) + 2, 78), z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -3)} ${at(6, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -2, -3)} ${at(6, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
  /** Puts `part` on side `face` of the cable bus at `cable` (the held part, `/ae2 grid part`). */
  const part = async (cable: string, face: string, part: string) => {
    await fixture.replace('hotbar.0', part); await fixture.select(0, part);
    assert.equal((await fixture.ae(`grid part ${cable} ${face}`)).attached, true, `${part} on ${face} of ${cable}`);
  };
  /** A fresh cable bus at `pos`, its parts gone. */
  const bare = async (pos: string) => { await fixture.command(`setblock ${pos} minecraft:air`); await fixture.command(`setblock ${pos} ae2:cable_bus`); };

  // The grid: power, a drive (no cell yet), the cable bus P with Q above it. Tank A east of P, tank B east of Q.
  const drive = at(1), p = at(2), q = at(2, 1), a = at(3), b = at(3, 1);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`); await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`setblock ${p} ae2:cable_bus`); await fixture.command(`setblock ${q} ae2:cable_bus`);
  await fixture.command(`setblock ${a} ${TANK}`); await fixture.command(`setblock ${b} ${TANK}`);
  for (const tank of [a, b]) await fixture.command(`data merge block ${tank} {component_config:{config2:[I;7,7,7,7,7,7],eject2:0b}}`);
  await fixture.command(`data merge block ${b} ${held(MB)}`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, s => s.online && s.powered, 'powered grid', 30000);
  assert.equal(await amount(b), MB);

  // Storage Bus on A, Import Bus on B: B's hydrogen goes through the grid into A.
  await part(p, 'east', 'ae2:storage_bus');
  assert.equal(hydrogen(await status()), 0, 'an empty tank lists nothing');
  await part(q, 'east', 'ae2:import_bus');
  const stored = await until(status, s => hydrogen(s) === MB, 'the storage bus lists the hydrogen the import bus moved', 60000);
  log.stored = stored.keys;
  assert.deepEqual([await amount(a), await amount(b)], [MB, 0]);
  fixture.mark('an Import Bus takes hydrogen from a chemical tank; a Storage Bus puts it into another and lists it');

  // Export Bus on B, filtered to hydrogen: out of A through the Storage Bus, into B. The cell goes in first.
  await fixture.command(`item replace block ${drive} container.0 with ${CELL}`);
  await bare(q);
  await part(q, 'east', 'ae2:export_bus');
  await fixture.ae(`bus filter_key ${q} east 0 appmek:chemical|${H}`);
  await until(async () => [await amount(a), await amount(b)], v => v[0] === 0 && v[1] === MB, 'the export bus moves A\'s hydrogen into B', 60000);
  assert.equal(hydrogen(await status()), 0);
  fixture.mark('an Export Bus filtered to hydrogen takes it out of a tank through a Storage Bus into another');

  // No Storage Bus now; an Import Bus on B fills the 1k chemical cell.
  await bare(p); await bare(q);
  await part(q, 'east', 'ae2:import_bus');
  await until(status, s => hydrogen(s) === MB, 'the import bus moves B\'s hydrogen into the cell', 60000);
  assert.deepEqual([await amount(a), await amount(b)], [0, 0]);
  const saved = (await fixture.command(`data get block ${drive}`)).chat?.join('\n') ?? '';
  log.drive = saved;
  assert.match(saved, /appmek:chemical_storage_cell_1k/);
  assert.match(saved, /mekanism:hydrogen/);
  fixture.mark('an Import Bus moves hydrogen into a 1k ME Chemical Storage Cell');

  // The ME Terminal lists it.
  await part(p, 'north', 'ae2:terminal');
  await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  await fixture.raw(`interact ${p} north`);
  // A chemical is drawn with its type's icon until slice 3 draws the chemical's own sprite.
  const listed = (view: any) => view.ae2_terminal?.entries?.find((entry: any) => entry.fluid && entry.item === TANK);
  const screen = await until(() => fixture.raw('screen'), listed, 'the ME Terminal lists the hydrogen', 30000);
  log.terminal = listed(screen);
  assert.equal(screen.ae2_terminal.total, 1, 'the hydrogen alone');
  console.log(`terminal slot: ${JSON.stringify(log.terminal)}`);
  await fixture.raw('close');
  fixture.mark('an ME Terminal lists the hydrogen the cell holds');

  await fixture.restart();
  await until(status, s => s.online && s.powered && hydrogen(s) === MB, 'the cell keeps its hydrogen through a restart', 60000);
  fixture.mark('the chemical cell keeps its hydrogen through save and load');

  // An Export Bus on A gives it back.
  await bare(q);
  await part(p, 'east', 'ae2:export_bus');
  await fixture.ae(`bus filter_key ${p} east 0 appmek:chemical|${H}`);
  await until(() => amount(a), v => v === MB, 'the export bus gives the cell\'s hydrogen to A', 60000);
  await until(status, s => hydrogen(s) === 0, 'the cell is empty', 30000);
  fixture.mark('an Export Bus gives the cell\'s hydrogen back to a chemical tank');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); console.error(JSON.stringify(log).slice(0, 3000)); throw error;
} finally { await fixture.close(); }
