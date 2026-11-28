#!/bin/bash
# Sends one command to a running macOS/Windows-build rippl started with
# RIPPL_CONTROL=<port> (default 7788), and prints its answer.
#   scripts/dctl.sh tabs
#   RIPPL_CONTROL=7799 scripts/dctl.sh go example.com
port="${RIPPL_CONTROL:-7788}"
exec 3<>"/dev/tcp/127.0.0.1/$port" || exit 1
printf '%s\n' "$*" >&3
IFS= read -r -t 15 line <&3
printf '%s\n' "$line"
