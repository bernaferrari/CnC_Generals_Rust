#!/usr/bin/env bash
# Fail if any sealed module still has mutable process globals or stub markers.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 - "$ROOT" <<'PY'
import os, re, sys, json

root = sys.argv[1]
toml_path = os.path.join(root, "sealed_modules.toml")
if not os.path.isfile(toml_path):
    print(f"missing {toml_path}", file=sys.stderr)
    sys.exit(2)

text = open(toml_path, encoding="utf-8").read()

# Minimal [[module]] parser (no toml dep required).
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

pat_static = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?static[ \t]+(?:mut[ \t]+|ref[ \t]+)?(\w+)[ \t]*:[ \t]*([^;=]+)"
)
pat_tls = re.compile(r"(?m)^[ \t]*thread_local!\s*\{")
mut_ty = re.compile(r"\b(Mutex|RwLock|Atomic\w+|RefCell|Cell)\b")
# Stub markers: todo!/unimplemented! macros, and common placeholder phrases.
pat_stub = re.compile(
    r"(?m)(?:\btodo!\s*(?:\(|$)|"
    r"\bunimplemented!\s*(?:\(|$)|"
    r"\bstub(?:bed)?\b|"
    r"not yet implemented|"
    r"TODO:\s*implement)",
    re.IGNORECASE,
)
skip_dir = re.compile(r"(?:^|/)(?:tests|examples|benches|target)(?:/|$)")

failed = False
if not modules:
    print("sealed_modules.toml: no modules sealed yet (ok)")
    sys.exit(0)

for mod in modules:
    rel = mod["path"].strip().strip("/")
    base = os.path.join(root, rel)
    if not os.path.exists(base):
        print(f"FAIL: sealed path missing: {rel}", file=sys.stderr)
        failed = True
        continue
    globals_hits = []
    stub_hits = []
    files = []
    if os.path.isfile(base) and base.endswith(".rs"):
        files = [base]
    else:
        for dp, dns, fns in os.walk(base):
            dns[:] = [d for d in dns if not skip_dir.search(d)]
            rel_dir = os.path.relpath(dp, root).replace("\\", "/")
            if skip_dir.search(rel_dir):
                continue
            for fn in fns:
                if not fn.endswith(".rs"):
                    continue
                if fn.endswith("_tests.rs") or fn == "tests.rs":
                    continue
                files.append(os.path.join(dp, fn))
    for path in files:
        src = open(path, encoding="utf-8", errors="ignore").read()
        rel_file = os.path.relpath(path, root).replace("\\", "/")
        for m in pat_tls.finditer(src):
            line = src.rfind("\n", 0, m.start()) + 1
            if src[line : line + 8].lstrip().startswith("//"):
                continue
            lineno = src.count("\n", 0, m.start()) + 1
            globals_hits.append(f"{rel_file}:{lineno}:thread_local!")
        for m in pat_static.finditer(src):
            line = src.rfind("\n", 0, m.start()) + 1
            head = src[line : m.end()]
            if head.lstrip().startswith("//"):
                continue
            name, ty = m.group(1), m.group(2)
            if re.search(r"\bstatic\s+mut\b", head) or mut_ty.search(ty):
                lineno = src.count("\n", 0, m.start()) + 1
                globals_hits.append(f"{rel_file}:{lineno}:{name}")
        # Stub scan: ignore pure comment lines
        for m in pat_stub.finditer(src):
            line_start = src.rfind("\n", 0, m.start()) + 1
            line_txt = src[line_start : src.find("\n", m.start())]
            stripped = line_txt.lstrip()
            if stripped.startswith("//") or stripped.startswith("///") or stripped.startswith("*"):
                continue
            # Ignore documentation mentioning stubs in string form inside comments already handled
            lineno = src.count("\n", 0, m.start()) + 1
            stub_hits.append(f"{rel_file}:{lineno}:{m.group(0)[:40]}")

    if globals_hits:
        print(f"FAIL: sealed module still has mutable globals: {rel}", file=sys.stderr)
        for h in globals_hits[:20]:
            print(f"  {h}", file=sys.stderr)
        failed = True
    if stub_hits:
        print(f"FAIL: sealed module has stub markers: {rel}", file=sys.stderr)
        for h in stub_hits[:20]:
            print(f"  {h}", file=sys.stderr)
        failed = True
    if not globals_hits and not stub_hits:
        note = mod.get("note", "")
        print(f"ok: {rel}" + (f" — {note}" if note else ""))

if failed:
    sys.exit(1)
PY
