// Builds and tests the pack's FTB Quests port (with FTB Teams, which it needs) in a checkout of this fork: the
// bundles' crates' unit tests, the game staged with those bundles only (horizon-mixins apply), its build, the content
// mod with the book item, a check that no fallback line is FTB Quests' own text, and the world tests.
// Usage: bun run pack/scripts/ftbquests.ts <step>... (steps: units, stage, build, content, check-text, world; all)
//
// Paths default to this machine's layout beside the checkout (`<base>` = the checkout's parent folder) and can be
// set in the environment:
//   FTBQ_STAGE        the staged, modded source         <base>/stage-ftbquests
//   FTBQ_TARGET       the staged game's target dir      <base>/.build/target-ftbquests-game
//   FTBQ_TOOL_TARGET  target dir of the bundles' crates <base>/.build/target-ftbquests
//   FTBQ_CONTENT_DIR  the content mod (the book item)   <base>/.build/content/ftbquests
//   HORIZON_MIXINS    the horizon-mixins binary         <base>/.build/target-mixins/debug/horizon-mixins
//   PORT_PACK         the horizon-port-pack binary      <base>/.build/target-portpack/release/horizon-port-pack
//   ATM10_DIR         the ATM10 instance (its jar, its quest book)
//   FTBQ_JOBS         cargo jobs                        2
//   FTBQ_FEATURES     the game's Cargo features         ae2-lab (`-p horizon-game --no-default-features`; the
//                                                     world tests load AE2's content, which needs it)
// `world` passes AE2_RESOURCES, AE2_CONTENT_DIR, FTBQ_CONTENT_DIR and ATM10_DIR to the harness, and
// FTBTEAMS_DEV=1 (the fake player command and the probes).
//
// No file of FTB Quests or FTB Teams is in the bundles: they are All Rights Reserved (see their READMEs). The
// content mod is carried from the player's jar into a folder beside the checkout; the quest book is the instance's.
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { profile } from '../../ae2/scripts/common';
import { addLocalBundles } from './local-bundles';
import { applyBundles, gameBuild, gameBuildEnv } from './hook-catalog';

const root = resolve(import.meta.dir, '../..');
const base = resolve(root, '..');
const exe = process.platform === 'win32' ? '.exe' : '';
/** The bundles of this port, in the order they depend on each other. */
const BUNDLES = ['ftbteams', 'ftbquests'].filter(name => existsSync(join(root, 'pack/mods', name, 'horizon-mixin.toml')));
const CRATES: Record<string, string> = { ftbteams: 'ftbteams-core', ftbquests: 'ftbquests-core' };
const env = (name: string, fallback: string) => resolve(process.env[name] ?? fallback);
const stage = env('FTBQ_STAGE', join(base, 'stage-ftbquests'));
const target = env('FTBQ_TARGET', join(base, '.build/target-ftbquests-game'));
const toolTarget = env('FTBQ_TOOL_TARGET', join(base, '.build/target-ftbquests'));
const mixins = env('HORIZON_MIXINS', join(base, `.build/target-mixins/debug/horizon-mixins${exe}`));
const jobs = process.env.FTBQ_JOBS ?? '2';
const content = env('FTBQ_CONTENT_DIR', join(base, '.build/content/ftbquests'));
const portPack = env('PORT_PACK', join(base, `.build/target-portpack/release/horizon-port-pack${exe}`));
const instance = env('ATM10_DIR', join(process.env.USERPROFILE ?? process.env.HOME ?? '', 'curseforge/minecraft/Instances/All the Mods 10 - ATM10'));
function jar(): string {
  const name = readdirSync(join(instance, 'mods')).find(f => /^ftb-quests-neoforge-.*\.jar$/.test(f));
  if (!name) throw new Error(`No FTB Quests jar in ${instance}/mods; set ATM10_DIR.`);
  return join(instance, 'mods', name);
}
const MARKER = 'ftbquests-content.json';
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
    addLocalBundles(mods);
    // horizon-mixins replaces an existing stage only on Linux: apply into a fresh folder, then copy what changed.
    const next = `${stage}-next`;
    rmSync(next, { recursive: true, force: true });
    await applyBundles(command => run(command), mixins, root, mods, next, toolTarget);
    console.log(`${syncStage(next, stage)} file(s) of ${stage} changed`);
    rmSync(next, { recursive: true, force: true });
  },
  async build() {
    await run(['cargo', 'build', '--locked', '--profile', profile, ...gameBuild(process.env.FTBQ_FEATURES ?? 'ae2-lab'), '-j', jobs, '--target-dir', target], stage, gameBuildEnv(root, cargoEnv));
  },
  /** The book item and its model, carried from the player's jar into a content mod beside the checkout. */
  async content() {
    if (existsSync(content)) {
      if (!existsSync(join(content, MARKER))) throw new Error(`${content} wasn't made by this script; not replacing it.`);
      rmSync(content, { recursive: true, force: true });
    }
    const work = mkdtempSync(join(tmpdir(), 'ftbquests-carry-'));
    try {
      await run([portPack, 'check', jar(), work]);
      mkdirSync(dirname(content), { recursive: true });
      cpSync(join(work, 'carry'), content, { recursive: true });
      writeFileSync(join(content, MARKER), JSON.stringify({ from: jar() }));
    } finally { rmSync(work, { recursive: true, force: true }); }
  },
  /** No fallback line of the bundle may be a line of FTB Quests' own lang files (read from the player's jar). */
  async 'check-text'() {
    const dir = mkdtempSync(join(tmpdir(), 'ftbquests-lang-'));
    try {
      await run(['unzip', '-q', '-o', jar(), 'assets/ftbquests/lang/*', '-d', dir], root, process.env);
      const theirs = new Set<string>();
      const langDir = join(dir, 'assets/ftbquests/lang');
      for (const f of readdirSync(langDir)) {
        for (const v of Object.values(JSON.parse(readFileSync(join(langDir, f), 'utf8')))) theirs.add(String(v).trim().toLowerCase());
      }
      const bundle = join(root, 'pack/mods/ftbquests');
      const ours = files(bundle).filter(f => f.endsWith('.rs')).flatMap(f => [...readFileSync(join(bundle, f), 'utf8')
        .matchAll(/tr\(\s*"([^"]+)",\s*"([^"]*)"/g)].map(m => ({ file: f, key: m[1], fallback: m[2] })));
      const same = ours.filter(o => o.fallback.length > 3 && theirs.has(o.fallback.trim().toLowerCase()));
      console.log(`${ours.length} fallback line(s) checked against ${theirs.size} FTB Quests lines; ${same.length} the same`);
      for (const o of same) console.log(`  ${o.file}: ${o.key} = "${o.fallback}"`);
      if (same.length) throw new Error("a fallback is FTB Quests' own text; reword it");
    } finally { rmSync(dir, { recursive: true, force: true }); }
  },
  async world() {
    const binary = join(target, `${profile}/horizon-client${exe}`);
    if (!existsSync(binary)) throw new Error(`No staged build at ${binary}; run the build step.`);
    const prepared = (path: string) => existsSync(join(root, path)) ? join(root, path) : join(base, 'horizon', path);
    const harness = {
      ...process.env, AE2_TEST_BINARY: binary, FTBTEAMS_DEV: '1', FTBQ_CONTENT_DIR: content, ATM10_DIR: instance,
      AE2_RESOURCES: process.env.AE2_RESOURCES ?? prepared('.local/resources/26.3'),
      AE2_CONTENT_DIR: process.env.AE2_CONTENT_DIR ?? prepared('content/ae2'),
      AE2_TEST_STARTUP_MS: process.env.AE2_TEST_STARTUP_MS ?? '600000',
    };
    // FTBQ_TESTS picks some (`ftbquests/quests`); the FTB Quests bundle's by default.
    const names = process.env.FTBQ_TESTS?.split(',').filter(Boolean)
      ?? ['ftbquests'].flatMap(name => existsSync(join(root, 'pack/mods', name, 'tests'))
        ? readdirSync(join(root, 'pack/mods', name, 'tests')).filter(f => f.endsWith('.ts')).map(f => `${name}/${f.slice(0, -3)}`).sort() : []);
    for (const name of names) {
      const [bundle, test] = name.split('/');
      await run(['bun', 'run', join(root, 'pack/mods', bundle, 'tests', `${test}.ts`)], root, harness);
    }
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
