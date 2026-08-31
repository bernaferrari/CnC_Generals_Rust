#!/usr/bin/env python3
"""Load and evaluate hash-pinned behavioral parity evidence for ported units.

``PORT_BEHAVIOR_EVIDENCE.json`` is the machine-editable input that can promote a
reviewed C++ translation unit to ``behavior_verified``.  Promotion is only ever
derived from *current, hash-pinned executable evidence*:

* ``original_cpp_differential`` -- differential against the original C++ build,
  pinned to scope ``original_cpp``.
* ``golden_retail`` -- golden capture from the retail game, pinned to scope
  ``retail``.
* ``snapshot_bytes`` -- byte-for-byte snapshot comparison against either oracle.
* ``deterministic_timing_rng`` -- deterministic timing/RNG trace equality
  against either oracle.
* ``rendering_wgpu_tolerance`` -- rendering comparison within declared
  tolerance under the allowed ``directx_to_wgpu`` deviation.
* ``enum_representation`` -- discriminant/serialization/defaults compatibility
  under the allowed ``representation_safe_enum_cleanup`` deviation.

Evidence never conflates proof levels: ``prose`` and ``rust_vs_rust`` records
are loadable annotations that can never promote.  A promoting record must bind
the scenario, every golden artifact, and every executed command; the unit must
additionally pin the C++ source hash and the hash of every implementation
destination it currently maps to.  Stale hashes, failed commands, partial
destination coverage, path-only mappings, and units without a reviewed mapping
all fail closed.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from pathlib import PurePosixPath
from typing import Sequence


SCHEMA_VERSION = 1
EVIDENCE_FILE = "PORT_BEHAVIOR_EVIDENCE.json"

SCOPE_ORIGINAL_CPP = "original_cpp"
SCOPE_RETAIL = "retail"
ALLOWED_SCOPES = (SCOPE_ORIGINAL_CPP, SCOPE_RETAIL)

KIND_ORIGINAL_CPP_DIFFERENTIAL = "original_cpp_differential"
KIND_GOLDEN_RETAIL = "golden_retail"
KIND_SNAPSHOT_BYTES = "snapshot_bytes"
KIND_DETERMINISTIC_TIMING_RNG = "deterministic_timing_rng"
KIND_RENDERING_WGPU_TOLERANCE = "rendering_wgpu_tolerance"
KIND_ENUM_REPRESENTATION = "enum_representation"
KIND_PROSE = "prose"
KIND_RUST_VS_RUST = "rust_vs_rust"

# Exact-byte executable evidence proves observable output equality directly.
EXACT_KINDS = frozenset(
    {
        KIND_ORIGINAL_CPP_DIFFERENTIAL,
        KIND_GOLDEN_RETAIL,
        KIND_SNAPSHOT_BYTES,
        KIND_DETERMINISTIC_TIMING_RNG,
    }
)
# Tolerance- or representation-scoped evidence proves parity only within the
# declared allowed deviation; it promotes at a lower confidence level.
TOLERANCE_KINDS = frozenset({KIND_RENDERING_WGPU_TOLERANCE, KIND_ENUM_REPRESENTATION})
PROMOTING_KINDS = EXACT_KINDS | TOLERANCE_KINDS
INFORMATIONAL_KINDS = frozenset({KIND_PROSE, KIND_RUST_VS_RUST})
ALL_KINDS = PROMOTING_KINDS | INFORMATIONAL_KINDS

# The executor-pinning kinds must declare their scope explicitly and cannot
# borrow the other oracle's scope.
REQUIRED_SCOPES_BY_KIND = {
    KIND_ORIGINAL_CPP_DIFFERENTIAL: (SCOPE_ORIGINAL_CPP,),
    KIND_GOLDEN_RETAIL: (SCOPE_RETAIL,),
    KIND_SNAPSHOT_BYTES: ALLOWED_SCOPES,
    KIND_DETERMINISTIC_TIMING_RNG: ALLOWED_SCOPES,
    KIND_RENDERING_WGPU_TOLERANCE: ALLOWED_SCOPES,
    KIND_ENUM_REPRESENTATION: ALLOWED_SCOPES,
}

DEVIATIONS_BY_KIND = {
    KIND_RENDERING_WGPU_TOLERANCE: "directx_to_wgpu",
    KIND_ENUM_REPRESENTATION: "representation_safe_enum_cleanup",
}

HEX64_RE = re.compile(r"\A[0-9a-f]{64}\Z")
ARTIFACT_ROLE_RE = re.compile(r"\A[a-z][a-z0-9]*(?:[_-][a-z0-9]+)*\Z")

# The schema deliberately keeps artifact roles extensible, but a record still
# needs to identify an observable result.  ``expected`` is retained as the
# historical spelling used by the existing evidence fixture and means the
# expected/oracle output, not an unverified assertion in prose.
OUTPUT_ARTIFACT_ROLES = frozenset(
    {
        "actual",
        "actual_output",
        "binary",
        "capture",
        "expected",
        "expected_output",
        "golden",
        "image",
        "output",
        "render",
        "rendering",
        "representation",
        "rng_trace",
        "serialization",
        "snapshot",
        "stderr",
        "stdout",
        "timing_trace",
        "trace",
    }
)

# These are accepted as input/reference artifacts, but never satisfy the
# output requirement on their own.  The scenario binding is itself the
# required input declaration for older records, so an input artifact remains
# optional for backward compatibility.
INPUT_ARTIFACT_ROLES = frozenset({"fixture", "input", "oracle", "reference", "scenario"})
KNOWN_ARTIFACT_ROLES = OUTPUT_ARTIFACT_ROLES | INPUT_ARTIFACT_ROLES

# A command is metadata, never something this validator executes.  Reject
# obvious no-op and shell-wrapper forms that can be made to report success
# without running a replay/differential program.
COMMAND_NOOP_PROGRAMS = frozenset(
    {":", "echo", "false", "printf", "print", "true", "yes"}
)
COMMAND_SHELL_PROGRAMS = frozenset(
    {
        "bash",
        "cmd",
        "cmd.exe",
        "powershell",
        "powershell.exe",
        "pwsh",
        "sh",
        "zsh",
    }
)
COMMAND_INLINE_FLAGS = frozenset({"-c", "-e", "--command", "--eval", "--execute"})
COMMAND_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f;&|<>`$]")


@dataclass(frozen=True)
class DestinationPin:
    """A Rust destination path plus the exact bytes the evidence verified."""

    path: str
    sha256: str


@dataclass(frozen=True)
class EvidenceRecord:
    kind: str
    scope: str | None
    scenario: tuple[str, str] | None
    artifacts: tuple[tuple[str, str, str], ...]
    commands: tuple[tuple[tuple[str, ...], int], ...]
    deviation: str | None
    note: str | None


@dataclass(frozen=True)
class EvidenceUnit:
    source: str
    source_sha256: str
    destinations: tuple[DestinationPin, ...]
    records: tuple[EvidenceRecord, ...]


@dataclass(frozen=True)
class DestinationState:
    """Current mapping destination facts supplied by the provenance manifest."""

    path: str
    sha256: str
    classification: str
    cargo_reachable: bool


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _hex64(value: object, where: str) -> str:
    if not isinstance(value, str) or not HEX64_RE.match(value):
        raise ValueError(f"{where} needs a lowercase 64-hex sha256, got {value!r}")
    return value


def _non_empty_str(value: object, where: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{where} needs a non-empty string")
    return value


def _is_safe_relative_path(value: str) -> bool:
    """Return whether *value* is a portable, repository-relative path.

    Evidence is data, not a command line.  Rejecting absolute paths, parent
    traversal, backslashes, and NULs here prevents a later ``repo_root / path``
    operation from accidentally inspecting files outside the worktree.  The
    resolved containment check below additionally catches symlinks that point
    out of the repository.
    """
    if not value or "\x00" in value or "\\" in value:
        return False
    if re.match(r"\A[A-Za-z]:", value):
        return False
    path = PurePosixPath(value)
    return not path.is_absolute() and ".." not in path.parts


def _repo_file(repo_root: Path, relative: str) -> Path | None:
    """Resolve a path only if it remains inside *repo_root*.

    ``Path.resolve`` is intentionally used even for paths that do not exist:
    existing symlink components are resolved, while a missing final artifact
    simply remains a missing file and fails the hash check.
    """
    if not _is_safe_relative_path(relative):
        return None
    try:
        root = repo_root.resolve()
        candidate = (root / relative).resolve()
        candidate.relative_to(root)
    except (OSError, RuntimeError, ValueError):
        return None
    return candidate


def _validate_relative_path(value: object, where: str) -> str:
    path = _non_empty_str(value, where)
    if not _is_safe_relative_path(path):
        raise ValueError(f"{where} must be a repository-relative path, got {path!r}")
    return path


def _parse_hash_binding(raw: object, where: str) -> tuple[str, str]:
    if not isinstance(raw, dict) or set(raw) != {"path", "sha256"}:
        raise ValueError(f"{where} must be an object with exactly path and sha256")
    return (
        _validate_relative_path(raw["path"], f"{where} path"),
        _hex64(raw["sha256"], f"{where} sha256"),
    )


def _validate_command(argv: Sequence[str], where: str) -> None:
    """Validate command metadata without executing it.

    The evidence format records argv and the observed exit status, not a
    shell script.  Requiring a program plus a replay target and rejecting
    shell/no-op forms makes ``exit_code: 0`` insufficient on its own while
    retaining ordinary interpreter, Cargo, and project-binary invocations.
    """
    if len(argv) < 2:
        raise ValueError(f"{where} argv must name an executable and replay target")
    if any(not item.strip() for item in argv):
        raise ValueError(f"{where} argv entries must be non-empty strings")
    if any(COMMAND_CONTROL_RE.search(item) for item in argv):
        raise ValueError(f"{where} argv must not contain shell/control syntax")

    program = argv[0]
    program_name = Path(program).name.lower()
    if program.startswith("-") or program_name in COMMAND_NOOP_PROGRAMS:
        raise ValueError(f"{where} argv executable is not a replay command: {program!r}")
    if program_name in COMMAND_SHELL_PROGRAMS:
        raise ValueError(f"{where} argv shell wrappers are not replay commands")
    if any(argument in COMMAND_INLINE_FLAGS for argument in argv[1:]):
        raise ValueError(f"{where} argv inline-code execution is not allowed")
    if not any(not argument.startswith("-") for argument in argv[1:]):
        raise ValueError(f"{where} argv needs a non-option replay target")


def _parse_record(raw: object, where: str) -> EvidenceRecord:
    if not isinstance(raw, dict):
        raise ValueError(f"{where} must be an object")
    kind = raw.get("kind")
    if not isinstance(kind, str) or kind not in ALL_KINDS:
        raise ValueError(
            f"{where} has unknown evidence kind {kind!r}; expected one of "
            + ", ".join(sorted(ALL_KINDS))
        )

    if kind in INFORMATIONAL_KINDS:
        if set(raw) != {"kind", "note"}:
            raise ValueError(f"{where} {kind} records take exactly kind and note")
        note = _non_empty_str(raw["note"], f"{where} note")
        return EvidenceRecord(
            kind=kind, scope=None, scenario=None, artifacts=(), commands=(),
            deviation=None, note=note,
        )

    expected_fields = {"kind", "scope", "scenario", "artifacts", "commands"}
    if kind in DEVIATIONS_BY_KIND:
        expected_fields.add("deviation")
    if set(raw) != expected_fields:
        raise ValueError(
            f"{where} {kind} records must have exactly "
            + ", ".join(sorted(expected_fields))
        )

    scope = raw["scope"]
    allowed_scopes = REQUIRED_SCOPES_BY_KIND[kind]
    if scope not in allowed_scopes:
        raise ValueError(
            f"{where} {kind} scope must be one of "
            + ", ".join(allowed_scopes)
            + f", got {scope!r}"
        )

    deviation = None
    if kind in DEVIATIONS_BY_KIND and raw["deviation"] != DEVIATIONS_BY_KIND[kind]:
        raise ValueError(
            f"{where} {kind} records must declare the deviation "
            f"{DEVIATIONS_BY_KIND[kind]!r}, got {raw['deviation']!r}"
        )
    if kind in DEVIATIONS_BY_KIND:
        deviation = DEVIATIONS_BY_KIND[kind]

    scenario = _parse_hash_binding(raw["scenario"], f"{where} scenario")

    raw_artifacts = raw["artifacts"]
    if not isinstance(raw_artifacts, list) or not raw_artifacts:
        raise ValueError(f"{where} artifacts must be a non-empty list")
    artifacts: list[tuple[str, str, str]] = []
    for index, raw_artifact in enumerate(raw_artifacts):
        spot = f"{where} artifact {index}"
        if not isinstance(raw_artifact, dict) or set(raw_artifact) != {
            "role",
            "path",
            "sha256",
        }:
            raise ValueError(
                f"{spot} must be an object with exactly role, path, and sha256"
            )
        role = _non_empty_str(raw_artifact["role"], f"{spot} role")
        if not ARTIFACT_ROLE_RE.match(role):
            raise ValueError(
                f"{spot} role must be a lowercase semantic name, got {role!r}"
            )
        artifact_path = _validate_relative_path(raw_artifact["path"], f"{spot} path")
        if any(existing_path == artifact_path for _role, existing_path, _sha in artifacts):
            raise ValueError(f"{where} duplicates artifact path {artifact_path}")
        artifacts.append(
            (
                role,
                artifact_path,
                _hex64(raw_artifact["sha256"], f"{spot} sha256"),
            )
        )

    raw_commands = raw["commands"]
    if not isinstance(raw_commands, list) or not raw_commands:
        raise ValueError(f"{where} commands must be a non-empty list")
    commands: list[tuple[tuple[str, ...], int]] = []
    for index, raw_command in enumerate(raw_commands):
        spot = f"{where} command {index}"
        if not isinstance(raw_command, dict) or set(raw_command) != {
            "argv",
            "exit_code",
        }:
            raise ValueError(
                f"{spot} must be an object with exactly argv and exit_code"
            )
        argv = raw_command["argv"]
        if (
            not isinstance(argv, list)
            or not argv
            or not all(isinstance(item, str) and item for item in argv)
        ):
            raise ValueError(f"{spot} argv must be a non-empty list of strings")
        _validate_command(argv, spot)
        exit_code = raw_command["exit_code"]
        if isinstance(exit_code, bool) or not isinstance(exit_code, int):
            raise ValueError(f"{spot} exit_code must be an integer")
        commands.append((tuple(argv), exit_code))

    return EvidenceRecord(
        kind=kind,
        scope=scope,
        scenario=scenario,
        artifacts=tuple(artifacts),
        commands=tuple(commands),
        deviation=deviation,
        note=None,
    )


def load_behavior_evidence(repo_root: Path) -> dict[str, EvidenceUnit]:
    """Load the evidence input; a missing file means no evidence at all."""
    path = repo_root / EVIDENCE_FILE
    if not path.is_file():
        return {}
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"{path} is not valid JSON: {error}") from error
    if not isinstance(raw, dict) or raw.get("schema_version") != SCHEMA_VERSION:
        raise ValueError(f"{path} schema_version must be {SCHEMA_VERSION}")
    raw_units = raw.get("units")
    if not isinstance(raw_units, list):
        raise ValueError(f"{path} units must be a list")

    result: dict[str, EvidenceUnit] = {}
    for index, raw_unit in enumerate(raw_units):
        where = f"{path} unit {index}"
        if not isinstance(raw_unit, dict):
            raise ValueError(f"{where} must be an object")
        if set(raw_unit) != {"source", "source_sha256", "destinations", "records"}:
            raise ValueError(
                f"{where} must have exactly source, source_sha256, destinations, "
                "and records"
            )
        source = _validate_relative_path(raw_unit["source"], f"{where} source")
        if source in result:
            raise ValueError(f"{path} duplicates evidence source {source}")
        source_sha256 = _hex64(raw_unit["source_sha256"], f"{where} source_sha256")

        raw_destinations = raw_unit["destinations"]
        if not isinstance(raw_destinations, list) or not raw_destinations:
            raise ValueError(f"{where} destinations must be a non-empty list")
        destinations: list[DestinationPin] = []
        seen_paths: set[str] = set()
        for destination_index, raw_destination in enumerate(raw_destinations):
            pin_path, pin_sha = _parse_hash_binding(
                raw_destination, f"{where} destination {destination_index}"
            )
            if pin_path in seen_paths:
                raise ValueError(f"{where} duplicates destination {pin_path}")
            seen_paths.add(pin_path)
            destinations.append(DestinationPin(path=pin_path, sha256=pin_sha))

        raw_records = raw_unit["records"]
        if not isinstance(raw_records, list) or not raw_records:
            raise ValueError(f"{where} records must be a non-empty list")
        records = tuple(
            _parse_record(raw_record, f"{where} record {record_index}")
            for record_index, raw_record in enumerate(raw_records)
        )

        result[source] = EvidenceUnit(
            source=source,
            source_sha256=source_sha256,
            destinations=tuple(destinations),
            records=records,
        )
    return result


def _hash_is_current(repo_root: Path, relative: str, expected: str) -> bool:
    path = _repo_file(repo_root, relative)
    if path is None or not path.is_file():
        return False
    return sha256_file(path) == expected


def _record_evidence_entry(record: EvidenceRecord) -> dict[str, object]:
    """Normalize a valid promoting record for deterministic manifest output."""
    entry: dict[str, object] = {
        "kind": record.kind,
        "scope": record.scope,
        "promoting": True,
        "level": "exact" if record.kind in EXACT_KINDS else "tolerance",
        "deviation": record.deviation,
        "scenario": {"path": record.scenario[0], "sha256": record.scenario[1]},
        "artifacts": [
            {"role": role, "path": path, "sha256": sha}
            for role, path, sha in record.artifacts
        ],
        "commands": [
            {"argv": list(argv), "exit_code": exit_code}
            for argv, exit_code in record.commands
        ],
    }
    return entry


def _informational_entry(record: EvidenceRecord) -> dict[str, object]:
    return {
        "kind": record.kind,
        "scope": None,
        "promoting": False,
        "note": record.note,
    }


def evaluate_unit(
    unit: EvidenceUnit | None,
    *,
    source_path: str,
    source_sha256: str,
    unit_kind: str,
    inventory_class: str,
    review_state: str,
    destination_states: Sequence[DestinationState],
    repo_root: Path,
) -> dict[str, object]:
    """Decide whether current executable evidence proves this unit's behavior.

    Fail-closed: every gate must pass for promotion, and every rejection is
    reported with a machine-readable reason.
    """
    result: dict[str, object] = {
        "status": "not_verified",
        "confidence": None,
        "scopes": [],
        "evidence": [],
        "rejected_evidence": [],
    }
    if unit is None:
        return result

    informational = [_informational_entry(r) for r in unit.records if r.kind in INFORMATIONAL_KINDS]
    result["evidence"] = sorted(
        informational, key=lambda entry: str(entry["kind"])
    )

    gate_reasons: list[str] = []
    if unit_kind != "translation_unit":
        gate_reasons.append("unit_kind_not_promotable")
    if inventory_class == "deferred_network":
        gate_reasons.append("deferred_network_unit")
    if review_state != "reviewed":
        gate_reasons.append("unreviewed_mapping")
    if unit.source_sha256 != source_sha256:
        gate_reasons.append("stale_source_hash")
    if gate_reasons:
        result["rejected_evidence"] = [{"reason": reason} for reason in gate_reasons]
        return result

    by_path = {state.path: state for state in destination_states}
    implementation_paths = sorted(
        state.path
        for state in destination_states
        if state.classification == "implementation"
        and state.cargo_reachable
        and _hash_is_current(repo_root, state.path, state.sha256)
    )

    # Informational notes are intentionally harmless annotations.  All of the
    # stricter destination and artifact gates below apply only when a record
    # could otherwise promote the unit, preserving the useful empty-evidence
    # behavior of the original format.
    promoting_records = [
        record for record in unit.records if record.kind in PROMOTING_KINDS
    ]
    if not promoting_records:
        return result

    # Destination pins must be current, mapped, and cover every implementation
    # fragment of the reviewed mapping (split modules pin every fragment).
    rejections: list[dict[str, object]] = []
    pinned: set[str] = set()
    for pin in sorted(unit.destinations, key=lambda pin: pin.path):
        if _repo_file(repo_root, pin.path) is None:
            rejections.append({"reason": f"path_outside_repo:{pin.path}"})
            continue
        state = by_path.get(pin.path)
        if state is None:
            rejections.append({"reason": f"unmapped_destination_pin:{pin.path}"})
            continue
        if state.classification != "implementation":
            rejections.append({"reason": f"non_implementation_destination:{pin.path}"})
        if not state.cargo_reachable:
            rejections.append({"reason": f"unreachable_destination:{pin.path}"})
        if not _hash_is_current(repo_root, state.path, state.sha256):
            rejections.append({"reason": f"stale_destination_state:{pin.path}"})
        if state.sha256 != pin.sha256:
            rejections.append({"reason": f"stale_destination_hash:{pin.path}"})
        if (
            state.classification == "implementation"
            and state.cargo_reachable
            and _hash_is_current(repo_root, state.path, state.sha256)
            and state.sha256 == pin.sha256
        ):
            pinned.add(pin.path)
    for path in implementation_paths:
        if path not in pinned:
            rejections.append({"reason": f"uncovered_destination:{path}"})
    if not any(path in pinned for path in implementation_paths):
        rejections.append({"reason": "no_reachable_implementation_destination"})

    valid_records: list[EvidenceRecord] = []
    destination_paths = set(by_path)
    for record in promoting_records:
        problems: list[str] = []
        scenario_path, scenario_sha = record.scenario or ("", "")
        scenario_file = _repo_file(repo_root, scenario_path)
        if scenario_file is None:
            problems.append(f"path_outside_repo:{scenario_path}")
        elif not _hash_is_current(repo_root, scenario_path, scenario_sha):
            problems.append(f"stale_scenario_hash:{scenario_path}")
        output_artifacts = 0
        seen_artifact_paths: set[str] = set()
        for role, artifact_path, artifact_sha in record.artifacts:
            if role not in KNOWN_ARTIFACT_ROLES:
                problems.append(f"unknown_artifact_role:{role}")
            if role in OUTPUT_ARTIFACT_ROLES:
                output_artifacts += 1
            if artifact_path in seen_artifact_paths:
                problems.append(f"duplicate_artifact_path:{artifact_path}")
            seen_artifact_paths.add(artifact_path)
            if artifact_path == scenario_path:
                problems.append(f"artifact_reuses_scenario:{artifact_path}")
            if artifact_path == source_path:
                problems.append(f"artifact_reuses_source:{artifact_path}")
            if artifact_path in destination_paths:
                problems.append(f"artifact_reuses_destination:{artifact_path}")
            artifact_file = _repo_file(repo_root, artifact_path)
            if artifact_file is None:
                problems.append(f"path_outside_repo:{artifact_path}")
            elif not _hash_is_current(repo_root, artifact_path, artifact_sha):
                problems.append(f"stale_artifact_hash:{artifact_path}")
        if output_artifacts == 0:
            problems.append("missing_output_artifact")
        for argv, exit_code in record.commands:
            if exit_code != 0:
                problems.append("failed_command:" + " ".join(argv))
        if problems:
            rejections.append({"kind": record.kind, "reasons": problems})
        else:
            valid_records.append(record)

    result["rejected_evidence"] = rejections
    if not valid_records or rejections:
        return result

    promoting_entries = [_record_evidence_entry(record) for record in valid_records]
    promoting_entries.sort(key=lambda entry: (str(entry["kind"]), str(entry["scope"])))
    result["evidence"] = sorted(
        promoting_entries + informational, key=lambda entry: str(entry["kind"])
    )
    result["status"] = "verified"
    result["confidence"] = (
        "exact" if any(record.kind in EXACT_KINDS for record in valid_records) else "tolerance"
    )
    result["scopes"] = sorted({str(entry["scope"]) for entry in promoting_entries})
    return result


def evidence_digest_rows(units: dict[str, EvidenceUnit]) -> list[str]:
    """Deterministic digest rows so generated artifacts track evidence edits."""
    rows: list[str] = []
    for source in sorted(units):
        unit = units[source]
        rows.append(f"evidence:{source}:source:{unit.source_sha256}")
        for pin in sorted(unit.destinations, key=lambda pin: pin.path):
            rows.append(f"evidence:{source}:dest:{pin.path}:{pin.sha256}")
        for record in unit.records:
            payload = json.dumps(
                {
                    "kind": record.kind,
                    "scope": record.scope,
                    "deviation": record.deviation,
                    "note": record.note,
                    "scenario": list(record.scenario) if record.scenario else None,
                    "artifacts": [list(artifact) for artifact in record.artifacts],
                    "commands": [
                        {"argv": list(argv), "exit_code": exit_code}
                        for argv, exit_code in record.commands
                    ],
                },
                sort_keys=True,
            )
            rows.append(f"evidence:{source}:record:{payload}")
    return rows
