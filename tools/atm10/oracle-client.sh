#!/bin/bash
# oracle-client.sh [--lock]: starts the whole of ATM10 on the NeoForge client (tools/neoforge_oracle.py client-pack, this
# checkout's), hidden, and writes what only a client holds of the pack's fluids (each fluid type's textures and tint, each
# fluid's render layer, each bucket's item colours) to $ATM10_DATA/dump/client-facts.json, beside oracle-pack.sh's facts
# (docs/mod-fluids.md). The game halts once its mods have loaded; its window is hidden, Crash Assistant (which opens a
# window of its own) is never loaded, and a Java process a mod left behind is stopped. The instance is only read.
#
# Settings (environment): ATM10_INSTANCE, ATM10_DATA as for oracle-pack.sh; HORIZON_NEOFORGE_CLIENT_HOME (where the
# NeoForge client of the instance's version is, or is installed once), HORIZON_ORACLE_JAVA (Java 21), PYTHON (3.11 or
# later). XMX is the heap (default 8G: the client bakes every model before its mods finish loading), JVM more options.
# With --lock it holds $ATM10_LOCK for the run; the run stops itself after TIMEOUT seconds (1800).
: "${ATM10_INSTANCE:?}" "${ATM10_DATA:?}" "${HORIZON_NEOFORGE_CLIENT_HOME:?}" "${HORIZON_ORACLE_JAVA:?}"
if [ "$1" = "--lock" ]; then
  LOCK="${ATM10_LOCK:?}"
  if ! (set -o noclobber; echo "oracle-client $$" > "$LOCK") 2> /dev/null; then
    echo "the lock is held: $(cat "$LOCK")"
    exit 1
  fi
  trap 'rm -f "$LOCK"' EXIT
fi
s=$(date +%s)
cd "$(dirname "$0")/../.." || exit 1
HORIZON_ORACLE_JVM="${JVM:--XX:+UseG1GC -XX:MaxGCPauseMillis=200}" \
HORIZON_ORACLE_XMX=${XMX:-8G} "${PYTHON:-python}" -I tools/neoforge_oracle.py client-pack \
  --instance "$ATM10_INSTANCE" --run "$ATM10_DATA/oracle-client-run" --out "$ATM10_DATA/dump/client-facts.json" \
  --timeout "${TIMEOUT:-1800}" 2>&1 | tail -45 | cut -c1-300
echo "exit ${PIPESTATUS[0]} after $(( $(date +%s) - s )) s"
