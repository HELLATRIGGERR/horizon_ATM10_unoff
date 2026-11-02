/** Mekanism Generators' Heat Generator, as the `mekanism` native bundle and its content mods make it, under ATM10's
 * `generators.toml` (heat 800, 30 a face of lava, a 24 000 mB tank burnt 10 mB a tick) in plains (300 K).
 *
 * - Lava on its six faces heats it 180 a tick with nothing to burn: it warms and settles near 96 J a tick, while one
 *   without lava stays at the air's 300 K and makes nothing.
 * - A lava bucket in its fuel slot empties into its tank, 1000 mB, and leaves the bucket; with its store full it burns
 *   nothing, with room it burns 10 mB a tick, shows `active=true`, and turns `active=false` once the tank is dry.
 * - A hopper above feeds its fuel slot coal, 80 mB of lava each (burn time / 20), and can't put stone in it.
 * - Its energy leaves through its front: a basic energy cube there takes what it makes; AE2 reads that face as Forge
 *   Energy (160 000 J = 64 000 FE) and no other face.
 * - Its lava, heat and active state survive a save and restart; its screen shows the fuel and energy slots.
 *
 * Joules, lava and heat are read from the saved compound (`data get block`, Mekanism's `energy_containers`,
 * `fluid_tanks` and `heat_capacitors`), the rest from `/mek probe`. Needs what `solar-generator.ts` needs. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const generators = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-heat', 'server-thread', { devMods: [content, generators] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/generators.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'generators.toml', 'generator-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'generators.toml'), '[solar_generator]\n\tsolarGeneration = 150\n[heat_generator]\n\theatGeneration = 800\n\theatGenerationLava = 30\n\theatGenerationNether = 100\n\ttankCapacity = 24000\n\theatGenerationFluidRate = 10\n');
  writeFileSync(join(config, 'generator-storage.toml'), 'heatGenerator = 160000\nsolarGenerator = 96000\n');
}

async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
const energyIn = (nbt: string) => Number(nbt.match(/energy_containers: ?\[\{[^\]]*stored: (\d+)L/)?.[1] ?? 0);
const lavaIn = (nbt: string) => Number(nbt.match(/fluid_tanks: ?\[\{[^\]]*amount: (\d+)/)?.[1] ?? 0);
const heatIn = (nbt: string) => Number(nbt.match(/heat_capacitors: ?\[\{[^\]]*stored: ([\d.E-]+)d/)?.[1] ?? NaN);
async function energy(pos: string): Promise<number> { return energyIn(await saved(pos)); }
async function lava(pos: string): Promise<number> { return lavaIn(await saved(pos)); }
async function probe(pos: string): Promise<any> {
  const needle = `"pos":[${pos.split(' ').join(',')}]`;
  const newest = async () => {
    const line = ((await fixture.raw('chat-log 100')).chat ?? []).find((t: string) => t.startsWith('MEK {'));
    return line ? JSON.parse(line.slice(4)) : { n: 0 };
  };
  const seen = (await newest()).n;
  await fixture.raw(`chat /mek probe ${pos}`);
  const answer = await until(newest, a => a.n > seen, `/mek probe ${pos}`, 60000);
  assert(JSON.stringify(answer).includes(needle) && !answer.error, `/mek probe ${pos}: ${JSON.stringify(answer)}`);
  return answer;
}
/** Each position's gain a held tick, over `ticks`. */
async function rates(positions: string[], ticks = 5): Promise<number[][]> {
  await fixture.hold();
  const out = positions.map(() => [] as number[]);
  let last = await Promise.all(positions.map(energy));
  for (let i = 0; i < ticks; i++) {
    await fixture.stepServer(1);
    const now = await Promise.all(positions.map(energy));
    now.forEach((v, j) => out[j].push(v - last[j]));
    last = now;
  }
  return out;
}
const activeShown = async (pos: string) => String((await fixture.raw(`block ${pos}`)).state ?? '');
const FULL = '{energy_containers:[{container:0b,stored:160000L}]}';
const EMPTY = '{energy_containers:[{container:0b,stored:0L}]}';

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 4;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -3)} ${at(20, 6, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -3)} ${at(20, -2, 8)} minecraft:stone`);
  await fixture.command(`fillbiome ${at(-10, -10, -10)} ${at(28, 12, 16)} minecraft:plains`);

  // A: lava on all six faces, in a stone shell. B: nothing around it.
  await fixture.hold();
  await fixture.command(`fill ${at(-2, -1, -2)} ${at(2, 3, 2)} minecraft:stone`);
  await fixture.command(`fill ${at(-1, 0, -1)} ${at(1, 2, 1)} minecraft:lava`);
  const a = at(0, 1, 0), b = at(6, 0, 0);
  await fixture.command(`setblock ${a} mekanismgenerators:heat_generator`);
  await fixture.command(`setblock ${b} mekanismgenerators:heat_generator`);
  await fixture.stepServer(300);
  const [passive, cold] = await rates([a, b]);
  log.passive = passive; log.cold = cold;
  for (const r of passive) assert(r >= 95 && r <= 97, `lava around it alone: about 96 J a tick, got ${passive}`);
  assert.deepEqual(cold, [0, 0, 0, 0, 0], 'without lava it makes nothing');
  await fixture.hold(false);
  const pa = await probe(a), pb = await probe(b);
  log.probe_a = pa; log.probe_b = pb;
  assert.equal(pa.lava_sides, 6);
  assert.equal(pa.lava, 0);
  assert.equal(pa.active, false, 'nothing burns');
  assert(pa.temperature > 640 && pa.temperature < 670, `it settles near 656 K: ${pa.temperature}`);
  assert.equal(Math.round(pa.ambient), 300);
  assert(Math.abs(pb.temperature - pb.ambient) < 1e-9, `B stays at the air's temperature: ${pb.temperature}`);
  assert.equal(pb.produced, 0);
  assert.equal(pa.max_output, 2220);
  fixture.mark(`lava on six faces: ${passive[4]} J a tick at ${Math.round(pa.temperature)} K; none without`);

  // C: a lava bucket in the fuel slot, its store full: 1000 mB in, the bucket back, nothing burnt.
  const c = at(12, 0, 0), front = at(12, 0, -1);
  await fixture.hold();
  await fixture.command(`setblock ${c} mekanismgenerators:heat_generator`);
  await fixture.command(`data merge block ${c} ${FULL}`);
  await fixture.command(`data merge block ${c} {Items:[{Slot:0b,id:"minecraft:lava_bucket",count:1}]}`);
  await fixture.stepServer(2);
  const filled = await saved(c);
  log.filled = filled;
  assert.equal(lavaIn(filled), 1000, 'a bucket gives 1000 mB');
  assert(/id: ?"minecraft:bucket"/.test(filled) && !filled.includes('lava_bucket'), `the bucket stays, emptied: ${filled}`);
  await fixture.stepServer(5);
  assert.equal(await lava(c), 1000, 'with a full store it burns nothing');
  fixture.mark('a lava bucket in the fuel slot gives 1000 mB and leaves an empty bucket');

  // Room in its store: it burns 10 mB a tick and shows it.
  await fixture.command(`data merge block ${c} ${EMPTY}`);
  await fixture.stepServer(10);
  assert.equal(await lava(c), 900, '10 mB a tick');
  await fixture.hold(false);
  await until(() => activeShown(c), s => s.includes('active=true'), 'the burning generator shows active=true');
  fixture.mark('with room it burns 10 mB a tick and shows active=true');

  // Its front feeds a cube; FE on the front only.
  await fixture.hold();
  await fixture.command(`setblock ${front} mekanism:basic_energy_cube`);
  await fixture.command(`data merge block ${front} {component_config:{eject0:0b}}`);
  await fixture.stepServer(3);
  const [toCube, kept] = await rates([front, c]);
  log.into_cube = { toCube, kept };
  for (const r of toCube) assert(r > 0, `the cube in front gains: ${toCube}`);
  const generatorStore = await energy(c);
  assert(generatorStore < 1110, `it passes on what it makes (${generatorStore} J kept)`);
  const fe = await fixture.ae(`power ${c} north`);
  log.fe_front = fe.fe;
  assert.equal(fe.fe?.capacity, 64000, '160 000 J are 64 000 FE');
  assert.equal((await fixture.ae(`power ${c} south`)).fe, null, 'no energy on its back');
  fixture.mark(`its front feeds a basic cube (${toCube.join(', ')} J a tick); Forge Energy on its front only`);

  // D: a hopper above with coal and stone; its store full, so lava builds up.
  const d = at(16, 0, 0), hopper = at(16, 1, 0);
  await fixture.command(`setblock ${d} mekanismgenerators:heat_generator`);
  await fixture.command(`data merge block ${d} ${FULL}`);
  await fixture.command(`setblock ${hopper} minecraft:hopper[facing=down]`);
  await fixture.command(`data merge block ${hopper} {Items:[{Slot:0b,id:"minecraft:coal",count:2},{Slot:1b,id:"minecraft:stone",count:1}]}`);
  await fixture.stepServer(40);
  const fed = await saved(d), left = await saved(hopper);
  log.coal = { generator: fed, hopper: left };
  assert.equal(lavaIn(fed), 160, 'two coal are 2 x 80 mB');
  assert(left.includes('minecraft:stone') && !left.includes('minecraft:coal'), `the stone stays in the hopper: ${left}`);
  assert(!fed.includes('minecraft:stone'), `no stone in the fuel slot: ${fed}`);
  fixture.mark('a hopper feeds it coal (80 mB each) and can\'t put stone in its fuel slot');

  // C runs dry: active=false.
  await fixture.stepServer(100);
  assert.equal(await lava(c), 0);
  await fixture.hold(false);
  await until(() => activeShown(c), s => s.includes('active=false'), 'a dry generator shows active=false', 30000);
  fixture.mark('dry, it shows active=false');

  // Save and restart: A keeps its heat, D its lava.
  await fixture.hold();
  const before = { heatA: heatIn(await saved(a)), lavaD: await lava(d), energyD: await energy(d) };
  await fixture.hold(false);
  await fixture.restart();
  await fixture.hold();
  const after = { heatA: heatIn(await saved(a)), lavaD: await lava(d), energyD: await energy(d) };
  log.restart = { before, after };
  assert(Number.isFinite(before.heatA) && before.heatA > 3000, `A's heat was saved: ${before.heatA}`);
  assert.equal(after.lavaD, before.lavaD);
  assert.equal(after.energyD, before.energyD);
  assert(Math.abs(after.heatA - before.heatA) < 1e-6 * before.heatA, `A's heat survives: ${before.heatA} -> ${after.heatA}`);
  await fixture.hold(false);
  fixture.mark('heat, lava and energy survive a save and restart');

  // Its screen: the fuel and energy slots, and half a tank of lava in its gauge.
  await fixture.command(`data merge block ${c} {fluid_tanks:[{tank:0b,stored:{id:"minecraft:lava",amount:12000}}]}`);
  await fixture.command(`tp @s ${x + 12.5} ${y} ${z + 1.5}`);
  await until(() => fixture.raw(`block ${c}`), v => v.block === 'mekanismgenerators:heat_generator', 'the generator reaches the client');
  await fixture.raw(`interact ${c} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the heat generator screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1], 'the fuel and energy slots');
  assert.equal(view.title?.[0]?.text, 'Heat Generator');
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'heat-screen.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw('close');
  fixture.mark('its screen shows the fuel and energy slots');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK heat', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK heat', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
