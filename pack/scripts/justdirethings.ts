// Builds and tests the pack's Just Dire Things port in a checkout of this fork: the crate's unit tests, the content mod,
// the game staged with the native bundle (horizon-mixins apply), its build and the world tests. The blocks, items,
// models, recipes and tags are the carry of the player's own jar (horizon-port-pack carry, ATM10's `tools/atm10/`):
// this script never makes one.
// Usage: bun run pack/scripts/justdirethings.ts <step>... (steps: content, units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   JUSTDIRETHINGS_CARRY       the carry of Just Dire Things     <base>/atm10-work/fullpack/carries/justdirethings
//   JUSTDIRETHINGS_CONTENT_DIR the content mod `content` makes    <base>/.build/content/justdirethings
//   JUSTDIRETHINGS_STAGE       the staged, modded source        <base>/stage-justdirethings
//   JUSTDIRETHINGS_TARGET      the staged game's target dir     <base>/.build/target-justdirethings-game
//   JUSTDIRETHINGS_TOOL_TARGET target dir of the pack's crate   <base>/.build/target-justdirethings
//   HORIZON_MIXINS           the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   AE2_RESOURCES            the prepared 26.3 resources      <base>/horizon/.local/resources/26.3
//   JUSTDIRETHINGS_FEATURES    the game's Cargo features (`-p horizon-game --no-default-features`): ae2-lab, the world
//                          tests load AE2's content
//   JUSTDIRETHINGS_JOBS        cargo jobs                       2
// `world` also passes ATM10_DIR (ATM10's settings) and AE2_CONTENT_DIR through to the harness. Nothing a step
// generates is committed: Just Dire Things' files stay local.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/justdirethings');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const carry = env('JUSTDIRETHINGS_CARRY', join(base, 'atm10-work/fullpack/carries/justdirethings'));
const content = env('JUSTDIRETHINGS_CONTENT_DIR', join(base, '.build/content/justdirethings'));
const stage = env('JUSTDIRETHINGS_STAGE', join(base, 'stage-justdirethings'));
const target = env('JUSTDIRETHINGS_TARGET', join(base, '.build/target-justdirethings-game'));
const toolTarget = env('JUSTDIRETHINGS_TOOL_TARGET', join(base, '.build/target-justdirethings'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.JUSTDIRETHINGS_JOBS ?? '2';
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
  rmSync(join(bundle, 'justdirethings-core/Cargo.lock'), { force: true });
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

/** The carry's Generator T1, Block Breaker T1, Block Placer T1, Fluid Collector T1, Item Collector and Paradox Machine sit on the diamond block template, which has no block
 * entity or menu: the content mod is the carry with them on the furnace template (the block entity, the menu, `facing` and `lit`, as Hostile Neural Networks'
 * machines and Iron Furnaces' furnaces), minus what that template already has. The goo blocks keep their template: a
 * block entity of a declared type is made for any block. A local copy of the player's own jar's content; never committed. */
function makeContent(): void {
  rmSync(content, { recursive: true, force: true });
  cpSync(carry, content, { recursive: true });
  const blocks = join(content, 'data/justdirethings/horizon/block');
  for (const file of readdirSync(blocks).filter(f => ['generatort1.json', 'blockbreakert1.json', 'paradoxmachine.json', 'blockplacert1.json', 'fluidcollectort1.json', 'itemcollector.json'].includes(f))) {
    const path = join(blocks, file);
    const decl = JSON.parse(readFileSync(path, 'utf8'));
    decl.template = 'minecraft:furnace';
    for (const field of ['state_properties', 'state_defaults']) {
      for (const owned of ['lit', 'facing']) delete decl[field]?.[owned];
    }
    for (const field of ['default_state', 'placement']) delete decl[field];
    // The shapes select on `facing`, which the furnace template owns: the content mod keeps the furnace's full cube.
    for (const field of ['collision_boxes', 'outline_boxes']) delete decl[field];
    writeFileSync(path, JSON.stringify(decl, null, 2));
  }
  // The time crystal's budding block ticks randomly (Java's `randomTicks()`); its carry declaration does not say so.
  const budding = join(blocks, 'time_crystal_budding_block.json');
  const decl = JSON.parse(readFileSync(budding, 'utf8'));
  decl.properties = { ...decl.properties, random_ticks: true };
  writeFileSync(budding, JSON.stringify(decl, null, 2));
  console.log(`${content}: ${readdirSync(blocks).length} block declarations`);
}

const steps: Record<string, () => Promise<void>> = {
  async content() {
    if (!existsSync(carry)) throw new Error(`No carry of Just Dire Things at ${carry} (JUSTDIRETHINGS_CARRY).`);
    makeContent();
  },
  async units() {
    try {
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'justdirethings-core/Cargo.toml'), '--target-dir', toolTarget]);
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    // Only this bundle (and HORIZON_LOCAL_BUNDLES): the others under pack/mods may still be on the fork's hooks.
    const mods = `${stage}-mods`;
    rmSync(mods, { recursive: true, force: true });
    cpSync(bundle, join(mods, basename(bundle)), { recursive: true, filter: path => !relative(bundle, path).split(/[\\/]/).some(part => part === 'tests' || part === 'target' || part === 'Cargo.lock') });
    addLocalBundles(mods);
    await applyBundles(command => run(command), mixins, root, mods, next, toolTarget);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.JUSTDIRETHINGS_FEATURES ?? 'ae2-lab'), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    if (!existsSync(content)) throw new Error(`No content mod at ${content}; run the content step.`);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, JUSTDIRETHINGS_CONTENT_DIR: content,
      AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // JUSTDIRETHINGS_TESTS picks some (`goo,generator`); all of pack/mods/justdirethings/tests by default.
    const names = process.env.JUSTDIRETHINGS_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/justdirethings.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
