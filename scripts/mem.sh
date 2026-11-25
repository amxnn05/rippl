#!/bin/bash
# What rippl costs right now: PSS (proportional set size, shared pages split
# fairly between the processes using them) of the rippl process and every
# WebKit process it started, in MB. PSS is the honest number for comparing
# browsers; RSS double-counts shared libraries.
#
#   scripts/mem.sh              rippl
#   scripts/mem.sh firefox      any other browser, by process name, to compare
#   scripts/mem.sh --pid 1234   one process and everything it started (use
#                               this to measure a browser you launched
#                               yourself next to one already running)
set -euo pipefail
if [ "${1:-}" = "--pid" ]; then
  root="${2:?--pid needs a process id}"
  [ -d "/proc/$root" ] || { echo "no process $root" >&2; exit 1; }
else
  name="${1:-rippl}"
  root=$(pgrep -x -o "$name" || true)
  [ -z "$root" ] && { echo "no $name running" >&2; exit 1; }
fi

# The root and all its descendants.
pids="$root"
frontier="$root"
while [ -n "$frontier" ]; do
  next=""
  for p in $frontier; do
    kids=$(pgrep -P "$p" || true)
    next="$next $kids"
  done
  frontier=$(echo $next)
  pids="$pids $frontier"
done

total=0
printf "%-8s %-28s %8s\n" PID PROCESS "PSS MB"
for p in $pids; do
  f=/proc/$p/smaps_rollup
  [ -r "$f" ] || continue
  kb=$(awk '/^Pss:/ {print $2}' "$f")
  comm=$(cat /proc/$p/comm)
  printf "%-8s %-28s %8.1f\n" "$p" "$comm" "$(awk "BEGIN{print $kb/1024}")"
  total=$((total + kb))
done
printf "%-37s %8.1f\n" "TOTAL" "$(awk "BEGIN{print $total/1024}")"
