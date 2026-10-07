#!/usr/bin/env python3
"""Check FSM continuation borrows using the actual crate in a temporary snapshot.

No production source is edited. These probes must fail at their intended borrow
sites; unrelated compilation errors cannot make this check pass. Ordinary FSM
and native factory tests separately cover behavior, ordering and snapshot wire.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


PROBE_NAME = "state_machine_step_ownership_probes.rs"
PROBES = """use crate::state_machine::{StateMachine, StateUpdate};
use crate::modules::AIUpdateInterface;
use std::any::Any;

fn conflicting_machine(core: &mut StateMachine, ai: &mut dyn AIUpdateInterface, owner: &mut dyn Any) {
    let step = core.begin_update_with_ai_and_owner(ai, owner);
    core.clear();
    let _ = step.finish();
}

fn conflicting_ai(core: &mut StateMachine, ai: &mut dyn AIUpdateInterface, owner: &mut dyn Any) {
    let step = core.begin_update_with_ai_and_owner(ai, owner);
    ai.set_queue_for_path_time(123);
    let _ = step.finish();
}

fn conflicting_owner(core: &mut StateMachine, ai: &mut dyn AIUpdateInterface, owner: &mut dyn Any) {
    let step = core.begin_update_with_ai_and_owner(ai, owner);
    let _ = owner.downcast_mut::<u32>();
    let _ = step.finish();
}

fn escaped_driver_reference(core: &mut StateMachine, ai: &mut dyn AIUpdateInterface, owner: &mut dyn Any) {
    let mut update = core.begin_update_with_ai_and_owner(ai, owner);
    let leaked = match &mut update {
        StateUpdate::Body(step) => step.with_driver(|core, _ai, _owner| core),
        StateUpdate::Complete(_) => return,
    };
    let _ = update.finish();
    leaked.clear();
}

fn concrete_driver_context(core: &mut StateMachine, ai: &mut crate::object::unit::UnitAIUpdate, owner: &mut dyn Any) {
    let mut update = core.begin_update_with_ai_and_owner(ai, owner);
    if let StateUpdate::Body(step) = &mut update {
        step.with_driver(|_core, ai, _owner| {
            let _: &mut crate::object::unit::UnitAIUpdate = ai;
        });
    }
    let _ = update.finish();
}

fn escaped_native_goal(machine: &mut crate::ai::states::AIStateMachine, ai: &mut crate::object::unit::UnitAIUpdate) {
    let mut leaked_goal = None;
    let _ = machine.update_state_machine(ai, |driver, _ai, _owner| {
        leaked_goal = driver.get_goal_path_position(0);
    });
    let _ = leaked_goal.map(|goal| goal.x);
}

fn concrete_native_driver(machine: &mut crate::ai::states::AIStateMachine, ai: &mut crate::object::unit::UnitAIUpdate) {
    let _ = machine.update_state_machine(ai, |driver, ai, _owner| {
        let _: &mut crate::object::unit::UnitAIUpdate = ai;
        driver.set_goal_path(&[]);
        let _: Option<&crate::common::Coord3D> = driver.get_goal_path_position(0);
    });
}
"""


def snapshot(repo: Path, destination: Path) -> Path:
    """Copy tracked/current source, preserving external asset links as links."""
    listed = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "GeneralsRust"],
        cwd=repo,
    )
    for entry in sorted(set(listed.decode().split("\0")) - {""}):
        source = repo / entry
        target = destination / entry
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            target.symlink_to(source.resolve(), target_is_directory=source.is_dir())
        elif source.is_file():
            shutil.copy2(source, target)

    # Existing C++ and provenance inputs are read through their original paths.
    # Cargo build output goes to target-dir, never into these input directories.
    for source in repo.iterdir():
        if source.name not in {"GeneralsRust", ".git", ".beads", ".venv-parity"}:
            (destination / source.name).symlink_to(source.resolve(), target_is_directory=source.is_dir())

    workspace = destination / "GeneralsRust"
    source_root = workspace / "Code/GameEngine/GameLogic/src"
    if not source_root.resolve().is_relative_to(destination.resolve()):
        raise RuntimeError("Probe sources must stay inside the temporary snapshot")
    library_path = source_root / "lib.rs"
    if library_path.is_symlink():
        contents = library_path.read_bytes()
        library_path.unlink()
        library_path.write_bytes(contents)
    if (source_root / PROBE_NAME).exists():
        raise RuntimeError(f"Probe filename already exists: {PROBE_NAME}")
    (source_root / PROBE_NAME).write_text(PROBES)
    with library_path.open("a") as library:
        library.write("\n#[cfg(test)]\nmod state_machine_step_ownership_probes;\n")
    return workspace


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[4])
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    repo = args.repo_root.resolve()
    target_dir = (args.target_dir or repo / "GeneralsRust/target").resolve()
    with tempfile.TemporaryDirectory(prefix="generals-fsm-ownership-") as temporary:
        workspace = snapshot(repo, Path(temporary))
        command = [
            "cargo", "check", "--locked", "-q", "-p", "gamelogic", "--tests",
            "--target-dir", str(target_dir), "--message-format=json",
        ]
        result = subprocess.run(command, cwd=workspace, text=True, capture_output=True)

    errors = []
    for line in result.stdout.splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        message = record.get("message", {})
        if record.get("reason") == "compiler-message" and message.get("level") == "error":
            errors.append(message)

    expected = {
        "machine": ("E0499", "cannot borrow `*core`"),
        "ai": ("E0499", "cannot borrow `*ai`"),
        "owner": ("E0499", "cannot borrow `*owner`"),
        "escape": (None, "lifetime may not live long enough"),
        "native_goal_escape": ("E0521", "borrowed data escapes outside of closure"),
    }
    matched = {}
    for name, (code, text) in expected.items():
        matched[name] = any(
            (error.get("code") or {}).get("code") == code
            and text in error.get("message", "")
            and any(
                span.get("is_primary") and Path(span.get("file_name", "")).name == PROBE_NAME
                for span in error.get("spans", [])
            )
            for error in errors
        )
    passed = result.returncode == 101 and len(errors) == len(expected) and all(matched.values())
    report = {
        "passed": passed,
        "cargo_exit": result.returncode,
        "checks": matched,
        "errors": errors,
        "stderr": result.stderr,
        "scope": "Compiler ownership constraints on actual production types; no gameplay or mutex-removal claim.",
    }
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("passed", "cargo_exit", "checks", "scope")}))
    if not passed and not args.output:
        print(result.stdout)
        print(result.stderr)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
