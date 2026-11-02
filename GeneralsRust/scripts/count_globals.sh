#!/usr/bin/env bash
# Count process-wide mutable statics per crate area.
# Counts: `static mut`, `static` with Mutex|RwLock|Atomic*|RefCell|Cell
# (including those nested under OnceLock/Lazy/LazyLock/OnceCell), and
# `thread_local!`. Excludes tests/, examples/, benches/, *_tests.rs, tests.rs,
# and items under #[cfg(test)] modules/attributes.
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


def cfg_test_depths(text):
    """Byte offsets that sit inside a #[cfg(test)] module body."""
    depths = set()
    depth = 0
    test_depth = None
    i = 0
    n = len(text)
    while i < n:
        if text.startswith("#[cfg(test)]", i):
            j = i + len("#[cfg(test)]")
            while j < n and text[j] in " \t\r\n":
                j += 1
            # Attribute on a module: track its body. Attribute on an item: mark
            # the next item span roughly via following '{'/';' .
            if text.startswith("mod ", j) or text.startswith("pub mod ", j):
                k = text.find("{", j)
                if k != -1 and k < n:
                    # enter module at k
                    pass
                # find opening brace of the mod
                k = j
                while k < n and text[k] != "{":
                    if text[k] == ";":  # mod foo;
                        break
                    k += 1
                if k < n and text[k] == "{":
                    test_depth = depth + 1
                    i = k
                    depth += 1
                    i += 1
                    continue
            else:
                # cfg(test) on a non-mod item — skip until next top-level-ish end
                depths.add(j)  # marker; handled per-match via lookback
            i = j
            continue
        ch = text[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            if test_depth is not None and depth == test_depth:
                test_depth = None
            depth -= 1
        if test_depth is not None and depth >= test_depth:
            depths.add(i)
        i += 1
    return depths

def item_has_cfg_test_attr(text, pos):
    line = text.rfind("\n", 0, pos) + 1
    window = text[max(0, line - 240):line]
    for raw in reversed(window.splitlines()):
        s = raw.strip()
        if not s:
            continue
        if s.startswith("#[cfg(test)]"):
            return True
        if s.startswith("#["):
            continue
        break
    return False

def inside_cfg_test_thread_local(text, pos):
    """True when pos sits in a thread_local! block preceded by #[cfg(test)]."""
    # Find the nearest thread_local! before pos whose braces still enclose pos.
    idx = text.rfind("thread_local!", 0, pos)
    while idx != -1:
        brace = text.find("{", idx)
        if brace == -1 or brace > pos:
            idx = text.rfind("thread_local!", 0, idx)
            continue
        depth = 0
        i = brace
        end = None
        while i < len(text):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    end = i
                    break
            i += 1
        if end is not None and brace < pos <= end:
            return item_has_cfg_test_attr(text, idx)
        idx = text.rfind("thread_local!", 0, idx)
    return False

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
            test_body = cfg_test_depths(text)
            for m in pat_tls.finditer(text):
                line = text.rfind("\n", 0, m.start()) + 1
                if text[line : line + 8].lstrip().startswith("//"):
                    continue
                if (
                    m.start() in test_body
                    or item_has_cfg_test_attr(text, m.start())
                ):
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
                    if (
                        m.start() in test_body
                        or item_has_cfg_test_attr(text, m.start())
                        or inside_cfg_test_thread_local(text, m.start())
                    ):
                        continue
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
