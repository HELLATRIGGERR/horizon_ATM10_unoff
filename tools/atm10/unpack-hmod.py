r"""Unpacks <hmod dir>\<id>.hmod (from pack-hmod.py) into <out>\<id>\, a carry folder on a fast disk.

Why folders and not the .hmod: the game reads each block and item declaration of a .hmod through a new zip
reader (horizon-mods Content::file -> Content::reader -> ZipArchive::new), which parses the whole central
directory again, one read call per entry. For ATM10 that is about 865 million entries (chipped alone: 6,967
declarations x 57,709 entries); the full-pack start was still in it after 8 minutes. A folder costs one
metadata call per file instead, cheap on the internal SSD.

    python tools/unpack-hmod.py C:\horizon-work\atm10-hmod C:\horizon-work\atm10-carries [--jobs 6]
    play-atm10.ts --carries C:\horizon-work\atm10-carries
"""
import os
import shutil
import sys
import time
import zipfile
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path


def unpack(job):
    hmod, out = job
    target = Path(out) / Path(hmod).stem
    if target.exists() and (target / "horizon-mod.toml").stat().st_mtime >= Path(hmod).stat().st_mtime:
        return target.name, -1
    if target.exists():
        shutil.rmtree(target)
    with zipfile.ZipFile(hmod) as z:
        z.extractall(target)
        n = len(z.namelist())
    # Newer than the .hmod: unpacked again only when the .hmod changes.
    os.utime(target / "horizon-mod.toml")
    return target.name, n


def main():
    src, out = sys.argv[1], sys.argv[2]
    jobs = int(sys.argv[sys.argv.index("--jobs") + 1]) if "--jobs" in sys.argv else 6
    Path(out).mkdir(parents=True, exist_ok=True)
    start = time.time()
    hmods = sorted(str(p) for p in Path(src).glob("*.hmod"))
    total = 0
    with ProcessPoolExecutor(jobs) as ex:
        for mod, n in ex.map(unpack, [(h, out) for h in hmods]):
            total += max(n, 0)
    print(f"{len(hmods)} .hmod, {total} files unpacked in {time.time() - start:.0f} s -> {out}")


if __name__ == "__main__":
    main()
