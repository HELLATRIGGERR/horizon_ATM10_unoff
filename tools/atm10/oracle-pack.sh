#!/bin/bash
# oracle-pack.sh [--lock]: starts the whole of ATM10 on NeoForge (tools/neoforge_oracle.py pack, this checkout's) and
# writes the final recipes, tags, loot tables and pack facts to $ATM10_DATA/dump, with each oracle batch's facts
# ($ATM10_WORK/oracle/batches.txt, from batches.py) beside them as facts-<batch>.json. KubeJS and Almost Unified are on.
# The instance is only read.
#
# Settings (environment): ATM10_INSTANCE (the CurseForge instance), ATM10_WORK (the port-pack work folder),
# ATM10_DATA (generated data: run folder and dump), HORIZON_NEOFORGE_HOME (an installed NeoForge server, the
# instance's version), HORIZON_ORACLE_JAVA (Java 21: NeoForge 1.21.1 does not run on 25; CurseForge ships one under
# Install\java\Jre_21), PYTHON (3.11 or later). XMX is the heap (default 5G), JVM more options: the serial collector
# and C1 cost the least memory beside the heap (a G1 run died with "the paging file is too small"). With --lock it
# holds $ATM10_LOCK for the run, which keeps other builds back; the run stops itself after TIMEOUT seconds (2600).
: "${ATM10_INSTANCE:?}" "${ATM10_WORK:?}" "${ATM10_DATA:?}" "${HORIZON_NEOFORGE_HOME:?}" "${HORIZON_ORACLE_JAVA:?}"
if [ "$1" = "--lock" ]; then
  LOCK="${ATM10_LOCK:?}"
  if ! (set -o noclobber; echo "oracle-pack $$" > "$LOCK") 2> /dev/null; then
    echo "the lock is held: $(cat "$LOCK")"
    exit 1
  fi
  trap 'rm -f "$LOCK"' EXIT
fi
s=$(date +%s)
cd "$(dirname "$0")/../.." || exit 1
batches=()
[ -f "$ATM10_WORK/oracle/batches.txt" ] && batches=(--batches "$ATM10_WORK/oracle/batches.txt")
HORIZON_ORACLE_JVM="${JVM:--XX:+UseSerialGC -XX:TieredStopAtLevel=1}" \
HORIZON_ORACLE_XMX=${XMX:-5G} "${PYTHON:-python}" -I tools/neoforge_oracle.py pack \
  --instance "$ATM10_INSTANCE" --run "$ATM10_DATA/oracle-run" --out "$ATM10_DATA/dump" \
  --leave-out "$ATM10_WORK/oracle/pack-leave-out.txt" "${batches[@]}" \
  --namespaces kubejs --timeout "${TIMEOUT:-2600}" 2>&1 | tail -45 | cut -c1-300
echo "exit ${PIPESTATUS[0]} after $(( $(date +%s) - s )) s"
