#!/bin/bash
# Sends one command to a rippl started with RIPPL_CONTROL=1 (see app/src/control.rs).
#   scripts/ctl.sh list | open example.com | select 2 | close 2 | sleep
#   RIPPL_PROFILE=work scripts/ctl.sh list     # a profile's rippl
exec python3 -c '
import os, socket, sys
s = socket.socket(socket.AF_UNIX)
p = os.environ.get("RIPPL_PROFILE")
s.connect(os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), f"rippl-{p}.sock" if p else "rippl.sock"))
s.sendall((" ".join(sys.argv[1:]) + "\n").encode())
print(s.makefile().read(), end="")
' "$@"
