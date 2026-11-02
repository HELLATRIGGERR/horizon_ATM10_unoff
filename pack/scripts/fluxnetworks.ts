// Builds and tests the pack's Flux Networks port in a checkout of this fork: the content mod from the player's jar
// (horizon-port-pack's carry plus the bundle's declarations), the game staged with the native bundles of pack/mods
// (horizon-mixins apply), its build, the crate's unit tests and the world tests.
// Usage: bun run pack/scripts/fluxnetworks.ts <step>... (steps: content, units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   FLUX_JAR          Flux Networks' jar               the ATM10 instance's mods/FluxNetworks-1.21.1-8.0.0.jar
//   FLUX_CONTENT_DIR  the content mod it writes        <base>/.build/content/fluxnetworks
//   FLUX_STAGE        the staged, modded source        <base>/stage-flux
//   FLUX_TARGET       the staged game's target dir     <base>/.build/target-flux-game
//   FLUX_TOOL_TARGET  target dir of the bundle's crate <base>/.build/target-flux
//   PORT_PACK         the horizon-port-pack binary     <base>/.build/target-portpack/release/horizon-port-pack
//   HORIZON_MIXINS    the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   FLUX_JOBS         cargo jobs                       2
// `world` also passes ATM10_DIR (the instance, whose settings it tests reading), AE2_RESOURCES and AE2_CONTENT_DIR to
// the harness.
//
// The content mod is generated and never committed: Flux Networks' assets stay on the player's machine (the GUI art
// is CC BY-NC-SA 4.0, see pack/mods/fluxnetworks/README.md).
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/fluxnetworks');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const instance = process.env.ATM10_DIR ?? 'C:/Users/mpoka/curseforge/minecraft/Instances/All the Mods 10 - ATM10';
const jar = env('FLUX_JAR', join(instance, 'mods/FluxNetworks-1.21.1-8.0.0.jar'));
const content = env('FLUX_CONTENT_DIR', join(base, '.build/content/fluxnetworks'));
const stage = env('FLUX_STAGE', join(base, 'stage-flux'));
const target = env('FLUX_TARGET', join(base, '.build/target-flux-game'));
const toolTarget = env('FLUX_TOOL_TARGET', join(base, '.build/target-flux'));
const portPack = env('PORT_PACK', join(base, `.build/target-portpack/release/horizon-port-pack${exe}`));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.FLUX_JOBS ?? '2';
// A folder this script made; only such a one is replaced.
const MARKER = 'fluxnetworks-content.json';
// Small, low-memory builds: no incremental caches, no debug info.
const cargoEnv = { ...process.env, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_FAST_DEBUG: '0', CARGO_PROFILE_FAST_INCREMENTAL: 'false' };

async function run(command: string[], cwd = root, extra: Record<string, string | undefined> = cargoEnv): Promise<void> {
  console.log('>', command.join(' '));
  const child = Bun.spawn(command, { cwd, env: extra, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  const code = await child.exited;
  if (code !== 0) throw new Error(`${command[0]} exited with status ${code}`);
}

/** A bundle crate may not hold a Cargo.lock (horizon-mixins refuses it); `units` makes one. */
function dropCrateLock(): void {
  rmSync(join(bundle, 'flux-core/Cargo.lock'), { force: true });
}

/** Every file under `dir`, relative to it. */
function files(dir: string, under = dir, out: string[] = []): string[] {
  for (const entry of readdirSync(under, { withFileTypes: true })) {
    const path = join(under, entry.name);
    if (entry.isDirectory()) files(dir, path, out);
    else if (entry.isFile()) out.push(relative(dir, path));
  }
  return out;
}

/** Makes `to` hold what `from` holds, writing only the files that differ, so Cargo (which goes by file times)
 * rebuilds only the crates the bundle touched, as the launcher does with its `native/stage/`. */
function syncStage(from: string, to: string): number {
  const fresh = new Set(files(from));
  let written = 0;
  for (const file of fresh) {
    const source = join(from, file), dest = join(to, file);
    if (existsSync(dest) && readFileSync(dest).equals(readFileSync(source))) continue;
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(source, dest);
    written++;
  }
  if (existsSync(to)) for (const file of files(to)) if (!fresh.has(file)) { rmSync(join(to, file)); written++; }
  return written;
}

const readJson = (path: string) => JSON.parse(readFileSync(path, 'utf8'));
const writeJson = (path: string, value: unknown) => {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(value, null, 2) + '\n');
};

/** A loot entry without `copy_components` of Flux's own item components, which the game's loot tables refuse: the
 * bundle's `native_block_entity_drop` puts them on the dropped item instead. */
function plainDrop(table: any): any {
  for (const pool of table.pools ?? []) for (const entry of pool.entries ?? []) delete entry.modifier;
  return table;
}

const steps: Record<string, () => Promise<void>> = {
  /** F0: the carry of the jar, the bundle's block declarations over it, and what the carry can't do by itself. */
  async content() {
    if (!existsSync(jar)) throw new Error(`No Flux Networks jar at ${jar}; set FLUX_JAR.`);
    if (existsSync(content)) {
      if (!existsSync(join(content, MARKER))) throw new Error(`${content} wasn't made by this script; not replacing it.`);
      rmSync(content, { recursive: true, force: true });
    }
    const work = mkdtempSync(join(tmpdir(), 'flux-carry-'));
    try {
      await run([portPack, 'check', jar, work]);
      const report = readJson(join(work, 'carry-report.json'));
      mkdirSync(dirname(content), { recursive: true });
      cpSync(join(work, 'carry'), content, { recursive: true });
      cpSync(join(bundle, 'content'), content, { recursive: true });
      // The plug's and point's recipes wait for their blocks in `outside/`; the overlay declares those.
      const outside = join(work, 'outside/data/fluxnetworks/recipe');
      const moved = existsSync(outside) ? readdirSync(outside) : [];
      for (const name of moved) copyFileSync(join(outside, name), join(content, 'data/fluxnetworks/recipe', name));
      const devices = ['flux_plug', 'flux_point', 'flux_controller', 'basic_flux_storage', 'herculean_flux_storage', 'gargantuan_flux_storage'];
      for (const device of devices) {
        const path = join(content, `data/fluxnetworks/loot_table/blocks/${device}.json`);
        writeJson(path, plainDrop(readJson(path)));
        // A storage's item model is `builtin/entity`, drawn by the mod's renderer: its block's model, until F6.
        const item = join(content, `assets/fluxnetworks/models/item/${device}.json`);
        const model = readJson(item);
        if (model.parent === 'builtin/entity') writeJson(item, { ...model, parent: `fluxnetworks:block/${device}` });
        // A plug's or a point's item model is NeoForge's composite loader, the block with all six arms, which the
        // game doesn't read: the block's own model.
        if (String(model.parent ?? '').endsWith('_inventory')) writeJson(item, { ...model, parent: `fluxnetworks:block/${device}` });
      }
      writeJson(join(content, MARKER), { from: relative(base, jar), copied: report.copied, recipes_moved_in: moved, recipes_kept: report.recipes_kept });
      console.log(`content: ${files(content).length} files in ${content}; ${moved.length} recipes moved in; ${report.recipes_kept?.length ?? 0} custom recipes kept, not craftable until F2`);
    } finally { rmSync(work, { recursive: true, force: true }); }
  },
  async units() {
    try {
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'flux-core/Cargo.toml'), '--target-dir', toolTarget]);
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    await run([mixins, 'apply', '--workspace', root, '--mods', join(root, 'pack/mods'), '--out', next]);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', 'fast', '-p', 'horizon-client', '--no-default-features', '--features', 'ae2-lab', '--bins', '-j', jobs, '--target-dir', target], stage);
  },
  async world() {
    const binary = join(target, `fast/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    if (!existsSync(content)) throw new Error(`No Flux content at ${content}; run the content step.`);
    // A worktree has neither the prepared resources nor AE2's content: the main checkout's (`<base>/horizon`).
    const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, FLUX_CONTENT_DIR: content, ATM10_DIR: instance,
      AE2_RESOURCES: process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3'),
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // FLUX_TESTS picks some; all of pack/mods/fluxnetworks/tests by default.
    const names = process.env.FLUX_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/fluxnetworks.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
