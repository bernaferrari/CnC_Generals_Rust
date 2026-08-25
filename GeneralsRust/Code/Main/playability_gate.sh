#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PROFILE="${1:-full}"

if [[ "$PROFILE" != "quick" && "$PROFILE" != "full" ]]; then
  echo "usage: $0 [quick|full]" >&2
  exit 2
fi

echo "=== Verifying generated split-aware provenance ==="
python3 "$SCRIPT_DIR/scripts/generate_port_provenance.py" \
  --repo-root "$REPO_ROOT" --verify-generated
python3 "$SCRIPT_DIR/scripts/generate_port_review_queue.py" \
  --repo-root "$REPO_ROOT" --verify-generated

echo "=== Enforcing shrinking Rust LOC ratchet ==="
python3 "$SCRIPT_DIR/scripts/check_rust_loc.py"

echo "=== Enforcing unsafe-code safety contract ratchet ==="
python3 "$SCRIPT_DIR/scripts/check_unsafe_contracts.py"

echo "=== Running evidence-backed non-network $PROFILE gates ==="
python3 "$SCRIPT_DIR/scripts/port_dashboard.py" \
  --repo-root "$REPO_ROOT" --run-gates "$PROFILE"
