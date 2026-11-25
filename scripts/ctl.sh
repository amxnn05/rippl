#!/bin/bash
# Sends one command to a rippl started with RIPPL_CONTROL=1 (see app/src/control.rs).
#   scripts/ctl.sh list | open example.com | select 2 | close 2 | sleep
exec python3 -c '
import os, socket, sys
s = socket.socket(socket.AF_UNIX)
s.connect(os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "rippl.sock"))
s.sendall((" ".join(sys.argv[1:]) + "\n").encode())
print(s.makefile().read(), end="")
' "$@"
