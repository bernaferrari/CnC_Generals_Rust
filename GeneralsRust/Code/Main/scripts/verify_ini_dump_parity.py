#!/usr/bin/env python3
"""Verify GeneralsMD and GeneralsRust INI dump parity."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[4]
DEFAULT_INI_ROOT = REPO_ROOT / "windows_game/extracted_big_files_v2/INIZH/Data/INI"
GENERALS_MD_DUMP = REPO_ROOT / "GeneralsMD/Code/Tools/ini_data_dump.py"
GENERALS_RUST_ROOT = REPO_ROOT / "GeneralsRust"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate and compare GeneralsMD/GeneralsRust INI data dumps"
    )
    parser.add_argument("--ini-root", type=Path, default=DEFAULT_INI_ROOT)
    parser.add_argument("--min-objects", type=int, default=1000)
    parser.add_argument("--min-weapons", type=int, default=350)
    parser.add_argument("--min-armors", type=int, default=50)
    parser.add_argument(
        "--keep-dumps",
        type=Path,
        help="Directory where generated JSON dumps should be kept",
    )
    args = parser.parse_args()

    output_dir_context = (
        tempfile.TemporaryDirectory()
        if args.keep_dumps is None
        else persistent_output_dir(args.keep_dumps)
    )

    with output_dir_context as output_dir_name:
        output_dir = Path(output_dir_name)
        md_dump_path = output_dir / "generalsmd_ini_dump.json"
        rust_dump_path = output_dir / "generalsrust_ini_dump.json"

        run_generalsmd_dump(args, md_dump_path)
        run_generalsrust_dump(args, rust_dump_path)

        md_dump = load_dump(md_dump_path)
        rust_dump = load_dump(rust_dump_path)
        differences = compare_dumps(md_dump, rust_dump)

        if differences:
            print("INI dump parity FAILED", file=sys.stderr)
            for difference in differences[:20]:
                print(f"- {difference}", file=sys.stderr)
            print(f"GeneralsMD dump: {md_dump_path}", file=sys.stderr)
            print(f"GeneralsRust dump: {rust_dump_path}", file=sys.stderr)
            return 1

        counts = md_dump["counts"]
        print(
            "INI dump parity OK: "
            f"{counts['object_files']} object files, "
            f"{counts['object_templates']} objects, "
            f"{counts['weapon_templates']} weapons, "
            f"{counts['armor_templates']} armors"
        )
        if args.keep_dumps is not None:
            print(f"Kept dumps in {output_dir}")
        return 0


class persistent_output_dir:
    def __init__(self, path: Path) -> None:
        self.path = path

    def __enter__(self) -> str:
        self.path.mkdir(parents=True, exist_ok=True)
        return str(self.path)

    def __exit__(self, _exc_type: object, _exc: object, _tb: object) -> None:
        return None


def run_generalsmd_dump(args: argparse.Namespace, output: Path) -> None:
    command = [
        sys.executable,
        str(GENERALS_MD_DUMP),
        "--ini-root",
        str(args.ini_root),
        "--output",
        str(output),
        "--min-objects",
        str(args.min_objects),
        "--min-weapons",
        str(args.min_weapons),
        "--min-armors",
        str(args.min_armors),
    ]
    subprocess.run(command, cwd=REPO_ROOT, check=True)


def run_generalsrust_dump(args: argparse.Namespace, output: Path) -> None:
    command = [
        "cargo",
        "run",
        "-p",
        "generals_main",
        "--bin",
        "ini_data_dump",
        "--",
        "--ini-root",
        str(args.ini_root),
        "--output",
        str(output),
        "--min-objects",
        str(args.min_objects),
        "--min-weapons",
        str(args.min_weapons),
        "--min-armors",
        str(args.min_armors),
    ]
    subprocess.run(command, cwd=GENERALS_RUST_ROOT, check=True)


def load_dump(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def compare_dumps(md_dump: dict[str, Any], rust_dump: dict[str, Any]) -> list[str]:
    differences: list[str] = []
    for section in ("counts", "objects", "weapons", "armors"):
        differences.extend(compare_value(section, md_dump.get(section), rust_dump.get(section)))
    return differences


def compare_value(path: str, md_value: Any, rust_value: Any) -> list[str]:
    if md_value == rust_value:
        return []

    if isinstance(md_value, dict) and isinstance(rust_value, dict):
        md_keys = set(md_value)
        rust_keys = set(rust_value)
        missing_in_rust = sorted(md_keys - rust_keys)
        missing_in_md = sorted(rust_keys - md_keys)
        differences = []
        if missing_in_rust:
            differences.append(f"{path}: missing in Rust: {missing_in_rust[:10]}")
        if missing_in_md:
            differences.append(f"{path}: missing in GeneralsMD: {missing_in_md[:10]}")
        for key in sorted(md_keys & rust_keys):
            child_differences = compare_value(
                f"{path}.{key}", md_value[key], rust_value[key]
            )
            if child_differences:
                differences.extend(child_differences)
                break
        return differences

    if isinstance(md_value, list) and isinstance(rust_value, list):
        if len(md_value) != len(rust_value):
            return [f"{path}: length differs GeneralsMD={len(md_value)} Rust={len(rust_value)}"]
        for index, (md_item, rust_item) in enumerate(zip(md_value, rust_value)):
            child_differences = compare_value(f"{path}[{index}]", md_item, rust_item)
            if child_differences:
                return child_differences
        return []

    return [f"{path}: GeneralsMD={md_value!r} Rust={rust_value!r}"]


if __name__ == "__main__":
    raise SystemExit(main())
