// Builds and tests the pack's woodworking station port in a checkout of this fork: the crate's
// unit tests, the game staged with the native bundles under pack/mods (horizon-mixins apply),
// its build and the world tests.
// Usage: bun run pack/scripts/woodworking.ts <step>... (steps: units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent):
//   WOODWORKING_STAGE   the staged, modded source      <base>/stage-woodworking
//   WOODWORKING_TARGET  the staged game's target dir   <base>/.build/target-woodworking-game
//   WOODWORKING_TOOL    target dir of the pack's crate <base>/.build/target-woodworking
//   WOODWORKING_CARRY   the station block's carry      <base>/../atm10-work/fullpack/carries/<ns>
//   HORIZON_MIXINS      the horizon-mixins binary      <base>/.build/target-mixins/debug/horizon-mixins
//   WOODWORKING_JOBS    cargo jobs                     2
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/woodworking');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const stage = env('WOODWORKING_STAGE', join(base, 'stage-woodworking'));
const target = env('WOODWORKING_TARGET', join(base, '.build/target-woodworking-game'));
const toolTarget = env('WOODWORKING_TOOL', join(base, '.build/target-woodworking'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.WOODWORKING_JOBS ?? '2';
const cargoEnv = { ...process.env, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_FAST_DEBUG: '0', CARGO_PROFILE_FAST_INCREMENTAL: 'false' };
const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
const resources = process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3');

function stationBlockId(): string {
  const toml = readFileSync(join(root, 'pack/pack.toml'), 'utf-8');
  const m = toml.match(/\[mods\.woodworking\][^\[]*station_block\s*=\s*"([^"]+)"/s);
  if (!m) throw new Error('station_block not found in pack/pack.toml [mods.woodworking]');
  return m[1];
}

async function run(command: string[], cwd = root, extra: Record<string, string | undefined> = cargoEnv): Promise<void> {
  console.log('>', command.join(' '));
  const child = Bun.spawn(command, { cwd, env: extra, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  const code = await child.exited;
  if (code !== 0) throw new Error(`${command[0]} exited with status ${code}`);
}

function files(dir: string, under = dir, out: string[] = []): string[] {
  for (const entry of readdirSync(under, { withFileTypes: true })) {
    const path = join(under, entry.name);
    if (entry.isDirectory()) files(dir, path, out);
    else if (entry.isFile()) out.push(relative(dir, path));
  }
  return out;
}

function syncStage(from: string, to: string): number {
  const fresh = new Set(files(from));
  let written = 0;
  for (const file of fresh) {
    const src = join(from, file), dest = join(to, file);
    if (existsSync(dest) && readFileSync(dest).equals(readFileSync(src))) continue;
    mkdirSync(dirname(dest), { recursive: true });
    writeFileSync(dest, readFileSync(src));
    written++;
  }
  if (existsSync(to)) for (const file of files(to)) if (!fresh.has(file)) { rmSync(join(to, file)); written++; }
  return written;
}

function dropCrateLock(): void {
  rmSync(join(bundle, 'woodworking-core/Cargo.lock'), { force: true });
}

const steps: Record<string, () => Promise<void>> = {
  async units() {
    try {
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'woodworking-core/Cargo.toml'), '--target-dir', toolTarget]);
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    const mods = `${stage}-mods`;
    rmSync(mods, { recursive: true, force: true });
    cpSync(bundle, join(mods, basename(bundle)), { recursive: true, filter: (path: string) => !relative(bundle, path).split(/[\\/]/).some(part => part === 'tests' || part === 'target' || part === 'Cargo.lock') });
    addLocalBundles(mods);
    await applyBundles(command => run(command), mixins, root, mods, next, toolTarget);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.WOODWORKING_FEATURES), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    const stationId = stationBlockId();
    const carryNs = stationId.split(':')[0];
    const carryDefault = join(base, '../atm10-work/fullpack/carries', carryNs);
    const carry = resolve(process.env.WOODWORKING_CARRY ?? carryDefault);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
      WOODWORKING_STATION: stationId,
      WOODWORKING_CARRY: carry,
    };
    const names = process.env.WOODWORKING_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/woodworking.ts <step>...\nSteps: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
