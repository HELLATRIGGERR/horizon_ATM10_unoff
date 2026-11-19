/** Iron Furnaces' furnaces, as the `ironfurnaces` native bundle and the carry of the player's jar make them.
 *
 * - The screen shows the input, fuel and output slots and none of the menu's other six, under the block's name.
 * - An iron furnace (160 ticks for a 200-tick recipe, ATM10's `config/ironfurnaces-common.toml`) lights one coal for
 *   1,280 ticks and smelts a cobblestone every 160 ticks: three more cobblestones in 480 game ticks, where vanilla's
 *   furnace takes 600. A diamond furnace takes 80 and a netherite one 5, and a coal is worth 640 and 40 ticks.
 * - The Allthemodium furnace smelts 16 items at once.
 * - A Gold Upgrade used by a sneaking player on a burning iron furnace turns it into a gold furnace with its input,
 *   fuel, output and timers; the gold furnace then cooks at 120 ticks. The same upgrade does nothing to a furnace
 *   that isn't an iron one.
 *
 * Reads the saved compound (`data get block`, the bundle's own keys). A picture of the screen, burning, goes to the
 * run folder (`furnace-burning.png`). Needs the staged native build (`AE2_TEST_BINARY`) and the content mod
 * made from the carry of Iron Furnaces (`IRONFURNACES_CONTENT_DIR`); `pack/scripts/ironfurnaces.ts world` sets both. With `ATM10_DIR` the world reads
 * ATM10's own `config/ironfurnaces-common.toml`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.IRONFURNACES_CONTENT_DIR ?? join(root, '../.build/content/ironfurnaces'));
const fixture = new WorldFixture('ironfurnaces-furnace', 'server-thread', { devMods: [content] });
const atm10 = process.env.ATM10_DIR;
const fromAtm10 = !!atm10 && existsSync(join(atm10, 'config/ironfurnaces-common.toml'));
if (fromAtm10) {
  const config = join(fixture.game, 'config');
  mkdirSync(config, { recursive: true });
  copyFileSync(join(atm10!, 'config/ironfurnaces-common.toml'), join(config, 'ironfurnaces-common.toml'));
}

/** The block entity's compound on the server, as `data get block` prints it. */
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const field = (nbt: string, name: string) => {
  const found = nbt.match(new RegExp(`${name}: ?(-?[\\d.]+)`));
  return found ? Number(found[1]) : 0;
};
/** The item in a container slot of the saved compound: a flat `{Slot: 0b, count: 3, id: "minecraft:coal"}`. */
function slotItem(nbt: string, slot: number): { id: string; count: number } | null {
  for (const entry of nbt.match(/\{[^{}]*\}/g) ?? []) {
    if (new RegExp(`Slot: ?${slot}b`).test(entry)) {
      return { id: entry.match(/id: ?"([^"]+)"/)?.[1] ?? '', count: field(entry, 'count') };
    }
  }
  return null;
}
const gametime = async () => Number(((await fixture.command('time query gametime')).chat ?? []).join(' ').match(/(\d+)/)?.[1]);
const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 1} ${y} ${z - 2} ${x + 12} ${y + 3} ${z + 2} minecraft:air`);
  await fixture.command(`fill ${x - 1} ${y - 1} ${z - 2} ${x + 12} ${y - 1} ${z + 2} minecraft:stone`);
  const at = (n: number) => `${x + n * 2} ${y} ${z}`;

  /** Places a furnace and fills it through its screen (the input from hotbar 0, the fuel from hotbar 1), leaving
   * the screen open when `keepOpen`. */
  async function place(n: number, block: string, input: [string, number], fuel: [string, number], keepOpen = false) {
    const pos = at(n);
    await fixture.command(`setblock ${pos} ${block}`);
    await fixture.command(`tp @s ${x + n * 2 + 0.5} ${y} ${z + 2.5}`);
    await fixture.replace('hotbar.0', input[0], input[1]);
    await fixture.replace('hotbar.1', fuel[0], fuel[1]);
    await until(() => fixture.raw(`block ${pos}`), v => v.block === block, `${block} reaches the client`);
    await fixture.raw(`interact ${pos} south`);
    const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), `${block} screen opens`);
    const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
    assert.deepEqual(own, [0, 1, 2], `the input, fuel and output slots only: ${JSON.stringify(view.slots)}`);
    await fixture.raw('click-slot 0');
    await until(() => fixture.raw('screen'), v => v.carried?.item === input[0], 'the input on the cursor');
    await fixture.raw('click-slot 300');
    await until(() => fixture.raw('screen'), v => !v.carried, 'the input in its slot');
    await fixture.raw('click-slot 1');
    await until(() => fixture.raw('screen'), v => v.carried?.item === fuel[0], 'the fuel on the cursor');
    await fixture.raw('click-slot 301');
    await until(() => fixture.raw('screen'), v => !v.carried, 'the fuel in its slot');
    if (!keepOpen) await fixture.raw('close');
    return { pos, view };
  }

  // The iron furnace: its screen, its numbers, and its speed in game time.
  const iron = await place(0, 'ironfurnaces:iron_furnace', ['minecraft:cobblestone', 32], ['minecraft:coal', 2], true);
  const title = (iron.view.title ?? []).map((span: any) => span.text).join('');
  log.title = title;
  assert.match(title, /Iron Furnace/);
  let nbt = await until(() => saved(iron.pos), s => field(s, 'BurnTime') > 0, 'the iron furnace lights its coal');
  log.iron = { total: field(nbt, 'CookTimeTotal'), used: field(nbt, 'RecipesUsed'), coal: slotItem(nbt, 1) };
  assert.equal(field(nbt, 'CookTimeTotal'), 160, 'an iron furnace cooks a 200-tick recipe in 160 ticks');
  assert.equal(field(nbt, 'RecipesUsed'), 1280, 'a coal (1,600 ticks) is worth 1,280 ticks of an iron furnace');
  assert.equal(slotItem(nbt, 1)?.count, 1, 'one coal of two went into the fire');
  fixture.mark('the iron furnace lights a coal for 1,280 ticks and cooks in 160');
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'furnace-burning.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('close');
  await until(() => saved(iron.pos), s => (slotItem(s, 2)?.count ?? 0) >= 1, 'the first cobblestone is smelted', 40000);
  const t0 = await gametime();
  nbt = await until(() => saved(iron.pos), s => (slotItem(s, 2)?.count ?? 0) >= 4, 'three more are smelted', 60000);
  const elapsed = (await gametime()) - t0;
  log.iron_elapsed = elapsed;
  assert(elapsed >= 440 && elapsed <= 520, `three cooks of 160 ticks take about 480 game ticks, not vanilla's 600: ${elapsed}`);
  assert.equal(slotItem(nbt, 2)?.id, 'minecraft:stone');
  fixture.mark(`the iron furnace smelts at 160 ticks: three cobblestones in ${elapsed} game ticks`);

  // Other tiers: the cooking time, and what a coal is worth.
  for (const [n, block, total, used] of [
    [1, 'ironfurnaces:diamond_furnace', 80, 640],
    [2, 'ironfurnaces:netherite_furnace', 5, 40],
    [3, 'ironfurnaces:copper_furnace', 180, 1440],
  ] as const) {
    const f = await place(n, block, ['minecraft:cobblestone', 8], ['minecraft:coal', 1]);
    const s = await until(() => saved(f.pos), v => field(v, 'RecipesUsed') > 0, `${block} lights`);
    assert.equal(field(s, 'CookTimeTotal'), total, `${block} cooking time`);
    assert.equal(field(s, 'RecipesUsed'), used, `${block} fuel value`);
  }
  fixture.mark('the diamond (80), netherite (5) and copper (180) furnaces cook at their own speeds and a coal is worth 640, 40 and 1,440 ticks');

  // The Allthemodium furnace smelts 16 items a cooking.
  const atm = await place(4, 'ironfurnaces:allthemodium_furnace', ['minecraft:cobblestone', 48], ['minecraft:coal', 1]);
  nbt = await until(() => saved(atm.pos), s => (slotItem(s, 2)?.count ?? 0) >= 16, 'the Allthemodium furnace smelts');
  assert.equal((slotItem(nbt, 2)?.count ?? 0) % 16, 0, `the output grows by 16: ${JSON.stringify(slotItem(nbt, 2))}`);
  fixture.mark('the Allthemodium furnace smelts 16 items in a cooking');

  // The upgrade: a burning iron furnace becomes a gold one with all it held.
  const up = await place(5, 'ironfurnaces:iron_furnace', ['minecraft:cobblestone', 20], ['minecraft:coal', 3]);
  await until(() => saved(up.pos), s => field(s, 'BurnTime') > 0 && (slotItem(s, 2)?.count ?? 0) >= 1, 'the furnace burns and has made one', 40000);
  const before = await saved(up.pos);
  await fixture.replace('hotbar.2', 'ironfurnaces:upgrade_diamond');
  await fixture.select(2, 'ironfurnaces:upgrade_diamond');
  await fixture.sneakInteract(`${up.pos} south`);
  await Bun.sleep(800);
  assert.equal((await fixture.raw(`block ${up.pos}`)).block, 'ironfurnaces:iron_furnace', 'a Diamond Upgrade does nothing to an iron furnace');
  await fixture.replace('hotbar.2', 'ironfurnaces:upgrade_gold');
  await fixture.select(2, 'ironfurnaces:upgrade_gold');
  await fixture.sneakInteract(`${up.pos} south`);
  await until(() => fixture.raw(`block ${up.pos}`), v => v.block === 'ironfurnaces:gold_furnace', 'the Gold Upgrade makes a gold furnace');
  nbt = await until(() => saved(up.pos), s => field(s, 'CookTimeTotal') === 120, 'the gold furnace cooks at 120 ticks');
  log.upgrade = { before, after: nbt };
  assert((slotItem(nbt, 0)?.count ?? 0) > 0 && slotItem(nbt, 0)?.id === 'minecraft:cobblestone', `the input stays: ${nbt}`);
  assert((slotItem(nbt, 1)?.count ?? 0) > 0 && slotItem(nbt, 1)?.id === 'minecraft:coal', `the fuel stays: ${nbt}`);
  assert((slotItem(nbt, 2)?.count ?? 0) >= (slotItem(before, 2)?.count ?? 0), 'the output stays');
  assert(field(nbt, 'BurnTime') > 0, 'it is still burning');
  fixture.mark('a Gold Upgrade turns a burning iron furnace into a gold one with its input, fuel, output and fire, and cooking at 120');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('IRONFURNACES furnace', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('IRONFURNACES furnace', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
