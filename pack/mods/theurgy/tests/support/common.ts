/** Shared by the Theurgy world tests: the fixture on the content mod, the saved compounds as `data get block` prints
 * them, block state checks and a cleared arena. Not a test itself (the `world` step runs the files beside this
 * folder). */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../../ae2/tests/world/harness';
import { root } from '../../../../../ae2/scripts/common';

export { until };
export const content = resolve(process.env.THEURGY_CONTENT_DIR ?? join(root, '../.build/content/theurgy'));

export function theurgyFixture(label: string): WorldFixture {
  return new WorldFixture(label, 'server-thread', { devMods: [content] });
}

export async function chat(f: WorldFixture, command: string): Promise<string> {
  return ((await f.command(command)).chat ?? []).join('\n');
}

/** The block entity's compound on the server. */
export async function saved(f: WorldFixture, pos: string): Promise<string> {
  const text = await chat(f, `data get block ${pos}`);
  assert(text.includes('has the following block data'), `no block entity at ${pos}: ${text}`);
  return text;
}

export const field = (nbt: string, name: string): number => {
  const found = nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?(-?[\\d.]+)`));
  return found ? Number(found[1]) : 0;
};

/** The entry of slot `slot` in the saved `Items` (`{Slot: 0b, id: .., count: ..}`), '' for none. */
export function slotEntry(nbt: string, slot: number): string {
  const at = nbt.indexOf('Items: [');
  if (at < 0) return '';
  for (const entry of nbt.slice(at).match(/\{[^{}]*\}/g) ?? []) {
    if (new RegExp(`Slot: ?${slot}b`).test(entry)) return entry;
  }
  return '';
}
export const slotCount = (nbt: string, slot: number) => field(slotEntry(nbt, slot), 'count');
export const slotItem = (nbt: string, slot: number) => slotEntry(nbt, slot).match(/id: ?"([^"]+)"/)?.[1] ?? '';

/** A tank as `FluidTank.writeToNBT` saves it: its fluid and amount ('' and 0 when empty). */
export function tank(nbt: string, name: string): { fluid: string; amount: number } {
  const m = nbt.match(new RegExp(`${name}: ?\\{Fluid: ?\\{([^}]*)\\}`));
  if (!m) return { fluid: '', amount: 0 };
  return { fluid: m[1].match(/id: ?"([^"]+)"/)?.[1] ?? '', amount: field(m[1], 'amount') };
}

export async function isBlock(f: WorldFixture, pos: string, state: string): Promise<boolean> {
  return (await chat(f, `execute if block ${pos} ${state}`)).includes('Test passed');
}

/** A stone floor and air above, beside the player: `at(dx, dy, dz)` names a block of it. */
export async function arena(f: WorldFixture) {
  const start = await f.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py), z = Math.floor(pz) + 3;
  const at = (dx = 0, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const c of [
    'gamerule doMobSpawning false', 'gamerule doDaylightCycle false', 'time set noon',
    `fill ${at(-4, -1, -4)} ${at(4, -1, 4)} minecraft:stone`, `fill ${at(-4, 0, -4)} ${at(4, 4, 4)} minecraft:air`,
  ]) await f.command(c);
  return { x, y, z, at };
}

/** Puts `count` of `item` in the hand (hotbar 0, selected); none for an empty hand. */
export async function hand(f: WorldFixture, item?: string, count = 1): Promise<void> {
  if (item) {
    await f.replace('hotbar.0', item, count);
    await f.select(0, item);
  } else {
    await f.command('item replace entity @s hotbar.0 with minecraft:air');
  }
}

export async function handCount(f: WorldFixture): Promise<{ item?: string; count: number }> {
  const slot = (await f.raw('inventory')).hotbar?.[0];
  return { item: slot?.item, count: slot?.count ?? 0 };
}

/** How many of `item` the player carries. */
export async function carried(f: WorldFixture, item: string): Promise<number> {
  const inv = await f.raw('inventory');
  const all = [...(inv.hotbar ?? []), ...(inv.main ?? inv.inventory ?? [])];
  return all.filter((s: any) => s?.item === item).reduce((n: number, s: any) => n + (s.count ?? 0), 0);
}
