#!/usr/bin/env python3
"""Compare the original C++ force vector and RNG requests with the Rust OCL helper."""

import argparse
import math
from pathlib import Path
import re
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[4]
WWMATH = ROOT / "GeneralsMD/Code/Libraries/Source/WWVegas/WWMath"
OCL_SOURCE = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp"
RUST_SOURCE = ROOT / "GeneralsRust/Code/GameEngine/GameLogic/src/object_creation_list/nuggets.rs"

CASES = [
    (0.0, 0.0, 10.0),
    (0.3, 1.2, 60.0),
    (1.8, -0.5, 120.125),
    (2.0, 1.57079632679, 50.0),
    (4.9, 0.7, 27.13),
    (6.2, 1.4, 101.7),
    (1.2, 0.8, 0.0),
]


def definition(text: str, signature: str) -> str:
    # The selected top-level functions have an unindented closing brace.
    match = re.search(signature, text)
    if match is None:
        raise ValueError(f"Missing source definition {signature}")
    end = text.index("\n}", match.end()) + 2
    return text[match.start():end]


def cpp_source() -> str:
    header = (WWMATH / "matrix3d.h").read_text()
    pi = re.search(r"^#define\s+WWMATH_PI\s+(\d+\.\d+f)\s*$",
                   (WWMATH / "wwmath.h").read_text(), re.MULTILINE)
    if pi is None:
        raise ValueError("Missing WWMATH_PI")
    matrix = "\n".join(definition(header, signature) for signature in (
        r"WWINLINE void Matrix3D::Rotate_Y\(float theta\)",
        r"WWINLINE void Matrix3D::Rotate_Z\(float theta\)",
        r"WWINLINE void\s+Matrix3D::Scale\(float scale\)",
    ))
    getter = re.search(r"WWINLINE Vector3 Get_X_Vector\(\) const \{[^\n]+\}", header)
    if getter is None:
        raise ValueError("Missing Matrix3D::Get_X_Vector")
    force = definition(OCL_SOURCE.read_text(), r"static void calcRandomForce\(")
    calls = "\n".join(
        f"    run({angle}f, {pitch}f, {magnitude}f);"
        for angle, pitch, magnitude in CASES
    )
    return f"""
#include <cassert>
#include <cmath>
#include <cstdio>
#define WWINLINE inline
constexpr float PI = {pi.group(1)};
using Real = float;
struct Coord3D {{ float x, y, z; }};
struct Vector3 {{
    float X, Y, Z;
    Vector3(float x, float y, float z) : X(x), Y(y), Z(z) {{}}
}};
class Matrix3D {{
    float Row[3][4] {{{{1,0,0,0}},{{0,1,0,0}},{{0,0,1,0}}}};
public:
    explicit Matrix3D(int) {{}}
    void Scale(float);
    void Rotate_Y(float);
    void Rotate_Z(float);
    {getter.group()}
}};
{matrix}
float draw_values[3];
int draw_index;
Real GameLogicRandomValueReal(Real lo, Real hi) {{
    assert(draw_index < 3);
    const float bounds[3][2] = {{{{0,2*PI}},{{-2,2}},{{0,200}}}};
    assert(lo == bounds[draw_index][0] && hi == bounds[draw_index][1]);
    return draw_values[draw_index++];
}}
{force}
void run(float angle, float pitch, float magnitude) {{
    draw_values[0] = angle;
    draw_values[1] = pitch;
    draw_values[2] = magnitude;
    draw_index = 0;
    Coord3D result;
    calcRandomForce(0.0f, 200.0f, -2.0f, 2.0f, &result);
    assert(draw_index == 3);
    std::printf("%.9g %.9g %.9g\\n", result.x, result.y, result.z);
}}
int main() {{
{calls}
}}
"""


def rust_source() -> str:
    source = RUST_SOURCE.read_text()
    vector = definition(source, r"pub fn calc_random_force_xyz\(")
    force = definition(source, r"(?:pub\(super\) )?fn calc_random_force_with\(")
    calls = "\n".join(
        f"    run([{angle}, {pitch}, {magnitude}]);"
        for angle, pitch, magnitude in CASES
    )
    return f"""
use std::f32::consts::PI;
type Real = f32;
pub struct Coord3D(f32, f32, f32);
impl Coord3D {{
    fn new(x: f32, y: f32, z: f32) -> Self {{ Self(x, y, z) }}
}}
{vector}
mod ocl {{
    use super::*;
    {force}
}}
fn run(values: [f32; 3]) {{
    let bounds = [(0.0, 2.0 * PI), (-2.0, 2.0), (0.0, 200.0)];
    let mut draws = bounds.into_iter().zip(values);
    let result = ocl::calc_random_force_with(|lo, hi| {{
        let (request, value) = draws.next().expect("extra force draw");
        assert_eq!((lo, hi), request);
        value
    }}, 0.0, 200.0, -2.0, 2.0);
    assert!(draws.next().is_none(), "missing force draw");
    println!("{{:.9e}} {{:.9e}} {{:.9e}}", result.0, result.1, result.2);
}}
fn main() {{
{calls}
}}
"""


def observed(binary: Path) -> list[tuple[float, float, float]]:
    output = subprocess.check_output([binary], text=True)
    return [tuple(map(float, line.split())) for line in output.splitlines()]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--rustc", default="rustc")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="ocl-force-parity-") as temp:
        temp = Path(temp)
        cpp = temp / "original.cpp"
        rust = temp / "ported.rs"
        cpp.write_text(cpp_source())
        rust.write_text(rust_source())
        originals, port = temp / "original", temp / "ported"
        subprocess.run([args.cxx, "-std=c++17", "-ffp-contract=off", cpp, "-o", originals],
                       check=True)
        subprocess.run([args.rustc, "--edition=2024", rust, "-o", port], check=True)
        original, ported = observed(originals), observed(port)
    if len(original) != len(CASES) or len(ported) != len(CASES):
        raise RuntimeError("Unexpected vector count")
    for scenario, expected, actual in zip(CASES, original, ported, strict=True):
        # The Rust scalar products group the products differently from the
        # original scaled matrix, so allow floating-point rounding differences.
        if not all(math.isclose(left, right, rel_tol=2e-7, abs_tol=1e-7)
                   for left, right in zip(expected, actual, strict=True)):
            raise RuntimeError(f"{scenario} C++ {expected} differs from Rust {actual}")
    print(f"Original C++ and Rust matched force draw order and {len(CASES)} vectors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
