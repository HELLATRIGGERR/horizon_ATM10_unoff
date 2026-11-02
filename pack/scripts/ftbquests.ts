// Builds and tests the pack's quest book in a checkout of this fork: the local content mod from the player's jars
// (ftbquests-content), the game staged with the native bundle pack/mods/ftbquests (horizon-mixins apply), its build,
// the crates' unit tests and the world tests.
// Usage: bun run pack/scripts/ftbquests.ts <step>... (steps: content, units, stage, build, world; all: every one)
//
// Paths default to this machine's layout (`<base>` = the checkout's parent folder) and can be set in the environment:
//   ATM10_DIR      the ATM10 instance, read only         CurseForge's folder for it
//   FTBQ_CARRY     the ATM10 carry of FTB Quests         <base>/ports/atm10/mods/ftbquests/carry
//   FTBQ_CONTENT   the content mod it writes             C:/horizon-work/content-quests
//   FTBQ_STAGE     the staged, modded source             C:/horizon-work/stage-quests
//   FTBQ_TARGET    the staged game's target dir          C:/horizon-work/target-quests-game
//   FTBQ_TOOLS     target dir of the bundle's crates     C:/horizon-work/target-quests-tools
//   HORIZON_MIXINS the horizon-mixins binary             <base>/.build/target-mixins/debug/horizon-mixins
//   FTBQ_JOBS      cargo jobs                            3
// `world` passes ATM10_DIR, AE2_RESOURCES and AE2_CONTENT_DIR through to the harness.
// Building native code is the player's own decision; the launcher's `native-build` is the player's route.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, utimesSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/ftbquests');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const atm10 = env('ATM10_DIR', 'C:/Users/mpoka/curseforge/minecraft/Instances/All the Mods 10 - ATM10');
const carry = env('FTBQ_CARRY', join(base, 'ports/atm10/mods/ftbquests/carry'));
const content = env('FTBQ_CONTENT', 'C:/horizon-work/content-quests');
const stage = env('FTBQ_STAGE', 'C:/horizon-work/stage-quests');
const target = env('FTBQ_TARGET', 'C:/horizon-work/target-quests-game');
const toolTarget = env('FTBQ_TOOLS', 'C:/horizon-work/target-quests-tools');
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.FTBQ_JOBS ?? '3';
// Small, low-memory builds: no incremental caches, no debug info.
const cargoEnv = { ...process.env, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_FAST_DEBUG: '0', CARGO_PROFILE_FAST_INCREMENTAL: 'false' };

async function run(command: string[], cwd = root, extra: Record<string, string | undefined> = cargoEnv): Promise<void> {
  console.log('>', command.join(' '));
  const child = Bun.spawn(command, { cwd, env: extra, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  const code = await child.exited;
  if (code !== 0) throw new Error(`${command[0]} exited with status ${code}`);
}

/** A bundle crate may not hold a Cargo.lock (horizon-mixins refuses it); `units` makes one. */
function dropCrateLocks(): void {
  rmSync(join(bundle, 'ftbquests-core/Cargo.lock'), { force: true });
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

/** Makes `to` hold what `from` holds, writing only the files that differ, so Cargo rebuilds only the crates touched. */
function syncStage(from: string, to: string): number {
  const fresh = new Set(files(from));
  let written = 0;
  for (const file of fresh) {
    const source = join(from, file), dest = join(to, file);
    if (existsSync(dest) && readFileSync(dest).equals(readFileSync(source))) continue;
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(source, dest);
    // Windows keeps the source's time on a copy, and Cargo goes by file times.
    const now = new Date();
    utimesSync(dest, now, now);
    written++;
  }
  if (existsSync(to)) for (const file of files(to)) if (!fresh.has(file)) { rmSync(join(to, file)); written++; }
  return written;
}

const steps: Record<string, () => Promise<void>> = {
  async content() {
    await run(['cargo', 'run', '--offline', '-q', '-j', jobs, '--manifest-path', join(bundle, 'ftbquests-content/Cargo.toml'), '--target-dir', toolTarget,
      '--', '--mods', join(atm10, 'mods'), '--out', content]);
  },
  async units() {
    try {
      for (const crate of ['ftbquests-core', 'ftbquests-content']) {
        await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, crate, 'Cargo.toml'), '--target-dir', toolTarget], root, { ...cargoEnv, ATM10_DIR: atm10 });
      }
    } finally { dropCrateLocks(); }
  },
  async stage() {
    dropCrateLocks();
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    // Only this bundle: the other native mods build on their own branches.
    const only = join(base, '.build/quests-mods');
    rmSync(only, { recursive: true, force: true });
    mkdirSync(only, { recursive: true });
    cpSync(bundle, join(only, 'ftbquests'), { recursive: true });
    try {
      await run([mixins, 'apply', '--workspace', root, '--mods', only, '--out', next]);
    } finally {
      rmSync(only, { recursive: true, force: true });
    }
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', 'fast', '-p', 'horizon-client', '--no-default-features', '--features', 'ae2-lab', '--bins', '-j', jobs, '--target-dir', target], stage);
  },
  async world() {
    const binary = join(target, `fast/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    // A worktree has neither the prepared resources nor AE2's content: the main checkout's (`<base>/horizon`).
    const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, FTBQ_CONTENT: content, FTBQ_CARRY: carry, ATM10_DIR: atm10,
      AE2_RESOURCES: process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3'),
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // FTBQ_TESTS picks some; all of pack/mods/ftbquests/tests by default.
    const names = process.env.FTBQ_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/ftbquests.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
