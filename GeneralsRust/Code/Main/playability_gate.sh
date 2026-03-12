#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKTREE="$(cd "$SCRIPT_DIR/../.." && pwd)"
PLAYABILITY_BIN="${WORKTREE}/target/debug/playability_audit"

cd "$WORKTREE"

echo "=== Verifying non-network workspace build ==="
cargo check --workspace --all-targets --exclude game_network

if [[ ! -x "$PLAYABILITY_BIN" ]]; then
  echo "playability_audit binary missing; building..."
  cargo build -p generals_main --bin playability_audit
fi

if [[ $# -gt 0 ]]; then
  "$PLAYABILITY_BIN" "$@"
  exit $?
fi

PHASES=(baseline gameplay saveload ui release)
FAILED_PHASES=()

echo
echo "=== GeneralsRust playability gate matrix (network deferred) ==="
for phase in "${PHASES[@]}"; do
  echo
  echo "Running phase: $phase"
  if ! "$PLAYABILITY_BIN" --phase "$phase"; then
    FAILED_PHASES+=("$phase")
  fi
done

echo
if [[ ${#FAILED_PHASES[@]} -gt 0 ]]; then
  echo "Playability phases failed: ${FAILED_PHASES[*]}"
  exit 1
fi

echo "All playability phases passed."
