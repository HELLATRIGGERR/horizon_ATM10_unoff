#!/bin/bash
# oracle-pack.sh [--lock]: starts the whole of ATM10 on NeoForge (tools/neoforge_oracle.py pack, the worktree's) and
# writes the final recipes, tags and loot tables to C:\horizon-work\oracle-pack\dump. KubeJS and Almost Unified are on.
# The instance is only read. XMX is the heap (default 5G; 6G ran in 209 s), JVM more options: the
# serial collector and C1 cost the least memory beside the heap (a G1 run died with "the paging file is too small"). With --lock it holds C:\horizon-work\fullpack.lock for the
# run, which keeps other builds back; the run stops itself after TIMEOUT seconds (default 1500).
source /e/PROJECTS/horizon-ae2/env.sh > /dev/null
LOCK=/c/horizon-work/fullpack.lock
if [ "$1" = "--lock" ]; then
  if ! (set -o noclobber; echo "oracle-pack $$" > "$LOCK") 2> /dev/null; then
    echo "the lock is held: $(cat "$LOCK")"
    exit 1
  fi
  trap 'rm -f "$LOCK"' EXIT
fi
mkdir -p /c/horizon-work/oracle-pack
s=$(date +%s)
cd /e/PROJECTS/horizon-ae2/wt-portpack || exit 1
HORIZON_NEOFORGE_HOME='E:\PROJECTS\horizon-ae2\oracle\server' \
HORIZON_ORACLE_JAVA='C:\Users\mpoka\curseforge\minecraft\Install\java\java-runtime-delta\bin\java.exe' \
HORIZON_ORACLE_JVM="${JVM:--XX:+UseSerialGC -XX:TieredStopAtLevel=1}" \
HORIZON_ORACLE_XMX=${XMX:-5G} python tools/neoforge_oracle.py pack \
  --instance 'C:\Users\mpoka\curseforge\minecraft\Instances\All the Mods 10 - ATM10' \
  --run 'C:\horizon-work\oracle-pack\run' --out 'C:\horizon-work\oracle-pack\dump' \
  --leave-out 'E:\PROJECTS\horizon-ae2\ports\atm10\oracle\pack-leave-out.txt' \
  --namespaces kubejs --timeout "${TIMEOUT:-1500}" 2>&1 | tail -45 | cut -c1-260
echo "exit ${PIPESTATUS[0]} after $(( $(date +%s) - s )) s"
