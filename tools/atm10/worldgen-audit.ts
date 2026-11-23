/** The worldgen audit (docs/atm10-port.md, "Worldgen in the full pack"): the full pack in a real world, then the
 * game's `worldgen-audit` command (crates/horizon-client/src/worldgen_audit.rs) generates a square of chunks of the
 * overworld, the nether and the end off the world with the world's generator data and seed, and counts what is
 * there by mod: ore blocks, biomes, placed features, structure starts; a biome survey and a structure start survey
 * go wider. The report adds the placed features that can't run and the biome modifiers present, read and applied.
 *
 *   bun run tools/atm10/worldgen-audit.ts
 *   bun run tools/atm10/worldgen-audit.ts --summary <report.json>      # print an earlier report
 *   bun run tools/atm10/worldgen-audit.ts --compare <before.json> <after.json>
 *
 * - ATM10_CARRIES: the carries folder `final-data` wrote (each carry and `atm10-pack`); every folder in it with a
 *   `horizon-mod.toml` loads as a `--dev-mod`, after AE2's content, in alphabetical order.
 * - WORLDGEN_AUDIT_ONLY / WORLDGEN_AUDIT_WITHOUT: comma-separated carry folder names to keep / leave out
 *   (WORLDGEN_AUDIT_ONLY=none: no carries, vanilla and AE2 alone).
 * - WORLDGEN_AUDIT_OUT: the report's path (default: `worldgen-audit.json` in the fixture's run folder).
 * - WORLDGEN_AUDIT_RADIUS (16: 33 x 33 chunks), WORLDGEN_AUDIT_THREADS (4), WORLDGEN_AUDIT_START_RADIUS (64).
 *
 * One full-pack game at a time: it wants about 9 GB. */
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

type Dim = {
  error?: string; chunks: number; failed_chunk_count: number; ores_by_namespace: Record<string, number>;
  ores: Record<string, number>; biomes_by_namespace: Record<string, number>; survey_biomes_by_namespace: Record<string, number>;
  survey_biomes: Record<string, number>;
  features_by_namespace: Record<string, { features: number; features_placing: number; runs: number; runs_placing: number }>;
  modifier_features: Record<string, { mod: string; runs: number; runs_placing: number; cannot_run: string | null }>;
  starts: Record<string, number>; survey_starts_by_namespace: Record<string, number>;
  cannot_run: { total: number; by_namespace: Record<string, number>; by_reason: Record<string, number> };
  blocks_by_namespace: Record<string, number>;
};
type Report = {
  registered_biomes: number; registered_biomes_by_namespace: Record<string, number>;
  biome_modifiers: { present: number; read: number; applied: number; none: number; not_read_types: Record<string, number>; unresolved: string[]; biomes_changed: number; error?: string };
  dimensions: Record<string, Dim>; seconds: number;
};

const top = (o: Record<string, number> | undefined, n = 12) =>
  Object.entries(o ?? {}).sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, v]) => `${k} ${v}`).join(', ');
const mods = (o: Record<string, number> | undefined) => Object.fromEntries(Object.entries(o ?? {}).filter(([k]) => k !== 'minecraft'));
const sum = (o: Record<string, number> | undefined) => Object.values(o ?? {}).reduce((a, b) => a + b, 0);

/** The headline numbers of a report, the ones docs/atm10-port.md keeps. */
function headline(r: Report) {
  const out: Record<string, number> = {
    'modifiers present': r.biome_modifiers.present, 'modifiers read': r.biome_modifiers.read,
    'modifiers applied': r.biome_modifiers.applied, 'biomes changed by modifiers': r.biome_modifiers.biomes_changed,
    'registered biomes': r.registered_biomes, 'registered mod biomes': sum(mods(r.registered_biomes_by_namespace)),
  };
  for (const [dim, d] of Object.entries(r.dimensions)) {
    if (d.error) continue;
    const short = dim.replace('minecraft:', '');
    const featureMods = Object.entries(d.features_by_namespace).filter(([ns]) => ns !== 'minecraft');
    Object.assign(out, {
      [`${short}: chunks`]: d.chunks,
      [`${short}: mod ore blocks`]: sum(mods(d.ores_by_namespace)),
      [`${short}: mods with ores`]: Object.keys(mods(d.ores_by_namespace)).length,
      [`${short}: mod biome quarts`]: sum(mods(d.biomes_by_namespace)),
      [`${short}: mod biomes in survey`]: Object.keys(d.survey_biomes).filter(b => !b.startsWith('minecraft:')).length,
      [`${short}: mod features placing`]: featureMods.reduce((a, [, c]) => a + c.features_placing, 0),
      [`${short}: mod feature runs placing`]: featureMods.reduce((a, [, c]) => a + c.runs_placing, 0),
      [`${short}: mod structure starts (survey)`]: sum(mods(d.survey_starts_by_namespace)),
      [`${short}: placed features that can't run`]: d.cannot_run.total,
    });
  }
  return out;
}

function summary(r: Report) {
  const m = r.biome_modifiers;
  if (m.error) console.log(`biome modifiers: ${m.error}`);
  console.log(`biome modifiers: ${m.present} present, ${m.read} read (+${m.none} none), ${m.applied} applied, ${m.biomes_changed} biomes changed`);
  console.log(`  not read, by type: ${top(m.not_read_types, 10)}`);
  console.log(`  unresolved ${m.unresolved?.length ?? 0}: ${(m.unresolved ?? []).slice(0, 4).map(s => s.slice(0, 120)).join(' | ')}`);
  console.log(`registered biomes ${r.registered_biomes}: ${top(r.registered_biomes_by_namespace)}`);
  for (const [dim, d] of Object.entries(r.dimensions)) {
    console.log(`\n${dim}`);
    if (d.error) { console.log(`  error: ${d.error}`); continue; }
    console.log(`  chunks ${d.chunks} (failed ${d.failed_chunk_count}); blocks by namespace: ${top(d.blocks_by_namespace, 8)}`);
    console.log(`  ore blocks by namespace: ${top(d.ores_by_namespace, 20)}`);
    console.log(`  mod ores: ${top(mods(Object.fromEntries(Object.entries(d.ores).filter(([k]) => !k.startsWith('minecraft:')))), 14)}`);
    console.log(`  biomes (quarts) by namespace: ${top(d.biomes_by_namespace)}; survey: ${top(d.survey_biomes_by_namespace)}`);
    console.log(`  features by namespace (placing/ran): ${Object.entries(d.features_by_namespace).sort((a, b) => b[1].runs_placing - a[1].runs_placing).slice(0, 14).map(([ns, c]) => `${ns} ${c.features_placing}/${c.features}`).join(', ')}`);
    const mf = Object.values(d.modifier_features);
    console.log(`  modifier features: ${mf.filter(f => f.runs_placing > 0).length} placing, ${mf.filter(f => f.runs > 0 && f.runs_placing === 0).length} ran without placing, ${mf.filter(f => f.cannot_run).length} can't run`);
    console.log(`  structure starts (survey) by namespace: ${top(d.survey_starts_by_namespace, 14)}`);
    console.log(`  can't run ${d.cannot_run.total}: ${top(d.cannot_run.by_namespace, 10)}`);
    for (const [reason, n] of Object.entries(d.cannot_run.by_reason).slice(0, 8)) console.log(`    ${String(n).padStart(5)}  ${reason}`);
  }
  console.log('\nheadline:');
  for (const [k, v] of Object.entries(headline(r))) console.log(`  ${k.padEnd(48)} ${v}`);
}

function compare(a: Report, b: Report) {
  const [ha, hb] = [headline(a), headline(b)];
  console.log(`${''.padEnd(48)} ${'before'.padStart(9)} ${'after'.padStart(9)}`);
  for (const k of new Set([...Object.keys(ha), ...Object.keys(hb)])) {
    console.log(`${k.padEnd(48)} ${String(ha[k] ?? '-').padStart(9)} ${String(hb[k] ?? '-').padStart(9)}`);
  }
  for (const dim of Object.keys(b.dimensions)) {
    const [oa, ob] = [a.dimensions[dim]?.ores_by_namespace ?? {}, b.dimensions[dim]?.ores_by_namespace ?? {}];
    const changed = [...new Set([...Object.keys(oa), ...Object.keys(ob)])].filter(k => k !== 'minecraft' && (oa[k] ?? 0) !== (ob[k] ?? 0));
    if (changed.length) console.log(`${dim} ores: ${changed.map(k => `${k} ${oa[k] ?? 0} -> ${ob[k] ?? 0}`).join(', ')}`);
  }
}

const args = process.argv;
if (args.includes('--summary')) {
  summary(JSON.parse(readFileSync(args[args.indexOf('--summary') + 1], 'utf8')));
  process.exit(0);
}
if (args.includes('--compare')) {
  const i = args.indexOf('--compare');
  compare(JSON.parse(readFileSync(args[i + 1], 'utf8')), JSON.parse(readFileSync(args[i + 2], 'utf8')));
  process.exit(0);
}

const { WorldFixture } = await import('../../ae2/tests/world/harness');
const carries = resolve(process.env.ATM10_CARRIES ?? '');
if (!process.env.ATM10_CARRIES || !existsSync(join(carries, 'atm10-pack', 'horizon-mod.toml'))) {
  throw new Error('ATM10_CARRIES must name the carries folder final-data wrote (with atm10-pack)');
}
const list = (name: string) => (process.env[name] ?? '').split(',').map(s => s.trim()).filter(Boolean);
const only = list('WORLDGEN_AUDIT_ONLY'), without = new Set(list('WORLDGEN_AUDIT_WITHOUT'));
const devMods = readdirSync(carries, { withFileTypes: true })
  .filter(d => d.isDirectory() && existsSync(join(carries, d.name, 'horizon-mod.toml')))
  .map(d => d.name)
  .sort()
  .filter(id => (only.length ? only.includes(id) : true) && !without.has(id));
process.env.AE2_TEST_STARTUP_MS = String(Math.max(Number(process.env.AE2_TEST_STARTUP_MS ?? 0), 1_800_000));
console.log(`${devMods.length} carries from ${carries}`);
const fixture = new WorldFixture('worldgen-audit', 'server-thread', { devMods: devMods.map(id => join(carries, id)) });
const out = resolve(process.env.WORLDGEN_AUDIT_OUT ?? join(fixture.run, 'worldgen-audit.json'));
try {
  const started = Date.now();
  await fixture.start();
  console.log(`in the world after ${((Date.now() - started) / 1000).toFixed(1)} s`);
  const env = (k: string, d: number) => Number(process.env[k] ?? d);
  const line = `worldgen-audit ${out.replaceAll('\\', '/')} ${env('WORLDGEN_AUDIT_RADIUS', 16)} ${env('WORLDGEN_AUDIT_THREADS', 4)} ${env('WORLDGEN_AUDIT_START_RADIUS', 64)}`;
  const reply = await fixture.raw(line, 60 * 60_000);
  if (reply?.error) throw new Error(`worldgen-audit: ${reply.error}`);
  console.log(`audit ran in ${Number(reply.seconds ?? 0).toFixed(1)} s; report ${out}\n`);
  const report: Report = JSON.parse(readFileSync(out, 'utf8'));
  summary(report);
  const overworld = report.dimensions['minecraft:overworld'];
  if (!overworld || overworld.error || overworld.chunks === 0) throw new Error('no overworld chunk generated');
  fixture.mark(`overworld: ${overworld.chunks} chunks, ${sum(mods(overworld.ores_by_namespace))} mod ore blocks`);
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
