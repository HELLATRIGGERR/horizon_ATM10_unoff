/** Modern Industrialization's multiblocks, as the `modern_industrialization` native bundle runs them: an Electric Blast
 * Furnace built block by block in a world (the carry of the player's own jar, its controller and hatches on the beacon
 * template: the script's `content` step), formed, fed through its hatches, and run on ATM10's own blast furnace recipes.
 *
 * The numbers are MI 2.5.11's (`ElectricBlastFurnaceBlockEntity`, `MachineTier.MULTIBLOCK`, `EnergyHatch`,
 * `CrafterComponent`), held by the crate's unit tests:
 *
 * - The shape: heatproof casings 3 by 3 below and above, two hollow rings of cupronickel coils between, the controller
 *   facing north in the bottom layer's front middle: 33 members. Unbuilt, `/mi shape` lists all 33 as missing.
 * - Hatches stand in casings: steel item input and output, steel fluid input, two steel fluid outputs and an LV energy
 *   input (19,200 EU: 30 seconds of LV); each is linked to the controller once the shape matches.
 * - `materials/blast_furnace/raw_oil_to_oil` (2 EU/t for 200 ticks: 400 EU) runs at MULTIBLOCK's base 8 EU/t: 50 ticks
 *   for 100 mB of synthetic oil, the second craft on the next tick.
 * - ATM10's coke (16 `#minecraft:coals`: 16 coke and 8,000 mB of creosote, 64 EU/t for 200 ticks) is banned by the
 *   cupronickel tier (32 EU/t); on the kanthal shape (`activeShape` 1) the cupronickel coils don't match, kanthal coils
 *   do, and it runs at 64 EU/t: 200 ticks, 12,800 EU.
 * - A casing broken mid-recipe unmatches the shape: no progress until it is back, then the recipe resumes where it was.
 * - Turned east (`facingDirection` 5) the same blocks don't match; turned back they do.
 * - The controller's screen shows MI's info panel (shape, status, progress, efficiency, EU); a hatch's its slots.
 *
 * State is read with `/mi probe` and `/mi shape` after `server-step`s of a held server. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const content = resolve(process.env.MI_CONTENT ?? '');
assert(existsSync(join(content, 'horizon-mod.toml')), `MI_CONTENT must name the content mod of Modern Industrialization (the script's content step): ${content}`);
const fixture = new WorldFixture('mi-multiblocks', 'server-thread', { devMods: [content] });
const atm10 = process.env.ATM10_DIR;
const serverToml = atm10 ? join(atm10, 'config/modern_industrialization-server.toml') : '';
if (serverToml && existsSync(serverToml)) {
  mkdirSync(join(fixture.game, 'config'), { recursive: true });
  copyFileSync(serverToml, join(fixture.game, 'config/modern_industrialization-server.toml'));
}
const FE_PER_EU = serverToml && existsSync(serverToml) ? 16 : 10;

const MI = (id: string) => `modern_industrialization:${id}`;
async function mi(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith('MI {'));
  assert(line, `no MI answer to ${command}: ${JSON.stringify(reply)}`);
  const json = JSON.parse(line.slice(3));
  assert(!json.error, `${command}: ${json.error}`);
  return json;
}
const probe = (pos: string) => mi(`mi probe ${pos}`);

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  // The controller at C facing north: the furnace stands from x - 1 to x + 1, y to y + 3, z to z + 2.
  const x = Math.floor(px) + 2, y = Math.min(Math.floor(py), 100), z = Math.floor(pz) + 3;
  const p = (dx: number, dy: number, dz: number) => `${x + dx} ${y + dy} ${z + dz}`;
  const C = p(0, 0, 0);
  await fixture.command(`fill ${x - 4} ${y} ${z - 4} ${x + 4} ${y + 6} ${z + 6} minecraft:air`);
  await fixture.command(`fill ${x - 4} ${y - 1} ${z - 4} ${x + 4} ${y - 1} ${z + 6} minecraft:stone`);
  await fixture.command(`setblock ${C} ${MI('electric_blast_furnace')}`);
  await fixture.hold();
  await fixture.stepServer(1);

  // Unbuilt: every member is missing.
  let c = await probe(C);
  const shape = await mi(`mi shape ${C}`);
  log.unbuilt = { c, missing: shape.missing.length };
  assert.equal(c.recipe_type, MI('blast_furnace'));
  assert(c.recipes > 0, `the EBF reads ATM10's blast furnace recipes: ${c.recipes}`);
  assert.deepEqual([c.shape_valid, c.members, c.facing, c.shape_name], [false, 33, 'north', 'Cupronickel']);
  assert.deepEqual([shape.members, shape.missing.length, shape.hatch_positions], [33, 33, 17]);
  assert(shape.missing.some((m: any) => m.block === MI('cupronickel_coil') && m.pos[1] === y + 1));

  // Built around the controller: casings, coils, casings.
  const casing = MI('heatproof_machine_casing'), coil = MI('cupronickel_coil');
  await fixture.command(`fill ${p(-1, 0, 1)} ${p(1, 0, 2)} ${casing}`);
  await fixture.command(`setblock ${p(-1, 0, 0)} ${casing}`);
  await fixture.command(`setblock ${p(1, 0, 0)} ${casing}`);
  await fixture.command(`fill ${p(-1, 1, 0)} ${p(1, 2, 2)} ${coil}`);
  await fixture.command(`fill ${p(0, 1, 1)} ${p(0, 2, 1)} minecraft:air`);
  await fixture.command(`fill ${p(-1, 3, 0)} ${p(1, 3, 2)} ${casing}`);
  await fixture.stepServer(1);
  c = await probe(C);
  log.built = c;
  assert.deepEqual([c.shape_valid, c.hatches.length, (await mi(`mi shape ${C}`)).missing.length], [true, 0, 0], 'formed with no hatch');

  // Hatches in the casings.
  const H = {
    itemIn: p(-1, 0, 0), itemOut: p(1, 0, 0), energy: p(0, 0, 2),
    fluidIn: p(-1, 3, 0), fluidOut: p(1, 3, 0), fluidOut2: p(0, 3, 0),
  };
  await fixture.command(`setblock ${H.itemIn} ${MI('steel_item_input_hatch')}`);
  await fixture.command(`setblock ${H.itemOut} ${MI('steel_item_output_hatch')}`);
  await fixture.command(`setblock ${H.energy} ${MI('lv_energy_input_hatch')}`);
  await fixture.command(`setblock ${H.fluidIn} ${MI('steel_fluid_input_hatch')}`);
  await fixture.command(`setblock ${H.fluidOut} ${MI('steel_fluid_output_hatch')}`);
  await fixture.command(`setblock ${H.fluidOut2} ${MI('steel_fluid_output_hatch')}`);
  await fixture.stepServer(1);
  c = await probe(C);
  log.hatches = c;
  assert.equal(c.shape_valid, true);
  assert.equal(c.hatches.length, 6, JSON.stringify(c.hatches));
  assert.deepEqual([c.item_inputs, c.item_outputs, c.fluid_inputs, c.fluid_outputs], [2, 2, 1, 2]);
  const energy = await probe(H.energy);
  assert.deepEqual([energy.hatch, energy.eu_capacity, energy.linked_to], ['energy_input', 19_200, [x, y, z]]);
  fixture.mark('the EBF: unbuilt 33 missing, formed with no hatch, then with 6 hatches linked to it');

  // Energy through the hatch.
  const fe = await mi(`mi offer_fe ${H.energy} 1000000`);
  assert.equal(fe.taken, 19_200 * FE_PER_EU, JSON.stringify(fe));
  assert.equal((await probe(H.energy)).eu, 19_200);

  // Raw synthetic oil to synthetic oil at 8 EU/t: 50 ticks a craft.
  await fixture.command(`data merge block ${H.fluidIn} {inventory_fluids:[{fluid:"${MI('raw_synthetic_oil')}",amount:200L}]}`);
  await fixture.stepServer(1);
  c = await probe(C);
  log.oil1 = c;
  assert.deepEqual([c.active_recipe, c.used_energy, c.recipe_max_eu], [MI('materials/blast_furnace/raw_oil_to_oil'), 8, 8]);
  assert.equal((await probe(H.fluidIn)).fluids[0].amount, 100);
  const oilOut = async () => {
    const outs = [await probe(H.fluidOut), await probe(H.fluidOut2)];
    return outs.flatMap(o => o.fluids).filter((f: any) => f.fluid === MI('synthetic_oil')).reduce((n: number, f: any) => n + f.amount, 0);
  };
  await fixture.stepServer(48);
  assert.equal(await oilOut(), 0, 'nothing out after 49 ticks');
  await fixture.stepServer(1);
  assert.equal(await oilOut(), 100, '100 mB of synthetic oil on tick 50');
  await fixture.stepServer(50);
  assert.equal(await oilOut(), 200, 'the second craft on ticks 51 to 100');
  assert.equal((await probe(H.energy)).eu, 19_200 - 800);
  fixture.mark('raw synthetic oil: 100 mB of synthetic oil every 50 ticks at 8 EU/t, 800 EU from the energy hatch');

  // Coke from coal needs 64 EU/t: banned by cupronickel.
  await fixture.command(`data merge block ${H.itemIn} {Items:[{Slot:0b,id:"minecraft:coal",count:16}]}`);
  await fixture.stepServer(5);
  c = await probe(C);
  log.banned = c.candidates;
  const coke = c.candidates.find((r: any) => r.eu === 64 && r.items);
  assert(coke, `ATM10's coke recipe is a candidate: ${JSON.stringify(c.candidates)}`);
  assert.deepEqual([c.active_recipe ?? null, coke.banned], [null, true], 'banned on cupronickel');

  // The kanthal shape: cupronickel coils don't match it, kanthal coils do.
  await fixture.command(`data merge block ${C} {activeShape:1}`);
  await fixture.stepServer(1);
  c = await probe(C);
  assert.deepEqual([c.shape_valid, c.shape_name], [false, 'Kanthal']);
  await fixture.command(`fill ${p(-1, 1, 0)} ${p(1, 2, 2)} ${MI('kanthal_coil')} replace ${coil}`);
  await fixture.stepServer(1);
  c = await probe(C);
  log.coke1 = c;
  assert.deepEqual([c.shape_valid, c.active_recipe, c.used_energy, c.recipe_max_eu], [true, coke.id, 64, 64]);
  assert.equal((await probe(H.itemIn)).slots[0], null, 'the 16 coal are taken');
  await fixture.stepServer(99);
  assert.equal((await probe(C)).used_energy, 6400);

  // A casing broken mid-recipe: no progress until it is back.
  await fixture.command(`setblock ${p(1, 0, 1)} minecraft:air`);
  await fixture.stepServer(1);
  c = await probe(C);
  log.broken = c;
  assert.deepEqual([c.shape_valid, c.operating_state, c.used_energy], [false, 'NotMatched', 6400]);
  await fixture.stepServer(4);
  assert.equal((await probe(C)).used_energy, 6400, 'no progress while unmatched');
  await fixture.command(`setblock ${p(1, 0, 1)} ${casing}`);
  await fixture.stepServer(1);
  c = await probe(C);
  assert.deepEqual([c.shape_valid, c.operating_state, c.used_energy], [true, 'NormalOperation', 6464], 'resumed');
  await fixture.stepServer(98);
  assert.equal((await probe(C)).used_energy, 12_736);
  await fixture.stepServer(1);
  const out = await probe(H.itemOut);
  const creosote = [await probe(H.fluidOut), await probe(H.fluidOut2)].flatMap(o => o.fluids).find((f: any) => f.fluid === MI('creosote'));
  log.coke = { out: out.slots, creosote };
  assert.deepEqual(out.slots[0], { item: MI('coke'), count: 16 });
  assert.equal(creosote?.amount, 8000);
  assert.equal((await probe(H.energy)).eu, 19_200 - 800 - 12_800);
  fixture.mark("ATM10's coke: banned on cupronickel, 200 ticks at 64 EU/t on kanthal, held while a casing was gone: 16 coke, 8,000 mB of creosote");

  // Turned east, the same blocks don't match; turned back they do.
  await fixture.command(`data merge block ${C} {facingDirection:5}`);
  await fixture.stepServer(1);
  c = await probe(C);
  assert.deepEqual([c.facing, c.shape_valid], ['east', false]);
  await fixture.command(`data merge block ${C} {facingDirection:2}`);
  await fixture.stepServer(1);
  assert.equal((await probe(C)).shape_valid, true);
  fixture.mark('turned east the furnace does not match, turned back it does');

  // A recipe running for the screens.
  await mi(`mi offer_fe ${H.energy} 1000000`);
  await fixture.command(`data merge block ${H.itemIn} {Items:[{Slot:0b,id:"minecraft:coal",count:16}]}`);
  await fixture.stepServer(30);
  await fixture.hold(false);
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z - 2.5} 0 10`);
  await fixture.replace('hotbar.0', 'minecraft:cobblestone', 3);
  await until(() => fixture.raw(`block ${C}`), v => v.block === MI('electric_blast_furnace'), 'the controller reaches the client');
  await fixture.raw(`interact ${C} north`);
  await until(() => fixture.raw('screen'), v => v.screen === 'container', 'the EBF screen opens');
  const shots: string[] = [];
  log.screenshots = shots;
  const shot = async (name: string) => {
    await Bun.sleep(1500);
    const path = join(fixture.run, `mi-${name}.png`);
    await fixture.raw(`screenshot ${path}`);
    shots.push(path);
  };
  await shot('electric-blast-furnace');
  await fixture.raw('close');
  await until(() => fixture.raw('screen'), v => v.screen !== 'container', 'the EBF screen closes');
  await fixture.command(`tp @s ${x + 1.5} ${y} ${z - 1.5} 0 30`);
  await fixture.raw(`interact ${H.itemOut} north`);
  const slot = (view: any, n: number) => (view.slots as any[] | undefined)?.find(s => s.slot === n);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && !!slot(v, 300), 'the output hatch screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.hatchSlots = own;
  assert(own.includes(0) && own.includes(1), `a steel hatch: its two slots ${JSON.stringify(own)}`);
  await shot('steel-item-output-hatch');
  await fixture.raw('close');
  fixture.mark('the EBF screen: MI info panel; the output hatch: its two slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MI multiblocks', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MI multiblocks', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
