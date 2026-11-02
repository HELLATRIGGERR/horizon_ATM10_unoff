#!/bin/sh
# oracle-batches.sh <batch>...: for each oracle/<batch>.txt (mod keys), copy the jars and their libraries
# (tools/oracle_batch.py), run the NeoForge oracle (the worktree's tools/neoforge_oracle.py) into
# oracle/facts-<batch>.json, and delete the copies. One line per batch.
source /e/PROJECTS/horizon-ae2/env.sh
W='E:/PROJECTS/horizon-ae2/ports/atm10'
cd /e/PROJECTS/horizon-ae2/ports/atm10
for b in "$@"; do
  until [ "$(tasklist | grep -c rustc.exe)" -lt 2 ]; do sleep 30; done
  python -I tools/oracle_batch.py "$b" $(cat oracle/$b.txt) > oracle/$b.prep.log 2>&1
  s=$(date +%s)
  (cd /e/PROJECTS/horizon-ae2/wt-portpack && HORIZON_NEOFORGE_HOME='E:\PROJECTS\horizon-ae2\oracle\server' \
    HORIZON_ORACLE_JAVA='C:\Users\mpoka\curseforge\minecraft\Install\java\java-runtime-delta\bin\java.exe' \
    HORIZON_ORACLE_XMX=${XMX:-3G} python tools/neoforge_oracle.py export --jars "$W/oracle/$b/jars" \
    --deps "$W/oracle/$b/deps" --out "$W/oracle/facts-$b.json") > oracle/$b.log 2>&1
  echo "$b: exit $? in $(( $(date +%s) - s )) s: $(tail -1 oracle/$b.log | cut -c1-120)"
  cp /e/PROJECTS/horizon-ae2/wt-portpack/.local/neoforge-oracle/runs/export/server.log oracle/$b.server.log 2>/dev/null
  rm -rf oracle/$b
done
