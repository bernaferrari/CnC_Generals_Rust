#!/usr/bin/env python3
"""Generate the authoritative, evidence-backed port status dashboard.

Inventory is read from the split-aware provenance manifest. Build/test grades
come only from commands executed by this tool against the current worktree.
Differential and retail grades remain red until corresponding machine evidence
exists; prose and historical percentages cannot promote them.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


QUICK_GATES = (
    ("common_gamelogic_check", ["cargo", "check", "--locked", "--lib", "-p", "game_engine", "-p", "gamelogic"]),
    ("main_check", ["cargo", "check", "--locked", "--lib", "-p", "generals_main"]),
    (
        "client_non_network_check",
        [
            "cargo", "check", "--locked", "--lib", "-p", "game-client-rust",
            "--no-default-features", "--features", "platform-native",
        ],
    ),
    ("attached_tests", ["cargo", "test", "--locked", "-p", "generals-root-tests"]),
    (
        "common_internal_thing_factory",
        [
            "cargo", "test", "--locked", "-p", "game_engine", "--features", "internal",
            "--lib", "common::thing::thing_factory::tests::", "--", "--test-threads=1",
        ],
    ),
    (
        "compression_parity",
        ["cargo", "test", "--locked", "-p", "game_engine", "--test", "compression_parity_tests"],
    ),
    (
        "deterministic_crc",
        ["cargo", "test", "--locked", "-p", "gamelogic", "--test", "crc_standalone_test"],
    ),
    ("ww3d_validation", ["cargo", "test", "--locked", "-p", "ww3d-validation"]),
    ("main_library_tests_compile", ["cargo", "test", "--locked", "-p", "generals_main", "--lib", "--no-run"]),
)

FULL_GATES = (
    (
        "main_behavior_modules",
        [
            "cargo", "test", "--locked", "-p", "generals_main", "--lib",
            "attack_team_persist_never_bleeds_into_same_faction_team",
            "--", "--test-threads=1",
        ],
    ),
    (
        "deterministic_frame_trace",
        [
            "cargo", "test", "--locked", "-p", "generals_main",
            "--test", "deterministic_frame_trace_tests", "--", "--test-threads=1",
        ],
    ),
    (
        "save_load_integration",
        ["cargo", "test", "--locked", "-p", "generals_main", "--test", "save_load_tests"],
    ),
    (
        "ui_state_integration",
        [
            "cargo", "test", "--locked", "-p", "generals_main",
            "--test", "state_machine_parity_tests", "--test", "selection_click_cluster",
        ],
    ),
    (
        "playable_smoke_integration",
        ["cargo", "test", "--locked", "-p", "generals_main", "--test", "playable_smoke_tests"],
    ),
    (
        "cpp_rust_randomvalue_differential",
        ["python3", "Code/Main/scripts/run_cpp_rust_differential.py"],
    ),
    ("golden_skirmish", ["cargo", "run", "--locked", "-p", "generals_main", "--bin", "golden_skirmish_gate", "--release", "--", "--frames", "30"]),
    ("ai_skirmish", ["cargo", "run", "--locked", "-p", "generals_main", "--bin", "ai_skirmish_gate", "--release"]),
    ("map_frame", ["cargo", "run", "--locked", "-p", "generals_main", "--bin", "map_frame_gate", "--release"]),
)


def command(repo: Path, args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=repo, text=True, capture_output=True, check=check)


def selected_gates(profile: str) -> tuple[tuple[str, list[str]], ...]:
    if profile == "quick":
        return QUICK_GATES
    if profile == "full":
        return QUICK_GATES + FULL_GATES
    raise ValueError(f"unknown gate profile: {profile}")


def test_result_counts(output: str) -> tuple[int, int] | None:
    """Return total passed/failed harness cases, if Cargo ran tests."""
    runs = test_result_runs(output)
    if not runs:
        return None
    return sum(passed for passed, _failed in runs), sum(
        failed for _passed, failed in runs
    )


def test_result_runs(output: str) -> list[tuple[int, int]]:
    """Return passed/failed counts for each Cargo test harness result line."""
    output = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", output)
    return [
        (int(passed), int(failed))
        for passed, failed in re.findall(
        r"test result:\s*[^;\n]*?(\d+) passed;\s*(\d+) failed;",
        output,
        )
    ]


def selected_test_harness_count(args: list[str]) -> int | None:
    """Count explicitly selected test harnesses, excluding generic packages."""
    if "--lib" in args:
        return 1
    selected = sum(arg == "--test" for arg in args)
    return selected or None


def gate_evidence_errors(
    evidence: dict[str, Any], expected_profile: str | None = None
) -> list[str]:
    """Validate that evidence is the complete, exact selected gate run."""
    profile = evidence.get("profile")
    if expected_profile is not None and profile != expected_profile:
        return [f"profile mismatch: expected {expected_profile}, got {profile!r}"]
    if profile not in ("quick", "full"):
        return [f"invalid gate profile: {profile!r}"]

    expected = selected_gates(profile)
    actual = evidence.get("gates")
    if not isinstance(actual, list):
        return ["gate list is missing or invalid"]
    errors: list[str] = []
    if len(actual) != len(expected):
        errors.append(f"incomplete gate count: expected {len(expected)}, got {len(actual)}")

    for index, (name, args) in enumerate(expected):
        if index >= len(actual):
            break
        gate = actual[index]
        if not isinstance(gate, dict):
            errors.append(f"gate {index} is not an object")
            continue
        if gate.get("name") != name:
            errors.append(
                f"gate {index} name mismatch: expected {name!r}, got {gate.get('name')!r}"
            )
        if gate.get("command") != list(args):
            errors.append(f"gate {name} command does not match the selected contract")
        exit_code = gate.get("exit_code")
        if type(exit_code) is not int or exit_code != 0:
            errors.append(f"gate {name} did not exit successfully with an integer status")
        if gate.get("passed") is not True:
            errors.append(f"gate {name} is not marked passed")

        is_test_run = args[:2] == ["cargo", "test"] and "--no-run" not in args
        if not is_test_run:
            if gate.get("tests_passed") is not None or gate.get("tests_failed") is not None:
                errors.append(f"gate {name} records test counts for a non-test command")
            continue

        lines = gate.get("test_result_lines")
        if not isinstance(lines, list) or any(not isinstance(line, str) for line in lines):
            errors.append(f"gate {name} has malformed test result lines")
            continue
        runs = test_result_runs("\n".join(lines))
        counts = test_result_counts("\n".join(lines))
        if len(runs) != len(lines):
            errors.append(f"gate {name} has invalid test result lines")
        if counts is None or counts[0] <= 0:
            errors.append(f"gate {name} has no positive executed-test result")
        expected_harnesses = selected_test_harness_count(args)
        if expected_harnesses is not None:
            if len(runs) != expected_harnesses:
                errors.append(
                    f"gate {name} expected {expected_harnesses} test harness results, got {len(runs)}"
                )
            elif any(passed <= 0 for passed, _failed in runs):
                errors.append(f"gate {name} has a selected harness with no passing tests")
        if counts is not None and counts[1] != 0:
            errors.append(f"gate {name} reports failed test cases")

        recorded_passed = gate.get("tests_passed")
        recorded_failed = gate.get("tests_failed")
        if type(recorded_passed) is not int or recorded_passed != (counts[0] if counts else None):
            errors.append(f"gate {name} passed-test count does not match its output")
        if type(recorded_failed) is not int or recorded_failed != (counts[1] if counts else None):
            errors.append(f"gate {name} failed-test count does not match its output")
    return errors


def git_metadata(repo: Path) -> dict[str, str]:
    sha = command(repo, ["git", "rev-parse", "HEAD"]).stdout.strip()
    timestamp = command(repo, ["git", "show", "-s", "--format=%cI", "HEAD"]).stdout.strip()
    return {"commit_sha": sha, "commit_timestamp": timestamp}


def worktree_digest(repo: Path) -> str:
    digest = hashlib.sha256()
    digest.update(command(repo, ["git", "diff", "--binary", "HEAD"]).stdout.encode())
    untracked = command(
        repo, ["git", "ls-files", "--others", "--exclude-standard", "-z"]
    ).stdout.split("\0")
    for relative in sorted(item for item in untracked if item):
        path = repo / relative
        digest.update(relative.encode())
        if path.is_file():
            digest.update(path.read_bytes())
    return digest.hexdigest()


def run_gates(repo: Path, rust_root: Path, full: bool) -> dict[str, Any]:
    results: list[dict[str, Any]] = []
    for name, args in selected_gates("full" if full else "quick"):
        try:
            completed = command(rust_root, list(args), check=False)
            exit_code = completed.returncode
            output = (completed.stdout + "\n" + completed.stderr).strip()
        except OSError as error:
            exit_code = 127
            output = f"could not start command: {error}"
        combined = output.splitlines()
        is_test_run = args[:2] == ["cargo", "test"] and "--no-run" not in args
        counts = test_result_counts(output) if is_test_run else None
        test_result_lines = [
            line for line in combined if "test result:" in line
        ] if is_test_run else []
        passed = exit_code == 0 and (
            not is_test_run or (counts is not None and counts[0] > 0 and counts[1] == 0)
        )
        results.append(
            {
                "name": name,
                "command": list(args),
                "exit_code": exit_code,
                "passed": passed,
                "tests_passed": counts[0] if counts is not None else None,
                "tests_failed": counts[1] if counts is not None else None,
                "test_result_lines": test_result_lines,
                "output_tail": combined[-20:],
            }
        )
    return {
        "schema_version": 1,
        **git_metadata(repo),
        "worktree_digest": worktree_digest(repo),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "profile": "full" if full else "quick",
        "gates": results,
    }


def evidence_is_current(evidence: dict[str, Any], repo: Path) -> bool:
    metadata = git_metadata(repo)
    return (
        evidence.get("commit_sha") == metadata["commit_sha"]
        and evidence.get("worktree_digest") == worktree_digest(repo)
    )


def beads_status(repo: Path) -> dict[str, Any]:
    try:
        stats = json.loads(command(repo, ["bd", "stats", "--json"]).stdout)
        active = json.loads(
            command(repo, ["bd", "list", "--status", "open", "--json"]).stdout
        )
        active += json.loads(
            command(repo, ["bd", "list", "--status", "in_progress", "--json"]).stdout
        )
        return {
            "available": True,
            "stats": stats,
            "active_count": len(active),
            "active_without_acceptance": sorted(
                issue["id"] for issue in active if not issue.get("acceptance_criteria")
            ),
        }
    except (FileNotFoundError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        return {"available": False, "error": str(error)}


def quality_status(repo: Path) -> dict[str, Any]:
    reports: dict[str, Any] = {}
    scripts = {
        "rust_loc": "GeneralsRust/Code/Main/scripts/check_rust_loc.py",
        "unsafe_contracts": "GeneralsRust/Code/Main/scripts/check_unsafe_contracts.py",
    }
    for name, relative in scripts.items():
        completed = command(repo, ["python3", relative, "--json"], check=False)
        try:
            payload = json.loads(completed.stdout)
        except json.JSONDecodeError:
            payload = {"error": (completed.stdout + completed.stderr).strip()[-2_000:]}
        payload["exit_code"] = completed.returncode
        payload["passed"] = completed.returncode == 0
        reports[name] = payload
    return reports


def confidence_ladder(summary: dict[str, Any]) -> dict[str, Any]:
    required = int(summary.get("required_translation_units", 0))
    blockers = summary.get("blockers_by_kind", {})

    def level(count: int, meaning: str) -> dict[str, Any]:
        return {
            "count": count,
            "percent": round((100.0 * count / required), 2) if required else 0.0,
            "meaning": meaning,
        }

    behavior_level = level(
        int(summary.get("behavior_verified", 0)),
        "Current machine evidence proves observable C++ behavior",
    )
    evidence_summary = summary.get("behavior_evidence")
    if isinstance(evidence_summary, dict):
        # Verified behavior must stay transparent about which oracle the
        # evidence ran against and how exact the comparison was.
        behavior_level["evidence"] = {
            "scope_counts": evidence_summary.get("by_evidence_scope", {}),
            "confidence_counts": evidence_summary.get("by_confidence", {}),
            "rejected_records": evidence_summary.get("rejected_records", 0),
        }
    return {
        "denominator": required,
        "reachable_candidate": level(
            int(summary.get("reachable_implementation_candidates", 0)),
            "Cargo-reachable Rust candidate only; not reviewed parity",
        ),
        "reviewed_path": level(
            int(summary.get("reviewed_path_implementations", 0)),
            "Human-reviewed C++ unit to Rust destination ownership",
        ),
        "reviewed_symbol_ownership": level(
            max(0, required - int(blockers.get("unreviewed_symbol_ownership", required))),
            "C++ symbols assigned to reachable Rust implementations",
        ),
        "behavior_verified": behavior_level,
    }


def build_dashboard(
    repo: Path,
    manifest: dict[str, Any],
    evidence: dict[str, Any] | None,
    quality: dict[str, Any] | None = None,
    expected_profile: str | None = None,
) -> dict[str, Any]:
    summary = manifest["summary"]
    current = evidence is not None and evidence_is_current(evidence, repo)
    gates = evidence.get("gates", []) if current and evidence else []
    contract_errors = (
        gate_evidence_errors(evidence, expected_profile) if current and evidence else []
    )
    evidence_valid = current and evidence is not None and not contract_errors
    graded_gates = gates if evidence_valid else []
    quick_names = {name for name, _args in QUICK_GATES}
    full_names = {name for name, _args in FULL_GATES}
    passed = {gate["name"] for gate in graded_gates if gate.get("passed")}
    attempted = {gate["name"] for gate in graded_gates}
    differential_name = "cpp_rust_randomvalue_differential"
    quality = quality or {}
    quality_known = bool(quality) and all(
        isinstance(value, dict) and "passed" in value for value in quality.values()
    )
    return {
        "schema_version": 1,
        **git_metadata(repo),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "worktree_digest": worktree_digest(repo),
        "scope": {
            "required_non_network_translation_units": summary["required_translation_units"],
            "deferred_network_translation_units": summary["deferred_network_translation_units"],
        },
        "grades": {
            "inventory": "pass" if summary["strict_blockers"] == 0 else "fail",
            "build_and_tests": (
                "pass"
                if evidence_valid and quick_names <= passed
                else "fail" if current and evidence and contract_errors
                else "unknown"
            ),
            "headless_behavior": (
                "pass"
                if evidence_valid and evidence and evidence.get("profile") == "full" and full_names <= passed
                else "fail" if current and evidence and evidence.get("profile") == "full" and contract_errors
                else "unknown"
            ),
            "cpp_rust_differential": (
                "component-pass"
                if differential_name in passed
                else "fail" if differential_name in attempted else "missing"
            ),
            "retail_wgpu_validation": "missing",
            "maintainability_ratchets": (
                "pass"
                if quality_known and all(value["passed"] for value in quality.values())
                else "fail" if quality_known else "unknown"
            ),
        },
        "inventory": summary,
        "parity_confidence": confidence_ladder(summary),
        "maintainability": quality,
        "differential_scope": {
            "level": "component",
            "authoritative_fields": ["rng_seed", "crc_framing"],
            "fixture_only_fields": ["commands", "objects", "players"],
            "full_game_loop": False,
        },
        "verification_evidence": {
            "present": evidence is not None,
            "current_worktree": current,
            "profile": evidence.get("profile") if current and evidence else None,
            "valid_gate_contract": evidence_valid,
            "validation_errors": contract_errors,
            "gates": gates,
        },
        "beads": beads_status(repo),
        "metric_contract": manifest["metric_contract"],
    }


def parse_args() -> argparse.Namespace:
    script = Path(__file__).resolve()
    repo = script.parents[4]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=repo)
    parser.add_argument("--run-gates", choices=("quick", "full"))
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo = args.repo_root.resolve()
    rust_root = repo / "GeneralsRust"
    evidence_path = args.evidence or rust_root / "target/port-verification-evidence.json"
    output_path = args.output or rust_root / "target/port-dashboard.json"
    if args.run_gates:
        evidence = run_gates(repo, rust_root, args.run_gates == "full")
        evidence_path.parent.mkdir(parents=True, exist_ok=True)
        evidence_path.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    elif evidence_path.is_file():
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    else:
        evidence = None
    manifest = json.loads(
        (repo / "PORT_PROVENANCE_MANIFEST.json").read_text(encoding="utf-8")
    )
    dashboard = build_dashboard(
        repo,
        manifest,
        evidence,
        quality_status(repo),
        expected_profile=args.run_gates,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(dashboard, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(dashboard["grades"], indent=2))
    print(f"Dashboard: {output_path}")
    if args.run_gates:
        errors = gate_evidence_errors(evidence, args.run_gates)
        if errors:
            print("Gate verification failed:", file=sys.stderr)
            for error in errors:
                print(f"- {error}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
