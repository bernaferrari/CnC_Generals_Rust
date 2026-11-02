#!/usr/bin/env python3
"""Execute the original cell-identity and movement checks."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp = root / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
header = root / "GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h"
ids = root / "GeneralsMD/Code/GameEngine/Include/Common/GameType.h"
cpp_bytes, header_bytes = cpp.read_bytes(), header.read_bytes()
if hashlib.sha256(cpp_bytes).hexdigest() != "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b":
    raise SystemExit("C++ source pin mismatch")
if hashlib.sha256(header_bytes).hexdigest() != "beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06":
    raise SystemExit("C++ header pin mismatch")
if hashlib.sha256(ids.read_bytes()).hexdigest() != "3679c35576f92874ef4a06b598128c6fc031ec65214eb4685bbfccbf56ccc3de":
    raise SystemExit("C++ object ID header pin mismatch")


def method(source, signature):
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == "{") - (source[index] == "}")
        if depth == 0:
            return source[start : index + 1]
    raise ValueError(f"unclosed method: {signature}")


source, declarations = cpp_bytes.decode(), header_bytes.decode()
program = r'''
#include <iostream>
using Bool = bool; using ObjectID = unsigned int; using LocomotorSurfaceTypeMask = int;
constexpr ObjectID INVALID_ID = 0;
constexpr int LOCOMOTORSURFACE_GROUND=1, LOCOMOTORSURFACE_WATER=2, LOCOMOTORSURFACE_CLIFF=4,
              LOCOMOTORSURFACE_AIR=8, LOCOMOTORSURFACE_RUBBLE=16, NO_SURFACES=0;
#define DEBUG_ASSERTCRASH(condition, message) ((void)0)
struct CellInfo { ObjectID m_obstacleID; };
struct PathfindCell {
    enum CellType { CELL_OBSTACLE, CELL_IMPASSABLE, CELL_BRIDGE_IMPASSABLE, CELL_CLEAR, CELL_WATER, CELL_RUBBLE, CELL_CLIFF };
    CellType type; CellInfo *m_info; bool fence = false;
    CellType getType() const { return type; }
    Bool isObstaclePresent(ObjectID) const;
    Bool isObstacleFence() const { return fence; }
};
struct Pathfinder {
    ObjectID m_ignoreObstacleID = INVALID_ID;
    static LocomotorSurfaceTypeMask validLocomotorSurfacesForCellType(PathfindCell::CellType);
    Bool validMovementPosition(Bool, LocomotorSurfaceTypeMask, PathfindCell *, PathfindCell *);
};
__CELL__
__SURFACES__
__MOVEMENT__
int main() {
    CellInfo first{41}, second{42}, invalid{INVALID_ID};
    PathfindCell a{PathfindCell::CELL_OBSTACLE, &first}, b{PathfindCell::CELL_OBSTACLE, &second};
    PathfindCell anonymous{PathfindCell::CELL_OBSTACLE, nullptr}, invalid_owner{PathfindCell::CELL_OBSTACLE, &invalid};
    PathfindCell rubble{PathfindCell::CELL_RUBBLE, &first};
    Pathfinder finder; finder.m_ignoreObstacleID = 41;
    for (auto *cell : {&a, &b, &anonymous, &rubble})
        std::cout << finder.validMovementPosition(false, LOCOMOTORSURFACE_GROUND, cell, nullptr) << ' ';
    finder.m_ignoreObstacleID = INVALID_ID;
    std::cout << finder.validMovementPosition(false, LOCOMOTORSURFACE_GROUND, &invalid_owner, nullptr) << '\n';
}
'''.replace("__CELL__", method(declarations, "inline Bool PathfindCell::isObstaclePresent("))
program = program.replace("__SURFACES__", method(source, "/*static*/ LocomotorSurfaceTypeMask Pathfinder::validLocomotorSurfacesForCellType("))
program = program.replace("__MOVEMENT__", method(source, "Bool Pathfinder::validMovementPosition("))
with tempfile.TemporaryDirectory(prefix="path-ignored-obstacle-") as directory:
    source_file, binary = Path(directory) / "oracle.cpp", Path(directory) / "oracle"
    source_file.write_text(program)
    subprocess.run(["c++", "-std=c++17", str(source_file), "-o", str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != "1 0 0 0 0\n":
        raise SystemExit(f"unexpected original result: {actual!r}")
    print(actual, end="")
