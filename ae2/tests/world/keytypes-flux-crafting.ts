/** An addon's key type through crafting, with Applied Flux's FE (`appflux:flux`) as the type: a processing pattern of
 * 100,000 FE into a diamond in AE2's form (`ae2:encoded_processing_pattern`, the FE input with `"#t":"appflux:flux"`)
 * loads into the Pattern Encoding Terminal, which shows the FE ghost and encodes it again onto a blank pattern; a
 * Pattern Provider part takes the new one, the ME Terminal requests the diamond, the crafting CPU reserves the FE from
 * an FE cell into its escrow (kept through a restart, the type's id in `#t`), the provider sends the FE through the
 * type's storage strategy into another grid's Energy Acceptor once there is one, and the diamond given back finishes
 * the job. A wrapped stack cannot be carried (AE2's `WrappedGenericStack`) and FE has no container item strategy yet,
 * so a loaded pattern is how the terminal gets an FE ghost. Needs content/appflux (ae2/scripts/prepare-addon.ts
 * appflux) and a client built with ae2-appflux. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('keytypes-flux-crafting', mode, { addons: ['appflux'] });
const fe = (view: any) => view.keys?.find((key: any) => key.type === 'appflux:flux' && key.id === 'appflux:fe')?.amount ?? 0;
const diamonds = (view: any) => (view.items ?? []).filter((entry: any) => entry.item === 'minecraft:diamond').reduce((sum: number, entry: any) => sum + entry.count, 0);
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
const text = (reply: any) => reply.chat?.join('\n') ?? '';
const START = 1_000_000, INPUT = 100_000;
const source = `ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"appflux:flux",type:"FE","#":${INPUT}L}],sparseOutputs:[{"#t":"ae2:i",id:"minecraft:diamond","#":1L}]}]`;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 8, y = Math.ceil(py) + 4, z = Math.floor(pz) + 8;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -4)} ${at(6, 4, 5)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -2, -4)} ${at(6, -1, 5)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');

  // Grid A: power, a drive with a 1k FE cell of 1,000,000 FE and an item cell, a cable bus (the ME Terminal north,
  // the encoding terminal south, the Pattern Provider part east) and a crafting storage above it.
  const drive = at(1), cable = at(2), cpu = at(2, 1), acceptor = at(3), battery = at(3, 0, 1);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`); await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`setblock ${cable} ae2:cable_bus`); await fixture.command(`setblock ${cpu} ae2:64k_crafting_storage`);
  await fixture.command(`item replace block ${drive} container.0 with appflux:fe_1k_cell[appflux:fe_energy=${START}L]`);
  await fixture.command(`item replace block ${drive} container.1 with ae2:item_storage_cell_1k`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, s => s.online && s.powered && fe(s) === START, 'grid A lists its FE', 30000);
  for (const [part, face] of [['ae2:terminal', 'north'], ['ae2:pattern_encoding_terminal', 'south'], ['ae2:cable_pattern_provider', 'east']]) {
    await fixture.replace('hotbar.0', part); await fixture.select(0, part);
    assert.equal((await fixture.ae(`grid part ${cable} ${face}`)).attached, true, `${part} on the ${face} face`);
  }

  // The encoding terminal loads the pattern: an FE input ghost and a diamond output; encoded again, the new pattern
  // names the FE by its type.
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.0', source); await fixture.replace('hotbar.2', 'ae2:blank_pattern');
  await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 2.5}`); await fixture.raw(`interact ${cable} south`);
  await until(() => fixture.raw('screen'), view => String(slot(view, 346)?.components ?? '').includes('Mode: crafting'), 'encoding terminal opens');
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), view => view.carried?.item === 'ae2:processing_pattern', 'the FE pattern on the cursor');
  await fixture.raw('click-slot 350');
  await until(() => fixture.raw('screen'), view => slot(view, 300)?.item === 'ae2:wrapped_generic_stack' && String(slot(view, 300)?.components ?? '').includes('appflux:flux')
    && slot(view, 318)?.item === 'minecraft:diamond', 'the loaded pattern: an FE input ghost, a diamond output');
  await fixture.raw('click-slot 350'); await fixture.raw('click-slot 0');
  await until(() => fixture.raw('inventory'), view => !view.carried && view.counts?.['ae2:processing_pattern'] === 1, 'the loaded pattern back in the inventory');
  await fixture.command('clear @s ae2:processing_pattern');
  await fixture.raw('click-slot 2'); await fixture.raw('click-slot 349');
  await until(() => fixture.raw('screen'), view => slot(view, 349)?.item === 'ae2:blank_pattern', 'blank pattern inserted');
  await fixture.raw('click-slot 313');
  const encoded = await until(() => fixture.raw('screen'), view => slot(view, 350)?.item === 'ae2:processing_pattern', 'an encoded FE processing pattern');
  const components = String(slot(encoded, 350)?.components ?? '');
  assert.match(components, /ae2:encoded_processing_pattern/); assert.match(components, /appflux:flux/); assert.match(components, new RegExp(`${INPUT}`));
  await fixture.raw('click-slot 350'); await fixture.raw('click-slot 2');
  await until(() => fixture.raw('inventory'), view => !view.carried && view.counts?.['ae2:processing_pattern'] === 1, 'the new pattern in the inventory');
  await fixture.raw('close');
  fixture.mark('the Pattern Encoding Terminal loads an FE processing pattern and encodes it again, the FE saved with its type in #t');

  // The provider part takes the pattern.
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 0.5}`); await fixture.raw(`interact ${cable} east`);
  const menu = await until(() => fixture.raw('screen'), view => /Pattern Provider/i.test(JSON.stringify(view.title ?? '')) && view.slots?.some((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern'), 'provider menu');
  await fixture.raw(`click-slot ${menu.slots.find((s: any) => s.slot < 36 && s.item === 'ae2:processing_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), view => !view.counts?.['ae2:processing_pattern'], 'pattern installed'); await fixture.raw('close');

  // The ME Terminal requests one diamond: the plan takes the FE from the cell, and the CPU holds it while the
  // provider has no target.
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`); await fixture.raw(`interact ${cable} north`);
  const screen = await until(() => fixture.raw('screen'), view => view.slots?.some((s: any) => s.slot >= 300 && s.slot < 345 && s.item === 'minecraft:diamond'), 'the diamond is craftable');
  await fixture.raw(`click-slot ${screen.slots.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === 'minecraft:diamond').slot} middle`);
  await until(() => fixture.raw('screen'), view => JSON.stringify(view.title ?? '').includes('AE2 Craft Amount'), 'craft amount'); await fixture.raw('click-slot 302');
  const confirm = await until(() => fixture.raw('screen'), view => JSON.stringify(view.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(view)), 'craft confirmation', 90000);
  assert.doesNotMatch(JSON.stringify(confirm), /Missing materials|craft planning:/);
  await fixture.raw('click-slot 302'); await fixture.raw('close');
  await until(status, s => fe(s) === START - INPUT, 'the CPU reserves 100,000 FE', 60000);
  const escrow = async () => text(await fixture.command(`data get block ${cpu} ae2_lab_config.ae2_cpu_fluid_escrow`));
  const held = await escrow();
  assert.match(held, /appflux:flux/); assert.match(held, /appflux:fe/); assert.match(held, new RegExp(`${INPUT}L`));
  fixture.mark('the crafting CPU reserves the FE input into its escrow, its type in #t');
  await fixture.restart();
  await until(status, s => s.online && s.powered && fe(s) === START - INPUT, 'grid A after the restart', 60000);
  assert.match(await escrow(), /appflux:flux/, 'the escrow survives the restart');
  fixture.mark('the FE escrow survives save and load');

  // Grid B's acceptor on the provider's face: the provider sends the FE through Applied Flux's storage strategy.
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`); await fixture.command(`setblock ${battery} ae2:energy_cell`);
  const other = () => fixture.ae(`grid status ${battery}`);
  await until(other, s => s.nodes === 2, 'grid B, the acceptor and its cell', 30000);
  const charged = await until(other, s => s.energy >= INPUT / 2 - 1000, 'the provider sends the FE into grid B', 60000);
  assert(charged.energy <= INPUT / 2 + 10, `the 100,000 FE and no more: ${charged.energy} AE`);
  assert.doesNotMatch(await escrow(), /appflux:flux/, 'the escrow is spent');
  assert.equal(fe(await status()), START - INPUT, 'grid A keeps the rest');
  fixture.mark(`the Pattern Provider sends the CPU's FE to another grid's Energy Acceptor (${charged.energy} AE)`);

  // The diamond comes back: the job ends with it in grid A's storage.
  await fixture.replace('hotbar.1', 'minecraft:diamond'); await fixture.select(1, 'minecraft:diamond');
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 1);
  await until(status, s => diamonds(s) === 1, 'the job delivers its diamond to storage', 60000);
  fixture.mark('the returned diamond finishes the FE job');

  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; } finally { await fixture.close(); }
