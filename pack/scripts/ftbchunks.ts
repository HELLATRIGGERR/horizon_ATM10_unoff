// Builds and tests the pack's FTB Teams / FTB Chunks / JourneyMap port in a checkout of this fork: the bundles'
// crates' unit tests, the game staged with those bundles only (horizon-mixins apply), its build and the world tests.
// Usage: bun run pack/scripts/ftbchunks.ts <step>... (steps: units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   FTB_STAGE        the staged, modded source        <base>/stage-ftb
//   FTB_TARGET       the staged game's target dir     <base>/.build/target-ftb-game
//   FTB_TOOL_TARGET  target dir of the bundles' crates <base>/.build/target-ftb
//   HORIZON_MIXINS   the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   FTB_JOBS         cargo jobs                       2
// `world` passes AE2_RESOURCES and AE2_CONTENT_DIR to the harness, and FTBTEAMS_DEV=1 (Java's development
// environment: the fake player command and the probes).
//
// No file of the FTB mods or JourneyMap is in the bundles: they are All Rights Reserved (see their READMEs).
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
/** The bundles of this port, in the order they depend on each other. */
const BUNDLES = ['ftbteams', 'ftbchunks', 'journeymap'].filter(name => existsSync(join(root, 'pack/mods', name, 'horizon-mixin.toml')));
const CRATES: Record<string, string> = { ftbteams: 'ftbteams-core', ftbchunks: 'ftbchunks-core', journeymap: 'journeymap-core' };
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const stage = env('FTB_STAGE', join(base, 'stage-ftb'));
const target = env('FTB_TARGET', join(base, '.build/target-ftb-game'));
const toolTarget = env('FTB_TOOL_TARGET', join(base, '.build/target-ftb'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.FTB_JOBS ?? '2';
const cargoEnv = { ...process.env, CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_FAST_DEBUG: '0' };

async function run(command: string[], cwd = root, extra: Record<string, string | undefined> = cargoEnv): Promise<void> {
  console.log('>', command.join(' '));
  const child = Bun.spawn(command, { cwd, env: extra, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  const code = await child.exited;
  if (code !== 0) throw new Error(`${command[0]} exited with status ${code}`);
}

/** A bundle crate may not hold a Cargo.lock (horizon-mixins refuses it); `units` makes one. */
function dropCrateLocks(): void {
  for (const name of BUNDLES) rmSync(join(root, 'pack/mods', name, CRATES[name], 'Cargo.lock'), { force: true });
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
      for (const name of BUNDLES) {
        await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(root, 'pack/mods', name, CRATES[name], 'Cargo.toml'), '--target-dir', toolTarget]);
      }
    } finally { dropCrateLocks(); }
  },
  async stage() {
    dropCrateLocks();
    // Only this port's bundles: a folder of them beside the stage, so the build is the game and these.
    const mods = `${stage}-mods`;
    rmSync(mods, { recursive: true, force: true });
    for (const name of BUNDLES) cpSync(join(root, 'pack/mods', name), join(mods, name), { recursive: true, filter: path => !path.includes(`${join('tests', '')}`) });
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    await run([mixins, 'apply', '--workspace', root, '--mods', mods, '--out', next]);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, '-p', 'horizon-atm10', '--no-default-features', '--features', 'ae2-lab', '--bins', '-j', jobs, '--target-dir', target], stage);
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, FTBTEAMS_DEV: '1',
      AE2_RESOURCES: process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3'),
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // FTB_TESTS picks some (`ftbteams/teams`); every bundle's tests/ by default.
    const names = process.env.FTB_TESTS?.split(',').filter(Boolean)
      ?? BUNDLES.flatMap(name => existsSync(join(root, 'pack/mods', name, 'tests'))
        ? readdirSync(join(root, 'pack/mods', name, 'tests')).filter(f => f.endsWith('.ts')).map(f => `${name}/${f.slice(0, -3)}`).sort() : []);
    for (const name of names) {
      const [bundle, test] = name.split('/');
      await run(['bun', 'run', join(root, 'pack/mods', bundle, 'tests', `${test}.ts`)], root, harness);
    }
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/ftbchunks.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
