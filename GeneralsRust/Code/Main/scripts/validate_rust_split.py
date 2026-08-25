#!/usr/bin/env python3
"""Validate a behavior-preserving Rust file-to-module split against git.

This is the mechanical contract for inexpensive split agents. It compares the
current module tree with the unsplit source at ``--before-ref`` and rejects the
failure modes that compilation alone misses: oversized fragments, numbered
shards, lost tests, and accidental public API growth.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tomllib
from pathlib import Path

HARD_LIMIT = 4_000
TARGET_LIMIT = 2_500
ROOT_LIMIT = 1_000
NUMERIC_SHARD_RE = re.compile(r"(?:^|_)(?:part|chunk|section|split)_?\d+(?:_|$)", re.I)
PUBLIC_DECL_RE = re.compile(
    r"(?m)^\s*pub(?:\(crate\))?\s+(?:async\s+)?"
    r"(?:unsafe\s+)?(?:fn|struct|enum|trait|type|const|static|mod)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)"
)
PUBLIC_USE_RE = re.compile(r"(?m)^\s*pub\s+use\s+([^;]+);")
TEST_RE = re.compile(r"#\s*\[\s*(?:(?:tokio|async_std)::)?test(?:\s*\([^]]*\))?\s*\]")
LITERAL_INCLUDE_RE = re.compile(r'include_str!\(\s*"([^"]+)"\s*\)')
PATH_MOD_RE = re.compile(
    r'(?ms)#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]\s*'
    r'(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*;'
)
MOD_RE = re.compile(
    r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"
)


def git_text(repo: Path, ref: str, relative: str) -> str:
    result = subprocess.run(
        ["git", "show", f"{ref}:{relative}"],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise ValueError(f"{relative} does not exist at {ref}: {result.stderr.strip()}")
    return result.stdout


def git_text_if_present(repo: Path, ref: str, relative: str) -> str | None:
    """Return a tracked companion's baseline without failing for new split files."""
    result = subprocess.run(
        ["git", "show", f"{ref}:{relative}"],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    return result.stdout if result.returncode == 0 else None


def line_count(text: str) -> int:
    return len(text.splitlines())


def public_names(text: str) -> set[str]:
    names = set(PUBLIC_DECL_RE.findall(text))
    for expression in PUBLIC_USE_RE.findall(text):
        expression = expression.strip()
        if "{" in expression and "}" in expression:
            body = expression.split("{", 1)[1].rsplit("}", 1)[0]
            for item in body.split(","):
                name = item.strip().split(" as ")[-1].strip()
                if name and name != "self" and "*" not in name:
                    names.add(name)
        else:
            name = expression.split(" as ")[-1].rsplit("::", 1)[-1].strip()
            if name and name != "*":
                names.add(name)
    return names


def current_fragments(repo: Path, source: Path) -> list[Path]:
    absolute = repo / source
    seeds: list[Path] = []
    if absolute.is_file():
        seeds.append(absolute)
    module_dir = absolute.with_suffix("")
    if module_dir.is_dir():
        seeds.extend(sorted(module_dir.rglob("*.rs")))

    candidates: set[Path] = set()
    pending = list(seeds)
    while pending:
        path = pending.pop()
        if path in candidates or not path.is_file():
            continue
        candidates.add(path)
        text = path.read_text(encoding="utf-8")
        explicit = set(PATH_MOD_RE.findall(text))
        for value in explicit:
            target = (path.parent / value).resolve()
            if target.is_file():
                pending.append(target)
        content_without_explicit = PATH_MOD_RE.sub("", text)
        child_root = path.parent if path.name == "mod.rs" else path.parent / path.stem
        for name in MOD_RE.findall(content_without_explicit):
            flat = child_root / f"{name}.rs"
            nested = child_root / name / "mod.rs"
            if flat.is_file():
                pending.append(flat.resolve())
            elif nested.is_file():
                pending.append(nested.resolve())
    return sorted(candidates)


def nearest_package(path: Path, stop: Path) -> tuple[str | None, Path | None]:
    current = path if path.is_dir() else path.parent
    while current == stop or stop in current.parents:
        manifest = current / "Cargo.toml"
        if manifest.is_file():
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
            return data.get("package", {}).get("name"), manifest
        if current == stop:
            break
        current = current.parent
    return None, None


def stale_source_references(repo: Path, rust_root: Path, source: Path) -> list[str]:
    """Find simple include_str! calls that still name a removed monolith."""
    original = (repo / source).resolve()
    if original.is_file():
        return []
    references: list[str] = []
    for consumer in sorted(rust_root.rglob("*.rs")):
        if "target" in consumer.parts:
            continue
        text = consumer.read_text(encoding="utf-8", errors="ignore")
        for value in LITERAL_INCLUDE_RE.findall(text):
            if (consumer.parent / value).resolve() == original:
                references.append(consumer.relative_to(repo).as_posix())
                break
    return references


def validate(repo: Path, rust_root: Path, source: Path, before_ref: str) -> dict[str, object]:
    # macOS may spell the same temporary directory as /var/... and
    # /private/var/.... Canonicalize both anchors before relative-path checks.
    repo = repo.resolve()
    rust_root = rust_root.resolve()
    relative = source.as_posix()
    before = git_text(repo, before_ref, relative)
    fragments = current_fragments(repo, source)
    problems: list[str] = []
    if not fragments:
        problems.append(f"split has no current Rust fragments for {relative}")

    files: list[dict[str, object]] = []
    after_texts: list[str] = []
    for path in fragments:
        text = path.read_text(encoding="utf-8")
        after_texts.append(text)
        lines = line_count(text)
        rel = path.relative_to(repo).as_posix()
        files.append({"path": rel, "lines": lines})
        if lines > HARD_LIMIT:
            problems.append(f"oversized fragment: {rel} has {lines} lines")
        if NUMERIC_SHARD_RE.search(path.stem):
            problems.append(f"mechanical numeric shard name: {rel}")

    module_dir = (repo / source).with_suffix("")
    module_root = module_dir / "mod.rs"
    if module_root.is_file():
        root_lines = line_count(module_root.read_text(encoding="utf-8"))
        if root_lines > ROOT_LIMIT:
            problems.append(f"module root exceeds {ROOT_LIMIT} lines: {module_root.relative_to(repo)}")

    if line_count(before) > HARD_LIMIT and len(fragments) < 2:
        problems.append("an oversized source must become at least two cohesive fragments")

    stale_references = stale_source_references(repo, rust_root, source)
    if stale_references:
        problems.append(
            "removed monolith still referenced by include_str!: "
            + ", ".join(stale_references)
        )

    before_tests = len(TEST_RE.findall(before))
    after_tests = sum(len(TEST_RE.findall(text)) for text in after_texts)
    if after_tests < before_tests:
        problems.append(f"test attributes decreased: {before_tests} -> {after_tests}")

    before_public = public_names(before)
    # A large source can already depend on separately tracked sibling modules.
    # Those declarations are part of the pre-split API baseline, not API growth
    # introduced by the split under validation.
    for path in fragments:
        fragment_relative = path.relative_to(repo).as_posix()
        if fragment_relative == relative:
            continue
        baseline = git_text_if_present(repo, before_ref, fragment_relative)
        if baseline is not None:
            before_public.update(public_names(baseline))
    after_public: set[str] = set()
    for text in after_texts:
        after_public.update(public_names(text))
    added_public = sorted(after_public - before_public)
    if added_public:
        problems.append("new public API names: " + ", ".join(added_public))

    package, manifest = nearest_package(
        fragments[0] if fragments else repo / source,
        rust_root,
    )
    commands = []
    if package:
        commands.extend(
            [
                ["cargo", "check", "--locked", "-p", package, "--tests"],
                ["cargo", "test", "--locked", "-p", package, "--no-run"],
            ]
        )
    commands.append(["git", "diff", "--check"])

    return {
        "schema_version": 1,
        "source": relative,
        "before_ref": before_ref,
        "before_lines": line_count(before),
        "target_lines": TARGET_LIMIT,
        "hard_limit": HARD_LIMIT,
        "module_root_limit": ROOT_LIMIT,
        "fragments": files,
        "tests": {"before": before_tests, "after": after_tests},
        "stale_source_references": stale_references,
        "public_api": {
            "before": sorted(before_public),
            "after": sorted(after_public),
            "added": added_public,
        },
        "package": package,
        "manifest": manifest.relative_to(repo).as_posix() if manifest else None,
        "recommended_commands": commands,
        "problems": problems,
        "passed": not problems,
    }


def parse_args() -> argparse.Namespace:
    script = Path(__file__).resolve()
    repo = script.parents[4]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="repository-relative pre-split .rs path")
    parser.add_argument("--repo-root", type=Path, default=repo)
    parser.add_argument("--before-ref", default="HEAD")
    parser.add_argument("--json", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo = args.repo_root.resolve()
    rust_root = repo / "GeneralsRust"
    report = validate(repo, rust_root, args.source, args.before_ref)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(
            f"Rust split: {report['source']} {report['before_lines']} lines -> "
            f"{len(report['fragments'])} fragments; passed={report['passed']}"
        )
        for fragment in report["fragments"]:
            print(f"  {fragment['lines']:>5}  {fragment['path']}")
        for problem in report["problems"]:
            print(f"ERROR: {problem}")
        print("Recommended verification:")
        for command in report["recommended_commands"]:
            print("  " + " ".join(command))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
