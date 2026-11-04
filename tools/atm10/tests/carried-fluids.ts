/** Carried pack fluids (docs/mod-fluids.md, slice 2): fluids that `horizon-port-pack` declares in ATM10's
 * carries from the NeoForge oracle's facts (the server's numbers, the client's textures, tints and layers), in a world
 * with those carries alone. For each sample:
 *
 * - its bucket places a source of its own liquid block (`<block>[level=0]`), and the bucket is empty after;
 * - it spreads by its own numbers on a flat floor: `drop` levels less a block (`getDropOff`), as far as that reaches,
 *   every flowing block still its block, at its own pace (`getTickDelay`);
 * - the pool draws: looked at from above it is no longer the floor, and nowhere the missing texture;
 * - an empty bucket takes the source back as the fluid's bucket.
 *
 * Samples, with Java's values (the oracle runs' facts of ATM10 8.4): Industrial Foregoing's latex (Titanium's fluid:
 * `latex_flowing`, its own textures, solid, no blockstate in its jar), Immersive Engineering's creosote (`IEFluid`: a
 * tick delay of 10, density 1230, viscosity 2000, its block `creosote_fluid_block`, translucent), Mekanism's sulfuric
 * acid (`mekanism:liquid/liquid` tinted 0xFF82802B, density and viscosity 1840, translucent) and All the Ores' molten
 * aluminum (lava's numbers: a tick delay of 30, two levels a block, light 15, tinted 0xFFE3E3E3). Modern
 * Industrialization's crude oil, a `Fluid` that doesn't flow, is checked in its declaration only (its bucket places
 * its own block, which the engine doesn't port).
 *
 * Needs ATM10_CARRIES: the carries folder `final-data` wrote (its `industrialforegoing`, `immersiveengineering`,
 * `mekanism`, `alltheores` and `modern_industrialization`), carried again from the client oracle's facts too. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { inflateSync } from 'node:zlib';
import { WorldFixture, until } from '../../../ae2/tests/world/harness';

const carries = resolve(process.env.ATM10_CARRIES ?? '');
type Sample = { carry: string; fluid: string; block: string; bucket: string; drop: number; tick: number; java: Record<string, unknown>; tint?: number };
const samples: Sample[] = [
  { carry: 'industrialforegoing', fluid: 'industrialforegoing:latex', block: 'industrialforegoing:latex', bucket: 'industrialforegoing:latex_bucket', drop: 1, tick: 5,
    java: { flowing: 'industrialforegoing:latex_flowing', still_texture: 'industrialforegoing:block/fluids/latex_still', translucent: false, tint: '0xFFFFFFFF', viscosity: 1000 } },
  { carry: 'immersiveengineering', fluid: 'immersiveengineering:creosote', block: 'immersiveengineering:creosote_fluid_block', bucket: 'immersiveengineering:creosote_bucket', drop: 1, tick: 10,
    java: { flowing: 'immersiveengineering:creosote_flowing', still_texture: 'immersiveengineering:block/fluid/creosote_still', translucent: true, density: 1230, viscosity: 2000, explosion_resistance: 100 } },
  { carry: 'mekanism', fluid: 'mekanism:sulfuric_acid', block: 'mekanism:sulfuric_acid', bucket: 'mekanism:sulfuric_acid_bucket', drop: 1, tick: 5, tint: 0xff82802b,
    java: { flowing: 'mekanism:flowing_sulfuric_acid', still_texture: 'mekanism:liquid/liquid', overlay_texture: 'minecraft:block/water_overlay', translucent: true, tint: '0xFF82802B', density: 1840, viscosity: 1840 } },
  { carry: 'alltheores', fluid: 'alltheores:molten_aluminum', block: 'alltheores:molten_aluminum', bucket: 'alltheores:molten_aluminum_bucket', drop: 2, tick: 30,
    java: { still_texture: 'alltheores:block/fluid/molten_metal', translucent: false, tint: '0xFFE3E3E3', light_level: 15, temperature: 1300, viscosity: 6000,
      sounds: { bucket_fill: 'minecraft:item.bucket.fill_lava', bucket_empty: 'minecraft:item.bucket.empty_lava' } } },
];
for (const name of [...samples.map(s => s.carry), 'modern_industrialization']) assert(existsSync(join(carries, name, 'horizon-mod.toml')), `ATM10_CARRIES has no ${name}`);
const declaration = (carry: string, fluid: string) => {
  const [ns, path] = fluid.split(':');
  return JSON.parse(readFileSync(join(carries, carry, `data/${ns}/horizon/fluid/${path}.json`), 'utf8'));
};
// The declarations the carries hold are Java's values.
for (const s of samples) {
  const decl = declaration(s.carry, s.fluid);
  assert.equal(decl.block, s.block, `${s.fluid}'s block`); assert.equal(decl.bucket, s.bucket, `${s.fluid}'s bucket`);
  assert.equal(decl.level_decrease_per_block, s.drop); assert.equal(decl.tick_rate, s.tick);
  for (const [key, value] of Object.entries(s.java)) assert.deepEqual(decl[key], value, `${s.fluid}'s ${key}`);
}
const oil = declaration('modern_industrialization', 'modern_industrialization:crude_oil');
assert.deepEqual([oil.flows, oil.block, oil.bucket, oil.still_texture], [false, 'minecraft:air', 'modern_industrialization:crude_oil_bucket', 'modern_industrialization:fluid/crude_oil_still']);

// Horizon writes 8-bit, non-interlaced RGBA PNG screenshots (as dynamic-item-models reads them).
function screenshot(path: string) {
  const png = readFileSync(path); assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  let width = 0, height = 0; const chunks: Buffer[] = [];
  for (let p = 8; p + 12 <= png.length;) {
    const n = png.readUInt32BE(p), type = png.toString('ascii', p + 4, p + 8), body = png.subarray(p + 8, p + 8 + n);
    if (type === 'IHDR') { width = body.readUInt32BE(0); height = body.readUInt32BE(4); assert.deepEqual([...body.subarray(8, 13)], [8, 6, 0, 0, 0]); }
    if (type === 'IDAT') chunks.push(body); p += n + 12; if (type === 'IEND') break;
  }
  const source = inflateSync(Buffer.concat(chunks)), stride = width * 4, pixels = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = source[y * (stride + 1)], start = y * stride;
    for (let x = 0; x < stride; x++) {
      const left = x >= 4 ? pixels[start + x - 4] : 0, up = y ? pixels[start + x - stride] : 0, corner = y && x >= 4 ? pixels[start + x - stride - 4] : 0;
      const paeth = () => { const p = left + up - corner, a = Math.abs(p - left), b = Math.abs(p - up), c = Math.abs(p - corner); return a <= b && a <= c ? left : b <= c ? up : corner; };
      const predictor = filter === 0 ? 0 : filter === 1 ? left : filter === 2 ? up : filter === 3 ? Math.floor((left + up) / 2) : filter === 4 ? paeth() : NaN;
      assert(Number.isFinite(predictor), 'supported PNG row filter'); pixels[start + x] = (source[y * (stride + 1) + 1 + x] + predictor) & 255;
    }
  }
  return { width, height, pixels };
}
// MissingTextureAtlasSprite's magenta.
const missing = (r: number, g: number, b: number) => r > 240 && g < 16 && b > 240;

const fixture = new WorldFixture('carried-fluids', 'server-thread', { devMods: samples.map(s => join(carries, s.carry)) });
const passed = (reply: any) => (reply.chat ?? []).some((line: string) => line.includes('Test passed'));
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  const isBlock = async (pos: string, state: string) => passed(await fixture.command(`execute if block ${pos} ${state}`));
  await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
  for (const [dx, dz] of [[-12, -12], [-12, 12], [12, -12], [12, 12]] as const) {
    await until(async () => passed(await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)), v => v, 'fixture corner loaded', 30000);
  }
  await fixture.command('clear @s'); await fixture.command('gamemode survival @s');
  await fixture.command('time set noon'); await fixture.command('gamerule doDaylightCycle false'); await fixture.command('weather clear');
  // The view straight down at the pool's middle from a spectator who doesn't fall: a patch just above the screen's
  // centre (the chat's lines are below it), inside even a 3-block pool's diamond.
  const look = async (name: string) => {
    await fixture.command('gamemode spectator @s');
    await fixture.command(`tp @s ${x + 0.5} ${y + 6} ${z + 0.5} 0 90`);
    await Bun.sleep(2000);
    const path = join(fixture.run, `${name}.png`).replaceAll('\\', '/');
    await fixture.raw(`screenshot ${path}`);
    const image = screenshot(path), out: number[][] = [];
    for (let py2 = Math.floor(image.height * 0.35); py2 < Math.floor(image.height * 0.5); py2 += 2) {
      for (let px2 = Math.floor(image.width * 0.45); px2 < Math.floor(image.width * 0.55); px2 += 2) {
        const o = (py2 * image.width + px2) * 4; out.push([image.pixels[o], image.pixels[o + 1], image.pixels[o + 2]]);
      }
    }
    await fixture.command('gamemode survival @s');
    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
    return out;
  };
  for (const s of samples) {
    const reach = Math.floor(7 / s.drop);
    await fixture.command(`fill ${at(-12, 0, -12)} ${at(12, 6, 12)} minecraft:air`);
    await fixture.command(`fill ${at(-12, -1, -12)} ${at(12, -1, 12)} minecraft:stone`);
    const floor = await look(`${s.carry}-floor`);
    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
    await fixture.replace('hotbar.0', s.bucket); await fixture.select(0, s.bucket);
    await fixture.raw(`look-at block ${at(0, -1, 0)} up`); await fixture.raw('use-item');
    await until(() => isBlock(at(0), `${s.block}[level=0]`), v => v, `${s.bucket} places a source of ${s.block}`);
    await until(() => fixture.raw('inventory'), v => v.hotbar?.[0]?.item === 'minecraft:bucket', 'the bucket is empty');
    fixture.mark(`${s.bucket} places a source of ${s.fluid}'s own block`);

    await until(() => isBlock(at(reach), `${s.block}[level=${reach * s.drop}]`), v => v, `${s.fluid} spreads ${reach} blocks`, 30000 + s.tick * reach * 100);
    for (let d = 1; d <= reach; d++) {
      for (const pos of [at(d), at(-d), at(0, 0, d), at(0, 0, -d)]) assert(await isBlock(pos, `${s.block}[level=${d * s.drop}]`), `level ${d * s.drop} at ${d} blocks`);
    }
    assert(await isBlock(at(reach + 1), 'minecraft:air'), `nothing past ${reach} blocks`);
    assert(!(await isBlock(at(1), 'minecraft:lava')) && !(await isBlock(at(1), 'minecraft:water')), 'neither lava nor water');
    fixture.mark(`${s.fluid} flows by its own numbers: ${s.drop} level a block, ${reach} blocks out, every ${s.tick} ticks`);

    const pool = await look(`${s.carry}-pool`);
    const changed = pool.filter((p, i) => p.some((v, c) => Math.abs(v - floor[i][c]) > 12)).length;
    // A third at least: molten aluminum's pale grey metal over the grey stone keeps many pixels near the floor's.
    assert(changed > pool.length / 3, `${s.fluid} draws over the floor (${changed} of ${pool.length})`);
    assert(!pool.some(p => missing(p[0], p[1], p[2])), `${s.fluid} draws with its own textures`);
    fixture.mark(`${s.fluid} draws in the world with its sprites`);

    await fixture.command(`tp @s ${x + 0.5} ${y} ${z + 4.5}`);
    await fixture.raw(`look-at ${x + 0.5} ${y + 0.5} ${z + 0.5}`); await fixture.raw('use-item');
    await until(() => fixture.raw('inventory'), v => v.hotbar?.[0]?.item === s.bucket, `the bucket takes ${s.fluid} back`);
    assert(!(await isBlock(at(0), `${s.block}[level=0]`)), 'the source is gone');
    fixture.mark(`an empty bucket picks the ${s.fluid} source up`);
  }

  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
