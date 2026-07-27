#!/usr/bin/env bash
# Sync rust/ to VM C:\pdm, run a cargo command there, fetch release exe if built.
# Usage: scripts/vm-build.sh [build|test|check]   (default: build --release)
set -euo pipefail
VM="${VM:-dev@192.168.69.110}"
CMD="${1:-build}"
cd "$(dirname "$0")/.."
tar czf /tmp/pdm-src.tgz --exclude=target -C rust .
scp -q -o BatchMode=yes /tmp/pdm-src.tgz "$VM:C:/Users/dev/pdm-src.tgz"
CARGO_ARGS="build --release"
[ "$CMD" = "test" ] && CARGO_ARGS="test"
[ "$CMD" = "check" ] && CARGO_ARGS="check"
scripts/vm.sh "
\$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
if (!(Test-Path C:\\pdm)) { mkdir C:\\pdm | Out-Null }
tar xzf C:\\Users\\dev\\pdm-src.tgz -C C:\\pdm
cd C:\\pdm
cargo $CARGO_ARGS 2>&1
\"CARGO_EXIT=\$LASTEXITCODE\""
if [ "$CMD" = "build" ]; then
  mkdir -p dist
  scp -q -o BatchMode=yes "$VM:C:/pdm/target/release/LogDump.exe" dist/ && \
    ls -la dist/LogDump.exe
fi
