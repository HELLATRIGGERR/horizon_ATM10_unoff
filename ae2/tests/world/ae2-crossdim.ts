/** Cross-dimension wireless: a wireless crafting terminal in the Nether reaches a grid in the Overworld. Through a Quantum
 * Network Bridge (AE2WTLib's Quantum Bridge Card and the singularity in the terminal) it opens, and not without the card;
 * its menu drains 22.5 AE/t and recharges its battery from the grid, never taking the grid below half; restock fills the
 * hotbar from the Overworld network. Through the grid's access point holding AEInfinityBooster's Dimension Card it opens
 * from the Nether, pays AE2's drain for 32 blocks while the grid pays one booster's, and the card keeps the access
 * point's chunk forced. */
import assert from 'node:assert/strict';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('ae2-crossdim', 'server-thread', { addons: ['ae2wtlib', 'aeinfinitybooster'] });
const OVERWORLD = 'minecraft:overworld', NETHER = 'minecraft:the_nether';
const CARD = 'ae2wtlib:quantum_bridge_card', DIMENSION_CARD = 'aeinfinitybooster:dimension_card';
const SINGULARITY = 'ae2:quantum_entangled_singularity', FREQUENCY = 731;
const chat = (reply: any) => JSON.stringify(reply.chat ?? '');
const energy = (text: string) => Number(text.match(/ae2:stored_energy\\?"?:\s*([0-9.E+-]+)d/)?.[1] ?? NaN);
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  // The grid in one chunk of the Overworld: a quantum bridge lying on the floor (its link at the centre), a dense
  // energy cell west of it, a drive east, an access point south.
  const x = (Math.floor(px / 16) + 3) * 16 + 8, y = Math.ceil(py) + 5, z = Math.floor(pz / 16) * 16 + 8;
  const at = (dx = 0, dz = 0, dy = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const cell = at(-2), drive = at(2), access = at(0, 2), link = at();
  // `/ae2` reads the level the player is in (it is not one of `execute`'s commands).
  const overworld = (command: string) => fixture.command(`execute in ${OVERWORLD} run ${command}`);
  const stored = async (id: string) => (await fixture.ae(`grid status ${drive}`)).items?.find((e: any) => e.item === id)?.count ?? 0;
  const idle = async () => (await fixture.ae(`power ${cell} north`)).idle as number;
  const forced = async () => {
    try { const text = chat(await overworld(`forceload query ${x} ${z}`)); return /marked for force loading/.test(text) && !/not marked/.test(text); }
    catch { return false; }
  };
  const base = `${x - 0.5} ${y} ${z + 3.5}`;
  await fixture.command(`tp @s ${base}`);
  await fixture.command(`forceload add ${x} ${z}`);
  await until(() => fixture.raw(`block ${x} ${y} ${z}`), block => block.loaded, 'the grid\'s chunk', 30000);
  await fixture.command(`fill ${at(-4, -3)} ${at(4, 5, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-4, -3, -1)} ${at(4, 5, -1)} minecraft:stone`);
  for (let dx = -1; dx <= 1; dx++) for (let dz = -1; dz <= 1; dz++) {
    await fixture.command(`setblock ${at(dx, dz)} ae2:${dx === 0 && dz === 0 ? 'quantum_link' : 'quantum_ring'}`);
    await fixture.ae(`grid status ${at(dx, dz)}`);
  }
  await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k`);
  await fixture.command(`setblock ${access} ae2:wireless_access_point[facing=south]`);
  await fixture.command('clear @s');
  // A full dense energy cell, placed from the hand onto the floor, west of the bridge.
  await fixture.command(`tp @s ${x - 2.5} ${y} ${z + 2.5}`);
  await fixture.replace('hotbar.0', 'ae2:dense_energy_cell[ae2:stored_energy=1600000.0d]'); await fixture.select(0, 'ae2:dense_energy_cell');
  await fixture.raw(`place ${cell}`);
  await fixture.command(`tp @s ${base}`);
  await until(() => fixture.ae(`grid status ${drive}`), state => state.online && state.powered, 'the powered grid', 60000);
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'the powered access point', 30000);
  // One half of a singularity pair in the bridge's link, through its menu.
  await fixture.replace('hotbar.2', `${SINGULARITY}[ae2:entangled_singularity_id=${FREQUENCY}]`); await fixture.select(2, SINGULARITY);
  await fixture.raw(`interact ${link} up`);
  await until(() => fixture.raw('screen'), s => slot(s, 2)?.item === SINGULARITY, 'the quantum link\'s menu', 30000);
  await fixture.raw('click-slot 2');
  await until(() => fixture.raw('inventory'), v => v.carried?.item === SINGULARITY, 'the singularity on the cursor');
  await fixture.raw('click-slot 300');
  await until(() => fixture.raw('screen'), s => slot(s, 300)?.item === SINGULARITY, 'the singularity in the link', 30000);
  await fixture.raw('close');
  const full = (await fixture.ae(`power ${cell} north`)).energy as number;
  assert(full > 1_500_000, `the placed cell keeps its energy: ${full}`);
  await fixture.replace('hotbar.0', 'minecraft:cobblestone', 64); await fixture.select(0, 'minecraft:cobblestone');
  assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, 64);
  await fixture.command('clear @s');
  fixture.mark('an Overworld grid with a quantum bridge holding a singularity, a dense energy cell and 64 cobblestone');

  // The Nether: a stone room, far from anything.
  const nx = x + 1000, ny = 100, nz = z;
  const n = (dx = 0, dz = 0, dy = 0) => `${nx + dx} ${ny + dy} ${nz + dz}`;
  await fixture.command(`execute in ${NETHER} run tp @s ${nx + 0.5} ${ny} ${nz + 0.5}`);
  await fixture.command(`execute in ${NETHER} run fill ${n(-7, -7, -1)} ${n(7, 7, 9)} minecraft:stone`);
  await fixture.command(`execute in ${NETHER} run fill ${n(-6, -6)} ${n(6, 6, 8)} minecraft:air`);
  const nether = `${nx + 0.5} ${ny} ${nz + 0.5}`;
  await fixture.command(`execute in ${NETHER} run tp @s ${nether}`);
  await fixture.command('gamemode survival @s');
  const singularity = `{id:"${SINGULARITY}",count:1,components:{"ae2:entangled_singularity_id":${FREQUENCY}L}}`;
  const terminal = (card: boolean, power: number) => `ae2:wireless_crafting_terminal[ae2:wireless_link_target={dimension:"${OVERWORLD}",pos:[I;${x},${y},${z + 2}]},`
    + `ae2:stored_energy=${power}.0d,ae2wtlib:restock=true,ae2wtlib:singularity=${singularity}`
    + `${card ? `,ae2:upgrades=[{slot:0,item:{id:"${CARD}",count:1}}]` : ''}]`;
  const battery = async () => energy(chat(await fixture.command('data get entity @s SelectedItem')));
  const hold = async (item: string) => { await fixture.replace('hotbar.8', item); await fixture.select(8, 'ae2:wireless_crafting_terminal'); await fixture.raw('look 0 -80'); };
  const opens = async (label: string) => {
    for (let attempt = 0; attempt < 10; attempt++) {
      await fixture.raw('use-item');
      try { return await until(() => fixture.raw('screen'), view => view.ae2_terminal?.kind === 'Crafting', label, 3000); }
      catch {}
    }
    throw new Error(label);
  };
  const lists = (label: string) => until(() => fixture.raw('screen'), view => view.ae2_terminal?.entries?.some((e: any) => e.item === 'minecraft:cobblestone'), label, 15000);
  // The battery's drain over 200 held ticks. `ItemMenuHost.consumeIdlePower` pays ten ticks' drain at once and
  // spends them over the next ten ticks, so it pays every eleventh tick: 18 or 19 payments in 200 ticks.
  const payments = (drain: number, amount: number) => [18, 19].some(k => Math.abs(amount - k * 10 * drain) < 1e-3);
  const drained = async () => {
    await fixture.hold();
    const before = await battery();
    await fixture.stepServer(200);
    const after = await battery();
    await fixture.hold(false);
    return before - after;
  };

  // Without the card the terminal reaches nothing from the Nether: its access point is in another dimension.
  await hold(terminal(false, 1000));
  for (let attempt = 0; attempt < 4; attempt++) {
    await fixture.raw('use-item');
    await Bun.sleep(2000);
    assert.equal((await fixture.raw('screen')).ae2_terminal, undefined, 'nothing opens');
  }
  fixture.mark('without the Quantum Bridge Card a wireless terminal in the Nether reaches no Overworld grid');

  // With the card it opens through the bridge, lists the Overworld network, and its battery is recharged from the grid.
  await hold(terminal(true, 1000));
  await opens('the terminal opens through the bridge');
  await lists('the Overworld network\'s cobblestone listed');
  const charged = await until(battery, value => value > 100_000, 'the battery recharged from the grid', 15000);
  fixture.mark(`through the bridge the terminal opens in the Nether and lists the Overworld network; its battery went from 1000 to ${charged} AE`);

  // The grid is at half now: no more recharge, and the menu drains 22.5 AE a tick from the battery.
  const quantum = await drained();
  assert(payments(22.5, quantum), `225 AE (ten ticks of 22.5 AE/t) every eleven ticks: ${quantum}`);
  await fixture.raw('close');
  fixture.mark(`then the menu drains its battery at 22.5 AE/t through the bridge: ${quantum} AE in 200 ticks`);

  // Restock reaches the Overworld network through the bridge: a block placed in the Nether is replaced from it.
  await fixture.replace('hotbar.1', 'minecraft:cobblestone', 10); await fixture.select(1, 'minecraft:cobblestone');
  await until(() => fixture.raw('inventory'), s => s.hotbar_counts?.[1] === String(10 + 64), 'the hotbar shows the Overworld stock');
  await fixture.raw(`place ${n(2)}`);
  await until(() => fixture.raw('inventory'), s => s.hotbar[1]?.count === 64, 'the stack restocked to 64');
  await until(() => fixture.raw('inventory'), s => s.hotbar_counts?.[1] === String(64 + 64 - 55), 'the count follows');
  fixture.mark('restock fills the hotbar in the Nether from the Overworld network');

  // Back at the grid: the recharge left it half its store, the restock took the cobblestone.
  await fixture.command(`execute in ${OVERWORLD} run tp @s ${base}`);
  const half = (await fixture.ae(`power ${cell} north`)).energy as number;
  assert(half <= 801_000 && half > 600_000, `the recharge stops at half the grid's store: ${half}`);
  assert.equal(await stored('minecraft:cobblestone'), 64 - 55, 'the restock took from the Overworld network');
  fixture.mark(`the dense energy cell kept half its store (${half} AE of 1.6M, the grid's idle draw since included)`);

  // AEInfinityBooster's Dimension Card: /forceload let go, the card into the access point.
  await fixture.command(`forceload remove ${x} ${z}`);
  const without = await idle();
  await fixture.command('clear @s');
  await fixture.replace('hotbar.1', DIMENSION_CARD);
  await fixture.replace('hotbar.0', 'minecraft:stick'); await fixture.select(0, 'minecraft:stick');
  await fixture.raw(`interact ${access} south`);
  await until(() => fixture.raw('screen'), view => view.ae2_screen?.style === 'wireless_access_point.json', 'the access point screen', 15000);
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('inventory'), view => view.carried?.item === DIMENSION_CARD, 'the card on the cursor');
  await fixture.raw('click-slot 300');
  const boosted = await until(() => fixture.raw('screen'), view => slot(view, 300)?.item === DIMENSION_CARD && view.ae2_screen?.data?.[3] === 32767, 'an unlimited range', 15000);
  assert.equal(boosted.ae2_screen.data[4], 10000, 'dimensionCardDrain 100 AE/t, in hundredths');
  await fixture.raw('close');
  await until(idle, value => Math.abs(value - without - 1) < 1e-6, 'the grid pays one booster', 15000);
  await until(forced, value => value, 'the access point holding the card forces its chunk', 15000);
  fixture.mark(`the Dimension Card shows an unlimited range and 100 AE/t, the grid's idle draw goes from ${without} to ${without + 1} AE/t, the chunk is forced`);

  // From the Nether a terminal without the bridge card reaches the grid through the access point, as 32 blocks away.
  await fixture.command(`execute in ${NETHER} run tp @s ${nether}`);
  await hold(terminal(false, 100000));
  await opens('the terminal opens through the Dimension Card');
  await lists('the Overworld network listed through the access point');
  const dimension = await drained();
  assert(payments(32, dimension), `AE2's drain for 32 blocks, 320 AE every eleven ticks: ${dimension}`);
  await fixture.raw('close');
  fixture.mark(`the Dimension Card answers a terminal in the Nether, its battery paying 32 AE/t (${dimension} AE in 200 ticks)`);

  // Taken out, the card lets its chunk go.
  await fixture.command(`execute in ${OVERWORLD} run tp @s ${base}`);
  await fixture.replace('hotbar.0', 'minecraft:stick'); await fixture.select(0, 'minecraft:stick');
  await fixture.raw(`interact ${access} south`);
  await until(() => fixture.raw('screen'), view => slot(view, 300)?.item === DIMENSION_CARD, 'the card in the access point', 15000);
  await fixture.raw('click-slot 300 shift');
  await until(() => fixture.raw('inventory'), view => view.counts?.[DIMENSION_CARD] === 1, 'the card back in the inventory', 15000);
  await fixture.raw('close');
  await until(forced, value => !value, 'the chunk let go', 15000);
  fixture.assertHidden(await fixture.raw('status'));
  fixture.mark('taking the Dimension Card out lets the chunk go');
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
