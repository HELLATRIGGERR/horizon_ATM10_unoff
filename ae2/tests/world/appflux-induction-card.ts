/** Applied Flux's Induction Card (`ItemInductionCard`, `EnergyTicker`, `MixinInterfaceLogic`): an ME Interface part
 * on grid A's cable bus faces grid B's Energy Acceptor (the part keeps the grids apart). Without the card the part
 * gives no FE; with the card in its second upgrade slot (the one Applied Flux adds) it pushes the FE of grid A's FE
 * cell into the acceptor, on its own side (`AFUtil.getSides`), and stops when the card comes out; the card in its
 * first slot survives a restart and the part pushes again. Needs content/appflux (ae2/scripts/prepare-addon.ts
 * appflux) and a client built with ae2-appflux. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('appflux-induction-card', mode, { addons: ['appflux'] });
const fe = (view: any) => view.keys?.find((key: any) => key.type === 'appflux:flux' && key.id === 'appflux:fe')?.amount ?? 0;
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
const text = (reply: any) => reply.chat?.join('\n') ?? '';
const CARD = 'appflux:induction_card', START = 500_000_000;
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 8, y = Math.ceil(py) + 4, z = Math.floor(pz) + 8;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -4)} ${at(7, 3, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-2, -2, -4)} ${at(7, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.raw('hotbar 1');

  // Grid A: power, a drive with a 1k FE cell of 500,000,000 FE, a cable bus with an ME Interface part east.
  const drive = at(1), cable = at(2), acceptor = at(3), battery = at(3, 0, 1);
  await fixture.command(`setblock ${at(0)} ae2:creative_energy_cell`); await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`setblock ${cable} ae2:cable_bus`);
  await fixture.command(`item replace block ${drive} container.0 with appflux:fe_1k_cell[appflux:fe_energy=${START}L]`);
  const status = () => fixture.ae(`grid status ${drive}`);
  await until(status, s => s.online && s.powered && fe(s) === START, 'grid A lists its FE', 30000);
  await fixture.replace('hotbar.0', 'ae2:cable_interface'); await fixture.select(0, 'ae2:cable_interface');
  assert.equal((await fixture.ae(`grid part ${cable} east`)).attached, true, 'the interface part on the east face');
  const nodes = (await status()).nodes;

  // Its menu, from the east, before grid B stands there: two card slots (entries 18 and 25).
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.1', CARD); await fixture.replace('hotbar.8', 'minecraft:stick'); await fixture.select(8, 'minecraft:stick');
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 0.5} 90 0`); await fixture.raw(`interact ${cable} east`);
  await until(() => fixture.raw('screen'), view => view.screen === 'container' && view.ae2_screen?.style === 'interface.json', 'the interface part\'s menu');

  // Grid B: an acceptor and an empty energy cell. No card: no FE.
  await fixture.command(`setblock ${acceptor} ae2:energy_acceptor`); await fixture.command(`setblock ${battery} ae2:energy_cell`);
  const other = () => fixture.ae(`grid status ${battery}`);
  await until(other, s => s.nodes === 2, 'grid B, the acceptor and its cell', 30000);
  assert.equal((await status()).nodes, nodes, 'grid B is a grid of its own');
  await Bun.sleep(3000);
  assert.ok((await other()).energy < 1, 'no FE from an interface without an Induction Card');
  assert.equal(fe(await status()), START);
  fixture.mark('an ME Interface part without an Induction Card gives no FE');

  // The card in the second slot: the part pushes grid A's FE into grid B's acceptor.
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), view => view.carried?.item === CARD, 'the card on the cursor');
  await fixture.raw('click-slot 325');
  await until(() => fixture.raw('screen'), view => slot(view, 325)?.item === CARD, 'the card in the second upgrade slot');
  const charged = await until(other, s => s.energy > 1000, 'the interface part charges grid B\'s acceptor', 60000);
  const full = await until(other, s => s.energy >= 199_000, 'grid B\'s energy cell fills', 60000);
  await Bun.sleep(1500);
  const given = START - fe(await status());
  assert(given >= 2 * full.energy && given < 2 * full.energy + 100_000, `grid A gave what B stored: ${given} FE for ${full.energy} AE`);
  fixture.mark(`an ME Interface part with an Induction Card in its second slot pushes its grid's FE into the block beside it (${charged.energy} AE early, ${given} FE given)`);

  // The card out, and an empty energy cell in grid B again: nothing moves.
  await fixture.raw('click-slot 325'); await until(() => fixture.raw('inventory'), view => view.carried?.item === CARD, 'the card taken out');
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), view => !view.carried?.item, 'the card back in the hotbar');
  await fixture.command(`setblock ${battery} minecraft:air`); await fixture.command(`setblock ${battery} ae2:energy_cell`);
  // The grid's own small buffer keeps what it had; the new cell adds nothing.
  const emptied = await until(other, s => s.nodes === 2, 'grid B with an empty cell', 30000);
  const kept = fe(await status());
  await Bun.sleep(3000);
  assert.ok((await other()).energy <= emptied.energy + 0.5, 'no FE once the card is out'); assert.equal(fe(await status()), kept);
  fixture.mark('taking the card out stops the push');

  // The card in the first slot, kept through a restart: the part pushes again.
  await fixture.raw('click-slot 1'); await until(() => fixture.raw('inventory'), view => view.carried?.item === CARD, 'the card on the cursor again');
  await fixture.raw('click-slot 318');
  await until(() => fixture.raw('screen'), view => slot(view, 318)?.item === CARD, 'the card in the first upgrade slot');
  await until(other, s => s.energy > 1000, 'the first slot\'s card charges grid B too', 60000);
  await fixture.raw('close');
  assert.match(text(await fixture.command(`data get block ${cable} ae2_lab_config.parts`)), /appflux:induction_card/, 'the part keeps its card');
  await fixture.restart();
  await until(status, s => s.online && s.powered, 'grid A after the restart', 60000);
  assert.match(text(await fixture.command(`data get block ${cable} ae2_lab_config.parts`)), /appflux:induction_card/, 'the card survives the restart');
  await fixture.command(`setblock ${battery} minecraft:air`); await fixture.command(`setblock ${battery} ae2:energy_cell`);
  const before = fe(await status());
  await until(other, s => s.energy > 1000, 'after the restart the part charges grid B again', 60000);
  await until(status, s => fe(s) < before, 'grid A\'s FE goes out again', 30000);
  fixture.mark('the Induction Card survives save and load, and the part pushes FE again');

  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; } finally { await fixture.close(); }
