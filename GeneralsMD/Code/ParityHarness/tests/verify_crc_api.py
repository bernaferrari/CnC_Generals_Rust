#!/usr/bin/env python3
"""Compile-only CRC safety boundary checks; never execute rejected value types.

By default this builds the actual CRC module as a tiny dependency-free library.
That is a module API check, not a replacement for the Common package build.
--library can instead check a prebuilt production game_engine rlib.
"""

import argparse
import json
from pathlib import Path
import shlex
import subprocess


def main():
    root = Path(__file__).resolve().parents[4]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--source", type=Path, default=root / "GeneralsRust/Code/GameEngine/Common/src/common/crc.rs")
    parser.add_argument("--library", type=Path)
    parser.add_argument("--dependency-dir", type=Path)
    args = parser.parse_args()
    out = args.output_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)

    def run(name, command):
        result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        (out / (name + ".log")).write_text(
            "$ " + shlex.join(command) + "\n" + result.stdout + result.stderr
            + "\nexit=" + str(result.returncode) + "\n"
        )
        return result

    library = args.library
    if library is None:
        wrapper = out / "library.rs"
        wrapper.write_text('#[path = ' + json.dumps(str(args.source.resolve())) + '] pub mod crc;\n')
        library = out / "libgame_engine.rlib"
        build = run("build", ["rustc", "--edition=2021", "--crate-name", "game_engine", "--crate-type", "rlib",
                              "-C", "overflow-checks=yes", str(wrapper), "-o", str(library)])
        if build.returncode:
            raise SystemExit("CRC library failed to build; no rejection is valid evidence")

    imports = "use game_engine::crc::{Crc, compute_crc_of_value};\n"
    padded = "#[repr(C)] struct Padded { tag: u8, word: u32 }\n"
    probes = {"positive": (imports + "fn main() { let mut crc = Crc::new(); "
                           "crc.compute_single(&7u32); crc.compute_multiple(&[1u32, 2]); "
                           "let _ = compute_crc_of_value(&[[1u32, 2], [3, 4]]); "
                           "let _ = compute_crc_of_value(&[] as &[u32; 0]); }", None)}
    for kind, declaration, value, type_name in [
        ("padded", padded, "Padded { tag: 1, word: 2 }", "Padded"),
        ("uninitialized", "", "std::mem::MaybeUninit::<u32>::uninit()", "MaybeUninit"),
    ]:
        for method, expression in [
            ("single", "Crc::new().compute_single(&value)"),
            ("multiple", "Crc::new().compute_multiple(&[value])"),
            ("value", "compute_crc_of_value(&value)"),
        ]:
            probes[kind + "_" + method] = (
                imports + declaration + "fn main() { let value = " + value + "; let _ = " + expression + "; }",
                ("CrcValue", type_name),
            )
    probes["external_impl"] = (
        imports + padded + "impl game_engine::crc::CrcValue for Padded { "
        "fn update_crc(&self, _: &mut Crc) {} } fn main() {}",
        ("sealed::Sealed", "Padded"),
    )
    failures = []
    for name, (source, required) in probes.items():
        path = out / (name + ".rs")
        path.write_text(source + "\n")
        command = ["rustc", "--edition=2021", "--error-format=json", "--emit=metadata",
                   str(path), "--extern", "game_engine=" + str(library.resolve()), "-o", str(out / (name + ".rmeta"))]
        if args.dependency_dir:
            command += ["-L", "dependency=" + str(args.dependency_dir.resolve())]
        result = run(name, command)
        if required is None:
            passed = result.returncode == 0
        else:
            errors = [json.loads(line) for line in result.stderr.splitlines() if line.startswith("{")]
            passed = result.returncode != 0 and any(
                (error.get("code") or {}).get("code") == "E0277"
                and all(text in (error.get("rendered") or "") for text in required)
                for error in errors
            )
        print(name + ": " + ("PASS" if passed else "FAIL (unexpected acceptance or diagnostic)"))
        if not passed:
            failures.append(name)
    raise SystemExit(1 if failures else 0)


if __name__ == "__main__":
    main()
