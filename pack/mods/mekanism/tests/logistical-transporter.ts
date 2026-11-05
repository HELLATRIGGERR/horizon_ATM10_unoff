/** Mekanism's Logistical Transporters between chests, as the `mekanism` native bundle and its content mods make them,
 * under ATM10's configs. The numbers are those a NeoForge 1.21.1 server with Mekanism 10.7.19 and ATM10's configs gave
 * for the same steps over RCON (`/tick step`, `data get block`; local `mekm2-work/oracle`, x4), counting the tick
 * they are placed as the first:
 *
 * - Speed and pull: a chest of two stacks of cobblestone, three transporters (the first pulling from the chest) and a
 *   chest. The first pull comes on the second tick, then every 11; a basic stack (1 item) is in the far chest on
 *   tick 59, an advanced one (16) on 29, an elite (32) on 14, an ultimate (64) on 5.
 * - The cheapest target first: an elite line with a chest of one free slot above its second transporter and an empty
 *   chest at its end: the near chest takes the first two pulls of 32, the third and fourth (the near one full with
 *   what is on its way) go to the far one, by tick 55.
 * - Blocked: the far chest taken away while an advanced stack travels: in the last transporter, past half way, it
 *   turns for home and is back in its chest on tick 49; with its chest gone too it wanders the line (idle), nothing
 *   dropped.
 * - Colours: a red puller beside a blue transporter pulls nothing, a plain one beside a red transporter nothing; a red
 *   puller beside a plain one sends red stacks through it.
 * - A Restrictive Transporter (a basic one costing 1000 a step) passes stacks when it is the only way.
 * - A save and a restart keep the stacks in transit, which then arrive. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-transporter', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'tiers.toml'), '[transmitters.items]\nbasicSpeed = 5\n');
}

const transporter = (tier: string) => `mekanism:${tier}_logistical_transporter`;
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
async function count(pos: string): Promise<number> {
  const reply = await fixture.command(`data get block ${pos} Items`);
  const line = (reply.chat ?? []).find((t: string) => t.includes('has the following block data')) ?? '';
  return [...line.matchAll(/count: ?(\d+)/g)].reduce((s, m) => s + Number(m[1]), 0);
}
const items = (...stacks: [number, string, number][]) => `{Items:[${stacks.map(([slot, id, n]) => `{Slot:${slot}b,id:"${id}",count:${n}}`).join(',')}]}`;

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-2, -2, -2)} ${at(10, 4, 26)} minecraft:air`);
  await fixture.hold();
  let row = 0;

  /** A chest at x0 of `stacks`, transporters of `tiers` from x1 (the first pulling west), and an empty chest after them. */
  async function line(tiers: string[], stacks: [number, string, number][], colors: (string | null)[] = []): Promise<[string, string]> {
    const a = at(0, 0, row), b = at(tiers.length + 1, 0, row);
    await fixture.command(`setblock ${a} minecraft:chest`);
    await fixture.command(`data merge block ${a} ${items(...stacks)}`);
    for (const [i, tier] of tiers.entries()) {
      await fixture.command(`setblock ${at(i + 1, 0, row)} ${transporter(tier)}`);
      if (colors[i]) await mek(`mek color ${at(i + 1, 0, row)} ${colors[i]}`);
    }
    await mek(`mek connection ${at(1, 0, row)} west pull`);
    await fixture.command(`setblock ${b} minecraft:chest`);
    return [a, b];
  }

  // Speed and pull, by tier: checkpoints (tick: source, target) from the oracle.
  const cobble: [number, string, number][] = [[0, 'minecraft:cobblestone', 64], [1, 'minecraft:cobblestone', 64]];
  const tiers: Record<string, [number, number, number][]> = {
    basic: [[2, 127, 0], [12, 127, 0], [13, 126, 0], [58, 122, 0], [59, 122, 1], [70, 121, 2]],
    advanced: [[2, 112, 0], [13, 96, 0], [28, 80, 0], [29, 80, 16], [40, 64, 32]],
    elite: [[2, 96, 0], [13, 64, 0], [14, 64, 32], [25, 32, 64]],
    ultimate: [[2, 64, 0], [4, 64, 0], [5, 64, 64], [13, 0, 64], [16, 0, 128]],
  };
  const speeds: Record<string, unknown> = {};
  for (const [tier, checks] of Object.entries(tiers)) {
    const [a, b] = await line([tier, tier, tier], cobble);
    await fixture.stepServer(1);
    let t = 1;
    const seen: [number, number, number][] = [];
    for (const [tick] of checks) {
      await fixture.stepServer(tick - t);
      t = tick;
      seen.push([tick, await count(a), await count(b)]);
      if (tier === 'basic' && tick === 12) {
        const p = await mek(`mek probe ${at(1, 0, row)}`);
        log.basic_t12 = p;
        assert.deepEqual(p.transit.map((s: any) => [s.count, s.progress, s.path_type]), [[1, 55, 'Dest']]);
        assert.deepEqual([p.faces.west, p.faces.east, p.faces.up], ['pull', 'normal', 'none']);
        const view = (await fixture.command(`data get block ${at(1, 0, row)} horizon_transit`)).chat?.join(' ') ?? '';
        assert(/cobblestone/.test(view), `the client view holds the stack: ${view}`);
      }
    }
    speeds[tier] = seen;
    assert.deepEqual(seen, checks, tier);
    row += 2;
  }
  log.speeds = speeds;
  fixture.mark('pulls of 1, 16, 32 and 64 every 11 ticks, stacks arriving on ticks 59, 29, 14 and 5 by tier, as on Mekanism');

  // The cheapest target first, and the stacks on their way.
  const near = at(2, 1, row);
  const [ta, tb] = await line(['elite', 'elite', 'elite', 'elite', 'elite'], cobble);
  await fixture.command(`setblock ${near} minecraft:chest`);
  await fixture.command(`data merge block ${near} ${items(...Array.from({ length: 26 }, (_, s) => [s, 'minecraft:stone', 64] as [number, string, number]))}`);
  await fixture.stepServer(1);
  const nearest: Record<number, number[]> = {};
  let t = 1;
  for (const tick of [10, 25, 35, 45, 55]) {
    await fixture.stepServer(tick - t);
    t = tick;
    nearest[tick] = [await count(ta), (await count(near)) - 26 * 64, await count(tb)];
  }
  log.nearest = nearest;
  assert.deepEqual(nearest, { 10: [96, 32, 0], 25: [32, 64, 0], 35: [0, 64, 0], 45: [0, 64, 32], 55: [0, 64, 64] });
  row += 3;
  fixture.mark('the near chest takes two pulls, then (full with what is on its way) the far one the rest');

  // Blocked: home, or idle.
  const [ha, hb] = await line(['advanced', 'advanced', 'advanced'], [[0, 'minecraft:cobblestone', 64]]);
  await fixture.stepServer(6);
  await fixture.command(`setblock ${hb} minecraft:air`);
  await fixture.stepServer(18);
  const turned = await mek(`mek probe ${at(3, 0, row)}`);
  log.turned = turned;
  assert.deepEqual(turned.transit.map((s: any) => [s.progress, s.path_type]), [[50, 'Home']]);
  await fixture.stepServer(24);
  const home = [await count(ha)];
  await fixture.stepServer(1);
  home.push(await count(ha), (await mek(`mek probe ${at(1, 0, row)}`)).transit.length);
  log.home = home;
  assert.deepEqual(home, [48, 64, 0], 'back home on tick 49');
  row += 2;
  const [ia, ib] = await line(['advanced', 'advanced', 'advanced'], [[0, 'minecraft:cobblestone', 16]]);
  await fixture.stepServer(6);
  await fixture.command(`setblock ${ib} minecraft:air`);
  await fixture.command(`setblock ${ia} minecraft:air`);
  await fixture.stepServer(18);
  const idle = await mek(`mek probe ${at(3, 0, row)}`);
  log.idle = idle;
  assert.deepEqual(idle.transit.map((s: any) => [s.progress, s.path_type, s.idle_dir]), [[50, 'None', 4]]);
  await fixture.stepServer(20);
  const wandering = [1, 2, 3].map(dx => at(dx, 0, row));
  let travelling = 0;
  for (const p of wandering) travelling += (await mek(`mek probe ${p}`)).transit.length;
  assert.equal(travelling, 1, 'still travelling');
  const dropped = await fixture.command(`execute if entity @e[type=minecraft:item,x=${x - 2},y=${y - 2},z=${z + row - 2},dx=8,dy=4,dz=4]`);
  assert(!(dropped.chat ?? []).some((l: string) => /passed/i.test(l)), 'nothing dropped');
  row += 2;
  fixture.mark('a stack whose target goes turns for home past half way; with no home it wanders the line');

  // Colours.
  const colours: Record<string, number[]> = {};
  for (const [name, c1, c2] of [['red_blue', 'red', 'dark_blue'], ['plain_red', null, 'red'], ['red_plain', 'red', null]] as const) {
    const [a, b] = await line(['elite', 'elite'], [[0, 'minecraft:cobblestone', 64]], [c1, c2]);
    await fixture.stepServer(20);
    colours[name] = [await count(a), await count(b)];
    if (name === 'red_plain') {
      const second = await mek(`mek probe ${at(2, 0, row)}`);
      assert.deepEqual(second.transit.map((s: any) => s.color), ['Red']);
    }
    row += 2;
  }
  log.colours = colours;
  assert.deepEqual(colours, { red_blue: [64, 0], plain_red: [64, 0], red_plain: [0, 32] });
  fixture.mark('transporters of two colours don\'t connect, a stack only enters one of its colour, and takes its puller\'s');

  // A Restrictive Transporter: the only way, it passes stacks (at the basic speed); it takes no colour.
  const ra = at(0, 0, row), rb = at(4, 0, row);
  await fixture.command(`setblock ${ra} minecraft:chest`);
  await fixture.command(`data merge block ${ra} ${items([0, 'minecraft:cobblestone', 64])}`);
  await fixture.command(`setblock ${at(1, 0, row)} ${transporter('ultimate')}`);
  await fixture.command(`setblock ${at(2, 0, row)} mekanism:restrictive_transporter`);
  await fixture.command(`setblock ${at(3, 0, row)} ${transporter('ultimate')}`);
  await fixture.command(`setblock ${rb} minecraft:chest`);
  await mek(`mek connection ${at(1, 0, row)} west pull`);
  await fixture.stepServer(30);
  const restricted = [await count(ra), await count(rb), (await mek(`mek color ${at(2, 0, row)} red`)).error];
  log.restrictive = restricted;
  assert.deepEqual(restricted, [0, 64, 'not a transporter']);
  row += 2;
  fixture.mark('a restrictive transporter passes stacks when it is the only way, and takes no colour');

  // A save and a restart with stacks in transit: none lost or made (the server runs on a little between the
  // count and the save), and they go on.
  const [sa, sb] = await line(['basic', 'basic', 'basic'], [[0, 'minecraft:cobblestone', 64]]);
  await fixture.stepServer(25);
  const carried = async (r: number) => {
    let n = 0;
    for (const dx of [1, 2, 3]) n += (await mek(`mek probe ${at(dx, 0, r)}`)).transit.reduce((s: number, t: any) => s + t.count, 0);
    return n;
  };
  const before = await carried(row);
  const saveRow = row;
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  await fixture.stepServer(1);
  const after = [await count(sa), await carried(saveRow), await count(sb)];
  await fixture.stepServer(60);
  const later = [await count(sa), await carried(saveRow), await count(sb)];
  log.restart = { before, after, later };
  assert.equal(before, 3);
  assert(after[1] >= 1, 'stacks in transit after the restart');
  assert.equal(after.reduce((s, v) => s + v, 0), 64, 'none lost or made');
  assert.equal(later.reduce((s, v) => s + v, 0), 64);
  assert(later[2] > after[2], `they arrive: ${later}`);
  await fixture.hold(false);
  fixture.mark('a restart keeps the stacks in transit, and they arrive');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK transporter', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK transporter', JSON.stringify(log));
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
