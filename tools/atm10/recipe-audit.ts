/** The recipe audit (docs/atm10-port.md, "Recipes that run"): the full pack in a real world, then every recipe of
 * vanilla's types (crafting shaped and shapeless, smelting, blasting, smoking, campfire, stonecutting, smithing) put
 * through the engine's own matcher by the game's `recipe-audit` command (crates/horizon-client/src/recipe_audit.rs):
 * parsed, every ingredient and the result registered (tags expanded), run as the menu runs it, the result matched.
 * The mod types the mods' recipe viewer categories read (Mekanism's, AE2's and its addons' machines) are audited
 * through those categories: parsed (a reader accepted it), resolved (its stacks are registered); no machine matcher runs.
 * Other mod types are counted.
 *
 *   bun run tools/atm10/recipe-audit.ts
 *
 * - ATM10_CARRIES: the carries folder `final-data` wrote (each carry and `atm10-pack`); every folder in it with a
 *   `horizon-mod.toml` loads as a `--dev-mod`, after AE2's content, in alphabetical order.
 * - RECIPE_AUDIT_ONLY / RECIPE_AUDIT_WITHOUT: comma-separated carry folder names to keep / leave out.
 * - RECIPE_AUDIT_OUT: the report's path (default: `recipe-audit.json` in the fixture's run folder, under
 *   .local/world-tests). It holds every failure, so it stays out of Git; `--summary FILE` prints an earlier one.
 * - RECIPE_AUDIT_MODS: comma-separated folders of more `--dev-mod`s after the carries (the Mekanism build's content mods, which
 *   take the place of the carries `mekanism`, `mekanismgenerators` and `alltheores`: leave those out with RECIPE_AUDIT_WITHOUT).
 * - RECIPE_AUDIT_THREADS: the matcher's threads (default 4).
 * - AE2_TEST_STARTUP_MS: at least 1800000 here: the full pack's first start takes minutes.
 *
 * One full-pack game at a time: it wants about 9 GB. */
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

type Counts = { total: number; parsed: number; resolved: number; ran: number; matched: number };
type Report = {
  sources: number; audited: Counts; by_type: Record<string, Counts>; by_namespace: Record<string, Counts>;
  causes: Record<string, { recipes: number; details: number; top_details: [string, number][]; by_namespace: [string, number][] }>;
  other_vanilla_types: Record<string, number>; unsupported_types: { recipes: number; types: number; by_type: Record<string, number> };
};

function summary(report: Report) {
  const row = (name: string, c: Counts) =>
    `${name.padEnd(30)} ${[c.total, c.parsed, c.resolved, c.ran, c.matched].map(n => String(n).padStart(7)).join(' ')}`;
  console.log(`${'type'.padEnd(30)} ${['total', 'parsed', 'resolved', 'ran', 'matched'].map(s => s.padStart(7)).join(' ')}`);
  for (const [type, c] of Object.entries(report.by_type)) console.log(row(type.replace('minecraft:', ''), c));
  console.log(row('all audited', report.audited));
  console.log(`\nsources ${report.sources}; mod types (not run) ${report.unsupported_types.recipes} in ${report.unsupported_types.types} types; other vanilla types ${Object.values(report.other_vanilla_types).reduce((a, b) => a + b, 0)}`);
  for (const [cause, c] of Object.entries(report.causes).sort((a, b) => b[1].recipes - a[1].recipes)) {
    console.log(`\n${cause}: ${c.recipes} recipes, ${c.details} details`);
    for (const [detail, n] of c.top_details.slice(0, 8)) console.log(`  ${String(n).padStart(6)}  ${detail}`);
    console.log(`  by namespace: ${c.by_namespace.slice(0, 8).map(([ns, n]) => `${ns} ${n}`).join(', ')}`);
  }
  const m = report.machines;
  if (m) {
    console.log(`\nmod types read through the mods' recipe viewer categories (checked: parsed, resolved)`);
    console.log(`${'type'.padEnd(40)} ${['total', 'parsed', 'resolved'].map(s => s.padStart(7)).join(' ')}  categories`);
    for (const [type, c] of Object.entries(m.by_type).sort((a, b) => a[0].localeCompare(b[0]))) {
      console.log(`${type.padEnd(40)} ${[c.total, c.parsed, c.resolved].map(n => String(n).padStart(7)).join(' ')}  ${c.categories.join(', ')}`);
    }
    console.log(`${'all'.padEnd(40)} ${[m.audited.total, m.audited.parsed, m.audited.resolved].map(n => String(n).padStart(7)).join(' ')}`);
    for (const [cause, c] of Object.entries(m.causes).sort((a, b) => b[1].recipes - a[1].recipes)) {
      console.log(`\n  ${cause}: ${c.recipes} recipes; by type ${c.by_type.slice(0, 8).map(([t, n]) => `${t} ${n}`).join(', ')}`);
      for (const [detail, n] of c.top_details.slice(0, 6)) console.log(`    ${String(n).padStart(6)}  ${detail}`);
    }
  }
  const worst = Object.entries(report.by_namespace).map(([ns, c]) => [ns, c.total - c.matched, c.total] as const).filter(r => r[1] > 0).sort((a, b) => b[1] - a[1]);
  console.log(`\nnamespaces with failures: ${worst.length}; most: ${worst.slice(0, 12).map(([ns, f, t]) => `${ns} ${f}/${t}`).join(', ')}`);
}

const at = process.argv.indexOf('--summary');
if (at >= 0) {
  summary(JSON.parse(readFileSync(process.argv[at + 1], 'utf8')));
  process.exit(0);
}

const { WorldFixture } = await import('../../ae2/tests/world/harness');
const { addonContents } = await import('../../ae2/scripts/common');
const carries = resolve(process.env.ATM10_CARRIES ?? '');
if (!process.env.ATM10_CARRIES || !existsSync(join(carries, 'atm10-pack', 'horizon-mod.toml'))) {
  throw new Error('ATM10_CARRIES must name the carries folder final-data wrote (with atm10-pack)');
}
const list = (name: string) => (process.env[name] ?? '').split(',').map(s => s.trim()).filter(Boolean);
const extra = list('RECIPE_AUDIT_MODS').map(p => resolve(p));
const only = list('RECIPE_AUDIT_ONLY'), without = new Set(list('RECIPE_AUDIT_WITHOUT'));
const mods = readdirSync(carries, { withFileTypes: true })
  .filter(d => d.isDirectory() && existsSync(join(carries, d.name, 'horizon-mod.toml')))
  .map(d => d.name)
  .sort()
  .filter(id => (only.length ? only.includes(id) : true) && !without.has(id));
// A first start of the whole pack beside other work took over 10 minutes.
process.env.AE2_TEST_STARTUP_MS = String(Math.max(Number(process.env.AE2_TEST_STARTUP_MS ?? 0), 1_800_000));
console.log(`${mods.length} carries from ${carries}`);

// AE2's addons (MEGA, ExtendedAE, AppFlux...) are part of the pack too: the kubejs recipes name their items.
const addons = process.env.RECIPE_AUDIT_ADDONS === '0' ? [] : addonContents().filter(a => existsSync(a.path)).map(a => a.id);
console.log(`AE2 addons: ${addons.join(', ') || 'none'}`);
const fixture = new WorldFixture('recipe-audit', 'server-thread', { addons, devMods: [...mods.map(id => join(carries, id)), ...extra] });
const out = resolve(process.env.RECIPE_AUDIT_OUT ?? join(fixture.run, 'recipe-audit.json'));
try {
  const started = Date.now();
  await fixture.start();
  console.log(`in the world after ${((Date.now() - started) / 1000).toFixed(1)} s`);
  const reply = await fixture.raw(`recipe-audit ${out.replaceAll('\\', '/')} ${process.env.RECIPE_AUDIT_THREADS ?? 4}`, 30 * 60_000);
  if (reply?.error) throw new Error(`recipe-audit: ${reply.error}`);
  console.log(`audit ran in ${Number(reply.seconds ?? 0).toFixed(1)} s; report ${out}\n`);
  const report: Report = JSON.parse(readFileSync(out, 'utf8'));
  summary(report);
  if (report.audited.total === 0) throw new Error('no recipe of the audited types in the world');
  fixture.mark(`${report.audited.matched} of ${report.audited.total} vanilla-type recipes run and give their result`);
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
