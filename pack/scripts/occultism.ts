// Builds and tests the pack's Occultism port in a checkout of this fork: the bundle crate's unit tests, the game staged
// with this bundle only (horizon-mixins apply), its build and the world tests.
// Usage: bun run pack/scripts/occultism.ts <step>... (steps: units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   OCCULTISM_CARRY        the carry of Occultism's jar     <base>/ports/atm10/mods/occultism/carry
//   OCCULTISM_PACK_CARRY   the pack mod `final-data` wrote (`atm10-pack`), for the `c:` tags the recipes name
//   OCCULTISM_STAGE        the staged, modded source        <base>/stage-occultism
//   OCCULTISM_TARGET       the staged game's target dir     <base>/.build/target-occultism-game
//   OCCULTISM_TOOL_TARGET  target dir of the bundle's crate <base>/.build/target-occultism
//   HORIZON_MIXINS         the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   OCCULTISM_FEATURES    the game's Cargo features (`-p horizon-game --no-default-features`): ae2-lab, the world
//                          tests load AE2's content
//   OCCULTISM_JOBS         cargo jobs                       2
// `units` also reads the carry's rituals, spirit fire recipes and pentacles when the carry is there. `world` passes
// the carries, AE2_RESOURCES and AE2_CONTENT_DIR to the harness. No file of Occultism is in the bundle: the carry,
// generated from the player's own jar, brings its blocks, items, recipes, pentacles and assets.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/occultism');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const carry = env('OCCULTISM_CARRY', join(base, 'ports/atm10/mods/occultism/carry'));
const stage = env('OCCULTISM_STAGE', join(base, 'stage-occultism'));
const target = env('OCCULTISM_TARGET', join(base, '.build/target-occultism-game'));
const toolTarget = env('OCCULTISM_TOOL_TARGET', join(base, '.build/target-occultism'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.OCCULTISM_JOBS ?? '2';
const cargoEnv = { ...process.env, CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_FAST_DEBUG: '0' };

async function run(command: string[], cwd = root, extra: Record<string, string | undefined> = cargoEnv): Promise<void> {
  console.log('>', command.join(' '));
  const child = Bun.spawn(command, { cwd, env: extra, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  const code = await child.exited;
  if (code !== 0) throw new Error(`${command[0]} exited with status ${code}`);
}

/** A bundle crate may not hold a Cargo.lock (horizon-mixins refuses it); `units` makes one. */
function dropCrateLock(): void {
  rmSync(join(bundle, 'occultism-core/Cargo.lock'), { force: true });
}

function files(dir: string, under = dir, out: string[] = []): string[] {
  for (const entry of readdirSync(under, { withFileTypes: true })) {
    const path = join(under, entry.name);
    if (entry.isDirectory()) files(dir, path, out);
    else if (entry.isFile()) out.push(relative(dir, path));
  }
  return out;
}

/** Makes `to` hold what `from` holds, writing only the files that differ, so Cargo rebuilds only what changed. */
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
  async units() {
    try {
      const extra = existsSync(carry) ? { ...cargoEnv, OCCULTISM_CARRY: carry } : cargoEnv;
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'occultism-core/Cargo.toml'), '--target-dir', toolTarget], root, extra);
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    // This bundle only: a folder of it beside the stage, so the build is the game and Occultism.
    const mods = `${stage}-mods`;
    rmSync(mods, { recursive: true, force: true });
    cpSync(bundle, join(mods, 'occultism'), { recursive: true, filter: path => !path.includes(`${join('tests', '')}`) });
    addLocalBundles(mods);
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    await applyBundles(command => run(command), mixins, root, mods, next, toolTarget);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.OCCULTISM_FEATURES ?? 'ae2-lab'), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, OCCULTISM_CARRY: carry,
      AE2_RESOURCES: process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3'),
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // OCCULTISM_TESTS picks some (`ritual`); all of the bundle's tests/ by default.
    const names = process.env.OCCULTISM_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/occultism.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
