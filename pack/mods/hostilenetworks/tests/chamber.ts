/** Hostile Neural Networks' Simulation Chamber and Loot Fabricator, as the `hostilenetworks` native bundle and the carry
 * of the player's jar make them.
 *
 * - A chamber holding a self-aware zombie data model (given with `hostilenetworks:data`, so it predicts every time),
 *   three prediction matrices and 1,000,000 FE starts a run (one matrix used, 240 ticks), draws the zombie's 128 FE a
 *   tick for 239 of them and puts out the base drop and one prediction of the zombie. The model has run once.
 * - The prediction goes to a Loot Fabricator (1,000,000 FE), whose screen picks the first drop with a click on the
 *   palette (a menu button on the server). The drop takes 60 ticks at 256 FE (15,360 FE) and the fabricator puts
 *   64 rotten flesh in its output and uses the prediction up.
 * - A training chamber (a basic model with 6 data, mode set by the screen's button id 4) adds one point of data and
 *   puts out nothing.
 *
 * Reads the saved compound (`data get block`, the bundle's own keys). A picture of the chamber's screen, running,
 * goes to the run folder (`chamber-running.png`). Needs the staged native build (`AE2_TEST_BINARY`) and the content
 * mod made from the carry of Hostile Neural Networks (`HOSTILENETWORKS_CONTENT_DIR`); `pack/scripts/hostilenetworks.ts
 * world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.HOSTILENETWORKS_CONTENT_DIR ?? join(root, '../.build/content/hostilenetworks'));
const fixture = new WorldFixture('hostilenetworks-chamber', 'server-thread', { devMods: [content] });

/** The block entity's compound on the server, as `data get block` prints it. */
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => {
  const found = nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?[\\d.]+)`));
  return found ? Number(found[1]) : 0;
};
/** The item in a container slot of the saved compound. Entries print as `{components: {...}, count: 1, Slot: 0b, id: "..."}`
 * (insertion order, nested braces for components), so the `Items: [...]` list is split at its top-level braces. */
function slotItem(nbt: string, slot: number): { id: string; count: number; text: string } | null {
  const from = nbt.indexOf('Items: [');
  if (from < 0) return null;
  let depth = 0, begin = -1, quoted = false;
  for (let i = from + 8; i < nbt.length; i++) {
    const c = nbt[i];
    if (quoted) { if (c === String.fromCharCode(92)) i++; else if (c === '"') quoted = false; continue; }
    if (c === '"') quoted = true;
    else if (c === '{') { if (depth++ === 0) begin = i; }
    else if (c === '}') {
      if (--depth === 0) {
        const text = nbt.slice(begin, i + 1);
        if (new RegExp(`Slot: ?${slot}b`).test(text)) return { id: text.match(/id: ?"([^"]+)"/)?.[1] ?? '', count: field(text, 'count'), text };
      }
    } else if (c === ']' && depth === 0) break;
  }
  return null;
}
const gametime = async () => Number(((await fixture.command('time query gametime')).chat ?? []).join(' ').match(/(\d+)/)?.[1]);
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

/** `/hnn give` makes a data model (the game's `/give` knows no components of a mod's): the hotbar slot it landed in. */
async function giveModel(model: string, data: number): Promise<number> {
  const reply = await fixture.command(`hnn give ${model} ${data}`);
  assert.match((reply.chat ?? []).join(" "), /Gave a/, `hnn give: ${JSON.stringify(reply.chat)}`);
  const view = await until(() => fixture.raw("inventory"), v => v.hotbar.some((s: any) => s.item === "hostilenetworks:data_model"), "the data model in the hotbar");
  return view.hotbar.find((s: any) => s.item === "hostilenetworks:data_model").slot;
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 1} ${y} ${z - 2} ${x + 12} ${y + 3} ${z + 2} minecraft:air`);
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 2} ${x + 12} ${y - 1} ${z + 2} minecraft:stone`);
  const at = (n: number) => `${x + n * 2} ${y} ${z}`;
  // The fixture window is 960x540 at GUI scale 2: a screen of a given size is centred in 480x270.
  const origin = (w: number, h: number) => [Math.trunc((480 - w) / 2), Math.trunc((270 - h) / 2)];

  /** Places a machine, gives it energy, and opens its screen. */
  async function open(n: number, block: string, energy: number) {
    const pos = at(n);
    await fixture.command(`setblock ${pos} ${block}`);
    await fixture.command(`data merge block ${pos} {energy:${energy}}`);
    await fixture.command(`tp @s ${x + n * 2 + 0.5} ${y} ${z + 2.5}`);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === block, `${block} reaches the client`);
    await fixture.raw(`interact ${pos} south`);
    const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), `${block} screen opens`);
    return { pos, view };
  }
  /** Moves the item in the player's slot `from` into the machine's slot `to` (300 is its first). */
  async function put(from: number, to: number, item: string) {
    await fixture.raw(`click-slot ${from}`);
    await until(() => fixture.raw('screen'), v => v.carried?.item === item, `${item} on the cursor`);
    await fixture.raw(`click-slot ${to}`);
    await until(() => fixture.raw('screen'), v => !v.carried, `${item} in slot ${to}`);
  }

  // The model carries its model and data in components (registered by the bundle, API 65); the matrices are plain.
  await fixture.replace('hotbar.7', 'hostilenetworks:prediction_matrix', 3);
  const modelSlot = await giveModel('zombie', 1254);
  const chamber = await open(0, 'hostilenetworks:sim_chamber', 1000000);
  const own = (chamber.view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(own, [0, 1, 2, 3], `the model, matrix and two output slots only: ${JSON.stringify(chamber.view.slots)}`);
  await put(modelSlot, 300, 'hostilenetworks:data_model');
  await put(7, 301, 'hostilenetworks:prediction_matrix');
  fixture.mark('the data model (with its data) and the matrices go into the chamber through its screen');

  // The run: one matrix used, 240 ticks, the zombie's 128 FE a tick for all but the last.
  let nbt = await until(() => saved(chamber.pos), s => field(s, 'runtime') >= 100, 'the chamber starts a run', 40000);
  log.start = { runtime: field(nbt, 'runtime'), matrices: slotItem(nbt, 1)?.count, predSuccess: field(nbt, 'predSuccess') };
  assert.equal(slotItem(nbt, 1)?.count, 2, 'a run uses one matrix of the three');
  assert.equal(field(nbt, 'predSuccess'), 1, 'a self-aware model predicts every time');
  const t0 = await gametime();
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'chamber-running.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  nbt = await until(() => saved(chamber.pos), s => !!slotItem(s, 2), 'the run finishes', 40000);
  const elapsed = (await gametime()) - t0;
  log.elapsed = elapsed;
  assert(elapsed >= 230 && elapsed <= 270, `a run seen at runtime ${(log.start as any).runtime} takes about 240 ticks: ${elapsed}`);
  assert.equal(field(nbt, 'energy'), 1000000 - 239 * 128, 'a run costs 128 FE for each of its first 239 ticks');
  assert.equal(slotItem(nbt, 2)?.id, 'hostilenetworks:overworld_prediction', 'the zombie\'s base drop');
  const prediction = slotItem(nbt, 3);
  assert.equal(prediction?.id, 'hostilenetworks:prediction', `a prediction: ${nbt}`);
  assert.match(prediction!.text, /hostilenetworks:zombie/, 'the prediction is of the zombie');
  const model = slotItem(nbt, 0);
  assert.match(model!.text, /1254/, 'inference adds no data');
  assert.match(model!.text, /hostilenetworks:iterations": ?1\b/, 'the model has run once');
  log.finished = { energy: field(nbt, 'energy'), base: slotItem(nbt, 2)?.id, prediction: prediction?.id };
  fixture.mark(`the chamber runs one inference in 240 ticks at 128 FE: a base drop, a prediction and one iteration (energy ${field(nbt, 'energy')})`);

  // Take the prediction out through the screen into the hotbar.
  await fixture.raw('screen');
  await fixture.raw('click-slot 303');
  await until(() => fixture.raw('screen'), v => v.carried?.item === 'hostilenetworks:prediction', 'the prediction on the cursor');
  await fixture.raw('click-slot 2');
  await until(() => fixture.raw('screen'), v => !v.carried, 'the prediction in the hotbar');
  await fixture.raw('close');

  // The fabricator: a click on the palette picks the first drop; 60 ticks at 256 FE make it.
  const fab = await open(1, 'hostilenetworks:loot_fabricator', 1000000);
  const fabOwn = (fab.view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  assert.deepEqual(fabOwn, Array.from({ length: 17 }, (_, i) => i), `the prediction slot and sixteen outputs: ${JSON.stringify(fab.view.slots)}`);
  await put(2, 300, 'hostilenetworks:prediction');
  await Bun.sleep(500);
  assert.equal(field(await saved(fab.pos), 'runtime'), 0, 'nothing is made until a drop is chosen');
  const [fx, fy] = origin(176, 178);
  await fixture.raw(`click-at ${fx + 17 + 8} ${fy + 9 + 8}`);
  const t1 = await gametime();
  await Bun.sleep(300);
  const fabShot = join(fixture.run, "fabricator-selected.png");
  await fixture.raw(`screenshot ${fabShot}`);
  log.fabricatorScreenshot = fabShot;
  nbt = await until(() => saved(fab.pos), s => !!slotItem(s, 1), 'the fabricator makes the drop', 40000);
  const made = (await gametime()) - t1;
  log.made = made;
  assert(made >= 55 && made <= 120, `a drop takes 61 ticks: ${made}`);
  assert.equal(slotItem(nbt, 1)?.id, 'minecraft:rotten_flesh');
  assert.equal(slotItem(nbt, 1)?.count, 64, 'the zombie\'s first drop is 64 rotten flesh');
  assert.equal(field(nbt, 'energy'), 1000000 - 60 * 256, 'a drop costs 256 FE for each of 60 ticks');
  assert.equal(slotItem(nbt, 0), null, 'the prediction is used up');
  assert.equal(field(nbt, 'selection'), 0, 'the first drop is the chosen one');
  fixture.mark(`the fabricator makes 64 rotten flesh of one prediction in ${made} ticks at 256 FE`);
  await fixture.raw('close');

  // Training: mode button id 4 (the screen's button, at the chamber's right edge); a basic model gains a point of data.
  await fixture.replace('hotbar.7', 'hostilenetworks:prediction_matrix', 2);
  const trainModel = await giveModel('zombie', 6);
  const train = await open(2, 'hostilenetworks:sim_chamber', 1000000);
  await put(trainModel, 300, 'hostilenetworks:data_model');
  await put(7, 301, 'hostilenetworks:prediction_matrix');
  const [cx, cy] = origin(232, 230);
  await fixture.raw(`click-at ${cx + 228 + 9} ${cy + 20 + 9}`);
  await until(() => saved(train.pos), s => field(s, 'simMode') === 1, 'the mode button switches to training');
  nbt = await until(() => saved(train.pos), s => /hostilenetworks:data": ?7\b/.test(slotItem(s, 0)?.text ?? ''), 'a training run adds a point of data', 60000);
  assert.equal(slotItem(nbt, 2), null, 'training puts out no base drop');
  assert.equal(slotItem(nbt, 3), null, 'and no prediction');
  fixture.mark('a training chamber adds one point of data to a basic model and puts nothing out');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('HOSTILENETWORKS chamber', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('HOSTILENETWORKS chamber', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
