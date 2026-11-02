// Starts the fork's game the way horizon\ae2\scripts\play.ts does (its binary and prepared resources, AE2's
// content as the first --dev-mod, --server-thread), with every ATM10 mod carried into ..\mods\<id>\carry added as
// another --dev-mod, in alphabetical order, except the ones listed in ..\leave-out.txt. Called by
// ..\play-atm10.cmd.
//
// - Command length: a cmd.exe line holds 8,191 characters and 287 carries take about 20,650, so the list is
//   built here and handed straight to the process (Windows allows 32,767 characters for one).
// - Profile: ..\play\ (saves, settings, logs and the content cache), not horizon\.local\play; --profile DIR
//   puts it elsewhere (the content root is ~610,000 files: on an internal SSD rather than the USB drive E:).
//   The game builds one content root per set of mods in <profile>\game\.horizon-cache; sharing play.cmd's would
//   rebuild it on every switch between AE2 alone and the whole pack, and would rewrite it under a game play.cmd
//   started.
//
//   play-atm10.cmd [--print] [--only id,id] [--without id,id] [--carries DIR] [--hmod DIR] [--profile DIR]
//                  [game arguments...]
//
// --print shows the command and its length instead of running it; --only and --without pick carries by mod
// folder name. Where each carry is read from: DIR\<id>\ of --carries (folders unpacked by tools\unpack-hmod.py
// on a fast disk), else DIR\<id>.hmod of --hmod (tools\pack-hmod.py), else ..\mods\<id>\carry. The game walks
// a folder and stats every file at each start (586,000 files for all carries: minutes on the USB drive E:), and
// a .hmod is slower still for mods with many blocks (see unpack-hmod.py), so the fast start is --carries on C:.
// Every other argument goes to the game, e.g. --world "ATM10 base" --creative.
import { existsSync, mkdirSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

// ATM10_WORK: the port-pack work folder (mods/<id>/carry, leave-out.txt, play/); HORIZON_REPO: the checkout whose
// target/fast/horizon-client.exe, .local/resources/26.3 and content/ae2 are the defaults below.
const here = resolve(process.env.ATM10_WORK ?? resolve(import.meta.dir, '..'));
const repo = resolve(process.env.HORIZON_REPO ?? resolve(import.meta.dir, '../..'));
const args = process.argv.slice(2);
const take = (flag: string): string[] | undefined => {
  const i = args.indexOf(flag);
  if (i < 0) return undefined;
  const value = args[i + 1] ?? '';
  args.splice(i, 2);
  return value.split(',').map(s => s.trim()).filter(Boolean);
};
const print = args.includes('--print');
if (print) args.splice(args.indexOf('--print'), 1);
const only = take('--only');
const without = new Set(take('--without') ?? []);
const hmod = take('--hmod')?.join(',');
const folders = take('--carries')?.join(',');
const profileDir = take('--profile')?.join(',');

// leave-out.txt: one mod folder name per line, `#` starts a comment (the reason).
const leaveOut = new Set(
  existsSync(join(here, 'leave-out.txt'))
    ? readFileSync(join(here, 'leave-out.txt'), 'utf8').split(/\r?\n/).map(l => l.replace(/#.*/, '').trim()).filter(Boolean)
    : [],
);
// A folder of --carries that has no carry under mods/ loads too: the generated atm10-pack.
const extra = folders
  ? readdirSync(folders, { withFileTypes: true })
      .filter(d => d.isDirectory() && existsSync(join(folders, d.name, 'horizon-mod.toml')))
      .map(d => d.name)
  : [];
const carries = [
  ...new Set([
    ...readdirSync(join(here, 'mods'), { withFileTypes: true })
      .filter(d => d.isDirectory() && existsSync(join(here, 'mods', d.name, 'carry', 'horizon-mod.toml')))
      .map(d => d.name),
    ...extra,
  ]),
]
  .sort()
  .filter(id => (only ? only.includes(id) : !leaveOut.has(id)) && !without.has(id));

// As play.ts: AE2_CLIENT, AE2_RESOURCES and AE2_CONTENT_DIR override the defaults.
const binary = resolve(process.env.AE2_CLIENT ?? join(repo, 'target/fast/horizon-client.exe'));
const resources = resolve(process.env.AE2_RESOURCES ?? join(repo, '.local/resources/26.3'));
const content = resolve(process.env.AE2_CONTENT_DIR ?? join(repo, 'content/ae2'));
const profile = resolve(profileDir ?? join(here, 'play'));
const game = join(profile, 'game');
const unpacked = (id: string) =>
  folders && existsSync(join(folders, id, 'horizon-mod.toml')) ? join(folders, id) : undefined;
const packed = (id: string) => (hmod && existsSync(join(hmod, `${id}.hmod`)) ? join(hmod, `${id}.hmod`) : undefined);
const source = (id: string) => unpacked(id) ?? packed(id) ?? join(here, 'mods', id, 'carry');
const devMods = carries.flatMap(id => ['--dev-mod', source(id)]);
const command = [binary, '--resources', resources, '--dev-mod', content, '--server-thread', ...devMods, ...args];

if (print) {
  console.log(`cd /d ${game}`);
  console.log(command.map(a => (/[\s"]/.test(a) ? `"${a}"` : a)).join(' '));
  const moved = carries.filter(id => unpacked(id)).length;
  const zips = carries.filter(id => !unpacked(id) && packed(id)).length;
  console.log(
    `\n${carries.length} carries (${moved} from --carries, ${zips} as .hmod), ${leaveOut.size} left out ` +
      `(leave-out.txt), ${command.join(' ').length} characters`,
  );
  process.exit(0);
}
for (const path of [binary, resources, content]) {
  if (!existsSync(path)) throw new Error(`Missing ${path}; follow the README build/preparation steps.`);
}
mkdirSync(game, { recursive: true });
const child = Bun.spawn(command, {
  cwd: game,
  stdio: ['inherit', 'inherit', 'inherit'],
  env: {
    ...process.env,
    HORIZON_HOME: game,
    HORIZON_LAUNCHER_HOME: join(profile, 'launcher'),
    XDG_DATA_HOME: join(profile, 'data'),
    XDG_CONFIG_HOME: join(profile, 'config'),
    HORIZON_LAUNCHER_NO_SELF_UPDATE: '1',
    HORIZON_TELEMETRY_URL: 'off',
    HORIZON_SERVER_THREAD: '1',
  },
});
process.exit(await child.exited);
