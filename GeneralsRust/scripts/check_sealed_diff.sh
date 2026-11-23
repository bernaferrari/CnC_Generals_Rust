#!/usr/bin/env bash
# Fail if a diff from <base> touches or removes a seal without an Unseal line.
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

python3 - "$REPO" "$BASE" <<'PYTHON'
import re, sys, subprocess

repo, base = sys.argv[1:]
git = ["git", "-C", repo]
registry = "GeneralsRust/sealed_modules.toml"
merge_base = subprocess.check_output(git + ["merge-base", base, "HEAD"], text=True).strip()

def sealed_paths_at(revision):
    # Older commits may predate the registry. If HEAD removes an entry,
    # the committed base registry still supplies the protected path.
    result = subprocess.run(
        git + ["show", f"{revision}:{registry}"],
        capture_output=True,
        text=True,
    )
    return {
        match.group(1).strip().strip("/")
        for match in re.finditer(r'^\s*path\s*=\s*"([^"]+)"', result.stdout, re.MULTILINE)
    }

previous_paths = sealed_paths_at(merge_base)
current_paths = sealed_paths_at("HEAD")
sealed_paths = previous_paths | current_paths
if not sealed_paths:
    print("no sealed modules; diff check ok")
    sys.exit(0)

# Changed files under GeneralsRust/ relative to GeneralsRust
diff = subprocess.check_output(
    git + ["diff", "--name-only", "--no-renames", "-z", merge_base, "HEAD"],
    text=True,
)
changed = [
    path[len("GeneralsRust/"):]
    for path in diff.split("\0")
    if path.lower().startswith("generalsrust/")
]

touched = {(path, path) for path in previous_paths - current_paths}
for path in changed:
    for sealed in sealed_paths:
        if path == sealed or path.startswith(sealed + "/"):
            touched.add((path, sealed))

if not touched:
    print("no sealed paths touched; ok")
    sys.exit(0)

# Collect Unseal lines from commits in range
log = subprocess.check_output(
    git + ["log", "--format=%B", f"{merge_base}..HEAD"],
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
for ch, sp in sorted(touched):
    allowed = any(ch == path or ch.startswith(path + "/") for path in unsealed)
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
PYTHON
