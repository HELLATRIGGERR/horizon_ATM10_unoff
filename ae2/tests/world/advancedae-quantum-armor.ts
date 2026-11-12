/** Advanced AE slices 7 and 7b: the Quantum Armor. Worn and charged, cards installed from the inventory (`/ae2 advae
 * armor`, which stands in for QuantumArmorConfigScreen) give their abilities on the server: night vision
 * (AAEPlayerEvents), the HP Buffer's and Step Assist's attributes, the magnet and flight (the client's abilities: a
 * double jump takes off), each drawing its cost from the piece's AE (tickUpgrades). The helmet's hotkeys turn its
 * cards on and off; a piece run empty loses what it gave; the cards, their states and the AE survive a restart. AE2's
 * Charger fills a piece. The damage events (AAELivingEntityEvents): a hit goes 30% into the pieces' AE and one left
 * under 1 is none, Lava Immunity against lava (a /damage and real lava), Evasion's 30%, charged boots against a
 * fall. The armor's recipes with the Quantum Alloy Plate. Needs content/advanced_ae (ae2/scripts/prepare-addon.ts
 * advancedae) and a client built with ae2-advancedae. */
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
const player = async () => (await fixture.raw('state')).player ?? {};
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

  // Flight (CREATIVE_FLIGHT): the server's abilities let the client fly, a double jump takes off, and the chestplate
  // draws Flight's 10 a tick on top of the HP Buffer's while the player flies (tickUpgrades).
  await until(player, p => p.may_fly === true, 'the Flight Card lets the client fly');
  let flying = false;
  for (let i = 0; i < 4 && !flying; i++) {
    // Each press held across a tick: movement can miss an instantaneous tap (as tools/pacing_bench.py does).
    for (const line of ['key space down', 'wait 2', 'key space up', 'wait 2', 'key space down', 'wait 2', 'key space up', 'wait 4'])
      await fixture.raw(line);
    flying = (await player()).flying === true;
  }
  assert(flying, 'a double jump took off');
  await fixture.hold();
  const c0 = (await armor(1)).stored; await fixture.stepServer(20); const c1 = (await armor(1)).stored;
  await fixture.hold(false);
  assert.equal(c0 - c1, 20 * 20, `flying, the chestplate draws 10 + 10 AE a tick: ${c0} -> ${c1}`);
  await armor(1, 'toggle', 'flight');
  await until(player, p => p.may_fly === false && p.flying === false, 'the card off: the client may not fly');
  fixture.mark('the Flight Card lets the player fly, and flying draws its cost');

  // A chestplate run empty loses what it gave.
  const low = 'ae2:stored_energy=60.0d,advanced_ae:flight_toggle=true,advanced_ae:flight_value=10,advanced_ae:flight_filter=[],advanced_ae:hp_buffer_toggle=true,advanced_ae:hp_buffer_value=20,advanced_ae:hp_buffer_filter=[]';
  await fixture.command(`item replace entity @s armor.chest with ${CHEST}[${low}]`);
  await until(() => attribute('minecraft:max_health'), v => v === 20, 'the HP Buffer gone with the AE');
  assert(((await armor(1)).stored ?? 0) < 5, 'the chestplate is spent');
  await until(player, p => p.may_fly === false, 'flight gone with the AE');
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

  // AE2's Charger fills a piece (IAEItemPowerStorage): getChargeRate's 20,000 AE a tick from a creative cell's grid.
  const charger = `${x + 3} ${y} ${z}`;
  await fixture.command(`setblock ${charger} ae2:charger[facing=north]`);
  await fixture.command(`setblock ${x + 4} ${y} ${z} ae2:creative_energy_cell`);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 1.5}`);
  const inCharger = async () => JSON.stringify(await fixture.command(`data get block ${charger}`));
  await fixture.replace('hotbar.1', `${HELMET}[ae2:stored_energy=1000.0d]`); await fixture.select(1, HELMET);
  await fixture.raw(`interact ${charger} north`);
  await until(inCharger, t => t.includes(HELMET), 'the helmet in the Charger', 30000);
  const charge = async () => Number((await inCharger()).match(/ae2:stored_energy\\?"?:\s*(-?[0-9.E+-]+)d?/)?.[1] ?? 0);
  await fixture.hold();
  const before = await charge(); await fixture.stepServer(20); const gained = await charge() - before;
  await fixture.hold(false);
  assert(gained > 0 && gained <= 20000 * 20 && gained % 20000 === 0, `charged in steps of 20,000 AE a tick: ${gained}`);
  fixture.mark('AE2\'s Charger charges a piece at 20,000 AE a tick');

  // The damage events (AAELivingEntityEvents), with charged pieces and no card that ticks: a hit takes 30% of itself, a
  // quarter from each worn piece's AE at 1,000 AE a point (incomingDamage); what is left under 1 is no hit.
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 0.5}`);
  const card = (name: string) => `,advanced_ae:${name}_toggle=true,advanced_ae:${name}_value=1,advanced_ae:${name}_filter=[]`;
  const wear = (slot: string, item: string, cards = '', ae = 1000000) =>
    fixture.command(`item replace entity @s armor.${slot} with ${item}[ae2:stored_energy=${ae}.0d${cards}]`);
  await wear('head', HELMET); await wear('chest', CHEST, card('lava_immunity')); await wear('feet', BOOTS);
  await until(() => armor(1), v => upgrade(v, 'lava_immunity')?.on === true && v.stored === 1000000, 'the Lava Immunity chestplate worn');
  await until(() => armor(3), v => v.upgrades.length === 0 && v.stored === 1000000, 'plain boots worn');
  await until(() => armor(0), v => v.upgrades.length === 0 && v.stored === 1000000, 'a plain helmet worn');
  const health = async () => Number(text(await fixture.command('data get entity @s Health')).match(/data: (-?[\d.]+)f/)?.[1] ?? NaN);
  const pieces = async () => [(await armor(0)).stored, (await armor(1)).stored, (await armor(3)).stored];
  const drops = (a: number[], b: number[]) => a.map((v, i) => Math.round((v - b[i]) * 1000) / 1000);
  // An evaded or called-off hit fails the command, as vanilla's DamageCommand does.
  const hurt = (amount: number, type: string) => fixture.raw(`command damage @s ${amount} ${type}`);
  await fixture.hold();
  let ae = await pieces(), hp = await health();
  await hurt(4, 'minecraft:generic');
  assert.deepEqual(drops(ae, await pieces()), [300, 300, 300], 'each piece took 0.3 of the 4 into its AE');
  const taken = hp - await health();
  assert(Math.abs(taken - 3.1) < 1e-4, `3.1 of the 4 taken (generic damage bypasses armor): ${taken}`);
  await fixture.stepServer(20);
  ae = await pieces(); hp = await health();
  await hurt(1.2, 'minecraft:generic');
  assert.deepEqual(drops(ae, await pieces()), [90, 90, 90], 'a hit of 1.2 gives each piece 0.09');
  assert.equal(await health(), hp, 'the 0.93 left is no hit');
  fixture.mark('a hit goes 30% into the pieces\' AE, and one left under 1 is no hit');

  // Lava Immunity: lava and fire are no hit, at the card's 10 AE each, and nothing goes into the AE; in real lava the
  // player is unharmed and does not burn after.
  ae = await pieces(); hp = await health();
  await hurt(4, 'minecraft:lava');
  assert.deepEqual(drops(ae, await pieces()), [0, 10, 0], 'the hit cost Lava Immunity\'s 10 alone');
  assert.equal(await health(), hp, 'no lava damage');
  await fixture.hold(false);
  await fixture.command(`setblock ${x + 2} ${y - 2} ${z} minecraft:stone`);
  await fixture.command(`setblock ${x + 2} ${y - 1} ${z} minecraft:lava`);
  ae = await pieces(); hp = await health();
  await fixture.command(`tp @s ${x + 2.5} ${y - 1} ${z + 0.5}`);
  await wait(2000);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 0.5}`);
  await fixture.command(`setblock ${x + 2} ${y - 1} ${z} minecraft:stone`);
  const lava = drops(ae, await pieces());
  assert(lava[1] >= 10 && lava[1] % 10 === 0 && lava[0] === 0 && lava[2] === 0, `lava drew Lava Immunity's 10 a hit, nothing else: ${lava}`);
  assert(await health() >= hp, 'unharmed in the lava');
  const fire = async () => Number(text(await fixture.command('data get entity @s Fire')).match(/data: (-?\d+)s/)?.[1] ?? NaN);
  await until(fire, f => f <= 0, 'not burning after the lava');
  fixture.mark('Lava Immunity keeps lava and fire off at its cost');

  // Evasion: 30% of hits, of any kind, are no hit at the card's 10 AE; every other one gives the boots their 150.
  await wear('feet', BOOTS, card('evasion'));
  await until(() => armor(3), v => upgrade(v, 'evasion')?.on === true, 'the boots with Evasion worn');
  await fixture.hold();
  let evaded = 0;
  for (let i = 0; i < 40; i++) {
    const a = (await armor(3)).stored; await hurt(2, 'minecraft:generic');
    const d = Math.round((a - (await armor(3)).stored) * 1000) / 1000;
    assert(d === 10 || d === 150, `a hit costs the boots Evasion's 10 or gives them 150: ${d}`);
    if (d === 10) evaded++;
  }
  await fixture.hold(false);
  assert(evaded >= 2 && evaded <= 26, `about 30% of 40 hits evaded: ${evaded}`);
  fixture.mark(`Evasion called off ${evaded} of 40 hits at its cost`);

  // The boots' fall (LivingFallDamage): charged boots take the fall's distance to 0; empty ones let it hurt, the other
  // pieces absorbing their part.
  const fall = async () => {
    await fixture.command(`tp @s ${x + 0.5} ${y + 12} ${z + 0.5}`);
    await until(player, p => p.on_ground === true && p.position?.[1] < y + 0.5, 'landed', 30000);
    await wait(1000);
  };
  await wear('feet', BOOTS);
  await until(() => armor(3), v => v.upgrades.length === 0, 'plain boots again');
  ae = await pieces(); hp = await health();
  await fall();
  assert.deepEqual(drops(ae, await pieces()), [0, 0, 0], 'the fall drew nothing');
  assert(await health() >= hp, 'charged boots: no fall damage');
  await wear('feet', BOOTS, '', 0);
  await until(() => armor(3), v => v.stored === 0, 'empty boots');
  ae = await pieces(); hp = await health();
  await fall();
  const fallen = drops(ae, await pieces());
  assert(fallen[0] > 0 && fallen[0] === fallen[1] && fallen[2] === 0, `the helmet and the chestplate absorbed part of the fall: ${fallen}`);
  assert(await health() < hp, 'empty boots: the fall hurt');
  fixture.mark('charged boots take no fall damage; empty ones do, the others absorbing part');

  // The armor's recipes, with the Quantum Alloy Plate: a Quantum Helmet at a crafting table.
  await fixture.command('clear @s');
  const table = `${x - 2} ${y} ${z}`;
  await fixture.command(`setblock ${table} minecraft:crafting_table`);
  const P = 'advanced_ae:quantum_processor', A = 'advanced_ae:quantum_alloy_plate';
  const grid = [P, 'ae2:wireless_access_point', P, P, 'minecraft:netherite_helmet', P, A, 'advanced_ae:quantum_storage_component', A];
  for (let i = 0; i < 9; i++) await fixture.replace(`hotbar.${i}`, grid[i]);
  await fixture.raw(`interact ${table} east`);
  await until(() => fixture.raw('screen'), s => s.craft_size === 3 && s.slots?.some((slot: any) => slot.slot === 108), 'crafting table grid');
  for (let i = 0; i < 9; i++) {
    await fixture.raw(`click-slot ${i}`); await until(() => fixture.raw('inventory'), s => !!s.carried, 'ingredient on the cursor');
    await fixture.raw(`click-slot ${100 + i}`); await until(() => fixture.raw('inventory'), s => !s.carried, 'ingredient in the grid');
  }
  await until(() => fixture.raw('screen'), s => s.slots?.some((slot: any) => slot.slot === 200 && slot.item === HELMET), 'the recipe offers a Quantum Helmet');
  await fixture.raw('click-slot 200 shift'); await fixture.raw('close');
  const made = await until(counts, c => c[HELMET] === 1, 'the helmet crafted');
  assert(!grid.some(i => made[i]), 'all nine ingredients used');
  fixture.mark('the Quantum Helmet\'s recipe crafts with the Quantum Alloy Plate');
  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
