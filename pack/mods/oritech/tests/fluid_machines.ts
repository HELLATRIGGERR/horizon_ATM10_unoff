/** Oritech's fluid machines, as the `oritech` native bundle and the carry of the player's jar make them (the test world has the
 * mod's defaults, no ATM10 config; the fluid tags of Oritech's own data are added to the content copy).
 *
 * - A water bucket used on a cooler (survival) fills its tank with 1,000 mB and leaves the empty bucket in the hand, with no
 *   menu opening (`MachineBlock.useItemOn`'s fluid interaction). The cooler (one core beside it) freezes the water into
 *   3 ice in 200 ticks at 32 FE/t.
 * - A centrifuge with a fluid addon turns a dirt and 250 mB of water into clay in 100 ticks at 64 FE/t.
 * - A lava generator (a core above) takes a lava bucket into its tank and burns 100 mB per 120 ticks at 64 FE/t, filling
 *   the pulverizer beside it; the empty bucket moves to its output slot.
 * - A fuel generator (seventeen cores) burns diesel (a fluid of the `c:diesel` tag) at 256 FE/t into the two pulverizers behind it.
 * - A refinery (nine cores) with two modules (five cores each) refines 1,000 mB of lava into 4,000 mB of steam and, in the
 *   modules' tanks, 100 mB of sulfuric acid and 200 mB of sheol fire, in 80 ticks at 64 FE/t.
 *
 * Reads the saved compound (`data get block`, the bundle's keys); screenshots of the refinery's and the lava generator's
 * screens go to the run folder. Needs the staged native build and the content mod of the carry. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.ORITECH_CONTENT_DIR ?? join(root, '../.build/content/oritech'));
const fixture = new WorldFixture('oritech-fluid-machines', 'server-thread', { devMods: [content] });

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
/** A tank of the compound: the fluid and the amount (none: empty). */
const tank = (nbt: string, name: string) => {
  const found = nbt.match(new RegExp(`(?:^|[^\\w])${name}: ?\\{([^}]*)\\}`));
  if (!found) return { id: '', amount: 0 };
  return { id: found[1].match(/id: ?"([^"]+)"/)?.[1] ?? '', amount: Number(found[1].match(/amount: ?(\d+)/)?.[1] ?? 0) };
};
const gametime = async () => Number(((await fixture.command('time query gametime')).chat ?? []).join(' ').match(/(\d+)/)?.[1]);
const energy = (nbt: string) => field(nbt, 'oritech.machine_energy');
const progress = (nbt: string) => field(nbt, 'oritech.machine_progress');
const assembled = (s: string) => s.includes('connectedCores') && !/connectedCores: ?\[\]/.test(s);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 3, y = Math.floor(py) + 1, z = Math.floor(pz) - 2;
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 4} ${x + 26} ${y + 5} ${z + 5} minecraft:air`);
  await fixture.command(`fill ${x - 2} ${y - 1} ${z - 4} ${x + 26} ${y - 1} ${z + 5} minecraft:stone`);
  const place = async (pos: string, block: string) => { await fixture.command(`setblock ${pos} ${block}`); };
  // A north-facing controller's offsets (x back, y up, z left) in the world: (z, y, x).
  const north = ([lx, ly, lz]: number[]) => [lz, ly, lx];
  const shot = async (pos: string, name: string, at: number[], front = false) => {
    // The refinery's cores fill its back; it is opened from the front (north).
    await fixture.command(`tp @s ${at[0] + 0.5} ${at[1]} ${at[2] + (front ? -1.5 : 2.5)}`);
    await fixture.raw(`interact ${pos} ${front ? 'north' : 'south'}`);
    await until(() => fixture.raw('screen'), v => v.screen === 'container', name + ' screen opens');
    await Bun.sleep(700);
    const path = join(fixture.run, name + '.png');
    await fixture.raw(`screenshot ${path}`);
    await fixture.raw('close');
    return path;
  };

  const recipes = ((await fixture.command('oritech recipes')).chat ?? []).join(' ');
  log.recipes = recipes;

  // --- Cooler: 1,000 mB of water -> 3 ice, 200 ticks at 32 FE/t ---
  const cool = `${x} ${y} ${z}`;
  await place(cool, 'oritech:cooler_block[facing=north]');
  await place(`${x - 1} ${y} ${z}`, 'oritech:machine_core_1');
  // A water bucket used on it fills the tank (survival: the empty bucket comes back), and no menu opens.
  await fixture.command('gamemode survival @s');
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 2.5} 180 20`);
  await until(() => fixture.raw(`block ${cool}`), v => v.block === 'oritech:cooler_block', 'the cooler reaches the client');
  await fixture.replace('hotbar.0', 'minecraft:water_bucket', 1);
  await fixture.raw(`interact ${cool} south`);
  {
    const filled = await until(() => saved(cool), s => tank(s, 'fluid').amount === 1000, 'the bucket fills the cooler', 20000);
    assert.equal(tank(filled, 'fluid').id, 'minecraft:water');
    await until(() => fixture.raw('inventory'), v => v.hotbar?.[0]?.item === 'minecraft:bucket', 'the empty bucket is in the hand');
    await Bun.sleep(500);
    assert.notEqual((await fixture.raw('screen')).screen, 'container', 'the bucket opens no menu');
  }
  await fixture.command('item replace entity @s hotbar.0 with minecraft:air');
  await fixture.command('gamemode creative @s');
  fixture.mark('a water bucket used on the cooler fills its tank with 1,000 mB and leaves the empty bucket in the hand');
  await fixture.command(`data merge block ${cool} {"oritech.machine_energy":50000L}`);
  const c0 = await gametime();
  let nbt = await until(() => saved(cool), s => s.includes('minecraft:ice') && progress(s) === 0, 'the cooler finishes', 90000);
  const coolTicks = (await gametime()) - c0;
  log.coolerTicks = coolTicks;
  assert(coolTicks >= 195 && coolTicks <= 300, `a freeze takes about 200 ticks: ${coolTicks}`);
  assert(/count: ?3/.test(nbt), `three ice: ${nbt.slice(0, 400)}`);
  assert.equal(tank(nbt, 'fluid').amount, 0, 'the water was used');
  assert.equal(energy(nbt), 50000 - 200 * 32, 'the energy of 200 ticks at 32 FE/t');
  fixture.mark(`the cooler froze a bucket of water into 3 ice in ${coolTicks} ticks, ${energy(nbt)} FE left`);

  // --- Centrifuge with a fluid addon: dirt + 250 mB of water -> clay, 100 ticks at 64 FE/t ---
  const cx = x + 4;
  const cen = `${cx} ${y} ${z}`;
  await place(cen, 'oritech:centrifuge_block[facing=north]');
  await place(`${cx} ${y + 1} ${z}`, 'oritech:machine_core_1');
  await place(`${cx - 1} ${y} ${z}`, 'oritech:machine_fluid_addon');
  await fixture.command(`data merge block ${cen} {"oritech.machine_energy":60000L,fluidin:{id:"minecraft:water",amount:1000}}`);
  nbt = await until(() => saved(cen), s => assembled(s) && s.includes('fluidAddon: 1b'), 'the centrifuge assembles with its fluid addon', 30000);
  await fixture.command(`data modify block ${cen} Items set value [{id:"minecraft:dirt",count:1,Slot:0b}]`);
  const f0 = await gametime();
  nbt = await until(() => saved(cen), s => s.includes('minecraft:clay') && progress(s) === 0, 'the fluid centrifuge finishes', 90000);
  const cenTicks = (await gametime()) - f0;
  log.fluidCentrifugeTicks = cenTicks;
  assert(cenTicks >= 98 && cenTicks <= 160, `a run takes about 100 ticks: ${cenTicks}`);
  assert.deepEqual(tank(nbt, 'fluidin'), { id: 'minecraft:water', amount: 750 }, '250 mB of the 1,000 went');
  assert.equal(energy(nbt), 60000 - 100 * 64, 'the energy of 100 ticks at 64 FE/t');
  fixture.mark(`the fluid centrifuge made clay from dirt and water in ${cenTicks} ticks, ${energy(nbt)} FE left`);

  // --- Lava generator (a core above) beside a pulverizer: a lava bucket fills the tank ---
  const lx = x + 9;
  const lava = `${lx} ${y} ${z}`;
  const lavaBeside = `${lx + 1} ${y} ${z}`;
  await place(lava, 'oritech:lava_generator_block[facing=north]');
  await place(`${lx} ${y + 1} ${z}`, 'oritech:machine_core_1');
  await place(lavaBeside, 'oritech:pulverizer_block');
  nbt = await until(() => saved(lava), assembled, 'the lava generator assembles', 20000);
  await fixture.command(`data modify block ${lava} Items set value [{id:"minecraft:lava_bucket",count:1,Slot:0b}]`);
  nbt = await until(() => saved(lava), s => progress(s) > 0 && progress(s) < 115, 'the lava generator burns', 30000);
  await Bun.sleep(1500);
  nbt = await saved(lava);
  const pulv = await saved(lavaBeside);
  const lavaTank = tank(nbt, 'fluid');
  log.lavaGenerator = { tank: lavaTank, storedBurn: field(nbt, 'storedBurn'), energy: energy(nbt), pulverizer: energy(pulv) };
  assert.equal(lavaTank.id, 'minecraft:lava');
  assert(lavaTank.amount <= 900 && lavaTank.amount >= 800, `a bucket in, 100 mB burning: ${JSON.stringify(lavaTank)}`);
  assert(nbt.includes('minecraft:bucket') && !nbt.includes('lava_bucket'), `the empty bucket moved out: ${nbt.slice(0, 500)}`);
  assert.equal(field(nbt, 'storedBurn'), 120, '100 mB of lava burns 120 ticks');
  assert(energy(pulv) >= 64 * 20, `the pulverizer beside it is filling: ${energy(pulv)}`);
  log.screenshotLava = await shot(lava, 'lava-generator-running', [lx, y, z]);
  fixture.mark(`the lava generator burns a bucket of lava and the pulverizer beside it holds ${energy(pulv)} FE`);

  // --- Fuel generator (seventeen cores): diesel at 256 FE/t into the two blocks behind it ---
  const gx = x + 14;
  const fuel = `${gx} ${y} ${z}`;
  await place(fuel, 'oritech:fuel_generator_block[facing=north]');
  for (const lx2 of [0, -1, -2]) for (const ly of [0, 1]) for (const lz of [1, 0, -1]) {
    if (lx2 === 0 && ly === 0 && lz === 0) continue;
    const [dx, dy, dz] = north([lx2, ly, lz]);
    await place(`${gx + dx} ${y + dy} ${z + dz}`, 'oritech:machine_core_1');
  }
  const [bx1, , bz1] = north([1, 0, -1]);
  const [bx2, , bz2] = north([1, 0, 1]);
  const behindA = `${gx + bx1} ${y} ${z + bz1}`;
  const behindB = `${gx + bx2} ${y} ${z + bz2}`;
  await place(behindA, 'oritech:pulverizer_block');
  await place(behindB, 'oritech:pulverizer_block');
  nbt = await until(() => saved(fuel), assembled, 'the fuel generator assembles', 30000);
  await fixture.command(`data merge block ${fuel} {fluid:{id:"oritech:still_diesel",amount:2000}}`);
  nbt = await until(() => saved(fuel), s => progress(s) > 0 && progress(s) < 75, 'the fuel generator burns', 30000);
  await Bun.sleep(1500);
  nbt = await saved(fuel);
  const pa = energy(await saved(behindA)), pb = energy(await saved(behindB));
  const fuelTank = tank(nbt, 'fluid');
  log.fuelGenerator = { tank: fuelTank, storedBurn: field(nbt, 'storedBurn'), energy: energy(nbt), behind: [pa, pb] };
  assert.equal(field(nbt, 'storedBurn'), 80, '100 mB of diesel burns 80 ticks');
  assert(fuelTank.amount <= 1900 && fuelTank.amount > 1000, `diesel taken 100 mB a burn: ${JSON.stringify(fuelTank)}`);
  assert(pa + pb >= 256 * 10, `the pulverizers behind it are filling: ${pa} ${pb}`);
  fixture.mark(`the fuel generator burns diesel and the blocks behind it hold ${pa + pb} FE`);

  // --- Refinery with two modules: lava -> steam, sulfuric acid, sheol fire ---
  const rx = x + 21;
  const ref = `${rx} ${y} ${z}`;
  await place(ref, 'oritech:refinery_block[facing=north]');
  for (const c of [[0, 1, 0], [0, 0, -1], [0, 1, -1], [1, 0, -1], [1, 1, -1], [1, 0, 0], [1, 1, 0], [2, 0, -1], [2, 1, -1]]) {
    const [dx, dy, dz] = north(c);
    await place(`${rx + dx} ${y + dy} ${z + dz}`, 'oritech:machine_core_1');
  }
  const modules: string[] = [];
  for (const up of [2, 3]) {
    const mpos = `${rx} ${y + up} ${z}`;
    modules.push(mpos);
    await place(mpos, 'oritech:refinery_module_block[facing=north]');
    for (const c of [[0, 0, -1], [1, 0, -1], [1, 0, 0], [2, 0, -1], [2, 0, 0]]) {
      const [dx, dy, dz] = north(c);
      await place(`${rx + dx} ${y + up + dy} ${z + dz}`, 'oritech:machine_core_1');
    }
  }
  await fixture.command(`data merge block ${ref} {"oritech.machine_energy":50000L}`);
  await until(() => saved(ref), assembled, 'the refinery assembles', 30000);
  for (const m of modules) await until(() => saved(m), assembled, 'a refinery module assembles', 30000);
  await Bun.sleep(3000); // the refinery counts its modules every 25 ticks
  await fixture.command(`data merge block ${ref} {fluidinmain:{id:"minecraft:lava",amount:1000}}`);
  const r0 = await gametime();
  nbt = await until(() => saved(ref), s => tank(s, 'fluidoutmain').amount > 0, 'the refinery finishes', 90000);
  const refTicks = (await gametime()) - r0;
  const modA = await saved(modules[0]);
  const modB = await saved(modules[1]);
  log.refinery = { ticks: refTicks, out: tank(nbt, 'fluidoutmain'), moduleA: tank(modA, 'fluid'), moduleB: tank(modB, 'fluid'), energy: energy(nbt) };
  assert(refTicks >= 78 && refTicks <= 160, `a refine takes about 80 ticks: ${refTicks}`);
  assert.deepEqual(tank(nbt, 'fluidoutmain'), { id: 'oritech:still_steam', amount: 4000 });
  assert.deepEqual(tank(modA, 'fluid'), { id: 'oritech:still_sulfuric_acid', amount: 100 }, "the first module's tank");
  assert.deepEqual(tank(modB, 'fluid'), { id: 'oritech:still_sheol_fire', amount: 200 }, "the second module's tank");
  assert.equal(tank(nbt, 'fluidinmain').amount, 0, 'the lava was used');
  assert.equal(energy(nbt), 50000 - 80 * 64, 'the energy of 80 ticks at 64 FE/t');
  log.screenshotRefinery = await shot(ref, 'refinery-done', [rx, y, z], true);
  fixture.mark(`the refinery made steam, acid and sheol fire from lava in ${refTicks} ticks, ${energy(nbt)} FE left`);

  fixture.assertHidden(await fixture.raw('status'));
  console.log('ORITECH fluid machines', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('ORITECH fluid machines', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
