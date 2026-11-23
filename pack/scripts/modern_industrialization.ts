// Builds and tests the pack's Modern Industrialization port in a checkout of this fork: the crate's unit tests,
// the game staged with the native bundles under pack/mods (horizon-mixins apply through pack/scripts/hook-catalog.ts),
// its build and the world tests.
// MI's blocks, items, recipes, loot and assets are the carry of the player's own jar (horizon-port-pack's
// `carry`, or the full pack's `final-data`), loaded as a `--dev-mod`; this script generates nothing from it.
// Usage: bun run pack/scripts/modern_industrialization.ts <step>... (steps: units, stage, build, content, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   MI_CARRY          the carry of Modern Industrialization  <base>/ports/atm10/mods/modern_industrialization/carry
//   MI_CONTENT        the carry with the ported machines' blocks on the beacon template (step `content`)
//                                                            <base>/.build/content/modern_industrialization
//   MI_STAGE          the staged, modded source              <base>/stage-mi
//   MI_TARGET         the staged game's target dir           <base>/.build/target-mi-game
//   MI_TOOL_TARGET    target dir of the bundle's crate       <base>/.build/target-mi
//   MI_ONLY           1: stage this bundle alone (a quicker build; with HORIZON_LOCAL_BUNDLES' beside it); unset:
//                     every bundle under pack/mods
//   HORIZON_MIXINS    the horizon-mixins binary              <base>/.build/target-mixins/debug/horizon-mixins
//   AE2_RESOURCES     the prepared 26.3 resources            <base>/horizon/.local/resources/26.3
//   ATM10_DIR         the player's ATM10 instance: `stage` reads its kubejs/startup_scripts/Modern-Industrialization/multiblocks/*.js
//                     into the staged bundle's mi-core/data/kubejs_multiblocks.json (never into this repository)
//                     C:/Users/<you>/curseforge/minecraft/Instances/All the Mods 10 - ATM10
//   MI_JOBS           cargo jobs                             2
//   MI_FEATURES       the game's Cargo features              ae2-lab (`-p horizon-game --no-default-features`)
//   MI_TESTS          world tests to run (`machines`), all of pack/mods/modern_industrialization/tests by default
// `world` also passes ATM10_DIR (ATM10's `config/modern_industrialization-server.toml`) through to the harness.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';
import { readInstance, writeData } from './mi-kubejs';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/modern_industrialization');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const carry = env('MI_CARRY', join(base, 'ports/atm10/mods/modern_industrialization/carry'));
const content = env('MI_CONTENT', join(base, '.build/content/modern_industrialization'));
const stage = env('MI_STAGE', join(base, 'stage-mi'));
const target = env('MI_TARGET', join(base, '.build/target-mi-game'));
const toolTarget = env('MI_TOOL_TARGET', join(base, '.build/target-mi'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.MI_JOBS ?? '2';
const instance = env('ATM10_DIR', join(process.env.USERPROFILE ?? process.env.HOME ?? '', 'curseforge/minecraft/Instances/All the Mods 10 - ATM10'));
/** The multiblocks of the instance's scripts, as the staged bundle's data file holds them (step `stage` writes it). */
const kubejsData = join(stage + '-mods', 'modern_industrialization', 'mi-core/data/kubejs_multiblocks.json');
const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
const resources = process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3');
const cargoEnv = { ...process.env, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_FAST_DEBUG: '0', CARGO_PROFILE_FAST_INCREMENTAL: 'false' };

async function run(command: string[], cwd = root, extra: Record<string, string | undefined> = cargoEnv): Promise<void> {
  console.log('>', command.join(' '));
  const child = Bun.spawn(command, { cwd, env: extra, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  const code = await child.exited;
  if (code !== 0) throw new Error(`${command[0]} exited with status ${code}`);
}

/** A bundle crate may not hold a Cargo.lock (horizon-mixins refuses it); `units` makes one. */
function dropCrateLock(): void {
  rmSync(join(bundle, 'mi-core/Cargo.lock'), { force: true });
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

/** Reads the instance's multiblock scripts into the staged copy of the bundle: one line for each script left out. */
function stageKubejs(mods: string): void {
  const { multiblocks, skipped } = readInstance(instance);
  for (const line of skipped) console.log(`KubeJS multiblock script skipped: ${line}`);
  const file = writeData(join(mods, 'modern_industrialization'), multiblocks);
  console.log(`${multiblocks.length} KubeJS multiblock(s) read from ${instance} into ${file}: ${multiblocks.map(m => m.id).join(', ')}`);
}

/** The bundles to apply, in a scratch copy under the stage (the KubeJS data is written into it, not into the
 * repository): all of pack/mods, or with MI_ONLY this one alone. */
function mods(): string {
  const scratch = `${stage}-mods`;
  if (process.env.MI_ONLY !== '1') {
    rmSync(scratch, { recursive: true, force: true });
    const leftOut = new Set(['tests', 'target', 'Cargo.lock', '.git']);
    cpSync(join(root, 'pack/mods'), scratch, { recursive: true, filter: path => !relative(join(root, 'pack/mods'), path).split(/[\\/]/).some(part => leftOut.has(part)) });
    addLocalBundles(scratch);
    stageKubejs(scratch);
    return scratch;
  }
  const only = scratch;
  rmSync(only, { recursive: true, force: true });
  const leftOut = new Set(['tests', 'target', 'Cargo.lock']);
  cpSync(bundle, join(only, 'modern_industrialization'), { recursive: true, filter: path => !relative(bundle, path).split(/[\\/]/).some(part => leftOut.has(part)) });
  addLocalBundles(only);
  stageKubejs(only);
  return only;
}

const steps: Record<string, () => Promise<void>> = {
  async units() {
    try {
      // Against the staged KubeJS data when a stage has written it: the test that forms each multiblock then runs.
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'mi-core/Cargo.toml'), '--target-dir', toolTarget], root, { ...cargoEnv, MI_KUBEJS_MULTIBLOCKS: existsSync(kubejsData) ? kubejsData : '' });
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    await applyBundles(command => run(command), mixins, root, mods(), next, toolTarget);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.MI_FEATURES ?? 'ae2-lab'), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  /** The carry as a content mod the world test loads. The carry's machine blocks are on a template that opens
   * no menu (`minecraft:andesite`: a use answers ok and nothing opens), so each machine, hatch and multiblock
   * controller the bundle ports takes the `minecraft:beacon` template, as Mekanism's machines do; nothing else of the
   * carry changes. */
  async content() {
    if (!existsSync(join(carry, 'horizon-mod.toml'))) throw new Error(`No carry of Modern Industrialization at ${carry} (MI_CARRY).`);
    rmSync(content, { recursive: true, force: true });
    cpSync(carry, content, { recursive: true });
    const ids = (file: string, pattern: RegExp) => [...readFileSync(join(bundle, 'mi-core/src', file), 'utf8').matchAll(pattern)].map(m => m[1]);
    const machines = [
      ...ids('machines.rs', /^\s+id: "(electric_\w+)",$/gm),
      ...ids('hatches.rs', /^\s+(?:item|fluid|energy)\("(\w+_hatch)",/gm),
      ...ids('multiblocks.rs', /^\s+id: "(\w+)",$/gm),
      ...(existsSync(kubejsData) ? (JSON.parse(readFileSync(kubejsData, 'utf8')).multiblocks as { id: string }[]).map(m => m.id) : []),
    ];
    if (machines.length === 0) throw new Error('No machine ids found in mi-core/src');
    const applied: string[] = [], skipped: string[] = [];
    for (const id of machines) {
      const file = join(content, `data/modern_industrialization/horizon/block/${id}.json`);
      if (!existsSync(file)) { skipped.push(id); continue; } // KubeJS-registered machines are not in the static carry
      const block = JSON.parse(readFileSync(file, 'utf8'));
      block.template = 'minecraft:beacon';
      writeFileSync(file, JSON.stringify(block, null, 2) + '\n');
      applied.push(id);
    }
    if (skipped.length > 0) console.log(`${skipped.length} machine(s) skipped (KubeJS-registered, not in carry): ${skipped.join(', ')}`);
    console.log(`${applied.length} machine block(s) of ${content} on the beacon template: ${applied.join(', ')}`);
  },
  async world() {
    await steps.content();
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, MI_CONTENT: content,
      AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    const names = process.env.MI_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/modern_industrialization.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
