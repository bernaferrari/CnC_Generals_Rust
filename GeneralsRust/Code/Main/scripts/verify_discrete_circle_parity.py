#!/usr/bin/env python3
"""Compare complete original C++ and production Rust DiscreteCircle behavior.

The C++ executable compiles DiscreteCircle.cpp and its original header with only
PreRTS types stubbed. The Rust executable includes the production module. For
each input, compare the ordered edge list, edge count, and ordered draw calls.
This is bounded host-compiler evidence, not a proof for untested integer ranges.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
CPP_SOURCE = "GeneralsMD/Code/GameEngine/Source/Common/DiscreteCircle.cpp"
CPP_INCLUDE = "GeneralsMD/Code/GameEngine/Include"
CPP_HEADER = "GeneralsMD/Code/GameEngine/Include/Common/DiscreteCircle.h"
RUST_SOURCE = "GeneralsRust/Code/GameEngine/Common/src/common/discrete_circle.rs"
DEFAULT_SCENARIO = "parity_scenarios/discrete_circle_grid.v1.json"
DEFAULT_GOLDEN = "parity_golden/discrete_circle_grid_cpp.v1.json"

CPP_DRIVER = r'''
#include "PreRTS.h"
#include "Common/DiscreteCircle.h"
#include <iostream>

static void emit_scanline(Int x_start, Int x_end, Int y, void *) {
    std::cout << "draw " << x_start << ' ' << x_end << ' ' << y << '\n';
}

int main() {
    Int x, y, radius;
    while (std::cin >> x >> y >> radius) {
        DiscreteCircle circle(x, y, radius);
        std::cout << "count " << circle.getEdgeCount() << '\n';
        for (const HorzLine &edge : circle.getEdges()) {
            std::cout << "edge " << edge.xStart << ' ' << edge.xEnd << ' ' << edge.yPos << '\n';
        }
        circle.drawCircle(emit_scanline, nullptr);
        std::cout << "end\n";
    }
}
'''

RUST_DRIVER = r'''
mod production {
    include!(r#"__RUST_SOURCE__"#);
}
use production::DiscreteCircle;
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut values = input.split_whitespace();
    while let Some(x) = values.next() {
        let x: i32 = x.parse().unwrap();
        let y: i32 = values.next().unwrap().parse().unwrap();
        let radius: i32 = values.next().unwrap().parse().unwrap();
        let circle = DiscreteCircle::new(x, y, radius);
        println!("count {}", circle.get_edge_count());
        for edge in circle.get_edges() {
            println!("edge {} {} {}", edge.x_start, edge.x_end, edge.y_pos);
        }
        circle.draw_circle(|start, end, row| println!("draw {start} {end} {row}"));
        println!("end");
    }
}
'''


def run(command: list[str], *, input_text: str | None = None) -> str:
    return subprocess.run(
        command, input=input_text, text=True, capture_output=True, check=True, timeout=60
    ).stdout


def cases(scenario: dict) -> list[tuple[int, int, int]]:
    if scenario.get("schema") != "generals.discrete_circle.scenario.v1":
        raise ValueError("unsupported discrete circle scenario schema")
    if not isinstance(scenario.get("scenario"), str) or not scenario["scenario"]:
        raise ValueError("scenario must have a nonempty name")
    radius_min, radius_max = scenario.get("radius_min"), scenario.get("radius_max")
    x_centers, y_centers = scenario.get("x_centers"), scenario.get("y_centers")
    if any(type(value) is not int for value in (radius_min, radius_max)):
        raise ValueError("radius bounds must be integers")
    if not (0 <= radius_min <= radius_max <= 1024):
        raise ValueError("radius bounds must be ordered and within 0..1024")
    for name, values in (("x_centers", x_centers), ("y_centers", y_centers)):
        if not isinstance(values, list) or not values or any(type(v) is not int for v in values):
            raise ValueError(f"{name} must be a nonempty integer array")
        if len(values) != len(set(values)):
            raise ValueError(f"{name} contains duplicates")
    # These bounds avoid invalid negative radii and signed arithmetic overflow.
    # Nonnegative y also avoids shifting a negative center in original C++.
    if any(abs(x) > 10_000 for x in x_centers) or any(not 0 <= y <= 10_000 for y in y_centers):
        raise ValueError("centers must be within tested safe bounds")
    inputs = [(x, y, radius) for radius in range(radius_min, radius_max + 1)
              for x in x_centers for y in y_centers]
    if type(scenario.get("expected_cases")) is not int or scenario["expected_cases"] != len(inputs):
        raise ValueError("expected_cases does not match scenario grid")
    return inputs


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def golden_record(repo: Path, scenario_path: Path, scenario: dict, output: str,
                  count: int) -> dict:
    return {
        "schema": "generals.discrete_circle.cpp_golden.v1",
        "scenario": scenario["scenario"],
        "scenario_sha256": sha256(scenario_path.read_bytes()),
        "cpp_source_sha256": sha256((repo / CPP_SOURCE).read_bytes()),
        "cpp_header_sha256": sha256((repo / CPP_HEADER).read_bytes()),
        "cases": count,
        "cpp_output_sha256": sha256(output.encode("utf-8")),
    }


def records(output: str, inputs: list[tuple[int, int, int]]) -> list[list[str]]:
    groups = [group.splitlines() for group in output.split("end\n")]
    if groups[-1] != []:
        raise AssertionError("producer omitted final end marker or emitted trailing data")
    groups.pop()
    if len(groups) != len(inputs):
        raise AssertionError(f"producer returned {len(groups)} cases, expected {len(inputs)}")
    return groups


def first_difference(
    expected: list[list[str]], actual: list[list[str]], inputs: list[tuple[int, int, int]]
) -> str | None:
    for case, (left, right) in enumerate(zip(expected, actual)):
        for line in range(max(len(left), len(right))):
            cpp = left[line] if line < len(left) else "<end>"
            rust = right[line] if line < len(right) else "<end>"
            if cpp != rust:
                x, y, radius = inputs[case]
                return (f"case {case} (x={x}, y={y}, radius={radius}), record {line}: "
                        f"C++={cpp!r}, Rust={rust!r}")
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--scenario", type=Path, default=Path(DEFAULT_SCENARIO))
    parser.add_argument("--golden", type=Path, default=Path(DEFAULT_GOLDEN))
    parser.add_argument("--update-golden", action="store_true",
                        help="replace the pinned C++ digest after reviewing source/fixture drift")
    args = parser.parse_args()
    repo = args.repo_root.resolve()
    scenario_path = args.scenario if args.scenario.is_absolute() else repo / args.scenario
    golden_path = args.golden if args.golden.is_absolute() else repo / args.golden
    scenario = json.loads(scenario_path.read_text())
    inputs = cases(scenario)
    input_text = "".join(f"{x} {y} {radius}\n" for x, y, radius in inputs)

    with tempfile.TemporaryDirectory(prefix="generals-discrete-circle-") as temp:
        work = Path(temp)
        (work / "PreRTS.h").write_text("#include <vector>\nusing Int = int;\n")
        cpp_driver = work / "driver.cpp"
        cpp_driver.write_text(CPP_DRIVER)
        rust_driver = work / "driver.rs"
        rust_source = repo / RUST_SOURCE
        rust_driver.write_text(RUST_DRIVER.replace("__RUST_SOURCE__", str(rust_source)))
        cpp_binary, rust_binary = work / "original", work / "ported"
        run([args.cxx, "-std=c++17", "-O0", "-I", str(work), "-I", str(repo / CPP_INCLUDE),
             str(repo / CPP_SOURCE), str(cpp_driver), "-o", str(cpp_binary)])
        run([args.rustc, "--edition=2024", "-A", "dead_code", str(rust_driver),
             "-o", str(rust_binary)])

        cpp_output = run([str(cpp_binary)], input_text=input_text)
        expected = records(cpp_output, inputs)
        digest = golden_record(repo, scenario_path, scenario, cpp_output, len(inputs))
        if not args.update_golden:
            golden = json.loads(golden_path.read_text())
            if golden != digest:
                differing = [key for key in digest if golden.get(key) != digest[key]]
                raise AssertionError(f"C++ golden digest mismatch: {', '.join(differing)}; "
                                     "review drift before --update-golden")
        actual = records(run([str(rust_binary)], input_text=input_text), inputs)
        difference = first_difference(expected, actual, inputs)
        if difference:
            print(f"FAIL: {difference}")
            return 1

        # A deliberately broken temporary copy confirms the comparison detects
        # callback-order/coverage regressions; the production file is untouched.
        original = rust_source.read_text()
        needle = "if edge.y_pos != self.y_center {"
        if original.count(needle) != 1:
            raise AssertionError("negative control cannot find production mirror branch")
        mutant = work / "mutant.rs"
        mutant.write_text(original.replace(needle, "if false && edge.y_pos != self.y_center {"))
        rust_driver.write_text(RUST_DRIVER.replace("__RUST_SOURCE__", str(mutant)))
        mutant_binary = work / "mutant"
        run([args.rustc, "--edition=2024", "-A", "dead_code", str(rust_driver),
             "-o", str(mutant_binary)])
        mutated = records(run([str(mutant_binary)], input_text=input_text), inputs)
        if first_difference(expected, mutated, inputs) is None:
            raise AssertionError("negative control failed to detect missing mirrored scanlines")

    if args.update_golden:
        golden_path.parent.mkdir(parents=True, exist_ok=True)
        golden_path.write_text(json.dumps(digest, indent=2) + "\n")
        print(f"Updated C++ golden digest: {golden_path}")

    print(f"PASS: {len(inputs)} C++/Rust cases; ordered edges and draw calls match; "
          "negative control detected")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
