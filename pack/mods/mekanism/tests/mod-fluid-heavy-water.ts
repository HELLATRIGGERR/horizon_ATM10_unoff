/** Mekanism's heavy water as a content mod's fluid (the engine's mod fluids, slice 1), from ATM10's Mekanism 10.7.19
 * through `mekanism-content`, whose carry declares it from the NeoForge oracle's facts
 * (`data/mekanism/horizon/fluid/heavy_water.json`, docs/mod-fluids.md slice 2):
 *
 * - Its bucket places a source of `mekanism:heavy_water` (its own `LiquidBlock`, not the lava its block copies), which
 *   spreads as `BaseFlowingFluid`'s defaults have it: a tick delay of 5, one level less a block, seven blocks out on a
 *   flat floor, every flowing block still heavy water.
 * - An empty bucket takes the source back as a heavy water bucket.
 * - An ME Chest's fluid cell takes the bucket's 1000 mB (`/ae2 fluid grid insert`), its terminal shows it as the heavy
 *   water bucket with its amount, an empty bucket clicked on it fills, a full one clicked empties into it, and the cell
 *   keeps the fluid's id through a save and restart.
 *
 * Needs MEK_CONTENT_DIR (what `pack/scripts/mekanism.ts content` writes) beside AE2's content; no native bundle. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const fixture = new WorldFixture('mod-fluid-heavy-water', 'server-thread', { devMods: [content] });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => line.includes('Test passed'));
const isBlock = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
const heavy = (view: any) => view.slots?.find((s: any) => s.slot >= 300 && s.slot < 345 && s.item === 'mekanism:heavy_water_bucket');
const amount = (view: any, n: number) => String(heavy(view)?.components ?? '').includes(`${n} mB`);
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
  for (const [dx, dz] of [[-12, -12], [-12, 12], [12, -12], [12, 12]] as const) {
    await until(async () => passed(await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)), v => v, 'fixture corner loaded', 30000);
  }
  await fixture.command(`fill ${at(-12, 0, -12)} ${at(12, 4, 12)} minecraft:air`);
  await fixture.command(`fill ${at(-12, -1, -12)} ${at(12, -1, 12)} minecraft:stone`);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.0', 'mekanism:heavy_water_bucket'); await fixture.replace('hotbar.8', 'minecraft:stick');
  await fixture.select(0, 'mekanism:heavy_water_bucket');

  // The bucket, aimed at the floor's top: a source of its own block, and an empty bucket in the hand.
  await fixture.raw(`look-at block ${at(0, -1, 0)} up`); await fixture.raw('use-item');
  await until(() => isBlock(at(0), 'mekanism:heavy_water[level=0]'), v => v, 'the bucket places a heavy water source');
  await until(() => fixture.raw('inventory'), v => v.hotbar?.[0]?.item === 'minecraft:bucket', 'the bucket is empty');
  fixture.mark('the heavy water bucket places a source of mekanism:heavy_water');

  // BaseFlowingFluid's defaults: level d at d blocks out, seven blocks, all of it heavy water.
  await until(async () => isBlock(at(7), 'mekanism:heavy_water[level=7]'), v => v, 'heavy water spreads seven blocks', 30000);
  for (let d = 1; d <= 7; d++) {
    assert(await isBlock(at(d), `mekanism:heavy_water[level=${d}]`), `level ${d} at ${d} blocks`);
    assert(await isBlock(at(-d), `mekanism:heavy_water[level=${d}]`), `level ${d} at -${d} blocks`);
  }
  assert(await isBlock(at(8), 'minecraft:air'), 'no eighth block');
  assert(!(await isBlock(at(1), 'minecraft:lava')) && !(await isBlock(at(1), 'minecraft:water')), 'neither lava nor water');
  fixture.mark('heavy water flows as BaseFlowingFluid: one level less a block, seven blocks out');
  // A look for whoever reads the run: tinted water textures, translucent (not asserted).
  try { await fixture.raw(`screenshot ${join(fixture.run, 'heavy-water.png').replaceAll('\\', '/')}`); } catch {}

  // The empty bucket takes the source back.
  await fixture.raw(`look-at ${x + 0.5} ${y + 0.5} ${z + 0.5}`); await fixture.raw('use-item');
  await until(() => fixture.raw('inventory'), v => v.hotbar?.[0]?.item === 'mekanism:heavy_water_bucket', 'the bucket takes heavy water back');
  assert(!(await isBlock(at(0), 'mekanism:heavy_water[level=0]')), 'the source is gone');
  fixture.mark('an empty bucket picks the heavy water source up');

  // An ME Chest with a fluid cell: the bucket's 1000 mB go in as mekanism:heavy_water.
  const chest = at(-3, 0, 6), cell = at(-4, 0, 6);
  await fixture.command(`setblock ${cell} ae2:creative_energy_cell`); await fixture.command(`setblock ${chest} ae2:chest`);
  await fixture.command(`item replace block ${chest} container.0 with ae2:fluid_storage_cell_1k`);
  await until(() => fixture.ae(`grid status ${chest}`), s => s.online && s.powered, 'powered ME Chest', 30000);
  const inserted = await fixture.ae(`fluid grid insert ${chest}`);
  assert.deepEqual([inserted.fluid, inserted.inserted_mb], ['mekanism:heavy_water', 1000]);
  const stored = await fixture.ae(`fluid grid status ${chest}`);
  assert(stored.entries.some((e: any) => e.fluid === 'mekanism:heavy_water' && e.amount_mb === 1000), JSON.stringify(stored));
  assert.equal((await fixture.raw('inventory')).hotbar?.[0]?.item, 'minecraft:bucket');
  fixture.mark('the fluid cell takes the heavy water bucket as 1000 mB of mekanism:heavy_water');

  // Its terminal: the fluid as its bucket with its amount; an empty bucket fills, a full one empties.
  const open = async () => {
    await fixture.select(8, 'minecraft:stick'); await fixture.command(`tp @s ${x - 2.5} ${y} ${z + 8.5}`);
    await fixture.raw(`interact ${chest} up`);
    return until(() => fixture.raw('screen'), s => heavy(s), 'ME Chest heavy water view', 30000);
  };
  let view = await open(); assert(amount(view, 1000), JSON.stringify(heavy(view)));
  await fixture.raw('click-slot 0'); await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:bucket', 'empty bucket held');
  await fixture.raw(`click-slot ${heavy(view).slot}`);
  await until(() => fixture.raw('inventory'), s => s.carried?.item === 'mekanism:heavy_water_bucket', 'left click fills the heavy water bucket');
  view = await until(() => fixture.raw('screen'), s => !heavy(s), 'the cell is empty');
  fixture.mark('the terminal shows heavy water as its bucket and fills an empty bucket from it');
  await fixture.raw('click-slot 300 right');
  await until(() => fixture.raw('inventory'), s => s.carried?.item === 'minecraft:bucket', 'right click empties the heavy water bucket');
  view = await until(() => fixture.raw('screen'), s => amount(s, 1000), '1000 mB back in the cell');
  await fixture.raw('click-slot 0'); await fixture.raw('close');
  fixture.mark('a carried heavy water bucket empties into the cell');

  await fixture.restart();
  await until(() => fixture.ae(`grid status ${chest}`), s => s.online && s.powered, 'reloaded chest powered', 60000);
  const reloaded = await fixture.ae(`fluid grid status ${chest}`);
  assert(reloaded.entries.some((e: any) => e.fluid === 'mekanism:heavy_water' && e.amount_mb === 1000), JSON.stringify(reloaded));
  view = await open(); assert(amount(view, 1000));
  assert.match(JSON.stringify(await fixture.command(`data get block ${chest}`)), /mekanism:heavy_water/);
  fixture.assertHidden(await fixture.raw('status'));
  fixture.mark('the cell keeps mekanism:heavy_water through a save and restart');
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
