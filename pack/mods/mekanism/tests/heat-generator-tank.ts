/** Mekanism Generators' Heat Generator's lava tank as a fluid handler (NeoForge's `Capabilities.FluidHandler.BLOCK`,
 * hook points API 160, docs/mod-fluids.md slice 5), met by AE2's buses, under ATM10's `generators.toml` (a 24 000 mB
 * tank). As `TileEntityHeatGenerator.getInitialFluidTanks` builds it, the tank is on every side but the front and is
 * `VariableCapacityFluidTank.input`: any block may fill it with lava (`FluidTags.LAVA`), none may drain it.
 *
 * - An export bus with a lava filter fills it from a fluid cell, every millibucket the cell loses arriving, up to the
 *   tank's 24 000 mB and no further; one facing its front moves nothing.
 * - A storage bus on it takes lava the network is given (the bus outranks the cell) and refuses water; by AE2's default
 *   storage filter (extractable only) the network doesn't list the lava it can't take back.
 * - An import bus takes nothing out of it.
 * - What it holds is saved as Mekanism saves it (`fluid_tanks: [{tank: 0b, stored: {amount, id}}]`) and survives a
 *   restart.
 *
 * Lava source blocks beside it only heat it (`getBoost`, heat-generator.ts): Java never draws them into the tank.
 * Needs what `heat-generator.ts` needs. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const generators = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-heat-tank', 'server-thread', { devMods: [content, generators] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'generators.toml', 'generator-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'generators.toml'), '[solar_generator]\n\tsolarGeneration = 150\n[heat_generator]\n\theatGeneration = 800\n\theatGenerationLava = 30\n\theatGenerationNether = 100\n\ttankCapacity = 24000\n\theatGenerationFluidRate = 10\n');
  writeFileSync(join(config, 'generator-storage.toml'), 'heatGenerator = 160000\nsolarGenerator = 96000\n');
}
const TANK = 24000;
const FULL = '{energy_containers:[{container:0b,stored:160000L}]}';
const tankOf = (mb: number) => `{fluid_tanks:[{tank:0b,stored:{id:"minecraft:lava",amount:${mb}}}]}`;

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const lavaIn = (nbt: string) => Number(nbt.match(/fluid_tanks: ?\[\{[^\]]*amount: (\d+)/)?.[1] ?? 0);
async function lava(pos: string): Promise<number> { return lavaIn(await saved(pos)); }
async function gridFluid(host: string, fluid: string): Promise<number> {
  const status = await fixture.ae(`fluid grid status ${host}`);
  return (status.entries ?? []).filter((e: any) => e.fluid === fluid).reduce((n: number, e: any) => n + e.amount_mb, 0);
}
/** Steps the held server in `step`-tick runs until `done`, at most `limit` ticks. */
async function stepUntil(done: () => Promise<boolean>, what: string, step = 20, limit = 4000): Promise<number> {
  for (let ticks = 0; ticks <= limit; ticks += step) {
    if (await done()) return ticks;
    await fixture.stepServer(step);
  }
  throw new Error(`${what}: not after ${limit} ticks`);
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(4, 4, 5)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(4, -2, 5)} minecraft:stone`);
  await fixture.command(`fillbiome ${at(-10, -10, -10)} ${at(12, 12, 12)} minecraft:plains`);
  await fixture.command('clear @s');

  // The network: power, a drive with an empty fluid cell, two cable buses side by side.
  const power = at(-1), h1 = at(0), drive = at(-1, 0, 1), h2 = at(0, 0, 1);
  // The generators: G1 east of H1 (facing north: H1 touches its west side), G2 above H1, G2b east of H2, and G3
  // south of H2 facing north, its front to H2.
  const g1 = at(1), g2 = at(0, 1, 0), g2b = at(1, 0, 1), g3 = at(0, 0, 2);
  await fixture.command(`setblock ${power} ae2:creative_energy_cell`);
  await fixture.command(`setblock ${drive} ae2:drive`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:fluid_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:f",id:"minecraft:lava","#":3000L}]]`);
  await fixture.command(`setblock ${h1} ae2:cable_bus`);
  await fixture.command(`setblock ${h2} ae2:cable_bus`);
  await fixture.hold();
  for (const [pos, state] of [[g1, ''], [g2, ''], [g2b, ''], [g3, '[facing=north]']]) {
    await fixture.command(`setblock ${pos} mekanismgenerators:heat_generator${state}`);
    // A full store: nothing burns, so the tank shows only what comes and goes.
    await fixture.command(`data merge block ${pos} ${FULL}`);
  }
  await fixture.command(`data merge block ${g2} ${tankOf(5000)}`);
  await fixture.command(`data merge block ${g2b} ${tankOf(5000)}`);
  await fixture.stepServer(2);
  await fixture.hold(false);
  const attach = async (item: string, host: string, face: string) => {
    await fixture.replace('hotbar.0', item); await fixture.select(0, item);
    assert.equal((await fixture.ae(`grid part ${host} ${face}`)).attached, true, `${item} on ${host} ${face}`);
  };
  await attach('ae2:export_bus', h1, 'east');
  await attach('ae2:storage_bus', h1, 'up');
  await attach('ae2:import_bus', h2, 'east');
  await attach('ae2:export_bus', h2, 'south');
  await until(() => fixture.ae(`grid status ${h1}`), s => s.online && s.powered, 'the network is online', 30000);
  await fixture.replace('hotbar.0', 'minecraft:lava_bucket'); await fixture.select(0, 'minecraft:lava_bucket');
  for (const [host, face] of [[h1, 'east'], [h2, 'east'], [h2, 'south']]) await fixture.ae(`bus filter_fluid ${host} ${face} 0`);
  await fixture.command('item replace entity @s hotbar.0 with minecraft:air');

  // G1: the export bus empties the cell into it, all 3000 mB arriving.
  await fixture.hold();
  const filled = await stepUntil(async () => (await lava(g1)) === 3000, 'the export bus fills G1');
  assert.equal(await gridFluid(h1, 'minecraft:lava'), 0, 'every millibucket the cell lost is in the tank');
  log.export_ticks = filled;
  const nbt = await saved(g1);
  log.g1 = nbt;
  assert.match(nbt, /fluid_tanks: ?\[\{stored: ?\{amount: 3000, id: ?"minecraft:lava"\}, tank: 0b\}\]/, `Mekanism's own form: ${nbt}`);
  fixture.mark(`an export bus fills the tank through its side: 3000 mB of lava in ${filled} ticks, none lost`);

  // Up to the tank's capacity and no further: 1000 mB more in the cell, room for 500.
  await fixture.command(`data merge block ${g1} ${tankOf(TANK - 500)}`);
  await fixture.command(`item replace block ${drive} container.0 with ae2:fluid_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:f",id:"minecraft:lava","#":1000L}]]`);
  await stepUntil(async () => (await lava(g1)) === TANK, 'G1 fills up');
  await fixture.stepServer(200);
  assert.equal(await lava(g1), TANK, `the tank holds ${TANK} mB`);
  assert.equal(await gridFluid(h1, 'minecraft:lava'), 500, 'what didn\'t fit stays in the cell');
  fixture.mark(`it fills up to its ${TANK} mB and takes no more`);

  // G3's front faces H2's export bus: no handler there.
  assert.equal(await lava(g3), 0);
  assert.equal(await gridFluid(h1, 'minecraft:lava'), 500, 'nothing went out through the front');
  fixture.mark('an export bus facing its front moves nothing');

  // G2b: the import bus takes nothing back.
  assert.equal(await lava(g2b), 5000, 'the import bus took none of its lava');
  fixture.mark('an import bus takes nothing out of it');

  // G2 under the storage bus: its lava isn't listed (extractable only), lava goes in once the bus outranks the cell,
  // and water doesn't.
  await fixture.hold(false);
  await fixture.ae(`bus priority ${h1} up 10`);
  await fixture.replace('hotbar.0', 'minecraft:lava_bucket', 1); await fixture.select(0, 'minecraft:lava_bucket');
  assert.equal((await fixture.ae(`fluid grid insert ${h1}`)).inserted_mb, 1000);
  assert.equal(await lava(g2), 6000, 'the storage bus put it into G2');
  assert.equal(await gridFluid(h1, 'minecraft:lava'), 500, 'the network lists only the cell\'s lava');
  await fixture.replace('hotbar.0', 'minecraft:water_bucket', 1); await fixture.select(0, 'minecraft:water_bucket');
  assert.equal((await fixture.ae(`fluid grid insert ${h1}`)).inserted_mb, 1000);
  assert.equal(await lava(g2), 6000, 'water went to the cell');
  assert.equal(await gridFluid(h1, 'minecraft:water'), 1000);
  fixture.mark('a storage bus fills it with lava only, and the network lists none of what it can\'t take back');

  // Save and restart.
  await fixture.restart();
  await fixture.hold();
  assert.equal(await lava(g1), TANK);
  assert.equal(await lava(g2), 6000);
  assert.match(await saved(g1), /id: ?"minecraft:lava"/);
  await fixture.hold(false);
  fixture.mark('the tanks survive a save and restart');

  fixture.assertHidden(await fixture.raw('status'));
  console.log(`MEK tank ${JSON.stringify(log)}`);
  fixture.report('passed');
} catch (error) { console.log(`MEK tank ${JSON.stringify(log)}`); fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
