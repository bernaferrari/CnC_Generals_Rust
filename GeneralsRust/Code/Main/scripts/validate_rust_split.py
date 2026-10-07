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
PUBLIC_DECL_OCCURRENCE_RE = re.compile(
    r"(?m)^\s*(?P<visibility>pub(?:\((?P<restricted>[^)]*)\))?)\s+"
    r"(?:async\s+)?(?:unsafe\s+)?"
    r"(?P<kind>fn|struct|enum|trait|type|const|static|mod)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b"
)
PUBLIC_USE_RE = re.compile(r"(?m)^\s*pub\s+use\s+([^;]+);")
PUBLIC_USE_OCCURRENCE_RE = re.compile(
    r"(?m)^\s*(?P<visibility>pub(?:\((?P<restricted>[^)]*)\))?)\s+use\s+([^;]+);"
)
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


def public_occurrences(text: str) -> list[dict[str, str]]:
    """Return public declaration and re-export name occurrences with visibility."""
    occurrences = [
        {
            "name": match.group("name"),
            "visibility": match.group("restricted") or "public",
            "kind": match.group("kind"),
            "form": "declaration",
        }
        for match in PUBLIC_DECL_OCCURRENCE_RE.finditer(text)
    ]
    for match in PUBLIC_USE_OCCURRENCE_RE.finditer(text):
        visibility = match.group("restricted") or "public"
        expression = match.group(3).strip()
        if "{" in expression and "}" in expression:
            body = expression.split("{", 1)[1].rsplit("}", 1)[0]
            names = [item.strip().split(" as ")[-1].strip() for item in body.split(",")]
        else:
            names = [expression.split(" as ")[-1].rsplit("::", 1)[-1].strip()]
        for name in names:
            if name and name != "self" and "*" not in name:
                occurrences.append(
                    {"name": name, "visibility": visibility, "kind": "use", "form": "re-export"}
                )
    return occurrences


def expected_crate_api_report(
    repo: Path,
    api_paths: set[Path],
    before_public: set[str],
    added_public: list[str],
    requested: list[tuple[Path, str]],
) -> tuple[list[dict[str, object]], list[str], set[str]]:
    """Validate narrowly allowed pub(crate) additions in explicitly named fragments."""
    reports: list[dict[str, object]] = []
    problems: list[str] = []
    accepted: set[str] = set()
    requested_keys: set[tuple[Path, str]] = set()
    for raw_path, name in requested:
        path = raw_path
        report: dict[str, object] = {"path": path.as_posix(), "name": name, "accepted": False}
        reports.append(report)
        key = (path, name)
        if key in requested_keys:
            problems.append(f"duplicate expected crate API request: {path.as_posix()}:{name}")
            continue
        requested_keys.add(key)
        if path.is_absolute():
            problems.append(f"expected crate API path must be repository-relative: {path}")
            continue
        resolved = (repo / path).resolve()
        try:
            resolved.relative_to(repo)
        except ValueError:
            problems.append(f"expected crate API path escapes repository: {path.as_posix()}")
            continue
        if resolved not in api_paths or not resolved.is_file():
            problems.append(
                f"expected crate API path is not an in-scope current fragment: {path.as_posix()}"
            )
            continue
        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
            problems.append(f"invalid expected crate API name: {name}")
            continue
        if name in before_public:
            problems.append(f"expected crate API is stale, already present before split: {name}")
            continue
        if name not in added_public:
            problems.append(f"expected crate API declaration is missing: {path.as_posix()}:{name}")
            continue

        by_path: list[dict[str, str]] = []
        all_occurrences: list[tuple[Path, dict[str, str]]] = []
        for candidate in api_paths:
            for occurrence in public_occurrences(candidate.read_text(encoding="utf-8")):
                if occurrence["name"] == name:
                    all_occurrences.append((candidate, occurrence))
                    if candidate == resolved:
                        by_path.append(occurrence)
        exact = [
            item for item in by_path
            if item["form"] == "declaration" and item["visibility"] == "crate"
        ]
        if len(exact) != 1 or len(by_path) != 1 or len(all_occurrences) != 1:
            problems.append(
                f"expected crate API must be one unique pub(crate) declaration in its named fragment: {path.as_posix()}:{name}"
            )
            continue
        accepted.add(name)
        report["accepted"] = True

    return reports, problems, accepted


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
        pending.extend(rust_module_children(path, text))
    return sorted(candidates)


def rust_module_children(path: Path, text: str) -> list[Path]:
    """Resolve file-backed Rust modules declared directly by one source file."""
    children: list[Path] = []
    for value in PATH_MOD_RE.findall(text):
        target = (path.parent / value).resolve()
        if target.is_file():
            children.append(target)

    content_without_explicit = PATH_MOD_RE.sub("", text)
    # A crate root's children live beside lib.rs/main.rs. A module file such as
    # ai.rs instead owns ai/*.rs; mod.rs already is that directory's root.
    if path.name in {"lib.rs", "main.rs", "mod.rs"}:
        child_root = path.parent
    else:
        child_root = path.parent / path.stem
    for name in MOD_RE.findall(content_without_explicit):
        flat = child_root / f"{name}.rs"
        nested = child_root / name / "mod.rs"
        if flat.is_file():
            children.append(flat.resolve())
        elif nested.is_file():
            children.append(nested.resolve())
    return children


def reachable_rust_sources(crate_root: Path) -> list[Path]:
    """Return the Rust source tree reachable from a crate's declared lib root."""
    pending = [crate_root.resolve()]
    sources: set[Path] = set()
    while pending:
        path = pending.pop()
        if path in sources or not path.is_file() or path.suffix != ".rs":
            continue
        sources.add(path)
        pending.extend(rust_module_children(path, path.read_text(encoding="utf-8")))
    return sorted(sources)


def path_dependencies(manifest_data: dict[str, object]) -> list[tuple[str, Path]]:
    """Find declared local path dependencies, including target-specific tables."""
    found: list[tuple[str, Path]] = []

    def inspect(table: object) -> None:
        if not isinstance(table, dict):
            return
        for dependency_name, dependency in table.items():
            if not isinstance(dependency, dict) or not isinstance(dependency.get("path"), str):
                continue
            found.append((str(dependency_name), Path(dependency["path"])))

    for key in ("dependencies", "dev-dependencies", "build-dependencies"):
        inspect(manifest_data.get(key))
    target_tables = manifest_data.get("target")
    if isinstance(target_tables, dict):
        for target in target_tables.values():
            if isinstance(target, dict):
                for key in ("dependencies", "dev-dependencies", "build-dependencies"):
                    inspect(target.get(key))
    return found


def declared_dependency_sources(manifest: Path) -> dict[Path, tuple[Path, list[Path]]]:
    """Map each source reachable through one direct local path dependency to its package."""
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    reachable: dict[Path, tuple[Path, list[Path]]] = {}
    for _, dependency_path in path_dependencies(data):
        package_root = (manifest.parent / dependency_path).resolve()
        dependency_manifest = package_root / "Cargo.toml"
        if not dependency_manifest.is_file():
            continue
        package_data = tomllib.loads(dependency_manifest.read_text(encoding="utf-8"))
        lib_config = package_data.get("lib", {})
        lib_relative = (
            lib_config.get("path", "src/lib.rs")
            if isinstance(lib_config, dict)
            else "src/lib.rs"
        )
        lib_root = (package_root / lib_relative).resolve()
        if not lib_root.is_file():
            continue
        package_sources = reachable_rust_sources(lib_root)
        for source in package_sources:
            reachable[source] = (package_root, package_sources)
    return reachable


def extracted_source_tree(
    repo: Path,
    original_manifest: Path | None,
    requested: list[Path],
) -> tuple[list[Path], list[Path], list[str]]:
    """Validate explicit cross-crate sources and return their reachable package trees."""
    if not requested:
        return [], [], []
    if original_manifest is None:
        return [], [], ["cannot validate extracted sources: original package manifest not found"]

    reachable = declared_dependency_sources(original_manifest)
    included: set[Path] = set()
    selected: set[Path] = set()
    problems: list[str] = []
    for source_arg in requested:
        if source_arg.is_absolute():
            problems.append(f"extracted source must be repository-relative: {source_arg}")
            continue
        source = (repo / source_arg).resolve()
        try:
            source.relative_to(repo)
        except ValueError:
            problems.append(f"extracted source escapes repository: {source_arg}")
            continue
        if not source.is_file() or source.suffix != ".rs":
            problems.append(f"extracted source is not a Rust source file: {source_arg}")
            continue
        package_tree = reachable.get(source)
        if package_tree is None:
            problems.append(
                "extracted source is not reachable from a declared local path dependency "
                f"of the original package: {source_arg.as_posix()}"
            )
            continue
        selected.add(source)
        included.update(package_tree[1])
    return sorted(included), sorted(selected), problems


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


def validate(
    repo: Path,
    rust_root: Path,
    source: Path,
    before_ref: str,
    extracted_sources: list[Path] | None = None,
    expected_crate_api: list[tuple[Path, str]] | None = None,
) -> dict[str, object]:
    # macOS may spell the same temporary directory as /var/... and
    # /private/var/.... Canonicalize both anchors before relative-path checks.
    repo = repo.resolve()
    rust_root = rust_root.resolve()
    relative = source.as_posix()
    before = git_text(repo, before_ref, relative)
    fragments = current_fragments(repo, source)
    package, manifest = nearest_package(repo / source, rust_root)
    extracted, selected_extracted, extracted_problems = extracted_source_tree(
        repo, manifest, extracted_sources or []
    )
    original_fragments = fragments
    fragments = sorted(set(fragments) | set(extracted))
    problems: list[str] = list(extracted_problems)
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
    # Only mapped source subtrees may satisfy this source's coverage count.
    # Tests in unrelated siblings of a shared dependency cannot replace lost tests.
    test_paths = set(original_fragments)
    for extracted_source in selected_extracted:
        test_paths.update(reachable_rust_sources(extracted_source))
    after_tests = sum(
        len(TEST_RE.findall(path.read_text(encoding="utf-8"))) for path in test_paths
    )
    if after_tests < before_tests:
        problems.append(f"test attributes decreased: {before_tests} -> {after_tests}")

    before_public = public_names(before)
    # A large source can already depend on separately tracked sibling modules.
    # Those declarations are part of the pre-split API baseline, not API growth
    # introduced by the split under validation.
    for path in original_fragments:
        fragment_relative = path.relative_to(repo).as_posix()
        if fragment_relative == relative:
            continue
        baseline = git_text_if_present(repo, before_ref, fragment_relative)
        if baseline is not None:
            before_public.update(public_names(baseline))
    after_public: set[str] = set()
    # The full extracted package tree contributes its tests and is checked for
    # fragment hygiene, but public-API comparison is scoped to the files the
    # caller explicitly maps from this original source. Sibling modules in a
    # path dependency can own unrelated contracts and must not mask or invent
    # API changes for the source being split.
    api_paths = set(original_fragments) | set(selected_extracted)
    for path in api_paths:
        after_public.update(public_names(path.read_text(encoding="utf-8")))
    added_public = sorted(after_public - before_public)
    expected_reports, expected_problems, accepted_crate_api = expected_crate_api_report(
        repo, api_paths, before_public, added_public, expected_crate_api or []
    )
    problems.extend(expected_problems)
    unaccepted_public = sorted(set(added_public) - accepted_crate_api)
    if unaccepted_public:
        problems.append("new public API names: " + ", ".join(unaccepted_public))

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
        "extracted_sources": [
            path.relative_to(repo).as_posix() for path in selected_extracted
        ],
        "tests": {"before": before_tests, "after": after_tests},
        "stale_source_references": stale_references,
        "public_api": {
            "before": sorted(before_public),
            "after": sorted(after_public),
            "added": added_public,
            "expected_crate_api": {
                "requested": [
                    {"path": path.as_posix(), "name": name}
                    for path, name in (expected_crate_api or [])
                ],
                "accepted": [
                    item
                    for item in expected_reports
                    if item["accepted"]
                ],
                "checks": expected_reports,
            },
        },
        "package": package,
        "manifest": manifest.relative_to(repo).as_posix() if manifest else None,
        "recommended_commands": commands,
        "problems": problems,
        "passed": not problems,
    }


def parse_expected_crate_api(value: str) -> tuple[Path, str]:
    path_text, separator, name = value.rpartition(":")
    if not separator or not path_text or not name:
        raise argparse.ArgumentTypeError("expected PATH:NAME")
    return Path(path_text), name


def parse_args() -> argparse.Namespace:
    script = Path(__file__).resolve()
    repo = script.parents[4]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="repository-relative pre-split .rs path")
    parser.add_argument("--repo-root", type=Path, default=repo)
    parser.add_argument("--before-ref", default="HEAD")
    parser.add_argument(
        "--extracted-source",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="repo-relative Rust source moved into a reachable local path dependency; repeatable",
    )
    parser.add_argument(
        "--expected-crate-api",
        action="append",
        type=parse_expected_crate_api,
        default=[],
        metavar="PATH:NAME",
        help="allow one exact new pub(crate) declaration in this in-scope fragment; repeatable",
    )
    parser.add_argument("--json", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo = args.repo_root.resolve()
    rust_root = repo / "GeneralsRust"
    report = validate(
        repo,
        rust_root,
        args.source,
        args.before_ref,
        args.extracted_source,
        args.expected_crate_api,
    )
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
