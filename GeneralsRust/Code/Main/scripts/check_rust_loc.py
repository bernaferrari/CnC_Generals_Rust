#!/usr/bin/env python3
"""Enforce a shrinking line-count ratchet for maintained Rust sources.

The ratchet does not pretend an oversized file is acceptable.  It records the
current ceiling so extraction can happen in reviewable waves while CI rejects
new oversized files and growth in existing ones.
"""

from __future__ import annotations

import argparse
import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

HARD_LIMIT = 4_000
WARN_LIMIT = 2_000


@dataclass(frozen=True)
class RustFile:
    path: str
    lines: int
    role: str


def is_test_path(path: Path) -> bool:
    """Classify test-only suites without treating ordinary implementations as tests."""
    parts = path.parts
    name = path.name
    return (
        name == "tests.rs"
        or name.startswith("test_")
        or name.endswith("_tests.rs")
        or any(part in {"tests", "benches", "examples"} or part.endswith("_tests") for part in parts)
    )


def maintained_rust_files(root: Path) -> Iterable[Path]:
    """Yield Rust files, pruning build output, VCS metadata, and nested repositories."""
    for current, dirs, files in os.walk(root):
        current_path = Path(current)
        if current_path != root and ".git" in files:
            dirs[:] = []
            continue
        dirs[:] = sorted(
            directory
            for directory in dirs
            if directory not in {".git", "target", "node_modules", "vendor"}
        )
        for filename in sorted(files):
            if filename.endswith(".rs"):
                yield current_path / filename


def inventory(root: Path) -> list[RustFile]:
    result: list[RustFile] = []
    for path in maintained_rust_files(root):
        relative = path.relative_to(root).as_posix()
        with path.open("rb") as source:
            lines = sum(1 for _ in source)
        result.append(
            RustFile(relative, lines, "test" if is_test_path(Path(relative)) else "production")
        )
    return sorted(result, key=lambda item: item.path)


def load_allowlist(path: Path) -> dict[str, dict[str, int]]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    if raw.get("hard_limit") != HARD_LIMIT:
        raise ValueError(f"{path} hard_limit must be {HARD_LIMIT}")
    return {
        role: {str(name): int(limit) for name, limit in raw.get(role, {}).items()}
        for role in ("production", "test")
    }


def write_allowlist(path: Path, files: list[RustFile]) -> None:
    payload: dict[str, object] = {
        "schema_version": 1,
        "hard_limit": HARD_LIMIT,
        "policy": "Entries are temporary ceilings: counts may decrease or disappear, never increase.",
        "production": {},
        "test": {},
    }
    for role in ("production", "test"):
        payload[role] = {
            item.path: item.lines
            for item in files
            if item.role == role and item.lines > HARD_LIMIT
        }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def violations(
    files: list[RustFile], allowlist: dict[str, dict[str, int]]
) -> list[str]:
    problems: list[str] = []
    current = {item.path: item for item in files}
    for item in files:
        ceiling = allowlist[item.role].get(item.path)
        if item.lines > HARD_LIMIT and ceiling is None:
            problems.append(
                f"new oversized {item.role} file: {item.path} has {item.lines} lines"
            )
        elif ceiling is not None and item.lines > ceiling:
            problems.append(
                f"LOC ratchet grew: {item.path} has {item.lines} lines (ceiling {ceiling})"
            )
    for role, entries in allowlist.items():
        for path, ceiling in entries.items():
            item = current.get(path)
            if item is None:
                problems.append(f"stale {role} allowlist entry: {path}")
            elif item.lines <= HARD_LIMIT:
                problems.append(
                    f"shrunk file must leave allowlist: {path} has {item.lines} lines"
                )
            elif ceiling < item.lines:
                # Kept separate from the normal growth case for malformed lists.
                problems.append(
                    f"invalid ceiling: {path} has {item.lines} lines (ceiling {ceiling})"
                )
    return problems


def report(files: list[RustFile]) -> dict[str, object]:
    return {
        "schema_version": 1,
        "hard_limit": HARD_LIMIT,
        "warning_limit": WARN_LIMIT,
        "production_over_limit": [
            item.__dict__ for item in files if item.role == "production" and item.lines > HARD_LIMIT
        ],
        "test_over_limit": [
            item.__dict__ for item in files if item.role == "test" and item.lines > HARD_LIMIT
        ],
        "warning_band": [
            item.__dict__ for item in files if WARN_LIMIT < item.lines <= HARD_LIMIT
        ],
    }


def parse_args() -> argparse.Namespace:
    script = Path(__file__).resolve()
    default_root = script.parents[3]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=default_root)
    parser.add_argument(
        "--allowlist",
        type=Path,
        default=script.with_name("rust_loc_allowlist.json"),
    )
    parser.add_argument("--write-allowlist", action="store_true")
    parser.add_argument("--json", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    files = inventory(args.root.resolve())
    if args.write_allowlist:
        write_allowlist(args.allowlist, files)
    allowlist = load_allowlist(args.allowlist)
    problems = violations(files, allowlist)
    data = report(files)
    data["violations"] = problems
    if args.json:
        print(json.dumps(data, indent=2, sort_keys=True))
    else:
        print(
            "Rust LOC: "
            f"{len(data['production_over_limit'])} production and "
            f"{len(data['test_over_limit'])} test files over {HARD_LIMIT}; "
            f"{len(data['warning_band'])} files in the {WARN_LIMIT + 1}-{HARD_LIMIT} warning band"
        )
        for problem in problems:
            print(f"ERROR: {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
