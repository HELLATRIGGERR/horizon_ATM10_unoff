// How the pack scripts' stage step applies the bundles to upstream's engine.
//
// horizon-mixins follows ordinary `mod` declarations, and upstream declares the `game/` modules through generated
// OUT_DIR includes, so (as the launcher's `prepare_mixins_source` does) the bundles are applied to a prepared copy of
// the checkout with those lists written out (pack/scripts/materialize). horizon-mixins also resolves a bundle's
// `hook = "<name>"` and `hook_version` through `native-hooks.json` at that copy's root. Upstream retired the file
// (its hooks are declared in crates/horizon-mod-vocab/src/hooks) and its build refuses one at the root, so the fork
// keeps the catalog at tools/fork/native-hooks.json (`python3 tools/fork/hook_catalog.py` writes it from the
// declarations), puts it in the prepared copy only and leaves it out of the stage.
//
// HORIZON_MIXINS must be built at upstream's pin (crates/horizon-mod-package/src/mixins_tool.rs: `REV` of
// https://github.com/horizon-rust/horizon-mixins with tools/mixins/narrow-reference-rule.patch applied,
// `cargo build --release -p horizon-mixins-cli`): the fork's older build refuses upstream's `#[path = "../.."]` modules.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';

export const CATALOG = 'tools/fork/native-hooks.json';
const AT_ROOT = 'native-hooks.json';
/** What the prepared copy leaves out: Git data, build output, the game's prepared resources and world tests. */
const LEFT_OUT = new Set(['.git', 'target', '.local', 'node_modules']);

function files(dir: string, under = dir, out: string[] = []): string[] {
  for (const entry of readdirSync(under, { withFileTypes: true })) {
    if (under === dir && LEFT_OUT.has(entry.name)) continue;
    if (entry.name === 'target' && existsSync(join(under, 'Cargo.toml'))) continue;
    const path = join(under, entry.name);
    if (entry.isDirectory()) files(dir, path, out);
    else if (entry.isFile()) out.push(relative(dir, path));
  }
  return out;
}

/** Makes `to` hold `from`'s files, writing only those that differ (the materialized ones always do). */
function sync(from: string, to: string): void {
  const fresh = new Set(files(from));
  for (const file of fresh) {
    const source = join(from, file), dest = join(to, file);
    if (existsSync(dest) && statSync(dest).size === statSync(source).size && readFileSync(dest).equals(readFileSync(source))) continue;
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(source, dest);
  }
  if (existsSync(to)) for (const file of files(to)) if (!fresh.has(file)) rmSync(join(to, file));
}

/**
 * horizon-mixins refuses a `#[path]` with `..` (E_INVALID_PATH), which upstream's game uses
 * (`game/mobs/creatures/mod.rs` reaches `game/creatures/*.rs`, tests reach fixtures). In the prepared copy each such
 * module file is copied beside its declaration and the attribute names the copy: the same source, one module, as
 * rustc reads the original (the reference's rules: an outer `mod` from the file's directory; one inside an inline
 * `mod m { }` from `<dir>[/<file stem>]/m`). Returns what it changed, for `restoreParentPaths`.
 */
interface Relocated { file: string; target: string; copy: string }
function relocateParentPaths(prepared: string): Relocated[] {
  const src = join(prepared, 'crates/horizon-client/src');
  const attribute = /^(\s*)#\[path = "([^"]*\.\.[^"]*)"\]/;
  const moved: Relocated[] = [];
  for (const file of files(src).filter(f => f.endsWith('.rs'))) {
    const path = join(src, file);
    const lines = readFileSync(path, 'utf8').split('\n');
    let changed = false;
    lines.forEach((line, i) => {
      const m = attribute.exec(line);
      if (!m) return;
      const [, indent, target] = m;
      let base = dirname(path);
      if (indent.length > 0) {
        const stem = file.split(/[\\/]/).pop()!.replace(/\.rs$/, '');
        const modRs = ['mod', 'lib', 'main'].includes(stem);
        let inline: string | undefined;
        for (let j = i - 1; j >= 0 && !inline; j--) {
          const open = /^(\s*)(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*\{/.exec(lines[j]);
          if (open && open[1].length < indent.length) inline = open[2];
        }
        if (!inline) throw new Error(`${file}:${i + 1}: #[path] in an inline module whose name is not found`);
        base = join(base, ...(modRs ? [] : [stem]), inline);
      }
      const resolved = join(base, target);
      if (!existsSync(resolved)) throw new Error(`${file}:${i + 1}: #[path = "${target}"] names no file (${resolved})`);
      const copy = `fork_up_${relative(prepared, resolved).replace(/[^A-Za-z0-9]+/g, '_')}.rs`;
      mkdirSync(base, { recursive: true });
      copyFileSync(resolved, join(base, copy));
      lines[i] = line.replace(target, copy);
      changed = true;
      moved.push({ file: join('crates/horizon-client/src', file), target, copy: relative(prepared, join(base, copy)) });
    });
    if (changed) writeFileSync(path, lines.join('\n'));
  }
  return moved;
}

/** Puts the original `#[path]`s back in the applied output and drops the copies: the game's build discovers every
 * file under `game/` as a module, so a copy left there would be a second one. */
function restoreParentPaths(out: string, moved: Relocated[]): void {
  for (const { file, target, copy } of moved) {
    const path = join(out, file);
    const name = copy.split(/[\\/]/).pop()!;
    const text = readFileSync(path, 'utf8');
    if (!text.includes(`#[path = "${name}"]`)) throw new Error(`${file}: the applied copy lost #[path = "${name}"]`);
    writeFileSync(path, text.replace(`#[path = "${name}"]`, `#[path = "${target}"]`));
    rmSync(join(out, copy), { force: true });
  }
}

/**
 * `horizon-mixins apply --mods mods --out out` on `root`: through a prepared copy of it beside `out`
 * (`<out>-workspace`), with the game's module lists written out and the fork's catalog at its root.
 * `toolTarget` is the Cargo target directory for the materializing tool.
 */
export async function applyBundles(run: (command: string[]) => Promise<void>, mixins: string, root: string, mods: string, out: string, toolTarget: string): Promise<void> {
  if (existsSync(join(root, AT_ROOT))) throw new Error(`${join(root, AT_ROOT)} exists: upstream's build refuses it; the catalog lives at ${CATALOG}`);
  const prepared = `${out}-workspace`;
  sync(root, prepared);
  await run(['cargo', 'run', '--quiet', '--offline', '--manifest-path', join(root, 'pack/scripts/materialize/Cargo.toml'), '--target-dir', toolTarget, '--', prepared]);
  const moved = relocateParentPaths(prepared);
  console.log(`${moved.length} #[path] module(s) with '..' copied beside their declarations for horizon-mixins`);
  copyFileSync(join(root, CATALOG), join(prepared, AT_ROOT));
  await run([mixins, 'apply', '--workspace', prepared, '--mods', mods, '--out', out]);
  rmSync(join(out, AT_ROOT), { force: true });
  restoreParentPaths(out, moved);
}

/** The game build's environment: a stage is no Git checkout, so horizon-mod-host's build script takes the build id
 * from HORIZON_GAME_BUILD_ID, here the checkout's HEAD with `-staged` (unless it is set already). */
export function gameBuildEnv(root: string, env: Record<string, string | undefined>): Record<string, string | undefined> {
  if (env.HORIZON_GAME_BUILD_ID) return env;
  const head = Bun.spawnSync(['git', 'rev-parse', 'HEAD'], { cwd: root }).stdout.toString().trim();
  return { ...env, HORIZON_GAME_BUILD_ID: `${head || 'unknown'}-staged` };
}

/** The game build's Cargo arguments: upstream's game package with the features a script names (none by default). */
export function gameBuild(features: string | undefined): string[] {
  const list = (features ?? '').split(/[ ,]+/).filter(Boolean);
  return ['-p', 'horizon-game', '--no-default-features', ...(list.length ? ['--features', list.join(',')] : []), '--bins'];
}
