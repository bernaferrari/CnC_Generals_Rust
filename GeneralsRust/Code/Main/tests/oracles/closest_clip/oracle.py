#!/usr/bin/env python3
"""Run original Pathfinder::clip and ClipLine2D with a small fake extent."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
files = [
    (root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp', '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b'),
    (root / 'GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp', '88ebcf0105ad90b2ab16f6ebb48b5c7e5f4539c03126e5d572b854b4988744f0'),
]
for path, digest in files:
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit(f'C++ source pin mismatch: {path}')

def function(source, signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError('unclosed function')

line = files[1][0].read_text()
pathfinder = files[0][0].read_text()
program = r'''
#include <cassert>
#include <cmath>
#include <iostream>
using Int = int; using Bool = bool;
constexpr bool TRUE = true, FALSE = false;
constexpr int CLIP_LEFT=1, CLIP_RIGHT=2, CLIP_BOTTOM=4, CLIP_TOP=8;
constexpr int PATHFIND_CELL_SIZE=10;
constexpr float PATHFIND_CELL_SIZE_F=10.0f;
#define REAL_TO_INT_FLOOR(x) static_cast<int>(std::floor(x))
struct ICoord2D { int x, y; };
struct IRegion2D { ICoord2D lo, hi; };
struct Coord3D { float x, y, z; };
__LINE__
struct Pathfinder { IRegion2D m_extent{{0,0},{19,19}}; void clip(Coord3D *, Coord3D *); };
__CLIP__
void test(const char *name, Coord3D a, Coord3D b) {
    Pathfinder p; p.clip(&a, &b);
    std::cout << name << " " << a.x << " " << a.y << " " << b.x << " " << b.y << "\n";
}
void test_even(Coord3D a, Coord3D adjustTo) {
    bool centerInCell = false;
    __EVEN__
    test("even-goal-right", a, adjustTo);
}
int main() {
    test("inside", {25,55,0}, {75,55,0});
    test("goal-right", {25,55,0}, {250,55,0});
    test("start-left", {-25,55,0}, {75,55,0});
    test("diagonal", {-30,20,0}, {230,160,0});
    test("exterior", {-50,55,0}, {-20,75,0});
    test_even({25,55,0}, {250,55,0});
}
'''.replace('__LINE__', function(line, 'Bool ClipLine2D(')).replace('__CLIP__', function(pathfinder, 'void Pathfinder::clip(')).replace('__EVEN__', function(pathfinder[pathfinder.index('Path *Pathfinder::findClosestPath('):], 'if (!centerInCell)'))
with tempfile.TemporaryDirectory(prefix='closest-clip-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ("inside 25 55 75 55\n"
                "goal-right 25 55 190.05 50.05\n"
                "start-left 0.05 50.05 75 55\n"
                "diagonal 0.05 30.05 190.05 140.05\n"
                "exterior -50 55 -20 75\n"
                "even-goal-right 25 55 190.05 60.05\n")
    if actual != expected:
        raise SystemExit(f"unexpected original result: {actual!r}")
    print(actual, end='')
