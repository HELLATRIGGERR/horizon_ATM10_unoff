/** Applied Mekanistics (docs/appmek.md, slices 2 and 3): AE2's ME Interface and Pattern Provider as Mekanism's chemical
 * handler (`GenericStackChemicalStorage` over their generic slots, reached by Mekanism through the helpers of API 437),
 * and the ME Terminal drawing a chemical as Applied Mekanistics' `ChemicalRenderer` does, with the `mekanism` native
 * bundle and the `appmek` addon built in:
 *
 * - a Basic Chemical Tank set to output on every side, ejecting, pushes its hydrogen into an ME Interface; the
 *   interface, with no config, hands it to its grid, where a 1k ME Chemical Storage Cell keeps it;
 * - a Pattern Provider holding a processing pattern of 1000 mB hydrogen to a diamond pushes the hydrogen into the
 *   Basic Chemical Tank beside it when the ME Terminal orders a diamond (the job then waits for the diamond);
 * - the ME Terminal draws the hydrogen left and a second tank's chlorine each with its own icon in its tint (hydrogen white, chlorine yellow-green), not the type's icon: a screenshot
 *   (`appmek-terminal.png`, in the run folder; logs/shots/appmek-handlers after a pass).
 *
 * Amounts by `/mek probe` and `/ae2 grid status`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-appmek-handlers', 'server-thread', { addons: ['appmek'], devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
}

const H = 'mekanism:hydrogen', CL = 'mekanism:chlorine', MB = 2000, ASK = 1000;
const TANK = 'mekanism:basic_chemical_tank', CELL = 'appmek:chemical_storage_cell_1k';
const pattern = `ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"appmek:chemical",id:"${H}","#":${ASK}L}],sparseOutputs:[{"#t":"ae2:i",id:"minecraft:diamond","#":1L}]}]`;
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const amount = async (pos: string) => (await mek(`mek probe ${pos}`)).amount ?? 0;
const held = (view: any, id: string) => view.keys?.find((key: any) => key.type === 'appmek:chemical' && key.id === id)?.amount ?? 0;
const hydrogen = (view: any) => held(view, H);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 6, y = Math.min(Math.ceil(py) + 2, 78), z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -3)} ${at(6, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -2, -3)} ${at(6, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');

  // The grid: power, a drive with an empty 1k chemical cell, an interface south of the drive with tank A south of
  // it, a crafting CPU, a pattern provider with tank B east of it, a terminal north of the drive.
  const drive = at(1), iface = at(1, 0, 1), a = at(1, 0, 2), cpu = at(2), provider = at(3), b = at(4), term = at(1, 0, -1);
  for (const [pos, id] of [[at(0), 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [iface, 'ae2:interface'],
    [cpu, 'ae2:64k_crafting_storage'], [provider, 'ae2:pattern_provider'], [term, 'ae2:cable_bus'], [b, TANK]] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ${CELL}`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, s => s.online && s.powered, 'powered grid', 30000);

  // Tank A: hydrogen, every side an output (`DataType.OUTPUT`), the ejector on. It pushes into the interface.
  await fixture.command(`setblock ${a} ${TANK}`);
  await fixture.command(`data merge block ${a} {component_config:{config2:[I;4,4,4,4,4,4],eject2:1b},chemical_tanks:[{tank:0b,stored:{id:"${H}",amount:${MB}L}}]}`);
  const landed = await until(status, s => hydrogen(s) === MB, 'the tank\'s hydrogen reaches the grid through the interface', 60000);
  log.landed = landed.keys;
  assert.equal(await amount(a), 0, 'the tank pushed all of it');
  fixture.mark('a chemical tank ejecting into an ME Interface puts its hydrogen into the network');

  // Tank C beside the interface: chlorine (yellow-green, hydrogen's tint is white), for the terminal's screenshot.
  const c = at(2, 0, 1);
  await fixture.command(`setblock ${c} ${TANK}`);
  await fixture.command(`data merge block ${c} {component_config:{config2:[I;4,4,4,4,4,4],eject2:1b},chemical_tanks:[{tank:0b,stored:{id:"${CL}",amount:${MB}L}}]}`);
  await until(status, s => held(s, CL) === MB, 'the second tank chlorine reaches the grid through the same interface', 60000);
  assert.equal(await amount(c), 0, 'the second tank pushed all of it');

  // The provider's processing pattern; the terminal orders one diamond.
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${term} north`)).attached, true, 'terminal');
  await until(() => fixture.ae(`grid status ${provider}`), s => s.online && s.powered, 'online pattern provider', 30000);
  await fixture.replace('hotbar.1', pattern); await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 2.5}`); await fixture.raw(`interact ${provider} north`);
  const menu = await until(() => fixture.raw('screen'), v => v.slots?.some((slot: any) => slot.slot < 36 && slot.item === 'ae2:processing_pattern'), 'the processing pattern in the player inventory');
  await fixture.raw(`click-slot ${menu.slots.find((slot: any) => slot.slot < 36 && slot.item === 'ae2:processing_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), v => !v.counts?.['ae2:processing_pattern'], 'the processing pattern installed'); await fixture.raw('close');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z - 3.5}`);
  await fixture.raw(`interact ${term} north`);
  const craftable = (view: any) => view.slots?.find((slot: any) => slot.slot >= 300 && slot.slot < 345 && slot.item === 'minecraft:diamond');
  const screen = await until(() => fixture.raw('screen'), craftable, 'the diamond is craftable from hydrogen', 30000);
  await fixture.raw(`click-slot ${craftable(screen).slot} middle`);
  await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Amount'), 'craft amount'); await fixture.raw('click-slot 302');
  const confirm = await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(v)), 'craft confirmation', 30000);
  assert.doesNotMatch(JSON.stringify(confirm), /Missing materials|craft planning:/);
  await fixture.raw('click-slot 302'); await fixture.raw('close');
  await until(() => amount(b), v => v === ASK, 'the pattern provider pushes the pattern\'s hydrogen into tank B', 60000);
  await until(status, s => hydrogen(s) === MB - ASK, 'the grid gave it', 30000);
  fixture.mark('a Pattern Provider pushes a processing pattern\'s hydrogen into a chemical tank');

  // The terminal draws the hydrogen and chlorine with their own sprites.
  await fixture.raw(`interact ${term} north`);
  const entries = (view: any) => (view.ae2_terminal?.entries ?? []).filter((e: any) => e.fluid);
  const shown = await until(() => fixture.raw('screen'), v => entries(v).length >= 2, 'the ME Terminal lists the hydrogen and the chlorine', 30000);
  log.entry = entries(shown);
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'appmek-terminal.png');
  await fixture.raw(`screenshot ${shot}`);
  console.log(`terminal: ${JSON.stringify(log.entry)} screenshot ${shot}`);
  await fixture.raw('close');
  fixture.mark('the ME Terminal shows the hydrogen (screenshot appmek-terminal.png)');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) {
  fixture.report('failed', error); console.error(JSON.stringify(log).slice(0, 3000)); throw error;
} finally { await fixture.close(); }
