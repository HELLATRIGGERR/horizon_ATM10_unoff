/** "The full pack opens" (docs/atm10-port.md, "The full pack"): the whole ATM10 pack started as a player starts it,
 * through the title screen, with sound on, and timed against Java's 200 s.
 *
 *   bun run tools/atm10/pack-start.ts [--fresh] [--once]
 *
 * Launch 1: the game with every carry, the window offscreen and no `--world`; on the title screen Singleplayer,
 * Create New World ("Pack Start"), create; in the world; the pause menu's Save and Quit to Title; Singleplayer again,
 * the saved world, Play Selected World; in the world; quit. Launch 2 (not with --once): the same profile started
 * again, title, Singleplayer, the saved world, in the world, quit. The game runs with `HORIZON_PLAYER_START=1`, so a
 * control socket keeps the player's start: the title shows while the world tables load behind it, and a world opens
 * without holding the frame loop. Sound is on (no HORIZON_NO_AUDIO) with the master volume at 0 in a new profile's
 * options.txt: the output opens and the sound engine runs, and the person at the computer hears nothing.
 *
 * - ATM10_CARRIES: the carries folder `final-data` wrote (each carry and `atm10-pack`); each loads as a `--dev-mod`
 *   after AE2's content and its addons, in alphabetical order. PACK_START_ONLY / PACK_START_WITHOUT: carry folder
 *   names to keep / leave out (`PACK_START_ONLY=none`: AE2 alone, to try the script). PACK_START_ADDONS=0: no addons.
 * - AE2_CLIENT, AE2_RESOURCES, AE2_CONTENT_DIR, AE2_ADDON_CONTENT_ROOT: as for the AE2 scripts (play-atm10.ts).
 * - PACK_START_OUT: the folder for `pack-start.json` (timings and memory), the logs, the startup traces and the
 *   profile (`profile/game`: saves, options, the content cache). --fresh empties the profile first: a first start,
 *   whose content root, data file and snapshots are built; without it the cache of the last run is reused.
 * - PACK_START_FLOOR_MB (default 4000): stop when the machine's free memory falls under it.
 * - PACK_START_TIMEOUT_S (default 1800): the longest wait for any one step.
 *
 * One full-pack game at a time: it wants about 9 GB. The caller holds the machine's full-pack lock. */
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { connect } from 'node:net';
import { freemem, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { addonContents } from '../../ae2/scripts/common';

const repo = resolve(import.meta.dir, '../..');
const argv = process.argv.slice(2);
const fresh = argv.includes('--fresh'), once = argv.includes('--once');
const out = resolve(process.env.PACK_START_OUT ?? join(repo, '.local/pack-start'));
const profile = join(out, 'profile'), game = join(profile, 'game');
const floorMb = Number(process.env.PACK_START_FLOOR_MB ?? 4000);
const stepTimeout = Number(process.env.PACK_START_TIMEOUT_S ?? 1800) * 1000;
const worldName = 'Pack Start';

const binary = resolve(process.env.AE2_CLIENT ?? join(repo, 'target/fast/horizon-client.exe'));
const resources = resolve(process.env.AE2_RESOURCES ?? join(repo, '.local/resources/26.3'));
const content = resolve(process.env.AE2_CONTENT_DIR ?? join(repo, 'content/ae2'));
const list = (name: string) => (process.env[name] ?? '').split(',').map(s => s.trim()).filter(Boolean);
const only = list('PACK_START_ONLY'), without = new Set(list('PACK_START_WITHOUT'));
const carries = process.env.ATM10_CARRIES ? resolve(process.env.ATM10_CARRIES) : '';
if (!carries || !existsSync(join(carries, 'atm10-pack', 'horizon-mod.toml'))) {
  throw new Error('ATM10_CARRIES must name the carries folder final-data wrote (with atm10-pack)');
}
const mods = readdirSync(carries, { withFileTypes: true })
  .filter(d => d.isDirectory() && existsSync(join(carries, d.name, 'horizon-mod.toml')))
  .map(d => d.name)
  .sort()
  .filter(id => (only.length ? only.includes(id) : true) && !without.has(id));
const addons = process.env.PACK_START_ADDONS === '0' ? [] : addonContents().filter(a => existsSync(a.path));
for (const path of [binary, resources, content]) if (!existsSync(path)) throw new Error(`Missing ${path}`);

if (fresh) rmSync(profile, { recursive: true, force: true });
// Each run makes its world anew; the content cache stays unless --fresh.
rmSync(join(game, 'saves'), { recursive: true, force: true });
mkdirSync(game, { recursive: true });
const options = join(game, 'options.txt');
if (!existsSync(options)) writeFileSync(options, 'soundCategory_master:0.0\n');
const transcript = join(out, 'transcript.jsonl');
rmSync(transcript, { force: true });

type Sample = { t: number; ws: number; priv: number; free: number };
type Launch = {
  label: string; content_root: 'built' | 'reused'; steps: Record<string, number>; trace: Record<string, number>;
  memory: Record<string, number>; exit?: string; error?: string;
};
const result = {
  date: new Date().toISOString(), binary, carries: mods.length, addons: addons.map(a => a.id), fresh,
  java: { title_s: 140, world_s: 58, launch_to_world_s: 200 }, launches: [] as Launch[], status: 'failed', error: '',
};
const save = () => writeFileSync(join(out, 'pack-start.json'), JSON.stringify(result, null, 2) + '\n');
console.log(`${mods.length} carries, ${addons.length} AE2 addons; profile ${profile}`);

/** One game process: spawn, the control socket, a memory sampler. */
class Game {
  readonly socket = join(tmpdir(), `pack-start-${process.pid}-${Date.now()}.sock`);
  readonly trace: string;
  readonly log: string;
  readonly started = performance.now();
  readonly samples: Sample[] = [];
  readonly child: ReturnType<typeof Bun.spawn>;
  private sampler: ReturnType<typeof Bun.spawn>;
  constructor(readonly launch: Launch, index: number) {
    this.trace = join(out, `trace-${index}.jsonl`);
    rmSync(this.trace, { force: true });
    const devMods = [content, ...addons.map(a => a.path), ...mods.map(id => join(carries, id))].flatMap(p => ['--dev-mod', p]);
    const env: Record<string, string> = {
      ...(process.env as Record<string, string>), HORIZON_HOME: game, HORIZON_LAUNCHER_HOME: join(profile, 'launcher'),
      XDG_DATA_HOME: join(profile, 'data'), XDG_CONFIG_HOME: join(profile, 'config'), HORIZON_LAUNCHER_NO_SELF_UPDATE: '1',
      HORIZON_TELEMETRY_URL: 'off', HORIZON_SERVER_THREAD: '1', HORIZON_PLAYER_START: '1', HORIZON_STARTUP_TRACE: this.trace,
    };
    delete env.HORIZON_NO_AUDIO;
    const log = (this.log = join(out, `game-${index}.log`));
    rmSync(log, { force: true });
    this.child = Bun.spawn(
      [binary, '--resources', resources, ...devMods, '--server-thread', '--name', 'PackStart', '--control-socket', this.socket,
        '--offscreen', '--no-pause'],
      { cwd: game, env, stdout: 'pipe', stderr: 'pipe' },
    );
    for (const stream of [this.child.stdout, this.child.stderr] as ReadableStream<Uint8Array>[]) {
      (async () => { for await (const bytes of stream) appendFileSync(log, bytes); })();
    }
    // Working set and private bytes of the game, and the machine's available memory, once a second.
    const ps = `$p = Get-Process -Id ${this.child.pid}; while (-not $p.HasExited) { $p.Refresh(); ` +
      `$f = [int]((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1KB); ` +
      `'{0},{1},{2}' -f [int]($p.WorkingSet64 / 1MB), [int]($p.PrivateMemorySize64 / 1MB), $f; Start-Sleep -Milliseconds 1000 }`;
    this.sampler = Bun.spawn(['powershell', '-NoProfile', '-Command', ps], { stdout: 'pipe', stderr: 'ignore' });
    (async () => {
      let rest = '';
      for await (const bytes of this.sampler.stdout as ReadableStream<Uint8Array>) {
        rest += new TextDecoder().decode(bytes);
        const lines = rest.split(/\r?\n/); rest = lines.pop() ?? '';
        for (const line of lines) {
          const [ws, priv, free] = line.split(',').map(Number);
          if (Number.isFinite(free)) this.samples.push({ t: this.now(), ws, priv, free });
        }
      }
    })();
  }
  now(): number { return Math.round(performance.now() - this.started) / 1000; }
  mark(step: string): number {
    const t = this.now(); this.launch.steps[step] = t; save();
    console.log(`${this.launch.label}: ${step} at ${t.toFixed(1)} s`);
    return t;
  }
  memoryNow(): string {
    const s = this.samples.at(-1);
    return s ? `working set ${s.ws} MB, private ${s.priv} MB, ${s.free} MB free` : 'no sample yet';
  }
  /** One request on the control socket (Windows: a TCP endpoint file and `auth TOKEN` first). */
  send(line: string, timeout = 60_000): Promise<any> {
    return new Promise((resolveReply, reject) => {
      let endpoint: { port: number; token: string } | undefined;
      try { endpoint = JSON.parse(readFileSync(this.socket, 'utf8')); }
      catch { if (process.platform === 'win32') { reject(new Error('ENOENT: no control endpoint yet')); return; } }
      let data = '', settled = false, authenticated = !endpoint;
      const socket = endpoint ? connect({ host: '127.0.0.1', port: endpoint.port }) : connect({ path: this.socket });
      const finish = (error?: unknown, reply?: any) => {
        if (settled) return; settled = true; clearTimeout(timer); socket.destroy();
        if (error) reject(error); else resolveReply(reply);
      };
      const timer = setTimeout(() => finish(new Error(`Control timeout: ${line}`)), timeout);
      socket.on('connect', () => socket.write(endpoint ? `auth ${endpoint.token}\n` : line + '\n'));
      socket.on('error', error => finish(error));
      socket.on('close', () => finish(new Error(`Control closed before reply: ${line}`)));
      socket.on('data', bytes => {
        data += bytes.toString(); if (!data.includes('\n')) return;
        if (!authenticated) {
          const end = data.indexOf('\n');
          if (!data.slice(0, end).includes('"ok"')) { finish(new Error(`Control refused the token: ${data.slice(0, end)}`)); return; }
          authenticated = true; data = data.slice(end + 1); socket.write(line + '\n');
          if (!data.includes('\n')) return;
        }
        try {
          const reply = JSON.parse(data.trim());
          appendFileSync(transcript, JSON.stringify({ launch: this.launch.label, t: this.now(), line, reply }).slice(0, 4000) + '\n');
          finish(reply.error ? new Error(reply.error) : undefined, reply);
        } catch (error) { finish(error); }
      });
    });
  }
  /** `read` again until `accept` takes its value; the game's death, the memory floor and the step timeout end it. */
  async until<T>(label: string, read: () => Promise<T>, accept: (value: T) => boolean): Promise<T> {
    const end = performance.now() + stepTimeout;
    let last: T | undefined, lastError: unknown;
    while (performance.now() < end) {
      if (this.child.exitCode !== null) throw new Error(`the game exited with ${this.child.exitCode} waiting for ${label}`);
      const free = Math.round(freemem() / 2 ** 20);
      if (free < floorMb) throw new Error(`free memory fell to ${free} MB waiting for ${label} (${this.memoryNow()})`);
      try { last = await read(); if (accept(last)) return last; }
      catch (error) {
        lastError = error;
        if (!/ENOENT|ECONNREFUSED|ECONNRESET|Control timeout|Control closed|no world open|loading/i.test(String(error))) throw error;
      }
      await Bun.sleep(250);
    }
    throw new Error(`${label}: timed out; last ${JSON.stringify(last) ?? String(lastError)}`.slice(0, 600));
  }
  async inWorld(step: string): Promise<void> {
    await this.until(step, () => this.send('status'),
      s => !!s.player && s.player.health > 0 && s.chunks >= 30 && s.server?.loaded !== false && s.screen !== 'title');
    this.mark(step);
  }
  async title(step: string): Promise<void> {
    await this.until(step, () => this.send('screen'), s => s.screen === 'title' && (s.buttons ?? []).some((b: any) => b.label === 'Singleplayer' && b.enabled !== false));
    this.mark(step);
  }
  /** Singleplayer, then the world list's entry for `worldName` and Play Selected World. */
  async playSaved(step: string): Promise<void> {
    await this.send(`click-button Singleplayer`);
    const ui = await this.until('the world list', () => this.send('ui'),
      u => (u.widgets ?? []).some((w: any) => String(w.label ?? '').includes(worldName)));
    writeFileSync(join(out, 'world-list-ui.json'), JSON.stringify(ui, null, 1));
    const entry = ui.widgets.find((w: any) => String(w.label ?? '').includes(worldName));
    await this.send(`click ${entry.id}`);
    await this.send(`click-button Play Selected World`);
    this.mark(step);
  }
  async quit(): Promise<void> {
    const asked = this.now();
    try { await this.send('quit'); } catch {}
    const timer = setTimeout(() => this.child.kill(), 600_000);
    const code = await this.child.exited;
    clearTimeout(timer);
    this.launch.steps.quit_s = Math.round((this.now() - asked) * 10) / 10;
    this.launch.exit = String(code);
  }
  /** The trace's marks (title shown, first frame in a world...) and its steps' ends, in seconds since the start. */
  readTrace(): void {
    if (!existsSync(this.trace)) return;
    for (const line of readFileSync(this.trace, 'utf8').split(/\r?\n/)) {
      let e: any; try { e = JSON.parse(line.replace(/,\s*$/, '')); } catch { continue; }
      const key = e.ph === 'i' ? e.name : e.ph === 'X' ? `${e.name}.end` : undefined;
      if (!key) continue;
      const t = Math.round((e.ph === 'X' ? e.ts + e.dur : e.ts) / 1e5) / 10;
      // A mark or step that comes again (the second world's first frame) keeps each time: name, name#2, ...
      let k = key, n = 1;
      while (k in this.launch.trace && n < 5) k = `${key}#${++n}`;
      if (!(k in this.launch.trace)) this.launch.trace[k] = t;
    }
  }
  close(): void {
    this.readTrace();
    try { this.sampler.kill(); } catch {}
    if (this.child.exitCode === null) this.child.kill();
    const ws = this.samples.map(s => s.ws), priv = this.samples.map(s => s.priv), free = this.samples.map(s => s.free);
    const at = (t?: number) => this.samples.find(s => t !== undefined && s.t >= t);
    const world = at(this.launch.steps.in_world ?? this.launch.steps.reopened_in_world);
    Object.assign(this.launch.memory, {
      peak_working_set_mb: Math.max(0, ...ws), peak_private_mb: Math.max(0, ...priv), lowest_free_mb: Math.min(...free),
      free_at_start_mb: free[0] ?? 0, in_world_working_set_mb: world?.ws ?? 0, in_world_private_mb: world?.priv ?? 0,
    });
    writeFileSync(join(out, `memory-${this.launch.label}.csv`), 'seconds,working_set_mb,private_mb,free_mb\n' +
      this.samples.map(s => `${s.t},${s.ws},${s.priv},${s.free}`).join('\n') + '\n');
    save();
  }
}

async function run(label: string, index: number, create: boolean): Promise<void> {
  const cache = join(game, '.horizon-cache');
  const launch: Launch = {
    label, content_root: existsSync(cache) && readdirSync(cache).length ? 'reused' : 'built', steps: {}, trace: {}, memory: {},
  };
  result.launches.push(launch);
  const g = new Game(launch, index);
  try {
    await g.until('the control socket', () => g.send('screen'), s => !!s);
    g.mark('socket');
    await g.title('title');
    if (create) {
      await g.send('click-button Singleplayer');
      // With no worlds, Singleplayer opens Create New World itself.
      // Every title menu is the `title` screen; its `menu` says which one is open.
      const menu = await g.until('the world screen', () => g.send('screen'), s => !!s.menu?.mode);
      if (menu.menu?.mode !== 'create-world') await g.send('click-button Create New World');
      await g.until('Create New World', () => g.send('screen'), s => s.menu?.mode === 'create-world');
      await g.send(`type --field name --replace ${worldName}`);
      await g.send('click-button Create New World');
      g.mark('create_clicked');
      await g.inWorld('in_world');
      await Bun.sleep(10_000);
      // The pause menu's Save and Quit to Title, then the saved world from the title screen.
      await g.send('pause');
      await g.until('the pause menu', () => g.send('screen'), s => s.screen === 'paused' || (s.buttons ?? []).some((b: any) => /Quit to Title/.test(b.label)));
      const button = ((await g.send('screen')).buttons ?? []).find((b: any) => /Quit to Title|Disconnect/.test(b.label));
      await g.send(`click-button ${button?.label ?? 'Save and Quit to Title'}`);
      g.mark('quit_to_title_clicked');
      await g.title('back_on_title');
      await g.playSaved('reopen_clicked');
      await g.inWorld('reopened_in_world');
      // A content root's `horizon-data.bin` is written behind the first world (30 s after it shows): a player's
      // session lasts that long, and the next start reads one file for the root's 600,000. Quit once it is there.
      // It is not what this test proves: after PACK_START_DATA_WAIT_S (default 300) the run goes on without it.
      const end = performance.now() + Number(process.env.PACK_START_DATA_WAIT_S ?? 300) * 1000;
      while (/content root built/.test(readFileSync(g.log, 'utf8')) && performance.now() < end && g.child.exitCode === null) {
        if (/wrote horizon-data\.bin|horizon-data\.bin not written/.test(readFileSync(g.log, 'utf8'))) { g.mark('data_file_written'); break; }
        await Bun.sleep(1000);
      }
    } else {
      await g.playSaved('play_clicked');
      await g.inWorld('in_world');
    }
    await Bun.sleep(10_000);
    await g.quit();
    if (launch.exit !== '0') throw new Error(`the game exited with ${launch.exit} after quit`);
  } catch (error) {
    launch.error = String(error);
    throw error;
  } finally {
    g.close();
  }
}

try {
  await run('first', 1, true);
  if (!once) await run('second', 2, false);
  result.status = 'passed';
} catch (error) {
  result.error = String(error);
  console.error(`FAILED: ${error}`);
} finally {
  save();
  for (const l of result.launches) {
    console.log(`${l.label} (content root ${l.content_root}): ${JSON.stringify(l.steps)}`);
    console.log(`  trace: ${Object.entries(l.trace).filter(([k]) => /^(core|rest|renderer\.world|server\.(wait_rest|open_world)|world\.prepare)\.end|\.(shown|opening|first_frame|view_full)(#|$)/.test(k)).map(([k, v]) => `${k} ${v}`).join(', ')}`);
    console.log(`  memory: ${JSON.stringify(l.memory)}`);
  }
  console.log(`${result.status.toUpperCase()}: ${join(out, 'pack-start.json')}`);
}
process.exit(result.status === 'passed' ? 0 : 1);
