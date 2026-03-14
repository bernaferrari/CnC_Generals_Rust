#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKTREE="$(cd "$SCRIPT_DIR/../.." && pwd)"
PLAYABILITY_BIN="${WORKTREE}/target/debug/playability_audit"
PORT_TRACKING_GENERATOR="${SCRIPT_DIR}/scripts/generate_port_tracking.py"
if [[ -d "${WORKTREE}/GeneralsMD" && -d "${WORKTREE}/GeneralsRust" ]]; then
  REPO_ROOT="${WORKTREE}"
else
  REPO_ROOT="$(cd "${WORKTREE}/.." && pwd)"
fi

cd "$WORKTREE"

if [[ -f "$PORT_TRACKING_GENERATOR" ]]; then
  if [[ "${NO_PORT_TRACKING_GEN:-0}" != "1" ]]; then
    echo "=== Refreshing PORT_* tracking artifacts ==="
    python3 "$PORT_TRACKING_GENERATOR" --repo-root "$REPO_ROOT" --output-root "$REPO_ROOT"
  fi
else
  echo "warning: port tracking generator missing at ${PORT_TRACKING_GENERATOR}; using existing PORT_* files"
fi

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
