#!/bin/bash
# carry-with-facts.sh [batch...]: carries each oracle batch's mods ($ATM10_WORK/oracle/<batch>.txt, from batches.py)
# with that batch's facts from the pack run ($ATM10_DATA/dump/facts-<batch>.json, copied to oracle/facts.json; one
# file for all would be ~1.5 GB to load) and the pack's own facts (dump/pack-facts.json: fuel, compost, pistons, light).
# Every key of all-keys.txt gets its mods/<key>/carry folder first, so each batch sees all the others as ported (the
# conditions of worldgen modifiers). The mods sharing a namespace no pack mod owns are one batch (`shared`), so one
# run sees all their claims and leaves the namespace to none of them. Logs: carry-<batch>.log.
# Settings: ATM10_WORK, ATM10_DATA, HORIZON_PORT_PACK (the tool's exe), HORIZON_RESOURCES (prepared 26.3).
: "${ATM10_WORK:?}" "${ATM10_DATA:?}" "${HORIZON_PORT_PACK:?}" "${HORIZON_RESOURCES:?}"
cd "$ATM10_WORK" || exit 1
cp "$ATM10_DATA/dump/pack-facts.json" oracle/pack-facts.json || exit 1
# Lists written on Windows may end their lines with CR.
keys() { tr -d '\015' < "$1"; }
for k in $(keys all-keys.txt); do mkdir -p "mods/$k/carry"; done
batches="$*"
[ -z "$batches" ] && batches="$(cd oracle && ls b??.txt | sed 's/[.]txt$//') shared"
for b in $batches; do
  cp "$ATM10_DATA/dump/facts-$b.json" oracle/facts.json || exit 1
  s=$(date +%s)
  "$HORIZON_PORT_PACK" carry "$ATM10_WORK" --resources "$HORIZON_RESOURCES" $(keys "oracle/$b.txt") > "carry-$b.log" 2>&1
  echo "$b: exit $? in $(( $(date +%s) - s )) s: $(tail -1 "carry-$b.log" | cut -c1-80)"
done
rm -f oracle/facts.json
echo "namespaces left to none: $(grep -h 'claimed by' carry-*.log | wc -l)"
