import { engine, manifest, profile, run, help, packFeatures } from './common';
if (!help('Usage: bun run ae2/scripts/build.ts [additional cargo build flags]\nBuilds the native AE2 client (the horizon-atm10 package, whose program is horizon-client) with the Rust toolchain the Horizon launcher pins (1.99.1),\nwith the Cargo features of the mods pack/pack.toml turns on (AE2 and its addons).\nThe profile is `fast`; AE2_PROFILE=iter builds the `iter` profile instead (target/iter/).')) {
  const extra = process.argv.slice(2);
  const jobs = extra.some(arg => arg === '-j' || arg.startsWith('-j') || arg.startsWith('--jobs')) ? [] : ['-j', '3'];
  await run(['cargo', 'build', '--manifest-path', manifest, '--locked', '--profile', profile, '-p', 'horizon-atm10', '--no-default-features', '--features', packFeatures(), '--bins', ...jobs, ...extra], engine);
}
