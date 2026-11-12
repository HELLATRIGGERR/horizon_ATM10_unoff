/** Runs world suites, each its own `bun run` of its file, `jobs` at a time. */
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { root } from './common';

export type Suite = { name: string; command: string[] };

/** `--jobs N` in `args` (removed from them): the suites run at once, 1 when absent. */
export function takeJobs(args: string[]): number {
  const at = args.indexOf('--jobs');
  if (at < 0) return 1;
  const jobs = Number(args[at + 1]);
  if (!Number.isInteger(jobs) || jobs < 1 || jobs > 16) throw new Error('--jobs needs a whole number from 1 to 16');
  args.splice(at, 2);
  return jobs;
}

/** How long each suite took when it last ran, beside the run folders. */
const timesFile = join(root, '.local/world-tests/suite-seconds.json');
function lastTimes(): Record<string, number> {
  try { return JSON.parse(readFileSync(timesFile, 'utf8')); } catch { return {}; }
}
function saveTimes(times: Record<string, number>): void {
  try {
    mkdirSync(join(root, '.local/world-tests'), { recursive: true });
    const tmp = `${timesFile}.${process.pid}`;
    writeFileSync(tmp, JSON.stringify({ ...lastTimes(), ...times }, null, 1) + '\n'); renameSync(tmp, timesFile);
  } catch {}
}

/** The names of the suites that failed. One job runs them in order with the terminal's streams, as before. More run
 * the longest known first; each one's output is printed whole when it ends, so the logs stay one suite at a time, and
 * the render lease gets at least `jobs` slots (AE2_RENDER_SLOTS, when set higher, stays as it is). */
export async function runSuites(suites: Suite[], options: { jobs: number; cwd: string; env: Record<string, string | undefined> }): Promise<string[]> {
  const failed: string[] = [], times: Record<string, number> = {}, began = performance.now();
  const { jobs, cwd } = options;
  if (jobs === 1) {
    for (const suite of suites) {
      const start = performance.now();
      const child = Bun.spawn(suite.command, { cwd, env: options.env, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit' });
      const code = await child.exited;
      times[suite.name] = Math.round((performance.now() - start) / 100) / 10;
      if (code !== 0) failed.push(suite.name);
    }
  } else {
    const env = { ...options.env, AE2_RENDER_SLOTS: String(Math.max(jobs, Number(options.env.AE2_RENDER_SLOTS ?? 0) || 0)) };
    const known = lastTimes(), queue = [...suites].sort((a, b) => (known[b.name] ?? Infinity) - (known[a.name] ?? Infinity));
    const worker = async () => {
      for (let suite = queue.shift(); suite; suite = queue.shift()) {
        const start = performance.now(), chunks: Uint8Array[] = [];
        const child = Bun.spawn(suite.command, { cwd, env, stdin: 'ignore', stdout: 'pipe', stderr: 'pipe' });
        await Promise.all([child.stdout, child.stderr].map(async stream => { for await (const bytes of stream) chunks.push(bytes); }));
        const code = await child.exited;
        times[suite.name] = Math.round((performance.now() - start) / 100) / 10;
        process.stdout.write(Buffer.concat(chunks));
        if (code !== 0) failed.push(suite.name);
      }
    };
    await Promise.all(Array.from({ length: Math.min(jobs, suites.length) }, worker));
  }
  if (existsSync(join(root, '.local'))) saveTimes(times);
  console.log(`World suites: ${suites.length} run, ${failed.length} failed, ${Math.round((performance.now() - began) / 1000)} s with ${jobs} job${jobs === 1 ? '' : 's'}`);
  return failed;
}
