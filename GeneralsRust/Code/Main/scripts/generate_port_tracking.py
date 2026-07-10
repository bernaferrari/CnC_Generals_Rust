#!/usr/bin/env python3
"""Generate PORT_* parity tracking artifacts used by playability_audit."""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable


SOURCE_STATUS_FOUND = "FOUND"
SOURCE_STATUS_FOUND_BY_BASENAME = "FOUND_BY_BASENAME"
SOURCE_STATUS_MISSING = "MISSING"

# Explicit C++ -> Rust mappings for renamed faithful ports where file names diverge.
# Keys are: (kind, relative_cpp_path_from_Source_or_Include_root).
MANUAL_CPP_TO_RUST: dict[tuple[str, str], str] = {
    ("Source", "GameLogic/AI/AIDock.cpp"): "GameLogic/src/ai/dock.rs",
    ("Source", "GameLogic/AI/AIGuard.cpp"): "GameLogic/src/ai/guard.rs",
    ("Source", "GameLogic/AI/AIGuardRetaliate.cpp"): "GameLogic/src/ai/guard_retaliate.rs",
    ("Source", "GameLogic/AI/AIPathfind.cpp"): "GameLogic/src/ai/pathfind.rs",
    ("Source", "GameLogic/AI/AISkirmishPlayer.cpp"): "GameLogic/src/ai/skirmish_player.rs",
    ("Source", "GameLogic/AI/AITNGuard.cpp"): "GameLogic/src/ai/tn_guard.rs",
    (
        "Source",
        "GameLogic/Object/Behavior/NeutonBlastBehavior.cpp",
    ): "GameLogic/src/object/behavior/neutron_blast_behavior.rs",
    (
        "Source",
        "GameLogic/Object/SpecialPower/SpecialAbility.cpp",
    ): "GameLogic/src/object/special_powers/special_ability.rs",
    ("Include", "GameLogic/AIDock.h"): "GameLogic/src/ai/dock.rs",
    ("Include", "GameLogic/AIGuard.h"): "GameLogic/src/ai/guard.rs",
    ("Include", "GameLogic/AIGuardRetaliate.h"): "GameLogic/src/ai/guard_retaliate.rs",
    ("Include", "GameLogic/AIPathfind.h"): "GameLogic/src/ai/pathfind.rs",
    ("Include", "GameLogic/AISkirmishPlayer.h"): "GameLogic/src/ai/skirmish_player.rs",
    ("Include", "GameLogic/AIStateMachine.h"): "GameLogic/src/ai/state_machine.rs",
    ("Include", "GameLogic/AITNGuard.h"): "GameLogic/src/ai/tn_guard.rs",
    ("Include", "GameLogic/ArmorSet.h"): "GameLogic/src/common/types.rs",
    ("Include", "GameLogic/FPUControl.h"): "GameLogic/src/system/game_logic.rs",
    ("Include", "GameLogic/LogicRandomValue.h"): "GameLogic/src/helpers.rs",
    (
        "Include",
        "GameLogic/Module/SpecialAbility.h",
    ): "GameLogic/src/object/special_powers/special_ability.rs",
    ("Include", "GameLogic/ObjectIter.h"): "GameLogic/src/object/simple_object_iterator.rs",
    (
        "Include",
        "GameLogic/ObjectScriptStatusBits.h",
    ): "GameLogic/src/common/types.rs",
    ("Include", "GameLogic/Powers.h"): "GameLogic/src/special_power.rs",
    (
        "Include",
        "GameLogic/WeaponBonusConditionFlags.h",
    ): "GameLogic/src/common/types.rs",
    ("Include", "GameLogic/WeaponSetFlags.h"): "GameLogic/src/weapon/weapon_set.rs",
    ("Include", "GameLogic/WeaponSetType.h"): "GameLogic/src/weapon/weapon_set.rs",
    ("Include", "GameLogic/WeaponStatus.h"): "GameLogic/src/weapon/weapon.rs",
}


@dataclass(frozen=True)
class MappingRow:
    kind: str  # Source | Include
    source_rel: Path
    subsystem: str
    status: str
    mapped_rel: Path | None


def discover_repo_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (candidate / "GeneralsMD").is_dir() and (candidate / "GeneralsRust").is_dir():
            return candidate
    raise SystemExit("Failed to locate repository root containing GeneralsMD and GeneralsRust")


def collect_files(root: Path, suffixes: tuple[str, ...]) -> list[Path]:
    rows: list[Path] = []
    for suffix in suffixes:
        rows.extend(root.rglob(f"*{suffix}"))
    return sorted(rows)


def normalize_name(value: str) -> str:
    return "".join(ch for ch in value.lower() if ch.isalnum())


def rust_subsystem(rel_path: Path) -> str:
    return rel_path.parts[0] if rel_path.parts else "Unknown"


def cpp_subsystem(rel_path: Path) -> str:
    return rel_path.parts[0] if rel_path.parts else "Unknown"


def best_candidate(
    subsystem: str,
    candidates: list[Path],
    rust_root: Path,
) -> Path:
    def score(path: Path) -> tuple[int, int, int]:
        rel = path.relative_to(rust_root)
        same_subsystem = int(rust_subsystem(rel).lower() == subsystem.lower())
        src_bonus = int("src" in rel.parts)
        return (-same_subsystem, -src_bonus, len(rel.parts))

    return sorted(candidates, key=score)[0]


def build_mapping_rows(
    kind: str,
    cpp_root: Path,
    rust_root: Path,
    rust_by_stem: dict[str, list[Path]],
    rust_by_normalized_stem: dict[str, list[Path]],
) -> list[MappingRow]:
    if kind == "Source":
        cpp_files = collect_files(cpp_root, (".cpp", ".cxx", ".cc"))
    else:
        cpp_files = collect_files(cpp_root, (".h", ".hpp", ".inl"))

    rows: list[MappingRow] = []
    for cpp_abs in cpp_files:
        cpp_rel = cpp_abs.relative_to(cpp_root)
        subsystem = cpp_subsystem(cpp_rel)
        manual_rel = MANUAL_CPP_TO_RUST.get((kind, cpp_rel.as_posix()))
        if manual_rel:
            manual_abs = rust_root / manual_rel
            if manual_abs.is_file():
                chosen_rel = manual_abs.relative_to(rust_root)
                chosen_subsystem = rust_subsystem(chosen_rel)
                status = (
                    SOURCE_STATUS_FOUND
                    if chosen_subsystem.lower() == subsystem.lower()
                    else SOURCE_STATUS_FOUND_BY_BASENAME
                )
                rows.append(
                    MappingRow(
                        kind=kind,
                        source_rel=cpp_rel,
                        subsystem=subsystem,
                        status=status,
                        mapped_rel=chosen_rel,
                    )
                )
                continue

        stem = cpp_abs.stem.lower()
        candidates = rust_by_stem.get(stem, [])
        if not candidates:
            normalized_stem = normalize_name(cpp_abs.stem)
            candidates = rust_by_normalized_stem.get(normalized_stem, [])

        if not candidates:
            rows.append(
                MappingRow(
                    kind=kind,
                    source_rel=cpp_rel,
                    subsystem=subsystem,
                    status=SOURCE_STATUS_MISSING,
                    mapped_rel=None,
                )
            )
            continue

        chosen = best_candidate(subsystem, candidates, rust_root)
        chosen_rel = chosen.relative_to(rust_root)
        chosen_subsystem = rust_subsystem(chosen_rel)
        if chosen_subsystem.lower() == subsystem.lower():
            status = SOURCE_STATUS_FOUND
        else:
            status = SOURCE_STATUS_FOUND_BY_BASENAME

        rows.append(
            MappingRow(
                kind=kind,
                source_rel=cpp_rel,
                subsystem=subsystem,
                status=status,
                mapped_rel=chosen_rel,
            )
        )

    return rows


def write_matrix(path: Path, rows: Iterable[MappingRow]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        handle.write("# Auto-generated by generate_port_tracking.py\n")
        handle.write("# Format: Kind | SourcePath | MappedPath | Status | Notes\n")
        for row in rows:
            source_field = f"{row.kind}/{row.source_rel.as_posix()}"
            mapped_field = row.mapped_rel.as_posix() if row.mapped_rel else "-"
            note = f"subsystem={row.subsystem}"
            handle.write(
                f"{row.kind} | {source_field} | {mapped_field} | {row.status} | {note}\n"
            )


def write_missing(path: Path, rows: Iterable[MappingRow]) -> None:
    source_missing: dict[str, list[MappingRow]] = defaultdict(list)
    include_missing: dict[str, list[MappingRow]] = defaultdict(list)

    for row in rows:
        if row.status != SOURCE_STATUS_MISSING:
            continue
        if row.kind == "Source":
            source_missing[row.subsystem].append(row)
        else:
            include_missing[row.subsystem].append(row)

    subsystems = sorted(set(source_missing.keys()) | set(include_missing.keys()))
    with path.open("w", encoding="utf-8") as handle:
        handle.write("# Auto-generated by generate_port_tracking.py\n")
        for subsystem in subsystems:
            handle.write(f"[{subsystem}]\n")
            handle.write("Source missing:\n")
            for row in source_missing.get(subsystem, []):
                handle.write(f"Source/{row.source_rel.as_posix()}\n")
            handle.write("Include missing:\n")
            for row in include_missing.get(subsystem, []):
                handle.write(f"Include/{row.source_rel.as_posix()}\n")
            handle.write("\n")


def write_mismatches(path: Path, rows: Iterable[MappingRow]) -> None:
    by_subsystem: dict[str, list[MappingRow]] = defaultdict(list)
    for row in rows:
        if row.status == SOURCE_STATUS_FOUND_BY_BASENAME and row.mapped_rel is not None:
            by_subsystem[row.subsystem].append(row)

    with path.open("w", encoding="utf-8") as handle:
        handle.write("# Auto-generated by generate_port_tracking.py\n")
        for subsystem in sorted(by_subsystem.keys()):
            handle.write(f"[{subsystem}]\n")
            for row in by_subsystem[subsystem]:
                expected = f"{subsystem}/{row.source_rel.stem}.rs"
                found = row.mapped_rel.as_posix() if row.mapped_rel else "-"
                handle.write(
                    f"{row.kind} | {row.kind}/{row.source_rel.as_posix()} | "
                    f"expected {expected} | found {found}\n"
                )
            handle.write("\n")


def write_state(path: Path, rows: Iterable[MappingRow]) -> None:
    total = 0
    found = 0
    found_by_basename = 0
    missing = 0
    for row in rows:
        total += 1
        if row.status == SOURCE_STATUS_FOUND:
            found += 1
        elif row.status == SOURCE_STATUS_FOUND_BY_BASENAME:
            found_by_basename += 1
        elif row.status == SOURCE_STATUS_MISSING:
            missing += 1

    parity = ((found + found_by_basename) / total * 100.0) if total else 0.0
    generated_at = datetime.now(timezone.utc).isoformat()
    with path.open("w", encoding="utf-8") as handle:
        handle.write("# Auto-generated by generate_port_tracking.py\n")
        handle.write(f"GeneratedAtUTC={generated_at}\n")
        handle.write(f"TotalEntries={total}\n")
        handle.write(f"Found={found}\n")
        handle.write(f"FoundByBasename={found_by_basename}\n")
        handle.write(f"Missing={missing}\n")
        handle.write(f"ParityPercent={parity:.2f}\n")


def write_subsystem_status(path: Path, rows: Iterable[MappingRow]) -> None:
    generated_at = datetime.now(timezone.utc).isoformat()
    subsystems: dict[str, dict[str, dict[str, int]]] = defaultdict(
        lambda: {
            "source": {"total": 0, "found": 0, "found_by_basename": 0, "missing": 0},
            "include": {"total": 0, "found": 0, "found_by_basename": 0, "missing": 0},
        }
    )

    for row in rows:
        kind = "source" if row.kind == "Source" else "include"
        bucket = subsystems[row.subsystem][kind]
        bucket["total"] += 1
        if row.status == SOURCE_STATUS_FOUND:
            bucket["found"] += 1
        elif row.status == SOURCE_STATUS_FOUND_BY_BASENAME:
            bucket["found_by_basename"] += 1
        elif row.status == SOURCE_STATUS_MISSING:
            bucket["missing"] += 1

    def add_parity(bucket: dict[str, int]) -> dict[str, int | float]:
        total = bucket["total"]
        covered = bucket["found"] + bucket["found_by_basename"]
        return {
            **bucket,
            "parity_percent": round((covered / total * 100.0) if total else 100.0, 2),
        }

    status = {
        "generated_at_utc": generated_at,
        "source": "generate_port_tracking.py",
        "inputs": {
            "cpp_source_root": "GeneralsMD/Code/GameEngine/Source",
            "cpp_include_root": "GeneralsMD/Code/GameEngine/Include",
            "rust_root": "GeneralsRust/Code/GameEngine",
        },
        "subsystems": {
            subsystem: {
                "source": add_parity(counts["source"]),
                "include": add_parity(counts["include"]),
            }
            for subsystem, counts in sorted(subsystems.items())
        },
    }

    with path.open("w", encoding="utf-8") as handle:
        json.dump(status, handle, indent=2, sort_keys=True)
        handle.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate PORT_* parity tracking artifacts"
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="Repository root containing GeneralsMD and GeneralsRust (auto-detected by default)",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=None,
        help="Directory to write PORT_* files (defaults to repo root)",
    )
    args = parser.parse_args()

    script_root = Path(__file__).resolve()
    repo_root = (
        args.repo_root.resolve()
        if args.repo_root is not None
        else discover_repo_root(script_root)
    )
    output_root = (
        args.output_root.resolve() if args.output_root is not None else repo_root
    )
    output_root.mkdir(parents=True, exist_ok=True)

    cpp_source_root = repo_root / "GeneralsMD" / "Code" / "GameEngine" / "Source"
    cpp_include_root = repo_root / "GeneralsMD" / "Code" / "GameEngine" / "Include"
    rust_root = repo_root / "GeneralsRust" / "Code" / "GameEngine"
    if not cpp_source_root.is_dir() or not cpp_include_root.is_dir() or not rust_root.is_dir():
        raise SystemExit("Expected GameEngine source/include/rust directories are missing")

    rust_files = collect_files(rust_root, (".rs",))
    rust_by_stem: dict[str, list[Path]] = defaultdict(list)
    rust_by_normalized_stem: dict[str, list[Path]] = defaultdict(list)
    for rust_file in rust_files:
        rust_by_stem[rust_file.stem.lower()].append(rust_file)
        rust_by_normalized_stem[normalize_name(rust_file.stem)].append(rust_file)

    source_rows = build_mapping_rows(
        "Source",
        cpp_source_root,
        rust_root,
        rust_by_stem,
        rust_by_normalized_stem,
    )
    include_rows = build_mapping_rows(
        "Include",
        cpp_include_root,
        rust_root,
        rust_by_stem,
        rust_by_normalized_stem,
    )
    all_rows = source_rows + include_rows

    matrix_path = output_root / "PORT_FILE_MATRIX.txt"
    missing_path = output_root / "PORT_MISSING_FILES_BY_SUBSYSTEM.txt"
    mismatch_path = output_root / "PORT_FILE_MISMATCHES_BY_SUBSYSTEM.txt"
    state_path = output_root / "PORT_STATE.txt"
    subsystem_status_path = output_root / "PORT_SUBSYSTEM_STATUS.json"

    write_matrix(matrix_path, all_rows)
    write_missing(missing_path, all_rows)
    write_mismatches(mismatch_path, all_rows)
    write_state(state_path, all_rows)
    write_subsystem_status(subsystem_status_path, all_rows)

    print(f"Generated: {matrix_path}")
    print(f"Generated: {missing_path}")
    print(f"Generated: {mismatch_path}")
    print(f"Generated: {state_path}")
    print(f"Generated: {subsystem_status_path}")


if __name__ == "__main__":
    main()
