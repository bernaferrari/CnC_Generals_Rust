#!/usr/bin/env python3
"""Compare the original C++ and production Rust skirmish placement search.

Compiles the search bodies directly from both sources. Only the vector and
legality-query boundaries are stubbed: legality succeeds at a specified query
index. Compares every candidate's f32 bits, query order, early termination, and
returned position. This proves this component, not terrain legality or gameplay.
Requires clang++ (or --cxx) and rustc. All build artifacts are temporary.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
CPP = "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPlayer.cpp"
RUST = "GeneralsRust/Code/Main/src/ai/economy.rs"

CPP_PREFIX = r'''
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <iostream>
using Real = float;
using Bool = bool;
constexpr float PATHFIND_CELL_SIZE_F = 10;
struct Coord3D { float x, y, z; };
uint32_t bits(float v) { uint32_t b; std::memcpy(&b, &v, sizeof b); return b; }
void emit(Coord3D p) {
    std::cout << bits(p.x) << " " << bits(p.y) << " " << bits(p.z) << "\n";
}
struct BuildAssistant {
    enum { CLEAR_PATH=1, TERRAIN_RESTRICTIONS=2, NO_OBJECT_OVERLAP=4 };
    int query = 0, accept;
    int isLocationLegalToBuild(Coord3D* p, ...) {
        emit(*p);
        return query++ == accept ? 0 : 1;
    }
};
constexpr int LBC_OK = 0;
bool isSkirmishAI() { return true; }
int main(int argc, char** argv) {
    BuildAssistant assistant;
    assistant.accept = std::atoi(argv[4]);
    auto TheBuildAssistant = &assistant;
    Coord3D pos{std::strtof(argv[1], nullptr), std::strtof(argv[2], nullptr),
                std::strtof(argv[3], nullptr)};
    int bldgPlan=0, angle=0, dozer=0, m_player=0;
'''
CPP_SUFFIX = r'''
    std::cout << "result " << valid << " ";
    emit(pos);
}
'''
RUST_PREFIX = r'''
#[derive(Clone, Copy)]
struct Vec3 { x: f32, y: f32, z: f32 }
impl Vec3 { fn new(x:f32, y:f32, z:f32) -> Self { Self {x,y,z} } }
mod game_logic { pub const PATHFIND_CELL_SIZE_F_RESIDUAL: f32 = 10.0; }
struct AIPlayer;
impl AIPlayer {
'''
RUST_SUFFIX = r'''
}
fn emit(p: Vec3) { println!("{} {} {}", p.x.to_bits(), p.z.to_bits(), p.y.to_bits()); }
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = Vec3::new(args[1].parse().unwrap(), args[3].parse().unwrap(),
                         args[2].parse().unwrap());
    let accept: i32 = args[4].parse().unwrap();
    let query = std::cell::Cell::new(0);
    let result = AIPlayer::wiggle_find_legal_build_position(seed, |p| {
        emit(p);
        let i = query.get();
        query.set(i + 1);
        i == accept
    });
    print!("result {} ", u8::from(result.is_some()));
    emit(result.unwrap_or(seed));
}
'''


def source_bodies(root: Path) -> tuple[str, str]:
    cpp = (root / CPP).read_text()
    start = cpp.index("\t\t\tReal posOffset;")
    end = cpp.index("\t\t\tif (!valid)", start)
    rust = (root / RUST).read_text()
    rstart = rust.index("    pub(super) fn wiggle_find_legal_build_position(")
    rend = rust.index("\n    /// Process one building", rstart)
    return cpp[start:end], rust[rstart:rend].replace("pub(super) fn", "fn", 1)


def source_constants(root: Path) -> tuple[str, str]:
    header = (root / "GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h").read_text()
    module = (root / "GeneralsRust/Code/Main/src/game_logic/object/mod.rs").read_text()
    cpp = re.search(r"^#define PATHFIND_CELL_SIZE_F\s+([^\s]+)", header, re.MULTILINE)
    rust = re.search(r"pub const PATHFIND_CELL_SIZE_F_RESIDUAL: f32 = ([^;]+);", module)
    if cpp is None or rust is None:
        raise ValueError("Cannot locate production pathfinding cell-size constants")
    return cpp.group(1), rust.group(1)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--cxx", default="clang++")
    parser.add_argument("--rustc", default="rustc")
    args = parser.parse_args()
    cpp, rust = source_bodies(args.repo_root)
    cpp_cell, rust_cell = source_constants(args.repo_root)
    cpp_prefix = CPP_PREFIX.replace("SIZE_F = 10;", f"SIZE_F = {cpp_cell};")
    rust_prefix = RUST_PREFIX.replace("RESIDUAL: f32 = 10.0;", f"RESIDUAL: f32 = {rust_cell};")
    cases = 0
    queries = 0
    with tempfile.TemporaryDirectory(prefix="generals-ai-placement-") as tmp:
        work = Path(tmp)
        cpp_file, rust_file = work / "original.cpp", work / "ported.rs"
        cpp_file.write_text(cpp_prefix + cpp + CPP_SUFFIX)
        rust_file.write_text(rust_prefix + rust + RUST_SUFFIX)
        original, ported = work / "original", work / "ported"
        subprocess.run([args.cxx, str(cpp_file), "-o", str(original)], check=True, timeout=60)
        subprocess.run([args.rustc, str(rust_file), "-o", str(ported)], check=True, timeout=60)
        # Fractional origins also detect changes in floating-point operation order.
        for origin in [("100", "100", "2"), ("0", "0", "0"),
                       ("123.456", "-199.5", "2.75"), ("-0.01", "0.07", "-3")]:
            for accept in [-1, 0, 1, 7, 8, 99, 3719, 3720]:
                params = [*origin, str(accept)]
                expected = subprocess.check_output([str(original), *params], text=True, timeout=10)
                actual = subprocess.check_output([str(ported), *params], text=True, timeout=10)
                if expected != actual:
                    print(f"FAIL: origin={origin}, first legal query={accept}")
                    lhs, rhs = expected.splitlines(), actual.splitlines()
                    for index in range(max(len(lhs), len(rhs))):
                        left = lhs[index] if index < len(lhs) else "<end>"
                        right = rhs[index] if index < len(rhs) else "<end>"
                        if left != right:
                            print(f"line {index}: C++ {left}; Rust {right}")
                            break
                    return 1
                cases += 1
                queries += len(expected.splitlines()) - 1
    print(f"PASS: {cases} scenarios, {queries} candidate queries; exact f32 bits and results")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
