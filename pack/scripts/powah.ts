// Builds and tests the pack's Powah port in a checkout of this fork: the carry of the player's own jar
// (horizon-port-pack), the content mod (powah-content), the crates' unit tests, the game staged with the native
// bundles under pack/mods (horizon-mixins apply), its build and the world tests.
// Usage: bun run pack/scripts/powah.ts <step>... (steps: carry, content, units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   POWAH_WORK         horizon-port-pack's work folder  <base>/ports/atm10
//   POWAH_CARRY        the carry of Powah               <work>/mods/powah/carry
//   POWAH_CONTENT_DIR  the content mod it writes        <base>/.build/content/powah
//   POWAH_STAGE        the staged, modded source        <base>/stage-powah
//   POWAH_TARGET       the staged game's target dir     <base>/.build/target-powah-game
//   POWAH_TOOL_TARGET  target dir of the pack's crates  <base>/.build/target-powah
//   HORIZON_PORT_PACK  the horizon-port-pack binary     <base>/.build/target-portpack/release/horizon-port-pack
//   HORIZON_MIXINS     the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   AE2_RESOURCES      the prepared 26.3 resources      <base>/horizon/.local/resources/26.3
//   POWAH_JOBS         cargo jobs                       2
// `carry` needs the work folder imported, with Powah's jar fetched (`horizon-port-pack fetch <work> --from <mods> powah`)
// and, for real hardness and sounds, the NeoForge oracle's facts in `<work>/oracle/facts.json` (NOTES.md says how).
// `world` also passes ATM10_DIR (ATM10's settings) and AE2_CONTENT_DIR through to the harness. Nothing a step
// generates is committed: Powah's files stay local.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/powah');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const work = env('POWAH_WORK', join(base, 'ports/atm10'));
const carry = env('POWAH_CARRY', join(work, 'mods/powah/carry'));
const content = env('POWAH_CONTENT_DIR', join(base, '.build/content/powah'));
const stage = env('POWAH_STAGE', join(base, 'stage-powah'));
const target = env('POWAH_TARGET', join(base, '.build/target-powah-game'));
const toolTarget = env('POWAH_TOOL_TARGET', join(base, '.build/target-powah'));
const portPack = env('HORIZON_PORT_PACK', join(base, `.build/target-portpack/release/horizon-port-pack${exe}`));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.POWAH_JOBS ?? '2';
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
  rmSync(join(bundle, 'powah-core/Cargo.lock'), { force: true });
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
    await run([portPack, 'carry', work, '--resources', resources, 'powah']);
  },
  async content() {
    await run(['cargo', 'run', '--offline', '-q', '-j', jobs, '--manifest-path', join(bundle, 'powah-content/Cargo.toml'), '--target-dir', toolTarget,
      '--', '--carry', carry, '--out', content]);
  },
  async units() {
    try {
      for (const crate of ['powah-core', 'powah-content']) {
        await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, crate, 'Cargo.toml'), '--target-dir', toolTarget]);
      }
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
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, POWAH_CONTENT_DIR: content,
      AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // POWAH_TESTS picks some (`furnator-screen`); all of pack/mods/powah/tests by default.
    const names = process.env.POWAH_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/powah.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
