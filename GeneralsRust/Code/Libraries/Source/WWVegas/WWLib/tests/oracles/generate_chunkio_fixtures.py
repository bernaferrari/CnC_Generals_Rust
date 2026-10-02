#!/usr/bin/env python3
"""Compile original WWLib chunk I/O with a memory-file adapter and emit fixtures."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True, help="Repository with GeneralsMD")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compiler", default="clang++")
    args = parser.parse_args()
    source = args.root / "GeneralsMD/Code/Libraries/Source/WWVegas/WWLib"
    here = Path(__file__).resolve().parent
    names = ["chunkio.cpp", "chunkio.h", "iostruct.h"]
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="generals-chunkio-oracle-") as directory:
        build = Path(directory)
        for name in names:
            shutil.copyfile(source / name, build / name)
        executable = build / "oracle"
        subprocess.run([args.compiler, "-std=c++17", "-include", str(here / "chunkio_support.h"),
                        "-I", str(build), str(build / "chunkio.cpp"), str(here / "chunkio_fixture.cpp"),
                        "-o", str(executable)], check=True)
        subprocess.run([str(executable), str(args.output / "chunkio_cpp_nested.bin"),
                        str(args.output / "chunkio_cpp_partial.bin")], check=True)
    metadata = {
        "scope": "Original WWLib chunkio on little-endian host; memory-file adapter only",
        "load_scope": "Raw-buffer Read only: original typed-load overloads use sizeof(pointer), not sizeof(value)",
        "source_sha256": {name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in names},
        "fixture_sha256": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(args.output.glob("chunkio_cpp_*.bin"))},
    }
    (args.output / "chunkio_cpp_provenance.json").write_text(json.dumps(metadata, indent=2) + "\n")


if __name__ == "__main__":
    main()
