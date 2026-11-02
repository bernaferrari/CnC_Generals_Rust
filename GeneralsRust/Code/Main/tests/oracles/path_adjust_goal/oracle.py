#!/usr/bin/env python3
"""Execute original adjustDestination with controlled candidate decisions."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp = (root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp').read_bytes()
if hashlib.sha256(cpp).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = cpp.decode()
start = source.index('Bool Pathfinder::adjustDestination(Object *obj, const LocomotorSet& locomotorSet, Coord3D *dest, const Coord3D *groupDest)')
opening = source.index('{', start)
depth = 0
for index in range(opening, len(source)):
    depth += (source[index] == '{') - (source[index] == '}')
    if depth == 0:
        method = source[start:index + 1]
        break
else:
    raise ValueError('unclosed adjustDestination')
program = r'''
#include <iostream>
using Int = int; using Bool = bool; using PathfindLayerEnum = int;
constexpr int KINDOF_PROJECTILE = 1, PLAYER_COMPUTER = 2, LAYER_GROUND = 1;
constexpr float PATHFIND_CELL_SIZE_F = 10;
struct Coord3D { float x, y, z; }; struct ICoord2D { int x, y; }; struct LocomotorSet {};
struct Player { int getPlayerType() const { return 0; } };
struct Object { Bool isKindOf(int) const { return false; } Player *getControllingPlayer() { return nullptr; } };
struct Terrain { int getLayerForDestination(const Coord3D*) { return LAYER_GROUND; } } terrain; Terrain *TheTerrainLogic = &terrain;
struct Pathfinder {
    void getRadiusAndCenter(Object*, Int& radius, Bool& center) { radius = 0; center = true; }
    void worldToCell(Coord3D* pos, ICoord2D* cell) { cell->x = int(pos->x / 10); cell->y = int(pos->y / 10); }
    Bool checkForAdjust(Object*, const LocomotorSet&, Bool, Int x, Int y, PathfindLayerEnum, Int, Bool, Coord3D* dest, const Coord3D*) {
        if (x == 4 && y == 4) return false;
        dest->x = (x + .5f) * 10; dest->y = (y + .5f) * 10; return true;
    }
    Bool adjustDestination(Object*, const LocomotorSet&, Coord3D*, const Coord3D*);
};
__METHOD__
int main() { Pathfinder finder; Object unit; LocomotorSet locomotors; Coord3D position{45, 45, 0}; std::cout << finder.adjustDestination(&unit, locomotors, &position, nullptr) << ' ' << position.x << ' ' << position.y << '\n'; }
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='path-adjust-goal-') as directory:
    filename, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    filename.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(filename), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '1 55 45\n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
