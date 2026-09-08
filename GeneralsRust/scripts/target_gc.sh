#!/bin/bash
# target/ GC sweep — Cargo never garbage-collects superseded artifacts, so
# every fingerprint change leaves the old rlib/rmeta/dSYM/binary in place.
# In a workspace this size (40+ crates, dozens of bins + integration tests,
# both dev and release profiles) that grew GeneralsRust/target to 88 GB.
#
# Usage:
#   scripts/target_gc.sh [--apply] [--days N] [--profile all|dev|release]
#
# Default: DRY RUN over the dev profile, deleting nothing, reporting what is
# older than --days (default 21). Re-run with --apply to actually delete.
# Run it after big refactors or monthly; `cargo clean --profile <p>` is the
# nuclear option when you want a guaranteed-lean baseline.
#
# Notes:
# - Deleting from target/ is always SAFE for correctness (Cargo rebuilds);
#   the cost is recompilation time, which is why the default is age-gated.
# - dSYM bundles are the biggest per-hash cost on macOS with
#   split-debuginfo="packed"; profiles now use "unpacked", so new dSYMs
#   should not appear. This script still sweeps old ones.
set -u
APPLY=0
DAYS=21
PROFILE="dev"
for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --days) DAYS="$NEXT"; NEXT=""; [[ "${DAYS:-21}" =~ ^[0-9]+$ ]] || { echo "bad --days"; exit 2; }; ;;
    --days=*) DAYS="${arg#*=}" ;;
    --profile) PROFILE="$NEXT"; NEXT=""; ;;
    --profile=*) PROFILE="${arg#*=}" ;;
    *) NEXT="$arg" ;;
  esac
done
cd "$(dirname "$0")/.." || exit 1

case "$PROFILE" in
  all) DIRS=(target/debug target/release) ;;
  dev) DIRS=(target/debug) ;;
  release) DIRS=(target/release) ;;
  *) echo "unknown --profile $PROFILE"; exit 2 ;;
esac

TOTAL=0
for d in "${DIRS[@]}"; do
  [ -d "$d" ] || continue
  echo "== sweeping $d (older than ${DAYS}d)"
  # Superseded hashed artifacts + stale bundles; keep the newest build alive
  # by only touching files last modified before the cutoff.
  while IFS= read -r -d '' f; do
    sz=$(stat -f%z "$f" 2>/dev/null) || continue
    TOTAL=$((TOTAL + sz))
    if [ "$APPLY" = "1" ]; then rm -rf "$f"; else echo "  would delete: $f ($((sz / 1024 / 1024)) MB)"; fi
  done < <(find "$d" -maxdepth 2 \( -name '*.dSYM' -o -name '*.rlib' -o -name '*.rmeta' -o -name '*.d' \
      -o -name 'libtest-*' -o -name 'generals_main-*' \) -mtime +"$DAYS" -print0)
done
echo "total reclaimable: $((TOTAL / 1024 / 1024 / 1024)) GB ($TOTAL bytes)"
[ "$APPLY" = "1" ] || echo "(dry run — pass --apply to delete)"
