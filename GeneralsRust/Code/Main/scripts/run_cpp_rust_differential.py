#!/usr/bin/env python3
"""Run the portable original-C++ versus Rust RandomValue trace differential."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


def run(repo: Path, args: list[str], *, stdout=None) -> None:
    subprocess.run(args, cwd=repo, check=True, stdout=stdout)


def parse_args() -> argparse.Namespace:
    default_repo = Path(__file__).resolve().parents[4]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=default_repo)
    return parser.parse_args()


def main() -> int:
    repo = parse_args().repo_root.resolve()
    harness = repo / "GeneralsMD/Code/ParityHarness"
    scenario = repo / "parity_scenarios/smoke_attack.v1.json"
    manifest = repo / "GeneralsRust/Code/Main/Cargo.toml"

    run(repo, ["make", "-C", str(harness), "test"])
    with tempfile.TemporaryDirectory(prefix="generals-parity-") as scratch:
        cpp_trace = Path(scratch) / "cpp.json"
        rust_trace = Path(scratch) / "rust.json"
        with cpp_trace.open("w", encoding="utf-8") as output:
            run(repo, [str(harness / "bin/generalsmd_frame_trace"), str(scenario)], stdout=output)
        with rust_trace.open("w", encoding="utf-8") as output:
            run(
                repo,
                [
                    "cargo",
                    "run",
                    "--locked",
                    "--manifest-path",
                    str(manifest),
                    "--bin",
                    "deterministic_fixture_trace",
                    "--",
                    str(scenario),
                ],
                stdout=output,
            )
        run(
            repo,
            [
                "cargo",
                "run",
                "--locked",
                "--manifest-path",
                str(manifest),
                "--bin",
                "deterministic_trace_compare",
                "--",
                str(cpp_trace),
                str(rust_trace),
            ],
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
