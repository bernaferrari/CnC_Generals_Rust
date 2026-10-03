#!/usr/bin/env python3
"""Compare original ThingTemplate build arithmetic with the production Rust calculator.

Compile unchanged C++ method bodies and include the actual Rust module. Player,
template and GlobalData queries are numeric fixtures; neither harness loads INI
or queries a live world. Finite, nonnegative inputs stay inside C++ Int ranges.
Debug instant-build is enabled in C++ to match the port's supported cheat policy.
This is bounded arithmetic evidence, not proof of gameplay or authored defaults.
"""
from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
CPP = "GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingTemplate.cpp"
RUST = "GeneralsRust/Code/GameEngine/GameLogic/src/object/production/build_cost_calculator.rs"
CPP_CONSTANTS = "GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h"
RUST_CONSTANTS = "GeneralsRust/Code/GameEngine/Common/src/common/game_common.rs"

CPP_PREFIX = r'''
#include <algorithm>
#include <iostream>
using Int = int;
using Real = float;
using Bool = bool;
using std::max;
using std::min;
constexpr Int LOGICFRAMES_PER_SECOND = __FPS__;
constexpr Int BC_APPEARS_AT_RALLY_POINT = 1;
class ThingTemplate;
struct Handicap {
    enum Type { BUILDCOST, BUILDTIME };
    Real cost, time;
    Real getHandicap(Type type, const ThingTemplate*) const {
        return type == BUILDCOST ? cost : time;
    }
};
struct Player {
    Real costPercent, kindCost, timePercent, energy;
    Handicap handicap;
    Bool instant;
    Int count;
    Real getProductionCostChangePercent(Int) const { return costPercent; }
    Real getProductionCostChangeBasedOnKindOf(Int) const { return kindCost; }
    const Handicap* getHandicap() const { return &handicap; }
    Real getProductionTimeChangePercent(Int) const { return timePercent; }
    Bool buildsInstantly() const { return instant; }
    const Player* getEnergy() const { return this; }
    Real getEnergySupplyRatio() const { return energy; }
    void countObjectsByThingTemplate(Int, const ThingTemplate**, Bool, Int* out) const {
        *out = count;
    }
};
struct GlobalData {
    Real m_LowEnergyPenaltyModifier, m_MinLowEnergyProductionSpeed;
    Real m_MaxLowEnergyProductionSpeed, m_MultipleFactory;
};
GlobalData data;
const GlobalData* TheGlobalData = &data;
class ThingTemplate {
public:
    Int cost, completion, m_kindof = 0;
    Real time;
    Int getName() const { return 0; }
    Int getBuildCost() const { return cost; }
    Real getBuildTime() const { return time; }
    Int getBuildCompletion() const { return completion; }
    const ThingTemplate* getBuildFacilityTemplate(const Player* p) const {
        return p->count > 0 ? this : nullptr;
    }
    Int calcCostToBuild(const Player*) const;
    Int calcTimeToBuild(const Player*) const;
};
'''
CPP_DRIVER = r'''
int main() {
    ThingTemplate tmpl;
    Player p;
    Int instant, rally;
    while (std::cin >> tmpl.cost >> p.costPercent >> p.kindCost >> p.handicap.cost
                    >> tmpl.time >> p.handicap.time >> p.timePercent >> p.energy
                    >> instant >> rally >> p.count
                    >> data.m_LowEnergyPenaltyModifier >> data.m_MinLowEnergyProductionSpeed
                    >> data.m_MaxLowEnergyProductionSpeed >> data.m_MultipleFactory) {
        p.instant = instant != 0;
        tmpl.completion = rally;
        std::cout << tmpl.calcCostToBuild(&p) << ' ' << tmpl.calcTimeToBuild(&p) << '\n';
    }
}
'''
RUST_DRIVER = r'''
extern crate self as game_engine;
mod common {
    pub const LOGICFRAMES_PER_SECOND: u32 = __FPS__;
    pub mod global_data {
        pub struct Data {
            pub low_energy_penalty_modifier: f32,
            pub min_low_energy_production_speed: f32,
            pub max_low_energy_production_speed: f32,
            pub multiple_factory: f32,
        }
        // Live configuration access is intentionally outside this numeric harness.
        pub fn read() -> Data { panic!("unexpected live GlobalData query") }
    }
}
#[path = __SOURCE__]
mod production;
use production::{BuildCostCalculator, GlobalBuildModifiers, PlayerBuildModifiers, BuildFacilityContext};
use std::io::{self, Read};
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    for line in input.lines() {
        let v: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(v.len(), 15);
        let f = |i: usize| v[i].parse::<f32>().unwrap();
        let modifiers = PlayerBuildModifiers {
            production_cost_change_percent: f(1),
            production_cost_change_by_kind: f(2),
            handicap_cost_multiplier: f(3),
            handicap_time_multiplier: f(5),
            production_time_change_percent: f(6),
            energy_supply_ratio: f(7),
            builds_instantly: v[8] == "1",
        };
        let calc = BuildCostCalculator::with_modifiers(GlobalBuildModifiers {
            low_energy_penalty_modifier: f(11),
            min_low_energy_production_speed: f(12),
            max_low_energy_production_speed: f(13),
            multiple_factory_bonus: f(14),
            logic_frames_per_second: common::LOGICFRAMES_PER_SECOND,
        });
        let facility = BuildFacilityContext {
            facility_count: v[10].parse().unwrap(),
            appears_at_rally_point: v[9] == "1",
        };
        println!("{} {}", calc.calc_cost_to_build(v[0].parse().unwrap(), &modifiers),
                 calc.calc_time_to_build(f(4), &modifiers, Some(&facility)));
    }
}
'''


def original_method(source: str, name: str) -> str:
    start = source.index(f"Int ThingTemplate::{name}(")
    brace = source.index("{", start)
    depth = 0
    for index in range(brace, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
    raise ValueError(f"unterminated original method {name}")


def scenarios() -> list[tuple[int | float, ...]]:
    # base, cost%, kind cost, handicap cost, seconds, handicap time, time%,
    # energy, instant, rally, facility count, energy modifier, min, max, factory.
    cases = [(100, -0.1, 1, 0.9, 10, 1, 0, 1, 0, 0, 1, 1, 0.5, 0.8, 0.8)]
    for base, percent, kind, handicap in itertools.product(
        [0, 1, 100, 350, 1000, 2500, 50000],
        [-0.8, -0.3, -0.2, -0.1, 0, 0.1, 0.333],
        [0.8, 1, 1.2], [0.7, 0.9, 1, 1.2],
    ):
        cases.append((base, percent, kind, handicap, 10, 1, 0, 1, 0, 0, 1, 1, 0.5, 0.8, 0.8))
    for seconds, handicap, percent, energy, instant, rally, count, globals_ in itertools.product(
        [0, 0.001, 0.034, 0.999, 10, 45],
        [0.7, 1, 1.2], [-0.2, 0, 0.333],
        [0, 0.5, 0.8, 0.99, 1, 1.5], [0, 1], [0, 1], [0, 1, 2, 3],
        [(1, 0.5, 0.8, 0.8), (0.5, 0.5, 0.8, 0.8), (1, 0, 0, 0)],
    ):
        cases.append((1000, 0, 1, 1, seconds, handicap, percent, energy, instant, rally, count, *globals_))
    return cases


def run(command: list[str], input_text: str | None = None) -> str:
    result = subprocess.run(command, input=input_text, text=True, capture_output=True, timeout=60)
    if result.returncode:
        raise RuntimeError(f"command failed: {command}\n{result.stdout}\n{result.stderr}")
    return result.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--cxx", default="clang++")
    parser.add_argument("--rustc", default="rustc")
    args = parser.parse_args()
    source = (args.repo_root / CPP).read_text()
    cpp_fps = re.search(r"LOGICFRAMES_PER_SECOND\s*=\s*(\d+)", (args.repo_root / CPP_CONSTANTS).read_text())
    rust_fps = re.search(r"pub const LOGICFRAMES_PER_SECOND:\s*u32\s*=\s*(\d+)", (args.repo_root / RUST_CONSTANTS).read_text())
    if not cpp_fps or not rust_fps or cpp_fps[1] != rust_fps[1]:
        raise ValueError("original and production logic frame constants differ or are missing")
    methods = "\n".join(original_method(source, name) for name in ("calcCostToBuild", "calcTimeToBuild"))
    cases = scenarios()
    inputs = "".join(" ".join(map(str, case)) + "\n" for case in cases)
    with tempfile.TemporaryDirectory(prefix="generals-build-cost-") as tmp:
        work = Path(tmp)
        cpp_file, rust_file = work / "original.cpp", work / "ported.rs"
        cpp_file.write_text(CPP_PREFIX.replace("__FPS__", cpp_fps[1]) + methods + CPP_DRIVER)
        rust_file.write_text(RUST_DRIVER.replace("__FPS__", rust_fps[1]).replace("__SOURCE__", json.dumps(str((args.repo_root / RUST).resolve()))))
        original, ported = work / "original", work / "ported"
        run([args.cxx, "-std=c++17", "-O2", "-fno-fast-math", "-D_ALLOW_DEBUG_CHEATS_IN_RELEASE", str(cpp_file), "-o", str(original)])
        run([args.rustc, "--edition=2024", "-O", str(rust_file), "-o", str(ported)])
        expected = run([str(original)], inputs).splitlines()
        actual = run([str(ported)], inputs).splitlines()
        if len(expected) != len(cases) or len(actual) != len(cases):
            raise ValueError(f"expected {len(cases)} output rows, got C++={len(expected)} Rust={len(actual)}")
        for index, (cpp, rust) in enumerate(zip(expected, actual)):
            if cpp != rust:
                raise ValueError(f"first divergence case {index}: inputs={cases[index]} C++={cpp} Rust={rust}")
    print(f"Build arithmetic: {len(cases)} original-C++/production-Rust cases agree; logic FPS={cpp_fps[1]}")
    print(f"Original source SHA256: {hashlib.sha256(source.encode()).hexdigest()}")
    print(f"Rust source SHA256: {hashlib.sha256((args.repo_root / RUST).read_bytes()).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
