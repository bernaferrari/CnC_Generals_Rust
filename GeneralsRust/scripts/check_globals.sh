#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec "$ROOT/scripts/count_globals.sh" --compare "$ROOT/scripts/globals_baseline.json" "$@"
