#!/usr/bin/env python3
"""Execute original linePassableCallback and obstacle validity decisions."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp = (root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp').read_bytes()
header = (root / 'GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h').read_bytes()
ids = (root / 'GeneralsMD/Code/GameEngine/Include/Common/GameType.h').read_bytes()
for data, expected in ((cpp, '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b'), (header, 'beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06'), (ids, '3679c35576f92874ef4a06b598128c6fc031ec65214eb4685bbfccbf56ccc3de')):
    if hashlib.sha256(data).hexdigest() != expected:
        raise SystemExit('C++ source pin mismatch')

def method(source, signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError(signature)

program = r'''
#include <iostream>
using Int = int; using Bool = bool; using ObjectID = unsigned int; using LocomotorSurfaceTypeMask = int;
constexpr ObjectID INVALID_ID=0; constexpr int LAYER_GROUND=1;
constexpr int LOCOMOTORSURFACE_GROUND=1, LOCOMOTORSURFACE_WATER=2, LOCOMOTORSURFACE_CLIFF=4, LOCOMOTORSURFACE_AIR=8, LOCOMOTORSURFACE_RUBBLE=16, NO_SURFACES=0;
#define DEBUG_ASSERTCRASH(a, b) ((void)0)
struct CellInfo { ObjectID m_obstacleID; }; struct Point { int x, y; };
struct PathfindCell {
    enum CellType { CELL_OBSTACLE, CELL_IMPASSABLE, CELL_BRIDGE_IMPASSABLE, CELL_CLEAR, CELL_WATER, CELL_RUBBLE, CELL_CLIFF };
    CellType type; CellInfo *m_info; int layer = LAYER_GROUND;
    CellType getType() const { return type; } int getLayer() const { return layer; }
    Bool isObstaclePresent(ObjectID) const; Bool isObstacleFence() const { return false; } Bool getPinched() const { return false; }
};
struct Object { int getCrusherLevel() const { return 0; } };
struct TCheckMovementInfo { Point cell; int layer, radius, acceptableSurfaces; bool centerInCell, considerTransient; int allyFixedCount=0; bool enemyFixed=false; };
struct LinePassableStruct { const Object *obj; int acceptableSurfaces, radius; bool centerInCell, blocked, allowPinched; };
struct Pathfinder {
    ObjectID m_ignoreObstacleID = INVALID_ID;
    static LocomotorSurfaceTypeMask validLocomotorSurfacesForCellType(PathfindCell::CellType);
    Bool validMovementPosition(Bool, LocomotorSurfaceTypeMask, PathfindCell*, PathfindCell*);
    Bool checkForMovement(const Object*, TCheckMovementInfo&) { return true; }
    static Int linePassableCallback(Pathfinder*, PathfindCell*, PathfindCell*, Int, Int, void*);
};
__CELL__
__SURFACES__
__MOVEMENT__
__CALLBACK__
int main() {
    CellInfo owner{41}; PathfindCell from{PathfindCell::CELL_CLEAR, nullptr}, to{PathfindCell::CELL_OBSTACLE, &owner};
    Pathfinder p; LinePassableStruct info{nullptr, LOCOMOTORSURFACE_GROUND, 0, true, false, true};
    for (ObjectID id : {41u, 42u, INVALID_ID}) { p.m_ignoreObstacleID=id; std::cout << Pathfinder::linePassableCallback(&p, &from, &to, 1, 0, &info) << ' '; }
    std::cout << '\n';
}
'''
program = program.replace('__CELL__', method(header.decode(), 'inline Bool PathfindCell::isObstaclePresent('))
program = program.replace('__SURFACES__', method(cpp.decode(), '/*static*/ LocomotorSurfaceTypeMask Pathfinder::validLocomotorSurfacesForCellType('))
program = program.replace('__MOVEMENT__', method(cpp.decode(), 'Bool Pathfinder::validMovementPosition('))
program = program.replace('__CALLBACK__', method(cpp.decode(), '/*static*/ Int Pathfinder::linePassableCallback('))
with tempfile.TemporaryDirectory(prefix='path-line-obstacle-') as directory:
    source, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '0 1 1 \n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
