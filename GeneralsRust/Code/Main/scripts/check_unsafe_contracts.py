#!/usr/bin/env python3
"""Ratchet unsafe Rust while parity-sensitive behavior is still being ported.

Unsafe removal is valuable, but broad rewrites before behavioral parity can be
more dangerous than well-contained unsafe code. This gate therefore makes the
existing debt explicit, rejects expansion, and requires a nearby ``SAFETY:``
invariant for every newly reviewed unsafe construct.
"""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import asdict, dataclass
from pathlib import Path

from check_rust_loc import is_test_path, maintained_rust_files


UNSAFE_RE = re.compile(r"\bunsafe\s+(?:fn\b|impl\b|trait\b|extern\b|\{)")
RAW_STRING_RE = re.compile(r'(?:br|rb|r)(#{0,16})"')


@dataclass(frozen=True)
class UnsafeFile:
    path: str
    role: str
    category: str
    constructs: int
    undocumented: int
    lines: tuple[int, ...]


def strip_comments_and_literals(source: str) -> str:
    """Replace comments/string/char contents with spaces while preserving lines."""
    result: list[str] = []
    index = 0
    block_depth = 0
    mode = "normal"
    raw_hashes = 0
    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""
        if mode == "line_comment":
            if char == "\n":
                result.append(char)
                mode = "normal"
            else:
                result.append(" ")
            index += 1
            continue
        if mode == "block_comment":
            if char == "/" and next_char == "*":
                block_depth += 1
                result.extend("  ")
                index += 2
            elif char == "*" and next_char == "/":
                block_depth -= 1
                result.extend("  ")
                index += 2
                if block_depth == 0:
                    mode = "normal"
            else:
                result.append("\n" if char == "\n" else " ")
                index += 1
            continue
        if mode in {"string", "char"}:
            delimiter = '"' if mode == "string" else "'"
            if char == "\\":
                result.append(" ")
                if next_char:
                    result.append("\n" if next_char == "\n" else " ")
                    index += 2
                else:
                    index += 1
            elif char == delimiter:
                result.append(" ")
                mode = "normal"
                index += 1
            else:
                result.append("\n" if char == "\n" else " ")
                index += 1
            continue
        if mode == "raw_string":
            terminator = '"' + ("#" * raw_hashes)
            if source.startswith(terminator, index):
                result.extend(" " * len(terminator))
                index += len(terminator)
                mode = "normal"
            else:
                result.append("\n" if char == "\n" else " ")
                index += 1
            continue

        if char == "/" and next_char == "/":
            result.extend("  ")
            index += 2
            mode = "line_comment"
        elif char == "/" and next_char == "*":
            result.extend("  ")
            index += 2
            mode = "block_comment"
            block_depth = 1
        elif char == '"':
            result.append(" ")
            index += 1
            mode = "string"
        elif char == "'" and next_char and next_char != " ":
            # Lifetimes have no closing quote. Treat only ordinary one-character
            # or escaped character literals as literals.
            tail = source[index + 1 : index + 5]
            if re.match(r"(?:\\.|[^\\'])'", tail):
                result.append(" ")
                index += 1
                mode = "char"
            else:
                result.append(char)
                index += 1
        else:
            raw = RAW_STRING_RE.match(source, index) if char in {"b", "r"} else None
            if raw:
                token = raw.group(0)
                raw_hashes = len(raw.group(1))
                result.extend(" " * len(token))
                index += len(token)
                mode = "raw_string"
            else:
                result.append(char)
                index += 1
    return "".join(result)


def category(path: Path) -> str:
    normalized = path.as_posix().lower()
    if any(marker in normalized for marker in ("ffi", "binding", "w3d_c_api", "/platform/")):
        return "ffi_or_platform"
    return "runtime"


def has_attached_safety_comment(lines: list[str], line: int) -> bool:
    """Accept only a contiguous comment/attribute prelude attached to the construct."""
    for index in range(line - 2, max(-1, line - 7), -1):
        value = lines[index].strip()
        if not value or value.startswith("#["):
            continue
        if value.startswith(("//", "/*", "*", "*/")):
            if "SAFETY:" in value:
                return True
            continue
        break
    return False


def scan_file(path: Path, root: Path) -> UnsafeFile | None:
    source = path.read_text(encoding="utf-8", errors="ignore")
    if "unsafe" not in source:
        return None
    stripped = strip_comments_and_literals(source)
    original_lines = source.splitlines()
    matches = list(UNSAFE_RE.finditer(stripped))
    if not matches:
        return None
    line_numbers: list[int] = []
    undocumented = 0
    for match in matches:
        line = stripped.count("\n", 0, match.start()) + 1
        line_numbers.append(line)
        if not has_attached_safety_comment(original_lines, line):
            undocumented += 1
    relative = path.relative_to(root)
    return UnsafeFile(
        path=relative.as_posix(),
        role="test" if is_test_path(relative) else "production",
        category=category(relative),
        constructs=len(matches),
        undocumented=undocumented,
        lines=tuple(line_numbers),
    )


def inventory(root: Path) -> list[UnsafeFile]:
    return [
        item
        for path in maintained_rust_files(root)
        if (item := scan_file(path, root)) is not None
    ]


def write_allowlist(path: Path, files: list[UnsafeFile]) -> None:
    payload = {
        "schema_version": 1,
        "policy": "Per-file ceilings must decrease or disappear; new unsafe requires explicit review and SAFETY documentation.",
        "files": {
            item.path: {
                "constructs": item.constructs,
                "undocumented": item.undocumented,
            }
            for item in sorted(files, key=lambda value: value.path)
        },
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_allowlist(path: Path) -> dict[str, dict[str, int]]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    if raw.get("schema_version") != 1 or not isinstance(raw.get("files"), dict):
        raise ValueError(f"{path} must use unsafe-ratchet schema version 1")
    return {
        str(name): {
            "constructs": int(value["constructs"]),
            "undocumented": int(value["undocumented"]),
        }
        for name, value in raw["files"].items()
    }


def violations(files: list[UnsafeFile], allowlist: dict[str, dict[str, int]]) -> list[str]:
    problems: list[str] = []
    current = {item.path: item for item in files}
    for item in files:
        ceiling = allowlist.get(item.path)
        if ceiling is None:
            problems.append(
                f"new unsafe file requires review: {item.path} "
                f"({item.constructs} constructs, {item.undocumented} undocumented)"
            )
            continue
        for metric in ("constructs", "undocumented"):
            actual = getattr(item, metric)
            expected = ceiling[metric]
            if actual > expected:
                problems.append(
                    f"unsafe {metric} grew: {item.path} has {actual} (ceiling {expected})"
                )
            elif actual < expected:
                problems.append(
                    f"unsafe {metric} ceiling must shrink: {item.path} has {actual} (ceiling {expected})"
                )
    for path in sorted(set(allowlist) - set(current)):
        problems.append(f"stale unsafe allowlist entry: {path}")
    return problems


def report(files: list[UnsafeFile], problems: list[str]) -> dict[str, object]:
    return {
        "schema_version": 1,
        "files_with_unsafe": len(files),
        "constructs": sum(item.constructs for item in files),
        "undocumented": sum(item.undocumented for item in files),
        "by_category": {
            name: sum(item.constructs for item in files if item.category == name)
            for name in ("ffi_or_platform", "runtime")
        },
        "files": [asdict(item) for item in files],
        "violations": problems,
    }


def parse_args() -> argparse.Namespace:
    script = Path(__file__).resolve()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=script.parents[3])
    parser.add_argument("--allowlist", type=Path, default=script.with_name("unsafe_allowlist.json"))
    parser.add_argument("--write-allowlist", action="store_true")
    parser.add_argument("--json", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    files = inventory(args.root.resolve())
    if args.write_allowlist:
        write_allowlist(args.allowlist, files)
    problems = violations(files, load_allowlist(args.allowlist))
    data = report(files, problems)
    if args.json:
        print(json.dumps(data, indent=2, sort_keys=True))
    else:
        print(
            f"Unsafe Rust: {data['constructs']} constructs in {data['files_with_unsafe']} files; "
            f"{data['undocumented']} lack a nearby SAFETY invariant"
        )
        for problem in problems:
            print(f"ERROR: {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
