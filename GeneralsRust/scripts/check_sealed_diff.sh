#!/usr/bin/env bash
# Fail if a diff from <base> touches a sealed path without an Unseal line.
# Usage: scripts/check_sealed_diff.sh <base-ref>
# Unseal form (in a commit message in base..HEAD):
#   Unseal: <path> — <reason>
# Path must match a sealed module path (GeneralsRust-relative) exactly or as a prefix.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$ROOT/.." && pwd)"
BASE="${1:-}"
if [[ -z "$BASE" ]]; then
  echo "usage: $0 <base-ref>" >&2
  exit 2
fi

python3 - "$ROOT" "$REPO" "$BASE" <<'PY'
import os, re, sys, subprocess

root, repo, base = sys.argv[1], sys.argv[2], sys.argv[3]
toml_path = os.path.join(root, "sealed_modules.toml")
text = open(toml_path, encoding="utf-8").read()
modules = []
current = None
for raw in text.splitlines():
    line = raw.split("#", 1)[0].rstrip()
    if not line.strip():
        continue
    if line.strip() == "[[module]]":
        if current and "path" in current:
            modules.append(current)
        current = {}
        continue
    if current is None:
        continue
    m = re.match(r'^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"(.*)"\s*$', line.strip())
    if m:
        current[m.group(1)] = m.group(2)
if current and "path" in current:
    modules.append(current)

if not modules:
    print("no sealed modules; diff check ok")
    sys.exit(0)

sealed_paths = [m["path"].strip().strip("/") for m in modules]

# Changed files under GeneralsRust/ relative to GeneralsRust
diff = subprocess.check_output(
    ["git", "-C", repo, "diff", "--name-only", f"{base}...HEAD"],
    text=True,
)
changed = []
for line in diff.splitlines():
    line = line.strip()
    if not line:
        continue
    if line.startswith("GeneralsRust/"):
        changed.append(line[len("GeneralsRust/"):])
    elif line.startswith("generalsrust/"):
        changed.append(line[len("generalsrust/"):])

touched = []
for ch in changed:
    for sp in sealed_paths:
        if ch == sp or ch.startswith(sp.rstrip("/") + "/"):
            touched.append((ch, sp))
            break

if not touched:
    print("no sealed paths touched; ok")
    sys.exit(0)

# Collect Unseal lines from commits in range
log = subprocess.check_output(
    ["git", "-C", repo, "log", "--format=%B", f"{base}..HEAD"],
    text=True,
)
# Unseal: path — reason   (em-dash or double-hyphen or ASCII ---)
unseal_re = re.compile(
    r"(?im)^Unseal:\s*(\S+)\s*[—–-]{1,3}\s*.+$"
)
unsealed = set()
for m in unseal_re.finditer(log):
    unsealed.add(m.group(1).strip().strip("/"))

failed = False
for ch, sp in touched:
    allowed = any(
        sp == u or sp.startswith(u.rstrip("/") + "/") or u == sp or u.startswith(sp.rstrip("/") + "/")
        for u in unsealed
    )
    # Also allow exact sealed path listed in Unseal
    if sp in unsealed or any(ch == u or ch.startswith(u.rstrip("/") + "/") for u in unsealed):
        allowed = True
    if not allowed:
        print(
            f"FAIL: sealed path touched without Unseal: {ch} (sealed as {sp})",
            file=sys.stderr,
        )
        failed = True

if failed:
    print(
        "Add a commit message line: Unseal: <path> — <reason>",
        file=sys.stderr,
    )
    sys.exit(1)

print("sealed touches covered by Unseal; ok")
PY
