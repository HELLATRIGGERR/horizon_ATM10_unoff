/** AEInfinityBooster's Infinity Range Booster: in the Wireless Access Point's booster slot it lets a wireless terminal
 * 10,000 blocks away in the same dimension reach the grid, keeps the access point's chunk loaded, shows an unlimited range
 * and 50 AE/t, and costs the grid what one booster does. */
import assert from 'node:assert/strict';
import { WorldFixture, until } from './harness';

const fixture = new WorldFixture('aeinfinitybooster-range', 'server-thread', { addons: ['aeinfinitybooster'] });
const CARD = 'aeinfinitybooster:infinity_card';
const slot = (view: any, index: number) => view.slots?.find((entry: any) => entry.slot === index);
const chat = (reply: any) => JSON.stringify(reply.chat ?? '');
const energy = (text: string) => Number(text.match(/ae2:stored_energy\\?"?:\s*([0-9.]+)d/)?.[1] ?? 0);
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  // The grid in one chunk: the access point keeps only its own chunk loaded.
  const x = (Math.floor(px / 16) + 2) * 16 + 4, y = Math.ceil(py) + 5, z = (Math.floor(pz / 16) + 2) * 16 + 8;
  const at = (dx: number) => `${x + dx} ${y} ${z}`;
  const access = at(1), cell = at(0);
  const near = async () => {
    await fixture.command(`tp @s ${x + 1.5} ${y} ${z + 2.5}`);
    await until(() => fixture.raw(`block ${x} ${y} ${z}`), block => block.loaded, 'the grid\'s chunk', 30000);
  };
  const far = async () => {
    // 10,000 blocks east, on a stone floor.
    await fixture.command(`tp @s ${x + 10000.5} ${y + 2} ${z + 0.5}`);
    await until(() => fixture.raw(`block ${x + 10000} ${y} ${z}`), block => block.loaded, 'the far chunk', 60000);
    await fixture.command(`fill ${x + 9998} ${y - 1} ${z - 2} ${x + 10002} ${y - 1} ${z + 2} minecraft:stone`);
    await fixture.command(`fill ${x + 9998} ${y} ${z - 2} ${x + 10002} ${y + 3} ${z + 2} minecraft:air`);
    await fixture.command(`tp @s ${x + 10000.5} ${y} ${z + 0.5}`);
  };
  const forced = async () => {
    try { const text = chat(await fixture.command(`forceload query ${x} ${z}`)); return /marked for force loading/.test(text) && !/not marked/.test(text); }
    catch { return false; }
  };
  // Use the terminal until it opens: the far chunks load first. On failure, what the grid looked like from there.
  const openFar = async (label: string) => {
    for (let attempt = 0; attempt < 10; attempt++) {
      await fixture.raw('use-item');
      try { return await until(() => fixture.raw('screen'), view => view.ae2_terminal?.kind === 'Terminal', label, 3000); }
      catch {}
    }
    throw new Error(`${label}: ${JSON.stringify(await fixture.ae(`grid status ${access}`))} ${await forced()}`);
  };
  await near();
  await fixture.command(`fill ${x - 2} ${y} ${z - 3} ${x + 5} ${y + 4} ${z + 3} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 3} ${x + 5} ${y - 1} ${z + 3} minecraft:stone`);
  await fixture.command(`setblock ${cell} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${access} ae2:wireless_access_point[facing=east]`);
  await fixture.command(`setblock ${at(-1)} ae2:drive`);
  await until(() => fixture.ae(`grid status ${access}`), state => state.online && state.powered, 'powered wireless access point', 30000);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  const terminal = `ae2:wireless_terminal[ae2:wireless_link_target={dimension:"minecraft:overworld",pos:[I;${x + 1},${y},${z}]},ae2:stored_energy=1000000.0d]`;
  await fixture.replace('hotbar.0', terminal);
  await fixture.replace('hotbar.1', CARD);
  await fixture.replace('hotbar.8', 'minecraft:stick');
  const idle = async () => (await fixture.ae(`power ${cell} north`)).idle as number;
  const before = await idle();
  assert.equal(await forced(), false);

  // Without the card the terminal is out of range 10,000 blocks away.
  await far();
  await fixture.select(0, 'ae2:wireless_terminal');
  // The server says it is out of range (a system message) and opens nothing, however often it is used.
  for (let attempt = 0; attempt < 4; attempt++) {
    await fixture.raw('use-item');
    await new Promise(resolve => setTimeout(resolve, 2000));
    assert.equal((await fixture.raw('screen')).ae2_terminal, undefined);
  }
  fixture.mark('a wireless terminal 10,000 blocks from its access point is out of range without the card');

  // The card in the access point's booster slot: an unlimited range and 50 AE/t on its screen.
  await near();
  await fixture.select(8, 'minecraft:stick');
  await fixture.raw(`interact ${access} south`);
  await until(() => fixture.raw('screen'), view => view.ae2_screen?.style === 'wireless_access_point.json', 'the access point screen', 15000);
  await fixture.raw('click-slot 1');
  await until(() => fixture.raw('inventory'), view => view.carried?.item === CARD, 'the card on the cursor');
  await fixture.raw('click-slot 300');
  const boosted = await until(() => fixture.raw('screen'), view => slot(view, 300)?.item === CARD && view.ae2_screen?.data?.[3] === 32767, 'an unlimited range', 15000);
  assert.deepEqual(boosted.ae2_screen.data.slice(0, 4), [-1, -1, -1, 32767], 'Long.MAX_VALUE');
  assert.equal(boosted.ae2_screen.data[4], 5000, 'infinityCardDrain 50 AE/t, in hundredths');
  await fixture.raw('close');
  // What the grid pays: AE2's drain for one booster, 8 + 1 = 9 AE/t, not the 50 the screen shows.
  const after = await until(() => idle(), value => Math.abs(value - before - 1) < 1e-6, 'one booster\'s drain on the grid', 15000);
  fixture.mark(`the card shows ∞ and 50 AE/t; the grid's idle draw goes from ${before} to ${after} AE/t`);
  await until(() => forced(), value => value, 'the access point keeps its chunk loaded', 15000);
  fixture.mark('the access point holding the card forces its chunk');

  // Now the terminal reaches the grid from 10,000 blocks away, and pays for 16 blocks.
  await far();
  await fixture.select(0, 'ae2:wireless_terminal');
  const charged = energy(chat(await fixture.command('data get entity @s SelectedItem')));
  await openFar('the far terminal opens with the card');
  await until(async () => energy(chat(await fixture.command('data get entity @s SelectedItem'))), value => value < charged, 'the battery pays the drain', 15000);
  await fixture.raw('close');
  fixture.mark('with the card a wireless terminal 10,000 blocks away opens and draws from its battery');

  // Through a restart the chunk stays forced and the far terminal still opens.
  await fixture.restart();
  assert.equal(await forced(), true, 'still forced after the restart');
  await fixture.select(0, 'ae2:wireless_terminal');
  await openFar('the far terminal opens after the restart');
  await fixture.raw('close');

  // The card taken out: the chunk is let go.
  await near();
  await fixture.select(8, 'minecraft:stick');
  await fixture.raw(`interact ${access} south`);
  await until(() => fixture.raw('screen'), view => slot(view, 300)?.item === CARD, 'the card in the access point', 15000);
  await fixture.raw('click-slot 300 shift');
  await until(() => fixture.raw('inventory'), view => view.counts?.[CARD] === 1, 'the card back in the inventory', 15000);
  await fixture.raw('close');
  await until(() => forced(), value => !value, 'the chunk is let go', 15000);
  await until(() => idle(), value => Math.abs(value - before) < 1e-6, 'the drain back to none', 15000);
  fixture.assertHidden(await fixture.raw('status'));
  fixture.mark('taking the card out releases the chunk and the drain, after a restart too');
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
