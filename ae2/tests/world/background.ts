/** A host-wide rendering lease of AE2_RENDER_SLOTS slots (default 1: one hidden game at a time, as the lease was made
 * for a shared laptop). Slot 0 is the lease file older harnesses take, so they still exclude a default run; slot N > 0
 * is `.N` beside it. No child process inherits a lease descriptor. */
import { dlopen, ptr } from 'bun:ffi';
import { closeSync, ftruncateSync, openSync, readFileSync, rmSync, statSync, writeSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** How many hidden games may render at once on this host: AE2_RENDER_SLOTS, 1 when unset. */
export function renderSlots(): number {
  const slots = Number(process.env.AE2_RENDER_SLOTS ?? 1);
  if (!Number.isInteger(slots) || slots < 1 || slots > 64) throw new Error(`AE2_RENDER_SLOTS must be a whole number from 1 to 64, not ${process.env.AE2_RENDER_SLOTS}`);
  return slots;
}
const slotPath = (owner: string | number, slot: number) => join(tmpdir(), `ae2-horizon-render-${owner}${slot ? `.${slot}` : ''}.lock`);
const holderLine = () => JSON.stringify({ pid: process.pid, acquired_at: new Date().toISOString() }) + '\n';

export async function acquireRenderLease(): Promise<() => void> {
  if (process.platform === 'win32') return acquireWindowsLease();
  if (process.platform !== 'darwin' && process.platform !== 'linux') throw new Error('The hidden world harness supports macOS, Linux and Windows only.');
  const libc = dlopen(process.platform === 'darwin' ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6', {
    flock: { args: ['i32', 'i32'], returns: 'i32' },
    fcntl: { args: ['i32', 'i32', 'i32'], returns: 'i32' },
  });
  const slots = renderSlots();
  try {
    for (;;) {
      for (let slot = 0; slot < slots; slot++) {
        const fd = openSync(slotPath(process.getuid?.() ?? 'user', slot), 'a+', 0o600);
        if (libc.symbols.fcntl(fd, 2, 1) !== 0) { closeSync(fd); throw new Error('Cannot make the render lease close on exec.'); }
        if (libc.symbols.flock(fd, 6) !== 0) { closeSync(fd); continue; }
        try { ftruncateSync(fd, 0); writeSync(fd, holderLine()); } catch (error) { closeSync(fd); throw error; }
        let released = false;
        return () => { if (!released) { released = true; closeSync(fd); libc.close(); } };
      }
      await Bun.sleep(250);
    }
  } catch (error) { libc.close(); throw error; }
}

// Windows has no flock: a slot is a file created exclusively, taken over
// only when the process that holds it is gone.
async function acquireWindowsLease(): Promise<() => void> {
  const slots = renderSlots();
  for (;;) {
    for (let slot = 0; slot < slots; slot++) {
      const path = slotPath(process.env.USERNAME ?? 'user', slot);
      try {
        const fd = openSync(path, 'wx', 0o600);
        writeSync(fd, holderLine());
        closeSync(fd);
        let released = false;
        return () => { if (!released) { released = true; rmSync(path, { force: true }); } };
      } catch (error: any) {
        if (error?.code !== 'EEXIST') throw error;
        let text = '', holder = 0;
        try { text = readFileSync(path, 'utf8'); holder = JSON.parse(text).pid; } catch {}
        // A file with no holder yet is one being written, unless it has been empty for a while.
        let alive = !holder && (() => { try { return Date.now() - statSync(path).mtimeMs < 5000; } catch { return false; } })();
        if (holder) try { process.kill(holder, 0); alive = true; } catch {}
        // Taken over only if it still names the same gone holder: another waiter may have taken it over just now.
        if (!alive) { let again = ''; try { again = readFileSync(path, 'utf8'); } catch {} if (again === text) rmSync(path, { force: true }); slot--; }
      }
    }
    await Bun.sleep(250);
  }
}

// Match Rust Instant on Darwin: mach_absolute_time excludes system sleep.
// A closed lid may pause a test, but must not consume its active-time budget.
const awakeClock = (() => {
  if (process.platform !== 'darwin') return () => performance.now();
  const lib = dlopen('/usr/lib/libSystem.B.dylib', {
    mach_absolute_time: { args: [], returns: 'u64' },
    mach_timebase_info: { args: ['ptr'], returns: 'i32' },
  });
  const info = new Uint32Array(2);
  if (lib.symbols.mach_timebase_info(ptr(info)) !== 0 || !info[1]) throw new Error('Cannot read Darwin awake clock');
  const millisecondsPerTick = info[0] / info[1] / 1e6;
  return () => Number(lib.symbols.mach_absolute_time()) * millisecondsPerTick;
})();
export function awakeNowMs(): number { return awakeClock(); }
export function awakeTimeout(callback: () => void, milliseconds: number): () => void {
  const end = awakeNowMs() + milliseconds;
  let active = true;
  const timer = setInterval(() => {
    if (active && awakeNowMs() >= end) { active = false; clearInterval(timer); callback(); }
  }, Math.min(250, milliseconds));
  return () => { active = false; clearInterval(timer); };
}
