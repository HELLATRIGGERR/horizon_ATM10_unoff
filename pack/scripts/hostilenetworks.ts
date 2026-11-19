// Builds and tests the pack's Hostile Neural Networks port in a checkout of this fork: the crate's unit tests, the game staged
// with the native bundles under pack/mods (horizon-mixins apply), its build and the world tests. The blocks, items,
// models and recipes are the carry of the player's own jar (horizon-port-pack carry, ATM10's `tools/atm10/`): this
// script never makes one.
// Usage: bun run pack/scripts/hostilenetworks.ts <step>... (steps: units, content, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   HOSTILENETWORKS_CARRY       the carry of Hostile Neural Networks       <base>/atm10-work/fullpack/carries/hostilenetworks (the carry that final-data laid ATM10's 17 KubeJS data models over)
//   HOSTILENETWORKS_CONTENT_DIR the content mod `content` makes  <base>/.build/content/hostilenetworks
//   HOSTILENETWORKS_STAGE       the staged, modded source        <base>/stage-hostilenetworks
//   HOSTILENETWORKS_TARGET      the staged game's target dir     <base>/.build/target-hostilenetworks-game
//   HOSTILENETWORKS_TOOL_TARGET target dir of the pack's crate   <base>/.build/target-hostilenetworks
//   HORIZON_MIXINS           the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   AE2_RESOURCES            the prepared 26.3 resources      <base>/horizon/.local/resources/26.3
//   HOSTILENETWORKS_JOBS        cargo jobs                       2
// `world` also passes ATM10_DIR (ATM10's settings) and AE2_CONTENT_DIR through to the harness. Nothing a step
// generates is committed: Hostile Neural Networks' files stay local.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { withLocalBundles } from './local-bundles';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/hostilenetworks');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const carry = env('HOSTILENETWORKS_CARRY', join(base, 'atm10-work/fullpack/carries/hostilenetworks'));
const content = env('HOSTILENETWORKS_CONTENT_DIR', join(base, '.build/content/hostilenetworks'));
const stage = env('HOSTILENETWORKS_STAGE', join(base, 'stage-hostilenetworks'));
const target = env('HOSTILENETWORKS_TARGET', join(base, '.build/target-hostilenetworks-game'));
const toolTarget = env('HOSTILENETWORKS_TOOL_TARGET', join(base, '.build/target-hostilenetworks'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.HOSTILENETWORKS_JOBS ?? '2';
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
  rmSync(join(bundle, 'hostilenetworks-core/Cargo.lock'), { force: true });
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
 * rebuilds only the crates the bundle touched. */
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

/** The carry's Simulation Chamber and Loot Fabricator sit on the carved pumpkin template, which has no right-click: the content mod is the carry
 * with those two on the furnace template (the block entity, the menu and `facing`, as Powah's furnators do), minus
 * the properties that template already has. A local copy of the player's own jar's content; never committed. */
function makeContent(): void {
  rmSync(content, { recursive: true, force: true });
  cpSync(carry, content, { recursive: true });
  const blocks = join(content, 'data/hostilenetworks/horizon/block');
  for (const file of readdirSync(blocks).filter(f => ['sim_chamber.json', 'loot_fabricator.json'].includes(f))) {
    const path = join(blocks, file);
    const decl = JSON.parse(readFileSync(path, 'utf8'));
    decl.template = 'minecraft:furnace';
    for (const field of ['state_properties', 'state_defaults']) {
      for (const owned of ['lit', 'facing']) delete decl[field]?.[owned];
    }
    for (const field of ['default_state', 'placement']) delete decl[field];
    decl.properties = { ...decl.properties, light: 1 };
    writeFileSync(path, JSON.stringify(decl, null, 2));
  }
  console.log(`${content}: ${readdirSync(blocks).length} block declarations`);
}

const steps: Record<string, () => Promise<void>> = {
  async content() {
    if (!existsSync(carry)) throw new Error(`No carry of Hostile Neural Networks at ${carry} (HOSTILENETWORKS_CARRY).`);
    makeContent();
  },
  async units() {
    try {
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'hostilenetworks-core/Cargo.toml'), '--target-dir', toolTarget]);
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    await run([mixins, 'apply', '--workspace', root, '--mods', withLocalBundles(join(root, 'pack/mods'), `${stage}-mods`), '--out', next]);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, '-p', 'horizon-atm10', '--no-default-features', '--features', 'ae2-lab', '--bins', '-j', jobs, '--target-dir', target], stage);
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    if (!existsSync(content)) throw new Error(`No content mod at ${content}; run the content step.`);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, HOSTILENETWORKS_CONTENT_DIR: content,
      AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // HOSTILENETWORKS_TESTS picks some (`chamber`); all of pack/mods/hostilenetworks/tests by default.
    const names = process.env.HOSTILENETWORKS_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/hostilenetworks.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
