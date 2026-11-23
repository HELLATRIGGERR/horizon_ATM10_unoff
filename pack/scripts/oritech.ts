// Builds and tests the pack's Oritech port in a checkout of this fork: the crate's unit tests, the game staged
// with the native bundles under pack/mods (horizon-mixins apply), its build and the world tests. The blocks, items,
// models and recipes are the carry of the player's own jar (horizon-port-pack carry, ATM10's `tools/atm10/`): this
// script never makes one.
// Usage: bun run pack/scripts/oritech.ts <step>... (steps: units, content, stage, build, world; all: every one)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   ORITECH_CARRY             the carry of Oritech             <base>/atm10-work/fullpack/carries/oritech
//   ORITECH_CONTENT_DIR       the content mod `content` makes  <base>/.build/content/oritech
//   ORITECH_STAGE             the staged, modded source        <base>/stage-oritech
//   ORITECH_TARGET            the staged game's target dir     <base>/.build/target-oritech-game
//   ORITECH_TOOL_TARGET       target dir of the pack's crate   <base>/.build/target-oritech
//   HORIZON_MIXINS            the horizon-mixins binary        <base>/.build/target-mixins/debug/horizon-mixins
//   AE2_RESOURCES             the prepared 26.3 resources      <base>/horizon/.local/resources/26.3
//   ORITECH_FEATURES          the game's Cargo features (`-p horizon-game --no-default-features`): ae2-lab, the world
//                            tests load AE2's content
//   ORITECH_JOBS              cargo jobs                       2
// `world` also passes ATM10_DIR (ATM10's settings) and AE2_CONTENT_DIR through to the harness. Nothing a step
// generates is committed: Oritech's files stay local.
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
const bundle = join(root, 'pack/mods/oritech');
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const carry = env('ORITECH_CARRY', join(base, 'atm10-work/fullpack/carries/oritech'));
const content = env('ORITECH_CONTENT_DIR', join(base, '.build/content/oritech'));
const stage = env('ORITECH_STAGE', join(base, 'stage-oritech'));
const target = env('ORITECH_TARGET', join(base, '.build/target-oritech-game'));
const toolTarget = env('ORITECH_TOOL_TARGET', join(base, '.build/target-oritech'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.ORITECH_JOBS ?? '2';
const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
const resources = process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3');
const cargoEnv = { ...process.env, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_FAST_DEBUG: '0', CARGO_PROFILE_FAST_INCREMENTAL: 'false' };

async function run(command: string[], cwd = root, extra: Record<string, string | undefined> = cargoEnv): Promise<void> {
  console.log('>', command.join(' '));
  const child = Bun.spawn(command, { cwd, env: extra, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
  const code = await child.exited;
  if (code !== 0) throw new Error(`${command[0]} exited with status ${code}`);
}

function dropCrateLock(): void {
  rmSync(join(bundle, 'oritech-core/Cargo.lock'), { force: true });
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
    const source = join(from, file), dest = join(to, file);
    if (existsSync(dest) && readFileSync(dest).equals(readFileSync(source))) continue;
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(source, dest);
    written++;
  }
  if (existsSync(to)) for (const file of files(to)) if (!fresh.has(file)) { rmSync(join(to, file)); written++; }
  return written;
}

/** The carry's processing machines sit on the carved pumpkin template, which has no right-click: the content mod is
 * the carry with each ported machine on the furnace template (the block entity, the menu and `facing`, as the other
 * ported bundles do), minus the properties that template already has. A local copy of the player's own jar's
 * content; never committed. */
function makeContent(): void {
  rmSync(content, { recursive: true, force: true });
  // A content mod refuses scripts and binaries (the jar carries a texture-colour helper script).
  cpSync(carry, content, { recursive: true, filter: path => !/\.(sh|bat|cmd|exe|ps1|py)$/i.test(path) });
  const blocks = join(content, 'data/oritech/horizon/block');
  const machines = ['pulverizer_block.json', 'fragment_forge_block.json', 'assembler_block.json', 'foundry_block.json', 'centrifuge_block.json', 'basic_generator_block.json', 'bio_generator_block.json', 'powered_furnace_block.json'];
  for (const file of machines) {
    const path = join(blocks, file);
    if (!existsSync(path)) { console.warn(`${file} not in carry`); continue; }
    const decl = JSON.parse(readFileSync(path, 'utf8'));
    decl.template = 'minecraft:furnace';
    for (const field of ['state_properties', 'state_defaults']) {
      for (const owned of ['lit', 'facing']) delete decl[field]?.[owned];
    }
    for (const field of ['default_state', 'placement']) delete decl[field];
    decl.properties = { ...decl.properties, light: 0 };
    writeFileSync(path, JSON.stringify(decl, null, 2));
  }
  console.log(`${content}: ${machines.length} machine blocks on furnace template, ${readdirSync(blocks).length} total`);
}

const steps: Record<string, () => Promise<void>> = {
  async content() {
    if (!existsSync(carry)) throw new Error(`No carry of Oritech at ${carry} (ORITECH_CARRY).`);
    makeContent();
  },
  async units() {
    try {
      await run(['cargo', 'test', '--offline', '-j', jobs, '--manifest-path', join(bundle, 'oritech-core/Cargo.toml'), '--target-dir', toolTarget]);
    } finally { dropCrateLock(); }
  },
  async stage() {
    dropCrateLock();
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    const mods = `${stage}-mods`;
    rmSync(mods, { recursive: true, force: true });
    cpSync(bundle, join(mods, basename(bundle)), { recursive: true, filter: path => !relative(bundle, path).split(/[\\/]/).some(part => part === 'tests' || part === 'target' || part === 'Cargo.lock') });
    addLocalBundles(mods);
    await applyBundles(command => run(command), mixins, root, mods, next, toolTarget);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.ORITECH_FEATURES ?? 'ae2-lab'), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    if (!existsSync(content)) throw new Error(`No content mod at ${content}; run the content step.`);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, ORITECH_CONTENT_DIR: content,
      AE2_RESOURCES: resources,
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    const names = process.env.ORITECH_TESTS?.split(',').filter(Boolean)
      ?? readdirSync(join(bundle, 'tests')).filter(f => f.endsWith('.ts')).map(f => f.slice(0, -3)).sort();
    for (const name of names) await run(['bun', 'run', join(bundle, 'tests', `${name}.ts`)], root, harness);
  },
};

const args = process.argv.slice(2);
if (args.length === 0 || args.includes('--help') || args.includes('-h')) {
  console.log('Usage: bun run pack/scripts/oritech.ts <step>...\nSteps, in order: ' + Object.keys(steps).join(', ') + '; all runs every one.');
  process.exit(0);
}
for (const step of args.includes('all') ? Object.keys(steps) : args) {
  const go = steps[step];
  if (!go) throw new Error(`Unknown step ${step}; steps: ${Object.keys(steps).join(', ')}`);
  await go();
}
