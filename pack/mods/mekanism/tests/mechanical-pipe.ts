/** Mekanism's Mechanical Pipes between Fluid Tanks, as the `mekanism` native bundle and its content mods make them,
 * under ATM10's configs. The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave
 * for the same steps over RCON (`/tick step`, `data get block`; local `mekm2-work/oracle`, x3 and x3b):
 *
 * - Pull per tier: a creative water tank, a pipe pulling from it, a second pipe and an ultimate tank: the tank gets
 *   250 a tick through basic pipes, 1000 advanced, 8000 elite, 32 000 ultimate, from the tick after they are placed.
 * - Capacity: two basic pipes pulling with nothing at their end hold 2000 each; a tank then placed at the end gets
 *   their 4000 the next tick and 250 a tick after (advanced: 8000 each, 16 000, then 1000 a tick).
 * - An active ultimate tank above a basic pipe above a tank: the network splits what it takes between both tanks,
 *   so the upper loses and the lower gets 1000 a tick; a push side takes nothing, a none side doesn't connect; a pull
 *   side sends nothing back, so 2000 a tick go down. An active ultimate tank gives a basic tank below 32 000, or the
 *   500 one holding 31 500 still takes.
 * - One fluid a network: a basic pipe placed between a water line and a pipe pulling lava joins the water; the lava
 *   pipe keeps giving its tank 250 a tick, the water line holds what it pulls.
 * - Shares: three basic pipes that pulled 19 times hold 4750, split evenly when saved (Mekanism: 1583, 1584,
 *   1583), which a restart keeps. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-pipe', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'tiers.toml'), '[fluid_tanks]\nbasicCapacity = 32000\n');
}

const W = 'minecraft:water', L = 'minecraft:lava';
const MAX = 2147483647;
const tank = (tier: string) => `mekanism:${tier}_fluid_tank`;
const pipe = (tier: string) => `mekanism:${tier}_mechanical_pipe`;
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
const amount = async (pos: string) => (await mek(`mek probe ${pos}`)).amount ?? 0;
const held = (fluid: string, mb: number) => `{fluid_tanks:[{tank:0b,stored:{id:"${fluid}",amount:${mb}}}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -2)} ${at(10, 4, 34)} minecraft:air`);
  await fixture.hold();
  let row = 0;

  // Pull per tier: creative tank, pipe pulling west, pipe, ultimate tank.
  const rates: Record<string, number[]> = {};
  for (const tier of ['basic', 'advanced', 'elite', 'ultimate']) {
    const a = at(0, 0, row), b = at(3, 0, row);
    await fixture.command(`setblock ${a} ${tank('creative')}`);
    await fixture.command(`data merge block ${a} ${held(W, MAX)}`);
    await fixture.command(`setblock ${at(1, 0, row)} ${pipe(tier)}`);
    await fixture.command(`setblock ${at(2, 0, row)} ${pipe(tier)}`);
    await fixture.command(`setblock ${b} ${tank('ultimate')}`);
    await mek(`mek connection ${at(1, 0, row)} west pull`);
    await fixture.stepServer(1);
    const seen: number[] = [];
    for (let t = 0; t < 4; t++) { await fixture.stepServer(1); seen.push(await amount(b)); }
    rates[tier] = seen;
    row += 2;
  }
  log.rates = rates;
  assert.deepEqual(rates, { basic: [250, 500, 750, 1000], advanced: [1000, 2000, 3000, 4000], elite: [8000, 16000, 24000, 32000], ultimate: [32000, 64000, 96000, 128000] });
  fixture.mark('a pull side draws 250, 1000, 8000 and 32 000 mB a tick by tier, and the network passes it on the same tick');

  // Capacity: two pipes pulling with nothing at their end, then a tank there.
  const filled: Record<string, unknown> = {};
  for (const [tier, each, rate] of [['basic', 2000, 250], ['advanced', 8000, 1000]] as const) {
    const a = at(0, 0, row), b = at(3, 0, row);
    await fixture.command(`setblock ${a} ${tank('creative')}`);
    await fixture.command(`data merge block ${a} ${held(W, MAX)}`);
    await fixture.command(`setblock ${at(1, 0, row)} ${pipe(tier)}`);
    await fixture.command(`setblock ${at(2, 0, row)} ${pipe(tier)}`);
    await mek(`mek connection ${at(1, 0, row)} west pull`);
    await fixture.stepServer(18);
    const shares = [(await mek(`mek probe ${at(1, 0, row)}`)).saved_share, (await mek(`mek probe ${at(2, 0, row)}`)).saved_share];
    await fixture.command(`setblock ${b} ${tank('ultimate')}`);
    const seen: number[] = [];
    for (let t = 0; t < 3; t++) { await fixture.stepServer(1); seen.push(await amount(b)); }
    filled[tier] = { shares, seen };
    assert.deepEqual(shares, [each, each]);
    assert.deepEqual(seen, [2 * each, 2 * each + rate, 2 * each + 2 * rate]);
    row += 2;
  }
  log.filled = filled;
  fixture.mark('two pipes fill to their capacity (2000, 8000 each), and a tank placed at their end gets it all the next tick');

  // An active tank above a pipe above a tank: the up side normal, push, none and pull.
  const stacked: Record<string, number[][]> = {};
  for (const [name, side] of [['normal', ''], ['push', 'push'], ['none', 'none'], ['pull', 'pull']]) {
    const a = at(0, 1, row), p = at(0, 0, row), b = at(0, -1, row);
    await fixture.command(`setblock ${a} ${tank('ultimate')}`);
    await fixture.command(`data merge block ${a} {fluid_tanks:[{tank:0b,stored:{id:"${W}",amount:100000}}],active_state:1b}`);
    await fixture.command(`setblock ${p} ${pipe('basic')}`);
    await fixture.command(`setblock ${b} ${tank('ultimate')}`);
    if (side) await mek(`mek connection ${p} up ${side}`);
    await fixture.stepServer(1);
    const seen: number[][] = [];
    for (let t = 0; t < 3; t++) { await fixture.stepServer(1); seen.push([await amount(a), await amount(b)]); }
    stacked[name] = seen;
    row += 2;
  }
  log.stacked = stacked;
  assert.deepEqual(stacked, {
    normal: [[99000, 1000], [98000, 2000], [97000, 3000]],
    push: [[100000, 0], [100000, 0], [100000, 0]],
    none: [[100000, 0], [100000, 0], [100000, 0]],
    pull: [[98000, 2000], [96000, 4000], [94000, 6000]],
  });
  for (const [name, below] of [['empty', 0], ['nearly_full', 31500]] as const) {
    const a = at(0, 1, row), b = at(0, 0, row);
    await fixture.command(`setblock ${a} ${tank('ultimate')}`);
    await fixture.command(`data merge block ${a} {fluid_tanks:[{tank:0b,stored:{id:"${W}",amount:100000}}],active_state:1b}`);
    await fixture.command(`setblock ${b} ${tank('basic')}`);
    if (below) await fixture.command(`data merge block ${b} ${held(W, below)}`);
    await fixture.stepServer(1);
    stacked[name] = [[await amount(a), await amount(b)]];
    row += 2;
  }
  assert.deepEqual([stacked.empty, stacked.nearly_full], [[[68000, 32000]], [[99500, 32000]]]);
  fixture.mark('an active tank gives down: a pipe\'s network splits it between the tanks, push and none take nothing, pull sends none back');

  // One fluid a network: a water line, a lava pipe, a pipe placed between them.
  const wa = at(0, 0, row), lb = at(5, 0, row);
  await fixture.command(`setblock ${wa} ${tank('creative')}`);
  await fixture.command(`data merge block ${wa} ${held(W, MAX)}`);
  await fixture.command(`setblock ${at(1, 0, row)} ${pipe('basic')}`);
  await fixture.command(`setblock ${at(2, 0, row)} ${pipe('basic')}`);
  await fixture.command(`setblock ${at(4, 1, row)} ${tank('creative')}`);
  await fixture.command(`data merge block ${at(4, 1, row)} ${held(L, MAX)}`);
  await fixture.command(`setblock ${at(4, 0, row)} ${pipe('basic')}`);
  await fixture.command(`setblock ${lb} ${tank('ultimate')}`);
  await mek(`mek connection ${at(1, 0, row)} west pull`);
  await mek(`mek connection ${at(4, 0, row)} up pull`);
  await fixture.stepServer(1);
  await fixture.command(`setblock ${at(3, 0, row)} ${pipe('basic')}`);
  const mixed: unknown[] = [];
  for (let t = 0; t < 3; t++) {
    await fixture.stepServer(1);
    const middle = await mek(`mek probe ${at(3, 0, row)}`);
    mixed.push([await amount(lb), (await mek(`mek probe ${lb}`)).fluid, middle.network?.fluid, middle.network?.buffer, middle.faces.east, middle.faces.west]);
  }
  log.mixed = mixed;
  assert.deepEqual(mixed, [
    [250, L, W, 250, 'none', 'normal'],
    [500, L, W, 500, 'none', 'normal'],
    [750, L, W, 750, 'none', 'normal'],
  ]);
  row += 2;
  fixture.mark('a pipe between a water line and a lava pipe joins the water; the lava pipe keeps feeding its tank');

  // Shares: three basic pipes pulling, the source then taken away, a save and a restart.
  const sa = at(0, 0, row);
  await fixture.command(`setblock ${sa} ${tank('creative')}`);
  await fixture.command(`data merge block ${sa} ${held(W, MAX)}`);
  for (const dx of [1, 2, 3]) await fixture.command(`setblock ${at(dx, 0, row)} ${pipe('basic')}`);
  await mek(`mek connection ${at(1, 0, row)} west pull`);
  await fixture.stepServer(20);
  await fixture.command(`setblock ${sa} minecraft:air`);
  await fixture.stepServer(1);
  const shares: number[] = [];
  for (const dx of [1, 2, 3]) shares.push((await mek(`mek probe ${at(dx, 0, row)}`)).saved_share);
  log.shares = shares;
  assert.equal(shares.reduce((s, v) => s + v, 0), 4750);
  assert(Math.max(...shares) - Math.min(...shares) <= 1, `even: ${shares}`);
  const shareRow = row;
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  await fixture.stepServer(2);
  const after = await mek(`mek probe ${at(2, 0, shareRow)}`);
  log.after = after;
  assert.deepEqual([after.network?.buffer, after.network?.fluid, after.network?.pipes], [4750, W, 3]);
  fixture.mark('three pipes save 4750 mB in even shares, and a restart brings the network back with all of it');

  fixture.report('passed');
  console.log(JSON.stringify(log));
} catch (error) {
  console.log(JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
