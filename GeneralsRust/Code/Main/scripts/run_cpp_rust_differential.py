#!/usr/bin/env python3
"""Compare executed original Common CRC fixtures and Rust public CRC behavior.

RNG sequences intentionally differ under the user-approved seeded_rust_rng
policy. This component gate establishes CRC arithmetic, not gameplay parity.
"""
from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


def run(repo: Path, args: list[str], *, stdout=None) -> None:
    subprocess.run(args, cwd=repo, check=True, stdout=stdout)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[4])
    return parser.parse_args()


def main() -> int:
    repo = parse_args().repo_root.resolve()
    harness = repo / "GeneralsMD/Code/ParityHarness"
    fixture = repo / "GeneralsRust/Code/GameEngine/Common/tests/fixtures/crc_original.txt"
    with tempfile.TemporaryDirectory(prefix="generals-crc-") as scratch:
        executable = Path(scratch) / "crc_original"
        output = Path(scratch) / "crc_original.txt"
        run(repo, ["c++", "-std=c++17", "-O2", "-Wall", "-Wextra", "-Wpedantic",
                   "-D_DEBUG", "-fsanitize=undefined", "-fno-sanitize-recover=all",
                   f"-I{harness / 'shims'}", str(harness / "tests/crc_original.cpp"),
                   str(harness / "original_random_adapter.cpp"), "-o", str(executable)])
        with output.open("w", encoding="utf-8") as stream:
            run(repo, [str(executable)], stdout=stream)
        if fixture.read_bytes() != output.read_bytes():
            raise RuntimeError("executed original CRC fixture differs from the pinned fixture")
    run(repo, ["cargo", "test", "--locked", "--manifest-path",
               str(repo / "GeneralsRust/Code/GameEngine/Common/Cargo.toml"),
               "--test", "crc_parity", "--", "--test-threads=1"])
    print("Common CRC component passed; RNG sequence is an approved deviation.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
