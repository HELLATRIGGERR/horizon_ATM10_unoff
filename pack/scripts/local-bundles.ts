// Bundles that live outside this repository, for the pack scripts' stage step. HORIZON_LOCAL_BUNDLES lists directories
// separated by `;`: each one is a bundle (it holds horizon-mixin.toml) or a folder of bundles. A bundle whose licence
// does not let it be published is kept on the machine that has it and staged from there, beside the bundles of
// pack/mods. With the variable unset, nothing is added and the scripts stage what they always did.
import { cpSync, existsSync, readdirSync, rmSync } from 'node:fs';
import { basename, join, relative, resolve, sep } from 'node:path';

/** What a staged copy leaves out: the world tests, a local repository's Git data, build output, crate locks. */
const LEFT_OUT = new Set(['tests', '.git', 'target', 'Cargo.lock']);

/** The bundle directories HORIZON_LOCAL_BUNDLES names, in its order; none when it is unset. */
export function localBundles(): string[] {
  const dirs = (process.env.HORIZON_LOCAL_BUNDLES ?? '').split(';').map(dir => dir.trim()).filter(Boolean);
  const bundles: string[] = [];
  for (const dir of dirs.map(dir => resolve(dir))) {
    if (!existsSync(dir)) throw new Error(`HORIZON_LOCAL_BUNDLES: no directory ${dir}`);
    if (existsSync(join(dir, 'horizon-mixin.toml'))) { bundles.push(dir); continue; }
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (entry.isDirectory() && existsSync(join(dir, entry.name, 'horizon-mixin.toml'))) bundles.push(join(dir, entry.name));
    }
  }
  return bundles;
}

function copyBundle(from: string, to: string): void {
  cpSync(from, to, { recursive: true, filter: path => !relative(from, path).split(sep).some(part => LEFT_OUT.has(part)) });
}

/** Copies the local bundles into `mods`, a folder of bundles being prepared for `horizon-mixins apply --mods`. */
export function addLocalBundles(mods: string): string[] {
  const bundles = localBundles();
  for (const bundle of bundles) {
    const to = join(mods, basename(bundle));
    if (existsSync(to)) throw new Error(`HORIZON_LOCAL_BUNDLES: ${basename(bundle)} is already a bundle of ${mods}`);
    copyBundle(bundle, to);
    console.log(`local bundle ${basename(bundle)} from ${bundle}`);
  }
  return bundles.map(bundle => basename(bundle));
}

/** The folder to pass as `--mods`: `mods` itself, or with local bundles, `scratch` made to hold `mods`'s and theirs. */
export function withLocalBundles(mods: string, scratch: string): string {
  if (localBundles().length === 0) return mods;
  rmSync(scratch, { recursive: true, force: true });
  for (const entry of readdirSync(mods, { withFileTypes: true })) {
    if (entry.isDirectory()) copyBundle(join(mods, entry.name), join(scratch, entry.name));
  }
  addLocalBundles(scratch);
  return scratch;
}
