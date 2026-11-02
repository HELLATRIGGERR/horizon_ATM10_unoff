#!/bin/sh
# carry-with-facts.sh <batch>...: re-carries each oracle batch's mods (oracle/<batch>.txt) with that batch's facts
# (oracle/facts-<batch>.json.gz unpacked to oracle/facts.json; one file for all would be ~0.5 GB to load), then the
# mods that share a namespace no pack mod owns (quark, thermal, xycraft) together, with their batches' facts
# merged, so the namespace stays with none of them as in one full run. Logs: carry-facts-<batch>.log.
PP='E:\PROJECTS\horizon-ae2\.build\target-portpack\release\horizon-port-pack.exe'
# ATM10_WORK: another work folder (with inventory.json, oracle/, tools/merge-facts.py, mods/<key>/carry), so a re-carry
# need not touch this one.
W="${ATM10_WORK:-E:\PROJECTS\horizon-ae2\ports\atm10}"
R='E:\PROJECTS\horizon-ae2\horizon\.local\resources\26.3'
SHARED="amendments create farmingforblockheads supplementaries dyenamics xycraft_world xycraft_machines"
cd "$(cygpath -u "$W")"
for b in "$@"; do
  keys=$(for k in $(cat oracle/$b.txt); do case " $SHARED " in *" $k "*) ;; *) case $k in +*) ;; *) echo $k ;; esac ;; esac; done)
  gzip -dc oracle/facts-$b.json.gz > oracle/facts.json
  s=$(date +%s)
  "$PP" carry "$W" --resources "$R" $keys > carry-facts-$b.log 2>&1
  echo "$b: exit $? in $(( $(date +%s) - s )) s: $(tail -1 carry-facts-$b.log | cut -c1-60)"
done
for b in n1 n2 n3 n4 n8; do gzip -dc oracle/facts-$b.json.gz > oracle/facts-$b.json; done
python tools/merge-facts.py facts-n1.json facts-n2.json facts-n3.json facts-n4.json facts-n8.json > /dev/null
rm -f oracle/facts-n?.json
"$PP" carry "$W" --resources "$R" $SHARED > carry-facts-shared.log 2>&1
echo "shared: exit $?: $(grep -c 'claimed by' carry-facts-shared.log) namespaces left to none"
rm -f oracle/facts.json
