#!/usr/bin/env bash
# Run a command with a wall-clock limit. Exit 124 on timeout (same as GNU timeout).
# Usage: scripts/with-timeout.sh [seconds] command [args...]
# Default: 30 seconds if the first argument is not a positive integer.
set -euo pipefail

secs=30
if [[ "${1:-}" =~ ^[0-9]+$ ]]; then
  secs="$1"
  shift
fi

if [[ $# -eq 0 ]]; then
  echo "usage: $0 [seconds] command [args...]" >&2
  exit 2
fi

exec timeout --foreground "${secs}s" "$@"
