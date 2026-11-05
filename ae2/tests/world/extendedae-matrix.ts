/** ExtendedAE slice 3, the Assembler Matrix: a box of its blocks forms only within its size (corners 6 apart
 * at most, ATM10's default) and takes one channel for all its blocks; its screen takes crafting patterns into a
 * pattern core and sets the Pattern Access Terminal for the cluster; a crafting CPU with seven co-processors
 * sends a crafter core eight jobs at once, its threads crafting side by side; speed cores make a craft faster;
 * a wall taken away breaks it, and put back forms it again. The Extended Molecular Assembler behind a pattern
 * provider runs several jobs at once too. Needs content/extendedae (ae2/scripts/prepare-addon.ts extendedae)
 * and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-matrix', mode, { addons: ['extendedae'] });
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slot = (view: any, n: number) => view.slots?.find((s: any) => s.slot === n);
const FRAME = 'extendedae:assembler_matrix_frame', WALL = 'extendedae:assembler_matrix_wall', GLASS = 'extendedae:assembler_matrix_glass';
const CRAFTER = 'extendedae:assembler_matrix_crafter', PATTERN = 'extendedae:assembler_matrix_pattern', SPEED = 'extendedae:assembler_matrix_speed';
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-6, -4], [-6, 12], [12, -4], [12, 12]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-6, 0, -4)} ${at(12, 7, 12)} minecraft:air`);
  await fixture.command(`fill ${at(-6, -1, -4)} ${at(12, -1, 12)} minecraft:stone`);
  await fixture.command('clear @s');
  const contents = async (pos: string) => (await fixture.command(`data get block ${pos}`)).chat?.join('\n') ?? '';
  const block = async (pos: string) => (await fixture.raw(`getblock ${pos}`)).block as string;
  const formed = async (pos: string) => (await block(pos)).includes('formed=true');
  // Threads with a job: AE2's assembler keeps it as `ae2_assembler_pattern`, each thread past the first in its own compound.
  const busy = async (pos: string) => ((await contents(pos)).match(/ae2_assembler_pattern/g) ?? []).length;
  /** A matrix box from (dx0, dy0, dz0) to (dx1, dy1, dz1): frames on its edges, walls on its faces, `inner` inside. */
  const box = async (d0: [number, number, number], d1: [number, number, number], inner: (dx: number, dy: number, dz: number) => string) => {
    const [a, b, c] = d0, [p, q, r] = d1;
    await fixture.command(`fill ${at(a, b, c)} ${at(p, q, r)} ${FRAME}`);
    await fixture.command(`fill ${at(a, b + 1, c + 1)} ${at(a, q - 1, r - 1)} ${WALL}`);
    await fixture.command(`fill ${at(p, b + 1, c + 1)} ${at(p, q - 1, r - 1)} ${WALL}`);
    await fixture.command(`fill ${at(a + 1, b, c + 1)} ${at(p - 1, b, r - 1)} ${WALL}`);
    await fixture.command(`fill ${at(a + 1, q, c + 1)} ${at(p - 1, q, r - 1)} ${WALL}`);
    await fixture.command(`fill ${at(a + 1, b + 1, c)} ${at(p - 1, q - 1, c)} ${WALL}`);
    await fixture.command(`fill ${at(a + 1, b + 1, r)} ${at(p - 1, q - 1, r)} ${WALL}`);
    for (let i = a + 1; i < p; i++) for (let j = b + 1; j < q; j++) for (let k = c + 1; k < r; k++) await fixture.command(`setblock ${at(i, j, k)} ${inner(i, j, k)}`);
  };

  // The grid: a creative cell against the matrix, a drive, a terminal, a CPU of a 4k storage and seven
  // co-processors (eight pushes in AE2's three-tick window), a pattern provider with an Extended Molecular
  // Assembler on top. No controller: an ad-hoc grid of eight channels.
  const power = at(-1), drive = at(-2), terminal = at(-2, 1), provider = at(-2, 2), assembler = at(-2, 3);
  await fixture.command(`setblock ${power} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:item_storage_cell_4k`);
  await fixture.command(`fill ${at(-4, 0, 0)} ${at(-3, 1, 1)} ae2:crafting_accelerator`);
  await fixture.command(`setblock ${at(-3, 0, 0)} ae2:4k_crafting_storage`);
  await fixture.command(`setblock ${terminal} ae2:cable_bus`);
  await fixture.command(`setblock ${provider} ae2:pattern_provider`);
  await fixture.command(`setblock ${assembler} extendedae:ex_molecular_assembler`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);

  // The matrix: 7 x 4 x 3, its corners 6 apart along x (the most, `assembler_matrix_max_size`), glass on its south
  // face; inside a pattern core and nine crafter cores. Its neighbour, 8 long, is a block too long.
  const core = at(1, 1, 1), crafterA = at(2, 1, 1), wall = at(3, 1, 0);
  await box([0, 0, 0], [6, 3, 2], (dx, dy) => dx === 1 && dy === 1 ? PATTERN : CRAFTER);
  await fixture.command(`fill ${at(1, 1, 2)} ${at(5, 2, 2)} ${GLASS}`);
  await box([0, 0, 5], [7, 2, 7], dx => dx === 1 ? PATTERN : CRAFTER);
  await until(() => formed(at(0)), f => f, 'the 7-long matrix formed', 30000);
  for (const pos of [at(6, 3, 2), wall, at(3, 2, 2), core, crafterA]) assert(await formed(pos), `${pos} formed`);
  assert.match(await block(at(3, 0, 0)), /shape=column_x/, 'a frame between frames is a column');
  const online = await until(() => fixture.ae(`grid status ${wall}`), v => v.online && v.powered, 'the matrix online', 30000);
  assert(online.online, 'its 84 blocks share one channel of the eight');
  assert.match(await block(at(0)), /powered=true/);
  for (const pos of [at(0, 0, 5), at(3, 1, 5), at(1, 1, 6)]) assert(!(await formed(pos)), `${pos} of the 8-long box stays unformed`);
  fixture.mark('a matrix with corners 6 apart forms on one channel, one with corners 7 apart does not');

  // Its screen from any block: shift-clicked patterns go into the pattern core; nothing else does.
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  const nugget = 'minecraft:iron_nugget';
  const encoded = await fixture.ae(`crafting encode ${Array(9).fill(nugget).join(' ')}`);
  assert.deepEqual([encoded.encoded, encoded.output], [true, 'minecraft:iron_ingot']);
  await until(() => fixture.raw('inventory'), v => v.hotbar[0]?.item === 'ae2:crafting_pattern', 'the pattern');
  await fixture.replace('hotbar.1', 'minecraft:stone', 1);
  await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 1.5}`);
  await fixture.raw(`interact ${wall} north`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'assembler_matrix.json', 'Assembler Matrix screen', 15000);
  for (const n of [36, 37, 38, 39]) assert(view.ae2_screen.toolbar?.some((b: any) => b.entry === n), `toolbar entry ${n}`);
  assert.equal(view.ae2_screen.data?.[1] ?? 0, 0, 'no jobs running');
  await fixture.raw('click-slot 0 shift');
  view = await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'ae2:crafting_pattern', 'the pattern in the core');
  await fixture.raw('click-slot 1 shift');
  await fixture.raw('wait 5');
  assert(!slot(await fixture.raw('screen'), 301)?.item, 'stone stays out');
  assert.match(await contents(core), /ae2:crafting_pattern/);
  await fixture.pressSetting(37);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.data?.[0] === 0, 'hidden from the Pattern Access Terminal');
  assert.match(await contents(at(0)), /terminal_visible: 0b/, 'set on every block of the cluster');
  await fixture.pressSetting(37);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.data?.[0] === 1, 'shown again');
  await fixture.raw('close');
  fixture.mark('its screen takes crafting patterns into the pattern core and sets the cluster\'s Pattern Access Terminal setting');

  // A craft through the terminal; the final press sent while the server is held, so every tick after it is counted.
  const deposit = async (item: string, n: number) => {
    for (let left = n; left > 0; left -= Math.min(64, left)) {
      await fixture.replace('hotbar.0', item, Math.min(64, left)); await fixture.select(0, item);
      assert.equal((await fixture.ae(`grid deposit ${drive}`)).inserted, Math.min(64, left));
    }
  };
  const craft = async (item: string, amount: number) => {
    await fixture.replace('hotbar.8', 'minecraft:blaze_rod'); await fixture.select(8, 'minecraft:blaze_rod');
    await fixture.command(`tp @s ${x - 1.5} ${y} ${z - 2.5}`); await fixture.raw(`interact ${terminal} north`);
    const list = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot >= 300 && s.slot < 345 && s.item === item), `craftable ${item}`);
    // AE2's craft gesture on a key the grid has, a middle click.
    await fixture.raw(`click-slot ${list.slots.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === item).slot} middle`);
    await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Amount'), 'craft amount');
    await fixture.raw('ui-key right'); await fixture.raw('ui-key backspace'); await fixture.raw(`type ${amount}`); await fixture.raw('wait 5'); await fixture.raw('click-slot 302');
    await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Confirm') && !/Missing materials/.test(JSON.stringify(v)), 'confirmation');
    await fixture.hold();
    await fixture.raw('click-slot 302');
  };
  /** Steps the held server a tick at a time until `item` reaches `total` in the grid; the ticks and what `each` saw. */
  const run = async (item: string, total: number, each: () => Promise<void> = async () => {}) => {
    for (let ticks = 1; ticks <= 600; ticks++) {
      await fixture.stepServer(1);
      await each();
      if (count(await fixture.ae(`grid status ${drive}`), item) >= total) {
        await fixture.hold(false); await fixture.raw('close');
        return ticks;
      }
    }
    await fixture.hold(false);
    throw new Error(`${total} ${item} never crafted`);
  };

  // One ingot with no speed core: 20 progress a tick of 100 (CraftingThread.tick with no cards).
  await deposit(nugget, 9 * 66);
  await craft('minecraft:iron_ingot', 1);
  const slow = await run('minecraft:iron_ingot', 1);
  fixture.mark(`one craft with no speed core: ${slow} ticks from the press to the ingot`);

  // 64 ingots: a pass of the CPU pushes all it may, so the first crafter core takes eight jobs at once.
  let most = 0;
  await craft('minecraft:iron_ingot', 64);
  const parallel = await run('minecraft:iron_ingot', 65, async () => { most = Math.max(most, await busy(crafterA)); });
  assert.equal(most, 8, 'all eight threads of a crafter core at work at once');
  assert(parallel < 64 * (slow - 1), `64 crafts in ${parallel} ticks, not one after another`);
  fixture.mark(`64 crafts in ${parallel} ticks: a crafter core ran 8 jobs at once`);

  // Five speed cores in place of the upper crafters: 100 progress a tick, a craft in one.
  await fixture.command(`fill ${at(1, 2, 1)} ${at(5, 2, 1)} ${SPEED}`);
  await until(() => contents(crafterA), text => /speed: 5/.test(text), 'the cluster counts five speed cores', 30000);
  await craft('minecraft:iron_ingot', 1);
  const fast = await run('minecraft:iron_ingot', 66);
  assert(fast + 3 <= slow, `with five speed cores ${fast} ticks, without ${slow}`);
  fixture.mark(`speed cores make a craft faster: ${fast} ticks with five, ${slow} with none`);

  // A wall taken away breaks the matrix; put back, it forms again.
  await fixture.command(`setblock ${wall} minecraft:air`);
  await until(() => formed(at(0)), f => !f, 'the matrix broken', 30000);
  assert(!(await formed(core)), 'its pattern core too');
  assert.equal((await fixture.ae(`grid status ${at(0)}`)).online, false, 'its blocks off the grid');
  await fixture.command(`setblock ${wall} ${WALL}`);
  await until(() => formed(at(0)), f => f, 'formed again', 30000);
  await until(() => fixture.ae(`grid status ${wall}`), v => v.online, 'back on the grid', 30000);
  fixture.mark('a wall taken away breaks the matrix and put back forms it again');

  // The Extended Molecular Assembler: a provider's eight jobs in its eight threads at once.
  const gold = 'minecraft:gold_nugget';
  await fixture.replace('hotbar.0', 'ae2:blank_pattern'); await fixture.select(0, 'ae2:blank_pattern');
  const goldPattern = await fixture.ae(`crafting encode ${Array(9).fill(gold).join(' ')}`);
  assert.deepEqual([goldPattern.encoded, goldPattern.output], [true, 'minecraft:gold_ingot']);
  await until(() => fixture.raw('inventory'), v => v.hotbar[0]?.item === 'ae2:crafting_pattern', 'the gold pattern');
  await fixture.command(`tp @s ${x - 1.5} ${y} ${z - 1.5}`);
  await fixture.raw(`interact ${provider} north`);
  const menu = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot < 36 && s.item === 'ae2:crafting_pattern'), 'provider menu', 5000);
  await fixture.raw(`click-slot ${menu.slots.find((s: any) => s.slot < 36 && s.item === 'ae2:crafting_pattern').slot} shift`);
  await until(() => fixture.raw('inventory'), v => !v.counts?.['ae2:crafting_pattern'], 'gold pattern installed');
  await fixture.raw('close');
  await deposit(gold, 9 * 8);
  let threads = 0;
  await craft('minecraft:gold_ingot', 8);
  await run('minecraft:gold_ingot', 8, async () => { threads = Math.max(threads, await busy(assembler)); });
  assert.equal(threads, 8, 'the Extended Molecular Assembler ran eight jobs at once');
  fixture.mark('the Extended Molecular Assembler runs eight jobs at once');

  fixture.assertHidden(await fixture.raw('status')); fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
