"""Packs every carry (..\\mods\\<id>\\carry) into <out>\\<id>.hmod, as `cargo horizon-mod build` does:
a zip with horizon-mod.toml first, then the files of assets/ and data/ in sorted order, deflated.

Why: a folder mod costs the game one directory walk and one metadata call per file at every start
(Content::read, then the content root's stamp). For 287 carries that is about 586,000 files and several
minutes on the USB drive E: before the window shows anything. A .hmod is one zip index and one metadata call,
but the game (Horizon 62f4792c7) opens the zip again for every block and item declaration, which makes a
.hmod of a mod with thousands of blocks far slower than its folder (see unpack-hmod.py). So these .hmod files
are the compact copy on C: that unpack-hmod.py turns into folders, and the format to load once the game reads
a .hmod's declarations through one reader.

    python tools/pack-hmod.py C:\\horizon-work\\atm10-hmod [--only id,id] [--jobs 6]

A .hmod newer than its carry's horizon-mod.toml is kept; --force packs again.
play-atm10.ts --hmod <out> starts the game with them.
"""
import argparse
import os
import sys
import time
import zipfile
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent


def files_of(carry: Path) -> list[str]:
    out = []
    for top in ("assets", "data"):
        root = carry / top
        if not root.is_dir():
            continue
        for dirpath, _dirs, names in os.walk(root):
            rel = Path(dirpath).relative_to(carry).as_posix()
            out.extend(f"{rel}/{n}" for n in names)
    out.sort()
    return out


def pack(job: tuple[str, str, bool]) -> tuple[str, int, float, str]:
    mod, out_dir, force = job
    carry = HERE / "mods" / mod / "carry"
    target = Path(out_dir) / f"{mod}.hmod"
    manifest = carry / "horizon-mod.toml"
    if not force and target.exists() and target.stat().st_mtime > manifest.stat().st_mtime:
        return mod, -1, 0.0, "kept"
    start = time.time()
    names = files_of(carry)
    partial = target.with_suffix(".hmod.partial")
    with zipfile.ZipFile(partial, "w", zipfile.ZIP_DEFLATED, compresslevel=1) as z:
        z.write(manifest, "horizon-mod.toml")
        for name in names:
            z.write(carry / name, name)
    os.replace(partial, target)
    return mod, len(names), time.time() - start, f"{target.stat().st_size >> 20} MiB"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--only", default="")
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--force", action="store_true")
    a = ap.parse_args()
    leave_out = set()
    lo = HERE / "leave-out.txt"
    if lo.exists():
        for line in lo.read_text(encoding="utf-8").splitlines():
            line = line.split("#", 1)[0].strip()
            if line:
                leave_out.add(line)
    mods = sorted(
        d.name for d in (HERE / "mods").iterdir()
        if (d / "carry" / "horizon-mod.toml").is_file() and d.name not in leave_out
    )
    if a.only:
        wanted = {m.strip() for m in a.only.split(",") if m.strip()}
        mods = [m for m in mods if m in wanted]
    Path(a.out).mkdir(parents=True, exist_ok=True)
    start = time.time()
    total = 0
    with ProcessPoolExecutor(a.jobs) as ex:
        for mod, n, secs, note in ex.map(pack, [(m, a.out, a.force) for m in mods]):
            if n >= 0:
                total += n
                print(f"{mod}: {n} files, {secs:.1f} s, {note}", flush=True)
    print(f"{len(mods)} mods, {total} files packed in {time.time() - start:.0f} s -> {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
