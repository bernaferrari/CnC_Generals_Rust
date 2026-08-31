#!/usr/bin/env python3
"""Generate a deterministic, repository-wide C++ -> Rust provenance inventory.

This inventory deliberately separates four questions which the historical stem
matrix conflated:

1. Is the C++ unit inside the declared port scope?
2. Is there a Rust candidate (possibly a split module tree)?
3. Is that Rust code production implementation code reachable from a Cargo root?
4. Has behavioral parity been proved?

Only explicit reviewed mappings receive ``reviewed_path_implementation`` credit.
Inferred candidates remain useful discovery evidence, but strict checking fails
until they are reviewed.  Tests, shims, and telemetry are never implementation
evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tempfile
from collections import Counter, defaultdict, deque
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

from behavior_evidence import (
    EVIDENCE_FILE,
    DestinationState,
    EvidenceUnit,
    INFORMATIONAL_KINDS,
    evaluate_unit,
    evidence_digest_rows,
    load_behavior_evidence,
)


SCHEMA_VERSION = 1
REVIEWED_MAPPINGS_FILE = "PORT_PROVENANCE_REVIEWED.json"
CPP_TRANSLATION_SUFFIXES = (".cpp", ".cxx", ".cc", ".c")
CPP_HEADER_SUFFIXES = (".h", ".hpp", ".inl")


@dataclass(frozen=True)
class ScopeRoot:
    scope_id: str
    cpp_root: str
    rust_roots: tuple[str, ...]
    inventory_class: str
    include_headers: bool = True


SCOPE_ROOTS: tuple[ScopeRoot, ...] = (
    ScopeRoot(
        "game_engine",
        "GeneralsMD/Code/GameEngine",
        ("GeneralsRust/Code/GameEngine",),
        "required_runtime",
    ),
    ScopeRoot(
        "game_engine_device",
        "GeneralsMD/Code/GameEngineDevice",
        ("GeneralsRust/Code/GameEngine/GameEngineDevice",),
        "required_device",
    ),
    ScopeRoot(
        "libraries",
        "GeneralsMD/Code/Libraries",
        ("GeneralsRust/Code/Libraries",),
        "required_library",
    ),
    ScopeRoot(
        "tools",
        "GeneralsMD/Code/Tools",
        ("GeneralsRust/Code/Tools",),
        "required_tooling",
    ),
    ScopeRoot(
        "main",
        "GeneralsMD/Code/Main",
        ("GeneralsRust/Code/Main",),
        "required_entrypoint",
    ),
)


ALLOWED_DEVIATIONS = (
    {
        "id": "directx_to_wgpu",
        "classification": "allowed_platform_substitution",
        "constraint": (
            "WGPU may replace DirectX rendering APIs, but observable rendering, "
            "resource lifetime, ordering, and state behavior still require evidence."
        ),
        "behavior_verified": False,
    },
    {
        "id": "representation_safe_enum_cleanup",
        "classification": "allowed_representation_cleanup",
        "constraint": (
            "Enum cleanup is allowed only when discriminants, flag layouts, "
            "serialization, defaults, and observable behavior remain compatible."
        ),
        "behavior_verified": False,
    },
)


ALLOWED_DEVIATION_IDS = tuple(deviation["id"] for deviation in ALLOWED_DEVIATIONS)


def load_reviewed_mappings(repo_root: Path) -> dict[str, tuple[str, ...]]:
    """Load human-reviewed path ownership from a data file agents can edit safely."""
    path = repo_root / REVIEWED_MAPPINGS_FILE
    if not path.is_file():
        return {}
    raw = json.loads(path.read_text(encoding="utf-8"))
    if raw.get("schema_version") != 1:
        raise ValueError(f"{path} schema_version must be 1")
    records = raw.get("mappings")
    if not isinstance(records, list):
        raise ValueError(f"{path} mappings must be a list")
    result: dict[str, tuple[str, ...]] = {}
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            raise ValueError(f"{path} mapping {index} must be an object")
        source = record.get("source")
        destinations = record.get("destinations")
        if not isinstance(source, str) or not source:
            raise ValueError(f"{path} mapping {index} needs a source path")
        if source in result:
            raise ValueError(f"{path} duplicates reviewed source {source}")
        if (
            not isinstance(destinations, list)
            or not destinations
            or not all(isinstance(item, str) and item for item in destinations)
        ):
            raise ValueError(f"{path} mapping {index} needs destination paths")
        if len(set(destinations)) != len(destinations):
            raise ValueError(f"{path} mapping {index} duplicates a destination")
        result[source] = tuple(destinations)
    return result


HEX64_RE = re.compile(r"[0-9a-f]{64}")


@dataclass(frozen=True)
class ReviewedAssignment:
    """One Rust destination claim for one C++ symbol occurrence."""

    path: str
    sha256: str
    symbols: tuple[str, ...]
    deviation: str | None


@dataclass(frozen=True)
class ReviewedSymbol:
    """A C++ symbol occurrence identified by qualified name and line.

    Line numbers keep overload sets distinct: every occurrence extracted from
    the translation unit must be claimed before ownership counts as reviewed.
    """

    name: str
    line: int
    assignments: tuple[ReviewedAssignment, ...]


@dataclass(frozen=True)
class ReviewedRange:
    start_line: int
    end_line: int
    path: str
    sha256: str


@dataclass(frozen=True)
class ReviewedOwnership:
    """Reviewed symbol and source-range ownership for one C++ translation unit."""

    source: str
    source_sha256: str
    symbols: tuple[ReviewedSymbol, ...]
    source_ranges: tuple[ReviewedRange, ...]


def load_reviewed_ownership(repo_root: Path) -> dict[str, ReviewedOwnership]:
    """Load reviewed symbol/source-range ownership from the agent-editable file.

    Structural errors (wrong types, duplicate claims, malformed hashes) raise
    immediately.  Semantic staleness (hash drift, moved files, uncovered
    ranges) is diagnosed later by the manifest builder against live files.
    """
    path = repo_root / REVIEWED_MAPPINGS_FILE
    if not path.is_file():
        return {}
    raw = json.loads(path.read_text(encoding="utf-8"))
    if raw.get("schema_version") != 1:
        raise ValueError(f"{path} schema_version must be 1")
    records = raw.get("ownership", [])
    if not isinstance(records, list):
        raise ValueError(f"{path} ownership must be a list")
    result: dict[str, ReviewedOwnership] = {}
    for index, record in enumerate(records):
        where = f"{path} ownership {index}"
        if not isinstance(record, dict):
            raise ValueError(f"{where} must be an object")
        source = record.get("source")
        if not isinstance(source, str) or not source:
            raise ValueError(f"{where} needs a source path")
        if source in result:
            raise ValueError(f"{path} duplicates ownership source {source}")
        source_sha256 = record.get("source_sha256")
        if not isinstance(source_sha256, str) or not HEX64_RE.fullmatch(source_sha256):
            raise ValueError(f"{where} needs a 64-hex source_sha256")
        symbols_raw = record.get("symbols", [])
        ranges_raw = record.get("source_ranges", [])
        if not isinstance(symbols_raw, list) or not isinstance(ranges_raw, list):
            raise ValueError(f"{where} symbols and source_ranges must be lists")

        symbols: list[ReviewedSymbol] = []
        seen_symbols: set[tuple[str, int]] = set()
        for symbol_index, symbol_raw in enumerate(symbols_raw):
            symbol_where = f"{where} symbol {symbol_index}"
            if not isinstance(symbol_raw, dict):
                raise ValueError(f"{symbol_where} must be an object")
            name = symbol_raw.get("name")
            line = symbol_raw.get("line")
            if not isinstance(name, str) or not name:
                raise ValueError(f"{symbol_where} needs a symbol name")
            if not isinstance(line, int) or isinstance(line, bool) or line < 1:
                raise ValueError(f"{symbol_where} needs a positive integer line")
            if (name, line) in seen_symbols:
                raise ValueError(f"{where} duplicates symbol {name}@{line}")
            seen_symbols.add((name, line))
            assignments_raw = symbol_raw.get("assignments")
            if not isinstance(assignments_raw, list) or not assignments_raw:
                raise ValueError(f"{symbol_where} needs assignments")
            assignments: list[ReviewedAssignment] = []
            for assignment_index, assignment_raw in enumerate(assignments_raw):
                assignment_where = f"{symbol_where} assignment {assignment_index}"
                if not isinstance(assignment_raw, dict):
                    raise ValueError(f"{assignment_where} must be an object")
                destination = assignment_raw.get("path")
                if not isinstance(destination, str) or not destination:
                    raise ValueError(f"{assignment_where} needs a destination path")
                sha256 = assignment_raw.get("sha256")
                if not isinstance(sha256, str) or not HEX64_RE.fullmatch(sha256):
                    raise ValueError(f"{assignment_where} needs a 64-hex sha256")
                rust_symbols = assignment_raw.get("symbols")
                if (
                    not isinstance(rust_symbols, list)
                    or not rust_symbols
                    or not all(isinstance(item, str) and item for item in rust_symbols)
                ):
                    raise ValueError(f"{assignment_where} needs destination symbol names")
                if len(set(rust_symbols)) != len(rust_symbols):
                    raise ValueError(f"{assignment_where} duplicates a destination symbol")
                deviation = assignment_raw.get("deviation")
                if deviation is not None and not isinstance(deviation, str):
                    raise ValueError(f"{assignment_where} deviation must be a string or null")
                assignments.append(
                    ReviewedAssignment(destination, sha256, tuple(rust_symbols), deviation)
                )
            symbols.append(ReviewedSymbol(name, line, tuple(assignments)))

        ranges: list[ReviewedRange] = []
        for range_index, range_raw in enumerate(ranges_raw):
            range_where = f"{where} source_range {range_index}"
            if not isinstance(range_raw, dict):
                raise ValueError(f"{range_where} must be an object")
            start_line = range_raw.get("start_line")
            end_line = range_raw.get("end_line")
            if (
                not isinstance(start_line, int)
                or isinstance(start_line, bool)
                or start_line < 1
                or not isinstance(end_line, int)
                or isinstance(end_line, bool)
                or end_line < start_line
            ):
                raise ValueError(f"{range_where} needs 1 <= start_line <= end_line")
            destination = range_raw.get("path")
            if not isinstance(destination, str) or not destination:
                raise ValueError(f"{range_where} needs a destination path")
            sha256 = range_raw.get("sha256")
            if not isinstance(sha256, str) or not HEX64_RE.fullmatch(sha256):
                raise ValueError(f"{range_where} needs a 64-hex sha256")
            ranges.append(ReviewedRange(start_line, end_line, destination, sha256))

        result[source] = ReviewedOwnership(
            source, source_sha256, tuple(symbols), tuple(ranges)
        )
    return result


MOD_RE = re.compile(
    r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;"
)
PATH_MOD_RE = re.compile(
    r'(?ms)#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]\s*'
    r'(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*;'
)
INCLUDE_RE = re.compile(r'include!\s*\(\s*"([^"]+)"\s*\)')
RUST_SYMBOL_RE = re.compile(
    r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
    r"(fn|struct|enum|trait|type|const|static)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
CPP_QUALIFIED_SYMBOL_RE = re.compile(
    r"(?m)^\s*[^#/{;\n][^;\n]*?\b"
    r"([A-Za-z_~][A-Za-z0-9_~]*(?:::[A-Za-z_~][A-Za-z0-9_~]*)+)\s*\("
)


def normalize_name(value: str) -> str:
    return "".join(ch for ch in value.lower() if ch.isalnum())


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def line_count(path: Path) -> int:
    with path.open("r", encoding="utf-8", errors="ignore") as handle:
        return sum(1 for _line in handle)


def extract_cpp_symbols(path: Path) -> list[dict[str, object]]:
    content = path.read_text(encoding="utf-8", errors="ignore")
    symbols: list[dict[str, object]] = []
    seen: set[tuple[str, int]] = set()
    for match in CPP_QUALIFIED_SYMBOL_RE.finditer(content):
        name = match.group(1)
        start_line = content.count("\n", 0, match.start()) + 1
        key = (name, start_line)
        if key in seen:
            continue
        seen.add(key)
        symbols.append({"name": name, "line": start_line})
    return symbols


def relative_posix(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def classify_inventory(scope: ScopeRoot, source_path: str) -> str:
    normalized = source_path.replace("\\", "/")
    if "/GameNetwork/" in normalized:
        return "deferred_network"
    return scope.inventory_class


def classify_rust_file(path: Path, content: str) -> str:
    lower_parts = {part.lower() for part in path.parts}
    lower_name = path.name.lower()
    if (
        "tests" in lower_parts
        or "examples" in lower_parts
        or lower_name in {"test.rs", "tests.rs", "parity_tests.rs"}
        or lower_name.endswith("_test.rs")
        or lower_name.endswith("_tests.rs")
    ):
        return "test"
    if "telemetry-only" in content.lower() or "telemetry only" in content.lower():
        return "telemetry"

    code_lines = [
        line.strip()
        for line in content.splitlines()
        if line.strip() and not line.lstrip().startswith(("//", "#!", "//!"))
    ]
    has_declaration = bool(RUST_SYMBOL_RE.search(content)) or bool(re.search(r"(?m)^\s*impl\b", content))
    if len(code_lines) <= 24 and "pub use " in content and not has_declaration:
        return "shim"
    return "implementation"


def cargo_roots(rust_repo_root: Path) -> list[Path]:
    roots: set[Path] = set()
    for manifest in sorted(rust_repo_root.rglob("Cargo.toml")):
        if "target" in manifest.parts:
            continue
        crate = manifest.parent
        for conventional in (crate / "src/lib.rs", crate / "src/main.rs"):
            if conventional.is_file():
                roots.add(conventional.resolve())
        for directory in (crate / "src/bin", crate / "tests", crate / "examples"):
            if directory.is_dir():
                roots.update(path.resolve() for path in directory.rglob("*.rs"))
    return sorted(roots)


def resolve_declared_module(parent: Path, name: str) -> Path | None:
    flat = parent / f"{name}.rs"
    nested = parent / name / "mod.rs"
    if flat.is_file():
        return flat.resolve()
    if nested.is_file():
        return nested.resolve()
    return None


def collect_reachable_rust(rust_repo_root: Path) -> set[Path]:
    """Conservatively walk Rust module/include declarations from Cargo roots."""
    reachable: set[Path] = set()
    queue: deque[Path] = deque(cargo_roots(rust_repo_root))
    while queue:
        path = queue.popleft().resolve()
        if path in reachable or not path.is_file():
            continue
        reachable.add(path)
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            content = path.read_text(encoding="utf-8", errors="ignore")

        explicit_paths = set(PATH_MOD_RE.findall(content))
        for value in sorted(explicit_paths):
            target = (path.parent / value).resolve()
            if target.is_file() and target not in reachable:
                queue.append(target)

        # Remove #[path] declarations before the conventional module pass so a
        # path-qualified declaration is not resolved twice.
        conventional_content = PATH_MOD_RE.sub("", content)
        for name in sorted(set(MOD_RE.findall(conventional_content))):
            target = resolve_declared_module(path.parent, name)
            if target is not None and target not in reachable:
                queue.append(target)

        for value in sorted(set(INCLUDE_RE.findall(content))):
            target = (path.parent / value).resolve()
            if target.is_file() and target not in reachable:
                queue.append(target)
    return reachable


@dataclass(frozen=True)
class RustFileInfo:
    absolute: Path
    relative: str
    classification: str
    cargo_reachable: bool
    sha256: str
    symbols: tuple[tuple[str, str, int], ...]
    line_count: int


def collect_rust_files(repo_root: Path) -> tuple[list[RustFileInfo], set[Path]]:
    rust_repo_root = repo_root / "GeneralsRust"
    reachable = collect_reachable_rust(rust_repo_root)
    infos: list[RustFileInfo] = []
    for path in sorted((rust_repo_root / "Code").rglob("*.rs")):
        if "target" in path.parts or any(part.startswith(".cargo-") for part in path.parts):
            continue
        content = path.read_text(encoding="utf-8", errors="ignore")
        symbols = tuple(
            (kind, name, content.count("\n", 0, match.start()) + 1)
            for match in RUST_SYMBOL_RE.finditer(content)
            for kind, name in [match.groups()]
        )
        infos.append(
            RustFileInfo(
                absolute=path.resolve(),
                relative=relative_posix(path, repo_root),
                classification=classify_rust_file(path, content),
                cargo_reachable=path.resolve() in reachable,
                sha256=sha256_file(path),
                symbols=symbols,
                line_count=content.count("\n") + (0 if not content else 1),
            )
        )
    return infos, reachable


def source_area(source_path: Path, scope: ScopeRoot) -> str:
    marker = scope.cpp_root.rstrip("/") + "/"
    source_posix = source_path.as_posix()
    if marker not in source_posix:
        return scope.scope_id
    rel = Path(source_posix.split(marker, 1)[1])
    parts = [part for part in rel.parts if part not in {"Source", "Include", "src", "include"}]
    return parts[0] if parts else scope.scope_id


def allowed_rust_prefixes(scope: ScopeRoot) -> tuple[str, ...]:
    return tuple(root.rstrip("/") + "/" for root in scope.rust_roots)


def score_candidate(info: RustFileInfo, scope: ScopeRoot, area: str) -> tuple[int, int, int, str]:
    prefixes = allowed_rust_prefixes(scope)
    in_scope = any(info.relative.startswith(prefix) for prefix in prefixes)
    area_marker = f"/{area}/".lower()
    same_area = area_marker in f"/{info.relative}/".lower()
    production = info.classification == "implementation"
    return (-int(in_scope), -int(same_area), -int(production), info.relative)


def expand_split_root(root: RustFileInfo, by_absolute: dict[Path, RustFileInfo]) -> list[RustFileInfo]:
    if root.absolute.name != "mod.rs":
        return [root]
    directory = root.absolute.parent
    members = [
        info
        for path, info in by_absolute.items()
        if path == root.absolute or directory in path.parents
    ]
    return sorted(members, key=lambda info: info.relative)


def destination_record(
    info: RustFileInfo,
    owned_symbols: Iterable[str] = (),
    owned_source_ranges: Iterable[tuple[int, int]] = (),
) -> dict[str, object]:
    return {
        "path": info.relative,
        "classification": info.classification,
        "cargo_reachable": info.cargo_reachable,
        # Populated only by complete valid reviewed ownership records that
        # assign C++ symbols or source ranges to this fragment. Empty means
        # ownership still needs review.
        "owned_symbols": sorted(set(owned_symbols)),
        "owned_source_ranges": [
            {"start_line": start, "end_line": end}
            for start, end in sorted(set(owned_source_ranges))
        ],
    }


def ownership_referenced_paths(record: ReviewedOwnership) -> set[str]:
    """Every Rust path a reviewed ownership record claims, for index building."""
    paths = {
        assignment.path
        for symbol in record.symbols
        for assignment in symbol.assignments
    }
    paths.update(owned.path for owned in record.source_ranges)
    return paths


def expand_assignment_files(
    path: str,
    by_relative: dict[str, RustFileInfo],
    by_absolute: dict[Path, RustFileInfo],
) -> list[RustFileInfo]:
    """Resolve an ownership target to itself plus, for split roots, its fragments."""
    info = by_relative.get(path)
    if info is None:
        return []
    return expand_split_root(info, by_absolute)


def ownership_destination_diagnostics(
    path: str,
    sha256: str,
    label: str,
    expanded_destinations: dict[str, RustFileInfo],
    by_relative: dict[str, RustFileInfo],
) -> list[str]:
    """Validate one claimed Rust destination against the live repository."""
    info = by_relative.get(path)
    if info is None:
        return [f"{label}:stale_destination_path:{path}"]
    diagnostics: list[str] = []
    if path not in expanded_destinations:
        diagnostics.append(f"{label}:unmapped_destination:{path}")
    if info.classification != "implementation":
        diagnostics.append(f"{label}:non_implementation_destination:{path}")
    if not info.cargo_reachable:
        diagnostics.append(f"{label}:unreachable_destination:{path}")
    if info.sha256 != sha256:
        diagnostics.append(f"{label}:stale_destination_hash:{path}")
    return diagnostics


@dataclass
class SymbolOwnershipEvaluation:
    status: str
    diagnostics: list[str]
    credited_deviations: set[str]
    path_symbols: dict[str, set[str]]


@dataclass
class RangeOwnershipEvaluation:
    status: str
    diagnostics: list[str]
    path_ranges: dict[str, list[tuple[int, int]]]


def evaluate_symbol_ownership(
    record: ReviewedOwnership | None,
    source_hash: str,
    cpp_symbols: Sequence[dict[str, object]],
    expanded_destinations: dict[str, RustFileInfo],
    by_relative: dict[str, RustFileInfo],
    by_absolute: dict[Path, RustFileInfo],
) -> SymbolOwnershipEvaluation:
    """Check that every C++ symbol occurrence is claimed by valid assignments."""
    if record is None:
        return SymbolOwnershipEvaluation("unreviewed", [], set(), {})
    diagnostics: list[str] = []
    credited_deviations: set[str] = set()
    path_symbols: dict[str, set[str]] = defaultdict(set)
    if record.source_sha256 != source_hash:
        diagnostics.append("ownership_symbol:stale_source_hash")
    occurrences = {
        (str(symbol["name"]), int(symbol["line"])) for symbol in cpp_symbols
    }
    assigned: set[tuple[str, int]] = set()
    for symbol in record.symbols:
        key = (symbol.name, symbol.line)
        if key not in occurrences:
            diagnostics.append(
                f"ownership_symbol:unknown_cpp_symbol:{symbol.name}@{symbol.line}"
            )
            continue
        assigned.add(key)
        for assignment in symbol.assignments:
            assignment_diagnostics = ownership_destination_diagnostics(
                assignment.path,
                assignment.sha256,
                "ownership_symbol",
                expanded_destinations,
                by_relative,
            )
            files = expand_assignment_files(assignment.path, by_relative, by_absolute)
            known_names = {name for info in files for _kind, name, _line in info.symbols}
            for rust_symbol in assignment.symbols:
                if rust_symbol not in known_names:
                    assignment_diagnostics.append(
                        f"ownership_symbol:missing_rust_symbol:{rust_symbol}@{assignment.path}"
                    )
            if (
                assignment.deviation is not None
                and assignment.deviation not in ALLOWED_DEVIATION_IDS
            ):
                assignment_diagnostics.append(
                    f"ownership_symbol:unknown_deviation:{assignment.deviation}"
                )
            if assignment_diagnostics:
                diagnostics.extend(assignment_diagnostics)
                continue
            path_symbols[assignment.path].add(symbol.name)
            if assignment.deviation is not None:
                credited_deviations.add(assignment.deviation)
    for name, line in sorted(occurrences - assigned):
        diagnostics.append(f"ownership_symbol:unassigned_cpp_symbol:{name}@{line}")
    status = "reviewed" if not diagnostics else "invalid"
    return SymbolOwnershipEvaluation(status, diagnostics, credited_deviations, path_symbols)


def evaluate_range_ownership(
    record: ReviewedOwnership | None,
    source_hash: str,
    total_lines: int,
    expanded_destinations: dict[str, RustFileInfo],
    by_relative: dict[str, RustFileInfo],
    required: bool,
) -> RangeOwnershipEvaluation:
    """Check that reviewed ranges cover the unit exactly, with valid destinations."""
    if record is None:
        return RangeOwnershipEvaluation(
            "unreviewed" if required else "not_required", [], {}
        )
    if not required and not record.source_ranges:
        return RangeOwnershipEvaluation("not_required", [], {})
    diagnostics: list[str] = []
    if record.source_sha256 != source_hash:
        diagnostics.append("ownership_range:stale_source_hash")
    bounded: list[ReviewedRange] = []
    for owned_range in record.source_ranges:
        if not (1 <= owned_range.start_line <= owned_range.end_line <= total_lines):
            diagnostics.append(
                f"ownership_range:out_of_bounds:{owned_range.start_line}-{owned_range.end_line}"
            )
            continue
        bounded.append(owned_range)
        diagnostics.extend(
            ownership_destination_diagnostics(
                owned_range.path,
                owned_range.sha256,
                "ownership_range",
                expanded_destinations,
                by_relative,
            )
        )
    bounded.sort(key=lambda item: (item.start_line, item.end_line, item.path))
    expected = 1
    for owned_range in bounded:
        if owned_range.start_line > expected:
            diagnostics.append(
                f"ownership_range:gap:{expected}-{owned_range.start_line - 1}"
            )
        elif owned_range.start_line < expected:
            diagnostics.append(
                f"ownership_range:overlap:{owned_range.start_line}-{owned_range.end_line}"
            )
        expected = max(expected, owned_range.end_line + 1)
    if expected <= total_lines:
        diagnostics.append(f"ownership_range:gap:{expected}-{total_lines}")
    status = "reviewed" if not diagnostics else "invalid"
    path_ranges: dict[str, list[tuple[int, int]]] = defaultdict(list)
    if status == "reviewed":
        for owned_range in bounded:
            path_ranges[owned_range.path].append(
                (owned_range.start_line, owned_range.end_line)
            )
    return RangeOwnershipEvaluation(status, diagnostics, path_ranges)


def rust_file_index_record(info: RustFileInfo, include_symbols: bool) -> dict[str, object]:
    """Store expensive hash/symbol metadata once, not in every mapping edge."""
    return {
        "path": info.relative,
        "sha256": info.sha256,
        "classification": info.classification,
        "cargo_reachable": info.cargo_reachable,
        "file_range": {"start_line": 1, "end_line": info.line_count},
        "symbol_index_status": "indexed" if include_symbols else "awaiting_mapping_review",
        "symbols": (
            [
                {"kind": kind, "name": name, "line": line}
                for kind, name, line in info.symbols
            ]
            if include_symbols
            else []
        ),
    }


def discover_destinations(
    source: Path,
    source_rel: str,
    scope: ScopeRoot,
    rust_infos: Sequence[RustFileInfo],
    by_relative: dict[str, RustFileInfo],
    by_absolute: dict[Path, RustFileInfo],
    by_stem: dict[str, list[RustFileInfo]],
    split_roots: dict[str, list[RustFileInfo]],
    reviewed_mappings: dict[str, tuple[str, ...]],
) -> tuple[str, str, list[RustFileInfo]]:
    reviewed_paths = reviewed_mappings.get(source_rel)
    if reviewed_paths is not None:
        destinations = [by_relative[path] for path in reviewed_paths if path in by_relative]
        expanded: dict[str, RustFileInfo] = {}
        for destination in destinations:
            for member in expand_split_root(destination, by_absolute):
                expanded[member.relative] = member
        return "explicit", "reviewed", [expanded[key] for key in sorted(expanded)]

    stem = normalize_name(source.stem)
    area = source_area(source, scope)
    roots = [
        info
        for info in split_roots.get(stem, [])
        if any(info.relative.startswith(prefix) for prefix in allowed_rust_prefixes(scope))
    ]
    if roots:
        root = sorted(roots, key=lambda info: score_candidate(info, scope, area))[0]
        return "split_inferred", "unreviewed", expand_split_root(root, by_absolute)

    candidates = by_stem.get(stem, [])
    if not candidates:
        return "missing", "unreviewed", []
    chosen = sorted(candidates, key=lambda info: score_candidate(info, scope, area))[0]
    return "stem_inferred", "unreviewed", [chosen]


def iter_cpp_units(repo_root: Path, scope: ScopeRoot) -> Iterable[tuple[Path, str]]:
    cpp_root = repo_root / scope.cpp_root
    if not cpp_root.is_dir():
        return
    for path in sorted(cpp_root.rglob("*")):
        if not path.is_file():
            continue
        suffix = path.suffix.lower()
        if suffix in CPP_TRANSLATION_SUFFIXES:
            yield path, "translation_unit"
        elif scope.include_headers and suffix in CPP_HEADER_SUFFIXES:
            yield path, "header"


def build_manifest(
    repo_root: Path,
    reviewed_mappings: dict[str, tuple[str, ...]] | None = None,
    evidence_units: dict[str, EvidenceUnit] | None = None,
    reviewed_ownership: dict[str, ReviewedOwnership] | None = None,
) -> dict[str, object]:
    if reviewed_mappings is None:
        reviewed_mappings = load_reviewed_mappings(repo_root)
    if evidence_units is None:
        evidence_units = load_behavior_evidence(repo_root)
    if reviewed_ownership is None:
        reviewed_ownership = load_reviewed_ownership(repo_root)
    rust_infos, _reachable = collect_rust_files(repo_root)
    by_relative = {info.relative: info for info in rust_infos}
    by_absolute = {info.absolute: info for info in rust_infos}
    by_stem: dict[str, list[RustFileInfo]] = defaultdict(list)
    split_roots: dict[str, list[RustFileInfo]] = defaultdict(list)
    for info in rust_infos:
        by_stem[normalize_name(Path(info.relative).stem)].append(info)
        split_roots[normalize_name(Path(info.relative).parent.name)].append(info)
    matched_evidence_sources: set[str] = set()
    entries: list[dict[str, object]] = []
    for scope in SCOPE_ROOTS:
        for source, unit_kind in iter_cpp_units(repo_root, scope):
            source_rel = relative_posix(source, repo_root)
            mode, review_state, destinations = discover_destinations(
                source,
                source_rel,
                scope,
                rust_infos,
                by_relative,
                by_absolute,
                by_stem,
                split_roots,
                reviewed_mappings,
            )
            implementation_destinations = [
                destination
                for destination in destinations
                if destination.classification == "implementation"
                and destination.cargo_reachable
            ]
            candidate_status = (
                "reachable_implementation_candidate"
                if implementation_destinations
                else "no_reachable_implementation"
            )
            reviewed_path_implementation = (
                bool(implementation_destinations) and review_state == "reviewed"
            )
            stale_reviewed_destinations = [
                path
                for path in reviewed_mappings.get(source_rel, ())
                if path not in by_relative
            ]
            inventory_class = classify_inventory(scope, source_rel)
            required = inventory_class != "deferred_network"
            source_sha256 = sha256_file(source)
            ownership_record = reviewed_ownership.get(source_rel)
            expanded_destinations = {
                member.relative: member
                for destination in destinations
                for member in expand_split_root(destination, by_absolute)
            }
            source_lines = line_count(source)
            cpp_symbols = (
                extract_cpp_symbols(source)
                if unit_kind == "translation_unit"
                and (ownership_record is not None or review_state == "reviewed")
                else []
            )
            if required and unit_kind == "translation_unit":
                symbol_evaluation = evaluate_symbol_ownership(
                    ownership_record,
                    source_sha256,
                    cpp_symbols,
                    expanded_destinations,
                    by_relative,
                    by_absolute,
                )
                range_evaluation = evaluate_range_ownership(
                    ownership_record,
                    source_sha256,
                    source_lines,
                    expanded_destinations,
                    by_relative,
                    len(implementation_destinations) > 1,
                )
            else:
                symbol_evaluation = SymbolOwnershipEvaluation(
                    "unreviewed", [], set(), {}
                )
                range_evaluation = RangeOwnershipEvaluation("not_required", [], {})
            ownership_diagnostics = list(symbol_evaluation.diagnostics)
            ownership_diagnostics.extend(range_evaluation.diagnostics)

            blockers: list[str] = []
            if required and unit_kind == "translation_unit":
                if mode == "missing":
                    blockers.append("missing_mapping")
                if stale_reviewed_destinations:
                    blockers.append("stale_mapped_path")
                if not implementation_destinations:
                    blockers.append("no_reachable_implementation")
                if review_state != "reviewed":
                    blockers.append("unreviewed_mapping")
                # Path review is intentionally narrower than symbol/range
                # provenance. Strict mode stays red until ownership is both
                # assigned and validated against current files and symbols.
                if symbol_evaluation.status != "reviewed":
                    blockers.append("unreviewed_symbol_ownership")
                blockers.extend(symbol_evaluation.diagnostics)
                if range_evaluation.status in ("unreviewed", "invalid"):
                    blockers.append("unreviewed_source_range_ownership")
                blockers.extend(range_evaluation.diagnostics)

            evidence_unit = evidence_units.get(source_rel)
            if evidence_unit is not None:
                matched_evidence_sources.add(source_rel)
            behavior = evaluate_unit(
                evidence_unit,
                source_path=source_rel,
                source_sha256=source_sha256,
                unit_kind=unit_kind,
                inventory_class=inventory_class,
                review_state=review_state,
                destination_states=[
                    DestinationState(
                        path=info.relative,
                        sha256=info.sha256,
                        classification=info.classification,
                        cargo_reachable=info.cargo_reachable,
                    )
                    for info in destinations
                ],
                repo_root=repo_root,
            )

            entries.append(
                {
                    "id": f"{scope.scope_id}:{source.relative_to(repo_root / scope.cpp_root).as_posix()}",
                    "scope": scope.scope_id,
                    "inventory_class": inventory_class,
                    "unit_kind": unit_kind,
                    "source": {
                        "path": source_rel,
                        "sha256": source_sha256,
                        "file_range": {"start_line": 1, "end_line": source_lines},
                        "symbols": cpp_symbols,
                    },
                    "mapping": {
                        "mode": mode,
                        "review_state": review_state,
                        "candidate_status": candidate_status,
                        "reviewed_path_implementation": reviewed_path_implementation,
                        "destinations": [
                            destination_record(
                                info,
                                owned_symbols=(
                                    symbol_evaluation.path_symbols.get(info.relative, ())
                                    if symbol_evaluation.status == "reviewed"
                                    else ()
                                ),
                                owned_source_ranges=(
                                    range_evaluation.path_ranges.get(info.relative, ())
                                    if range_evaluation.status == "reviewed"
                                    else ()
                                ),
                            )
                            for info in destinations
                        ],
                        "stale_reviewed_destinations": stale_reviewed_destinations,
                        "symbol_validation": symbol_evaluation.status,
                        "range_validation": range_evaluation.status,
                        "ownership_diagnostics": ownership_diagnostics,
                    },
                    "allowed_deviations": sorted(symbol_evaluation.credited_deviations),
                    "behavior": behavior,
                    "blockers": blockers,
                }
            )


    # A production implementation destination shared by multiple required C++
    # translation units needs explicit review; headers intentionally share code.
    destination_owners: dict[str, list[dict[str, object]]] = defaultdict(list)
    for entry in entries:
        if entry["unit_kind"] != "translation_unit" or entry["inventory_class"] == "deferred_network":
            continue
        mapping = entry["mapping"]
        assert isinstance(mapping, dict)
        for destination in mapping["destinations"]:
            assert isinstance(destination, dict)
            if destination["classification"] == "implementation" and destination["cargo_reachable"]:
                destination_owners[str(destination["path"])].append(entry)
    duplicate_paths = sorted(path for path, owners in destination_owners.items() if len(owners) > 1)
    for path in duplicate_paths:
        for entry in destination_owners[path]:
            blockers = entry["blockers"]
            assert isinstance(blockers, list)
            blockers.append(f"duplicate_implementation_destination:{path}")

    translation_units = [entry for entry in entries if entry["unit_kind"] == "translation_unit"]
    required_units = [
        entry for entry in translation_units if entry["inventory_class"] != "deferred_network"
    ]
    blocker_counts = Counter(
        blocker.split(":", 1)[0]
        for entry in required_units
        for blocker in entry["blockers"]
    )
    scope_counts: dict[str, dict[str, int]] = {}
    for scope in SCOPE_ROOTS:
        scoped = [entry for entry in translation_units if entry["scope"] == scope.scope_id]
        scope_counts[scope.scope_id] = {
            "translation_units": len(scoped),
            "required": sum(entry["inventory_class"] != "deferred_network" for entry in scoped),
            "deferred_network": sum(entry["inventory_class"] == "deferred_network" for entry in scoped),
            "reachable_implementation_candidates": sum(
                entry["mapping"]["candidate_status"] == "reachable_implementation_candidate"
                for entry in scoped
            ),
            "reviewed_path_implementations": sum(
                bool(entry["mapping"]["reviewed_path_implementation"]) for entry in scoped
            ),
            "symbol_ownership_reviewed": sum(
                entry["mapping"]["symbol_validation"] == "reviewed" for entry in scoped
            ),
            "source_range_ownership_reviewed": sum(
                entry["mapping"]["range_validation"] == "reviewed" for entry in scoped
            ),
            "behavior_evidence_units": sum(
                entry["source"]["path"] in matched_evidence_sources for entry in scoped
            ),
            "behavior_verified": sum(entry["behavior"]["status"] == "verified" for entry in scoped),
        }

    referenced_rust_paths = {
        str(destination["path"])
        for entry in entries
        for destination in entry["mapping"]["destinations"]
    }
    digest_rows = [
        f"cpp:{entry['source']['path']}:{entry['source']['sha256']}"
        for entry in entries
    ]
    digest_rows.extend(
        f"rust:{info.relative}:{info.sha256}"
        for info in rust_infos
        if info.relative in referenced_rust_paths
    )
    digest_rows.extend(
        f"reviewed:{source}:{destination}"
        for source, destinations in sorted(reviewed_mappings.items())
        for destination in destinations
    )
    digest_rows.extend(
        f"ownership:{source}:{record.source_sha256}"
        for source, record in sorted(reviewed_ownership.items())
    )
    digest_rows.extend(
        f"ownership-symbol:{source}:{symbol.name}:{symbol.line}:"
        f"{assignment.path}:{assignment.sha256}:{rust_symbol}"
        for source, record in sorted(reviewed_ownership.items())
        for symbol in record.symbols
        for assignment in symbol.assignments
        for rust_symbol in assignment.symbols
    )
    digest_rows.extend(
        f"ownership-range:{source}:{owned.start_line}:{owned.end_line}:"
        f"{owned.path}:{owned.sha256}"
        for source, record in sorted(reviewed_ownership.items())
        for owned in record.source_ranges
    )
    digest_rows.extend(evidence_digest_rows(evidence_units))
    input_digest = hashlib.sha256("\n".join(digest_rows).encode("utf-8")).hexdigest()
    reviewed_rust_paths = {
        str(destination["path"])
        for entry in entries
        if entry["mapping"]["review_state"] == "reviewed"
        for destination in entry["mapping"]["destinations"]
    }
    reviewed_rust_paths.update(
        path
        for record in reviewed_ownership.values()
        for path in ownership_referenced_paths(record)
        if path in by_relative
    )
    required_sources = {str(entry["source"]["path"]) for entry in required_units}
    orphan_ownership_sources = sorted(set(reviewed_ownership) - required_sources)
    invalid_ownership_sources = sorted(
        str(entry["source"]["path"])
        for entry in required_units
        if entry["mapping"]["ownership_diagnostics"]
    )
    behavior_evidence_summary = {
        "verified_units": sum(
            entry["behavior"]["status"] == "verified" for entry in required_units
        ),
        "units_with_records": sum(
            entry["source"]["path"] in matched_evidence_sources
            for entry in required_units
        ),
        "by_provenance_scope": {
            scope.scope_id: {
                "verified_units": scope_counts[scope.scope_id]["behavior_verified"],
                "units_with_records": scope_counts[scope.scope_id][
                    "behavior_evidence_units"
                ],
            }
            for scope in SCOPE_ROOTS
        },
        # Evidence scope names the oracle the executable evidence ran against
        # (original_cpp or retail); it is deliberately distinct from the
        # provenance scope of the unit.
        "by_evidence_scope": dict(
            sorted(
                Counter(
                    evidence_scope
                    for entry in required_units
                    if entry["behavior"]["status"] == "verified"
                    for evidence_scope in entry["behavior"]["scopes"]
                ).items()
            )
        ),
        "by_confidence": {
            "exact": sum(
                entry["behavior"]["confidence"] == "exact" for entry in required_units
            ),
            "tolerance": sum(
                entry["behavior"]["confidence"] == "tolerance"
                for entry in required_units
            ),
        },
        "rejected_records": sum(
            len(entry["behavior"]["rejected_evidence"]) for entry in required_units
        ),
        "informational_records": sum(
            1
            for entry in required_units
            for item in entry["behavior"]["evidence"]
            if item["kind"] in INFORMATIONAL_KINDS
        ),
        "unmatched_units": sorted(set(evidence_units) - matched_evidence_sources),
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "generator": "GeneralsRust/Code/Main/scripts/generate_port_provenance.py",
        "input_digest": input_digest,
        "reviewed_mapping_input": {
            "path": REVIEWED_MAPPINGS_FILE,
            "mapping_count": len(reviewed_mappings),
            "ownership_record_count": len(reviewed_ownership),
            "sha256": (
                sha256_file(repo_root / REVIEWED_MAPPINGS_FILE)
                if (repo_root / REVIEWED_MAPPINGS_FILE).is_file()
                else None
            ),
        },
        "ownership_review": {
            "record_count": len(reviewed_ownership),
            "invalid_sources": invalid_ownership_sources,
            "orphan_sources": orphan_ownership_sources,
        },
        "behavior_evidence_input": {
            "path": EVIDENCE_FILE,
            "unit_count": len(evidence_units),
            "record_count": sum(
                len(unit.records) for unit in evidence_units.values()
            ),
            "sha256": (
                sha256_file(repo_root / EVIDENCE_FILE)
                if (repo_root / EVIDENCE_FILE).is_file()
                else None
            ),
        },
        "metric_contract": {
            "inventory_is_behavior_parity": False,
            "candidate_mapping_is_reviewed": False,
            "test_shim_telemetry_credit_implementation": False,
            "behavior_requires_explicit_evidence": True,
        },
        "scope_roots": [
            {
                "id": scope.scope_id,
                "cpp_root": scope.cpp_root,
                "rust_roots": list(scope.rust_roots),
                "classification": scope.inventory_class,
                "headers_in_inventory": scope.include_headers,
            }
            for scope in SCOPE_ROOTS
        ],
        "allowed_deviation_policy": list(ALLOWED_DEVIATIONS),
        "rust_file_index": [
            rust_file_index_record(info, info.relative in reviewed_rust_paths)
            for info in rust_infos
            if info.relative in referenced_rust_paths
        ],
        "summary": {
            "all_inventory_entries": len(entries),
            "translation_units": len(translation_units),
            "required_translation_units": len(required_units),
            "deferred_network_translation_units": len(translation_units) - len(required_units),
            "reachable_implementation_candidates": sum(
                entry["mapping"]["candidate_status"] == "reachable_implementation_candidate"
                for entry in required_units
            ),
            "reviewed_path_implementations": sum(
                bool(entry["mapping"]["reviewed_path_implementation"])
                for entry in required_units
            ),
            "behavior_verified": sum(entry["behavior"]["status"] == "verified" for entry in required_units),
            "symbol_ownership_reviewed": sum(
                entry["mapping"]["symbol_validation"] == "reviewed"
                for entry in required_units
            ),
            "source_range_ownership_reviewed": sum(
                entry["mapping"]["range_validation"] == "reviewed"
                for entry in required_units
            ),
            "orphan_ownership_records": len(orphan_ownership_sources),
            "behavior_evidence": behavior_evidence_summary,
            "strict_blockers": sum(blocker_counts.values()),
            "blockers_by_kind": dict(sorted(blocker_counts.items())),
            "duplicate_implementation_destinations": len(duplicate_paths),
            "by_scope": scope_counts,
        },
        "entries": entries,
    }


def write_manifest(path: Path, manifest: dict[str, object]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2, sort_keys=True)
        handle.write("\n")


def write_state(path: Path, manifest: dict[str, object]) -> None:
    summary = manifest["summary"]
    assert isinstance(summary, dict)
    required = int(summary["required_translation_units"])
    candidates = int(summary["reachable_implementation_candidates"])
    reviewed = int(summary["reviewed_path_implementations"])
    symbol_ownership = int(summary["symbol_ownership_reviewed"])
    range_ownership = int(summary["source_range_ownership_reviewed"])
    behavior = int(summary["behavior_verified"])

    def percent(value: int) -> float:
        return value / required * 100.0 if required else 100.0

    lines = [
        "# Auto-generated by generate_port_provenance.py",
        "# Inventory candidates, reviewed provenance, and behavioral proof are separate metrics.",
        "# No inventory percentage is a playable-game or behavioral-parity claim.",
        f"SchemaVersion={manifest['schema_version']}",
        f"InputDigest={manifest['input_digest']}",
        f"ScopedTranslationUnits={summary['translation_units']}",
        f"RequiredNonNetworkTranslationUnits={required}",
        f"DeferredNetworkTranslationUnits={summary['deferred_network_translation_units']}",
        f"ReachableImplementationCandidates={candidates}",
        f"ReachableImplementationCandidatePercent={percent(candidates):.2f}",
        f"ReviewedPathImplementations={reviewed}",
        f"ReviewedPathImplementationPercent={percent(reviewed):.2f}",
        f"ReviewedSymbolOwnership={symbol_ownership}",
        f"ReviewedSymbolOwnershipPercent={percent(symbol_ownership):.2f}",
        f"ReviewedSourceRangeOwnership={range_ownership}",
        f"ReviewedSourceRangeOwnershipPercent={percent(range_ownership):.2f}",
        f"BehaviorVerified={behavior}",
        f"BehaviorVerifiedPercent={percent(behavior):.2f}",
        f"StrictProvenanceBlockers={summary['strict_blockers']}",
    ]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_subsystem_status(path: Path, manifest: dict[str, object]) -> None:
    summary = manifest["summary"]
    assert isinstance(summary, dict)
    payload = {
        "schema_version": manifest["schema_version"],
        "source": manifest["generator"],
        "input_digest": manifest["input_digest"],
        "metric_contract": manifest["metric_contract"],
        "summary": summary,
    }
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2, sort_keys=True)
        handle.write("\n")


def generate(repo_root: Path, output_root: Path) -> dict[str, object]:
    manifest = build_manifest(repo_root.resolve())
    output_root.mkdir(parents=True, exist_ok=True)
    write_manifest(output_root / "PORT_PROVENANCE_MANIFEST.json", manifest)
    write_state(output_root / "PORT_STATE.txt", manifest)
    write_subsystem_status(output_root / "PORT_SUBSYSTEM_STATUS.json", manifest)
    return manifest


def verify_generated(repo_root: Path, output_root: Path) -> list[str]:
    """Return generated artifacts whose checked-in bytes do not match current inputs."""
    names = (
        "PORT_PROVENANCE_MANIFEST.json",
        "PORT_STATE.txt",
        "PORT_SUBSYSTEM_STATUS.json",
    )
    with tempfile.TemporaryDirectory() as temporary:
        generated_root = Path(temporary)
        generate(repo_root, generated_root)
        return [
            name
            for name in names
            if not (output_root / name).is_file()
            or (output_root / name).read_bytes() != (generated_root / name).read_bytes()
        ]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit non-zero when required translation units have provenance blockers",
    )
    parser.add_argument(
        "--verify-generated",
        action="store_true",
        help="exit non-zero when checked-in generated artifacts are stale",
    )
    args = parser.parse_args()
    repo_root = args.repo_root.resolve()
    output_root = args.output_root.resolve() if args.output_root else repo_root
    if args.verify_generated:
        stale = verify_generated(repo_root, output_root)
        if stale:
            print("Stale generated provenance artifacts: " + ", ".join(stale))
            raise SystemExit(1)
        print("Generated provenance artifacts match current inputs")
        return
    manifest = generate(repo_root, output_root)
    summary = manifest["summary"]
    assert isinstance(summary, dict)
    print(f"Generated: {output_root / 'PORT_PROVENANCE_MANIFEST.json'}")
    print(f"Generated: {output_root / 'PORT_STATE.txt'}")
    print(f"Generated: {output_root / 'PORT_SUBSYSTEM_STATUS.json'}")
    print(
        "provenance inventory: "
        f"required={summary['required_translation_units']} "
        f"candidates={summary['reachable_implementation_candidates']} "
        f"reviewed_paths={summary['reviewed_path_implementations']} "
        f"behavior_verified={summary['behavior_verified']} "
        f"blockers={summary['strict_blockers']}"
    )
    if args.check and int(summary["strict_blockers"]) > 0:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
