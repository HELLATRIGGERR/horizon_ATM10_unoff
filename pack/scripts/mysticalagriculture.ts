// Builds and tests the pack's Mystical Agriculture port in a checkout of this fork: the carry of the player's own jar
// (horizon-port-pack, with the NeoForge oracle's facts so seeds plant their crops), the core crate's unit tests, the
// game staged with this bundle (horizon-mixins apply through pack/scripts/hook-catalog.ts), its build and the world tests.
// Usage: bun run pack/scripts/mysticalagriculture.ts <step>... (steps: carry, units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   MA_WORK            horizon-port-pack's work folder  <base>/ports/atm10
//   MA_CARRY           the carry of MA (a content mod)  <work>/mods/mysticalagriculture/carry
//   MA_STAGE           the staged, modded source        <base>/stage-ma
//   MA_TARGET          the staged game's target dir     <base>/.build/target-ma-game
//   MA_TOOL_TARGET     target dir of the bundle's crate <base>/.build/target-ma
//   HORIZON_PORT_PACK  the horizon-port-pack binary     <base>/.build/target-portpack/release/horizon-port-pack
//   HORIZON_MIXINS     the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   AE2_RESOURCES      the prepared 26.3 resources      <base>/horizon/.local/resources/26.3
//   MA_JOBS            cargo jobs                       2
//   MA_FEATURES        the game's Cargo features        ae2-lab (`-p horizon-game --no-default-features`)
// `carry` needs the work folder imported, MA's jar fetched (`horizon-port-pack fetch <work> --from <mods>
// mysticalagriculture`) and the oracle's facts of MA's batch in `<work>/oracle/facts.json` (the pack run's
// `facts-<batch>.json`): without them seeds carry no `place_block` and plant nothing. `world` also passes ATM10_DIR,
// AE2_CONTENT_DIR and MA_CONFIG (a folder holding ATM10's config files the bundle reads) to the harness. `stage` applies
// this bundle alone (with HORIZON_LOCAL_BUNDLES' beside it), so the build is the game and Mystical Agriculture. Nothing a
// step generates is committed: MA's files stay local.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/mysticalagriculture');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const work = env('MA_WORK', join(base, 'ports/atm10'));
const carry = env('MA_CARRY', join(work, 'mods/mysticalagriculture/carry'));
const stage = env('MA_STAGE', join(base, 'stage-ma'));
const target = env('MA_TARGET', join(base, '.build/target-ma-game'));
const toolTarget = env('MA_TOOL_TARGET', join(base, '.build/target-ma'));
const portPack = env('HORIZON_PORT_PACK', join(base, `.build/target-portpack/release/horizon-port-pack${exe}`));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.MA_JOBS ?? '2';
// A worktree has neither the prepared resources nor AE2's content: the main checkout's (`<base>/horizon`).
const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
const resources = process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3');
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
  rmSync(join(bundle, 'ma-core/Cargo.lock'), { force: true });
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

const steps: Record<string, () => Promise<void>> = {
  async carry() {
    await run([portPack, 'carry', work, '--resources', resources, 'mysticalagriculture']);
  },
  async units() {
    try {
      for (const crate of ['ma-core']) {
        await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, crate, 'Cargo.toml'), '--target-dir', toolTarget]);
      }
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    // Only this bundle (and HORIZON_LOCAL_BUNDLES'): a folder of them beside the stage, without the world tests.
    const mods = `${stage}-mods`;
    rmSync(mods, { recursive: true, force: true });
    cpSync(bundle, join(mods, 'mysticalagriculture'), { recursive: true, filter: path => !relative(bundle, path).split(/[\\/]/).some(part => part === 'tests' || part === 'target') });
    addLocalBundles(mods);
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    await applyBundles(command => run(command), mixins, root, mods, next, toolTarget);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.MA_FEATURES ?? 'ae2-lab'), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, MA_CARRY: carry,
      AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // MA_TESTS picks some (`crops-infusion`); all of pack/mods/mysticalagriculture/tests by default.
    const names = process.env.MA_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/mysticalagriculture.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
