// Builds and tests the pack's Theurgy port in a checkout of this fork: the core crate's unit tests, the content mod, the
// game staged with the native bundle (horizon-mixins apply), its build and the world tests. The blocks, items, fluids,
// models, recipes and tags are the carry of the player's own jar (horizon-port-pack carry, ATM10's `tools/atm10/`):
// this script never makes one.
// Usage: bun run pack/scripts/theurgy.ts <step>... (steps: content, units, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   THEURGY_CARRY        the carry of Theurgy                `<base>/atm10-work/fullpack/carries/theurgy`
//   THEURGY_CONTENT_DIR  the content mod `content` makes      `<base>/.build/content/theurgy`
//   ATM10_DIR            the ATM10 instance, whose Theurgy jar `content` reads the accumulation recipes from
//   THEURGY_STAGE        the staged, modded source           `<base>/stage-theurgy`
//   THEURGY_TARGET       the staged game's target dir        `<base>/.build/target-theurgy-game`
//   THEURGY_TOOL_TARGET  target dir of the core crate        `<base>/.build/target-theurgy`
//   HORIZON_MIXINS       the horizon-mixins binary           `<base>/.build/target-mixins/debug/horizon-mixins`
//   AE2_RESOURCES        the prepared 26.3 resources         `<base>/horizon/.local/resources/26.3`
//   THEURGY_FEATURES     the game's Cargo features (`-p horizon-game --no-default-features`): ae2-lab, the world
//                        tests load AE2's content
//   THEURGY_JOBS         cargo jobs                          2
// `units` also reads the content mod's recipes (THEURGY_CONTENT_DIR) when it is there and counts what the ported
// machines run. `world` passes AE2_CONTENT_DIR through to the harness. Nothing a step generates is committed:
// Theurgy's files stay local.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/theurgy');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const carry = env('THEURGY_CARRY', join(base, 'atm10-work/fullpack/carries/theurgy'));
const content = env('THEURGY_CONTENT_DIR', join(base, '.build/content/theurgy'));
const stage = env('THEURGY_STAGE', join(base, 'stage-theurgy'));
const target = env('THEURGY_TARGET', join(base, '.build/target-theurgy-game'));
const toolTarget = env('THEURGY_TOOL_TARGET', join(base, '.build/target-theurgy'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.THEURGY_JOBS ?? '2';
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
  rmSync(join(bundle, 'theurgy-core/Cargo.lock'), { force: true });
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

/** The content mod: the carry of the player's jar, plus the jar's accumulation recipes (`theurgy:accumulation`, the sal
 * ammoniac accumulator's four), which the carry leaves out. A recipe whose `neoforge:conditions` ask that an item tag
 * not be empty is kept when the content or the jar has that tag with members (the only condition they use), and
 * written without the conditions. A local copy of the player's own jar's content; never committed. */
function makeContent(): void {
  rmSync(content, { recursive: true, force: true });
  cpSync(carry, content, { recursive: true });
  const atm10 = process.env.ATM10_DIR;
  const mods = atm10 ? join(atm10, 'mods') : '';
  const jar = mods && existsSync(mods) ? readdirSync(mods).find(f => /^theurgy-1\.21\.1-neoforge-.*\.jar$/.test(f)) : undefined;
  if (!jar) throw new Error(`No Theurgy jar under ${mods || 'ATM10_DIR/mods'} (ATM10_DIR): the accumulation recipes come from it.`);
  const tmp = `${content}-jar`;
  rmSync(tmp, { recursive: true, force: true });
  mkdirSync(tmp, { recursive: true });
  // A jar is a zip: Windows' own tar (bsdtar) reads it; elsewhere unzip.
  const unpack = process.platform === 'win32'
    ? [join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe'), '-xf', join(mods, jar), '-C', tmp, 'data/theurgy/recipe/accumulation', 'data/c/tags/item']
    : ['unzip', '-q', join(mods, jar), 'data/theurgy/recipe/accumulation/*', 'data/c/tags/item/*', '-d', tmp];
  const unpacked = Bun.spawnSync(unpack);
  if (unpacked.exitCode !== 0) throw new Error(`${unpack[0]} could not read ${jar}: ${unpacked.stderr}`);
  const from = join(tmp, 'data/theurgy/recipe/accumulation');
  const to = join(content, 'data/theurgy/recipe/accumulation');
  mkdirSync(to, { recursive: true });
  // The tag as the content or the jar itself has it.
  const tagHasMembers = (tag: string): boolean => {
    const [ns, path] = tag.split(':');
    return [content, tmp].some(dir => {
      const file = join(dir, 'data', ns, 'tags/item', `${path}.json`);
      return existsSync(file) && (JSON.parse(readFileSync(file, 'utf8')).values ?? []).length > 0;
    });
  };
  let kept = 0;
  for (const name of readdirSync(from).filter(f => f.endsWith('.json'))) {
    const recipe = JSON.parse(readFileSync(join(from, name), 'utf8'));
    const conditions: any[] = recipe['neoforge:conditions'] ?? [];
    const holds = conditions.every(c => c.type === 'neoforge:not' && c.value?.type === 'neoforge:tag_empty' && tagHasMembers(c.value.tag));
    if (!holds) continue;
    delete recipe['neoforge:conditions'];
    writeFileSync(join(to, name), JSON.stringify(recipe, null, 2));
    kept++;
  }
  rmSync(tmp, { recursive: true, force: true });
  console.log(`${content}: the carry and ${kept} accumulation recipe(s) from ${jar}`);
}

const steps: Record<string, () => Promise<void>> = {
  async content() {
    if (!existsSync(carry)) throw new Error(`No carry of Theurgy at ${carry} (THEURGY_CARRY).`);
    makeContent();
  },
  async units() {
    try {
      const extra = existsSync(content) ? { ...cargoEnv, THEURGY_CONTENT_DIR: content } : cargoEnv;
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'theurgy-core/Cargo.toml'), '--target-dir', toolTarget, '--', '--nocapture'], root, extra);
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
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.THEURGY_FEATURES ?? 'ae2-lab'), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    if (!existsSync(content)) throw new Error(`No content mod at ${content}; run the content step.`);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, THEURGY_CONTENT_DIR: content,
      AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // THEURGY_TESTS picks some (`goo,generator`); all of pack/mods/theurgy/tests by default.
    const names = process.env.THEURGY_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/theurgy.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}