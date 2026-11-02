#!/usr/bin/env bash
# Count process-wide mutable statics per crate area.
# Counts: `static mut`, `static` with Mutex|RwLock|Atomic*|RefCell|Cell
# (including those nested under OnceLock/Lazy/LazyLock/OnceCell), and
# `thread_local!`. Excludes tests/, examples/, benches/, *_tests.rs, tests.rs.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 - "$ROOT" "$@" <<'PY'
import os, re, sys, json, collections

root = sys.argv[1]
want_json = "--json" in sys.argv[1:]
compare = None
for i, a in enumerate(sys.argv):
    if a == "--compare" and i + 1 < len(sys.argv):
        compare = sys.argv[i + 1]

pat_static = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?static[ \t]+(?:mut[ \t]+)?(\w+)[ \t]*:[ \t]*([^;=]+)"
)
pat_tls = re.compile(r"(?m)^[ \t]*thread_local!\s*\{")
mut_ty = re.compile(r"\b(Mutex|RwLock|Atomic\w+|RefCell|Cell)\b")

roots = {
    "GameClient": "Code/GameEngine/GameClient",
    "GameLogic": "Code/GameEngine/GameLogic",
    "Common": "Code/GameEngine/Common",
    "WWVegas": "Code/Libraries/Source/WWVegas",
    "Main": "Code/Main",
}
skip_dir = re.compile(r"(?:^|/)(?:tests|examples|benches|target)(?:/|$)")

counts = collections.OrderedDict((k, 0) for k in roots)
details = collections.defaultdict(list)

for label, rel in roots.items():
    base = os.path.join(root, rel)
    if not os.path.isdir(base):
        continue
    for dirpath, dirnames, fns in os.walk(base):
        # prune skipped dirs
        dirnames[:] = [d for d in dirnames if not skip_dir.search(d)]
        rel_dir = os.path.relpath(dirpath, root).replace("\\", "/")
        if skip_dir.search(rel_dir):
            continue
        for fn in fns:
            if not fn.endswith(".rs"):
                continue
            if fn.endswith("_tests.rs") or fn == "tests.rs":
                continue
            path = os.path.join(dirpath, fn)
            text = open(path, encoding="utf-8", errors="ignore").read()
            for m in pat_tls.finditer(text):
                line = text.rfind("\n", 0, m.start()) + 1
                if text[line : line + 8].lstrip().startswith("//"):
                    continue
                counts[label] += 1
                details[label].append(f"{rel_dir}/{fn}:thread_local!")
            for m in pat_static.finditer(text):
                line = text.rfind("\n", 0, m.start()) + 1
                head = text[line : m.end()]
                if head.lstrip().startswith("//"):
                    continue
                name, ty = m.group(1), m.group(2)
                if re.search(r"\bstatic\s+mut\b", head) or mut_ty.search(ty):
                    counts[label] += 1
                    details[label].append(f"{rel_dir}/{fn}:{name}")

if want_json:
    print(json.dumps(dict(counts), indent=2, sort_keys=True))
else:
    print("Mutable process-wide statics (excl. tests/examples/benches):")
    total = 0
    for k, v in counts.items():
        print(f"  {k}: {v}")
        total += v
    print(f"  TOTAL: {total}")

if compare:
    base = json.load(open(compare, encoding="utf-8"))
    failed = False
    for k, v in counts.items():
        b = int(base.get(k, 0))
        if v > b:
            print(f"FAIL: {k} rose {b} -> {v}", file=sys.stderr)
            failed = True
        elif v < b and not want_json:
            print(f"note: {k} fell {b} -> {v} (update baseline)")
    if failed:
        sys.exit(1)
PY
