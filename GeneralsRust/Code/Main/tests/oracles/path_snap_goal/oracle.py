#!/usr/bin/env python3
"""Execute the original snapClosestGoalPosition neighborhood ordering."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp = (root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp').read_bytes()
if hashlib.sha256(cpp).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = cpp.decode()
start = source.index('void Pathfinder::snapClosestGoalPosition(Object *obj, Coord3D *pos)')
opening = source.index('{', start)
depth = 0
for index in range(opening, len(source)):
    depth += (source[index] == '{') - (source[index] == '}')
    if depth == 0:
        method = source[start:index + 1]
        break
else:
    raise ValueError('unclosed snapClosestGoalPosition')
program = r'''
#include <iostream>
using Int = int; using Bool = bool; using ObjectID = unsigned int; using PathfindLayerEnum = int;
constexpr int LAYER_GROUND = 1; constexpr ObjectID INVALID_ID = 0; constexpr float PATHFIND_CELL_SIZE_F = 10;
struct Coord3D { float x, y, z; }; struct ICoord2D { Int x, y; };
struct Object { ObjectID getID() const { return 100; } };
struct Terrain { int getLayerForDestination(Coord3D*) { return LAYER_GROUND; } } terrain; Terrain *TheTerrainLogic = &terrain;
struct PathfindCell { enum { UNIT_PRESENT_FIXED = 3 }; ObjectID getGoalUnit() { return INVALID_ID; } int getFlags() { return 0; } };
struct Pathfinder {
    void getRadiusAndCenter(Object*, Int& radius, Bool& center) { radius = 0; center = true; }
    void worldToCell(Coord3D* pos, ICoord2D* cell) { cell->x = int(pos->x / 10); cell->y = int(pos->y / 10); }
    void adjustCoordToCell(Int x, Int y, Bool, Coord3D& pos, PathfindLayerEnum) { pos.x = (x + .5f) * 10; pos.y = (y + .5f) * 10; }
    Bool checkDestination(Object*, Int x, Int y, PathfindLayerEnum, Int, Bool) { return x != 4 || y != 4; }
    PathfindCell* getCell(PathfindLayerEnum, Int, Int) { return nullptr; }
    void snapClosestGoalPosition(Object*, Coord3D*);
};
__METHOD__
int main() { Pathfinder finder; Object unit; Coord3D position{45, 45, 0}; finder.snapClosestGoalPosition(&unit, &position); std::cout << position.x << ' ' << position.y << '\n'; }
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='path-snap-goal-') as directory:
    filename, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    filename.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(filename), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '35 35\n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
