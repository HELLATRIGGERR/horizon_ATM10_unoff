/** Advanced AE slice 7: the Quantum Armor. Worn and charged, cards installed from the inventory (`/ae2 advae armor`,
 * which stands in for QuantumArmorConfigScreen) give their abilities on the server: night vision (AAEPlayerEvents),
 * the HP Buffer's and Step Assist's attributes and the magnet, each drawing its cost from the
 * piece's AE (tickUpgrades). The helmet's hotkeys turn its cards on and off; a piece run empty loses what it gave; the cards, their states and the AE survive a restart. Needs
 * content/advanced_ae (ae2/scripts/prepare-addon.ts advancedae) and a client built with ae2-advancedae. */
import assert from 'node:assert/strict';
import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('advancedae-quantum-armor', 'server-thread', { addons: ['advanced_ae'] });
appendFileSync(join(fixture.game, 'options.txt'), 'version:5023\nkey_key.advanced_ae.quantum_night_vision_upgrade:key.keyboard.n\n');
const HELMET = 'advanced_ae:quantum_helmet', CHEST = 'advanced_ae:quantum_chestplate', BOOTS = 'advanced_ae:quantum_boots';
const text = (reply: any) => JSON.stringify(reply.chat ?? '');
const attribute = async (id: string) => Number(text(await fixture.command(`attribute @s ${id} get`)).match(/is (-?[\d.]+)/)?.[1] ?? NaN);
const effects = async () => text(await fixture.command('data get entity @s active_effects'));
const armor = (slot: number, ...rest: string[]) => fixture.ae(`advae armor ${slot} ${rest.join(' ')}`.trim());
const upgrade = (view: any, name: string) => view.upgrades.find((u: any) => u.upgrade === name);
const wait = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 0.5}`);
  for (const [dx, dz] of [[-10, -10], [10, 10]] as const)
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), block => block.loaded, 'fixture chunks loaded', 30000);
  await fixture.command(`fill ${x - 10} ${y} ${z - 10} ${x + 10} ${y + 4} ${z + 10} minecraft:air`);
  await fixture.command(`fill ${x - 10} ${y - 1} ${z - 10} ${x + 10} ${y - 1} ${z + 10} minecraft:stone`);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await fixture.command('effect clear @s');

  // Worn and charged, with no card: nothing but the armor's points.
  const energy = 'ae2:stored_energy=1000000.0d';
  await fixture.command(`item replace entity @s armor.head with ${HELMET}[${energy}]`);
  await fixture.command(`item replace entity @s armor.chest with ${CHEST}[${energy}]`);
  await fixture.command(`item replace entity @s armor.feet with ${BOOTS}[${energy}]`);
  await until(() => attribute('minecraft:armor'), v => v === 17, 'the three pieces\' 4 + 9 + 4 armor points');
  assert.equal(await attribute('minecraft:armor_toughness'), 20, 'three times 10, at the most the attribute allows');
  assert.equal((await armor(0)).stored, 1000000, 'no card, no draw');
  for (const card of ['night_vision_card', 'magnet_card', 'flight_card', 'hp_buffer_card', 'step_assist_card', 'luck_card'])
    await fixture.command(`give @s advanced_ae:${card}`);
  await assert.rejects(armor(0, 'install', 'advanced_ae:flight_card'), 'the helmet takes no flight card');
  fixture.mark('the pieces give their armor and toughness; a card of another piece is refused');

  // The cards go in from the inventory, on and at their most.
  for (const [slot, card] of [[0, 'night_vision_card'], [0, 'magnet_card'], [1, 'flight_card'], [1, 'hp_buffer_card'], [3, 'step_assist_card']] as const)
    await armor(slot, 'install', `advanced_ae:${card}`);
  const counts = async () => (await fixture.raw('inventory')).counts ?? {};
  const left = await until(counts, v => v['advanced_ae:step_assist_card'] === undefined, 'the last card gone from the inventory');
  for (const card of ['night_vision_card', 'magnet_card', 'flight_card', 'hp_buffer_card', 'step_assist_card'])
    assert.equal(left[`advanced_ae:${card}`], undefined, `the ${card} was used up`);
  assert.equal(left['advanced_ae:luck_card'], 1, 'the luck card was not');
  const helmet = await armor(0);
  assert.deepEqual(helmet.upgrades.map((u: any) => u.upgrade), ['magnet', 'night_vision'], 'in the helmet\'s order');
  assert.equal(upgrade(helmet, 'magnet').value, 12);
  fixture.mark('the cards go in from the inventory, on, at their most, in the piece\'s order');

  // The abilities, each drawing its cost.
  await until(effects, t => t.includes('night_vision'), 'night vision from the helmet');
  await until(() => attribute('minecraft:max_health'), v => v === 40, 'the HP Buffer\'s 20');
  await until(() => attribute('minecraft:step_height'), v => Math.abs(v - 3.6) < 1e-6, 'Step Assist\'s 3 over 0.6');
  // Flight goes through the server's abilities (Hub::set_flight); `data get .. abilities` derives mayfly from the
  // game mode alone (save.rs), so this suite cannot read it.
  // The helmet: the magnet's 5 and night vision's 10 (a buff) in tickUpgrades, night vision's 10 again as it renews.
  await fixture.hold();
  const a = (await armor(0)).stored; await fixture.stepServer(20); const b = (await armor(0)).stored;
  await fixture.hold(false);
  assert.equal(a - b, 25 * 20, `the helmet draws 25 AE a tick: ${a} -> ${b}`);
  // Step Assist is a PASSIVE_USE card: the boots have no passive or buff card, so nothing ticks them.
  assert.equal((await armor(3)).stored, 1000000, 'the boots draw nothing');
  fixture.mark('night vision, the HP Buffer and Step Assist apply and draw their costs');

  // The magnet: an item 9 blocks away comes into the inventory.
  const drop = (id: string, dx: number) =>
    fixture.command(`summon minecraft:item ${x + dx + 0.5} ${y + 0.2} ${z + 0.5} {Item:{id:"${id}",count:3},PickupDelay:0s}`);
  await drop('minecraft:cobblestone', 9);
  await until(counts, v => v['minecraft:cobblestone'] === 3, 'the magnet pulled the cobblestone in');
  fixture.mark('the magnet picks up what lies in its range of 12');

  // The helmet's hotkeys: G turns the magnet off, the bound night vision key the effect.
  await fixture.raw('key g tap');
  await until(async () => upgrade(await armor(0), 'magnet').on, on => on === false, 'G turned the magnet off');
  await drop('minecraft:dirt', 9);
  await wait(1500);
  assert.equal((await counts())['minecraft:dirt'], undefined, 'the magnet is off');
  await fixture.raw('key n tap');
  await until(async () => upgrade(await armor(0), 'night_vision').on, on => on === false, 'the key turned night vision off');
  await until(effects, t => !t.includes('night_vision'), 'the helmet took its night vision back');
  fixture.mark('the hotkeys turn the helmet\'s magnet and night vision off, and they stop');

  // A chestplate run empty loses what it gave.
  const low = 'ae2:stored_energy=60.0d,advanced_ae:flight_toggle=true,advanced_ae:flight_value=10,advanced_ae:flight_filter=[],advanced_ae:hp_buffer_toggle=true,advanced_ae:hp_buffer_value=20,advanced_ae:hp_buffer_filter=[]';
  await fixture.command(`item replace entity @s armor.chest with ${CHEST}[${low}]`);
  await until(() => attribute('minecraft:max_health'), v => v === 20, 'the HP Buffer gone with the AE');
  assert(((await armor(1)).stored ?? 0) < 5, 'the chestplate is spent');
  fixture.mark('a piece run empty stops its abilities');

  // Through a restart: the cards, their states and the AE stay.
  const kept = await armor(0);
  await fixture.restart();
  const back = await armor(0);
  assert.deepEqual(back.upgrades.map((u: any) => [u.upgrade, u.on, u.value]), kept.upgrades.map((u: any) => [u.upgrade, u.on, u.value]));
  assert(back.stored > 0 && back.stored <= kept.stored, 'the helmet kept its AE');
  assert.equal(upgrade(await armor(3), 'step_assist').on, true);
  await until(() => attribute('minecraft:step_height'), v => Math.abs(v - 3.6) < 1e-6, 'Step Assist again after the restart');
  fixture.mark('the cards, their states and the AE survive a restart');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
