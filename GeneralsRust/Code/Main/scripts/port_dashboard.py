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
import subprocess
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
    for name, args in QUICK_GATES + (FULL_GATES if full else ()):
        completed = command(rust_root, list(args), check=False)
        combined = (completed.stdout + "\n" + completed.stderr).strip().splitlines()
        results.append(
            {
                "name": name,
                "command": args,
                "exit_code": completed.returncode,
                "passed": completed.returncode == 0,
                "output_tail": combined[-20:],
            }
        )
        if completed.returncode != 0:
            break
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
) -> dict[str, Any]:
    summary = manifest["summary"]
    current = evidence is not None and evidence_is_current(evidence, repo)
    gates = evidence.get("gates", []) if current and evidence else []
    quick_names = {name for name, _args in QUICK_GATES}
    full_names = {name for name, _args in FULL_GATES}
    passed = {gate["name"] for gate in gates if gate.get("passed")}
    attempted = {gate["name"] for gate in gates}
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
            "build_and_tests": "pass" if quick_names <= passed else "unknown",
            "headless_behavior": "pass" if full_names <= passed else "unknown",
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
    dashboard = build_dashboard(repo, manifest, evidence, quality_status(repo))
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(dashboard, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(dashboard["grades"], indent=2))
    print(f"Dashboard: {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
