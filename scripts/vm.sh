#!/usr/bin/env bash
# Usage: scripts/vm.sh 'powershell script text'   (multi-line OK, no quoting hell)
set -euo pipefail
VM="${VM:-dev@192.168.69.110}"
B64=$(printf '%s' "$1" | iconv -t UTF-16LE | base64 -w0)
ssh -o BatchMode=yes -o ConnectTimeout=5 "$VM" "powershell -NoProfile -EncodedCommand $B64" \
  | grep -v -a -i "post-quantum\|store now\|upgraded\|openssh.com\|^\*\* \|CLIXML\|^<Objs"
