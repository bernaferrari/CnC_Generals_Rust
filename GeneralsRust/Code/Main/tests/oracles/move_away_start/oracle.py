#!/usr/bin/env python3
"""Execute source-pinned C++ cell allocation and move-away start indexing.

This is a component oracle for cell-info identity, not a full game/path oracle.
"""
import hashlib
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[6]
CPP = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
HEADER = ROOT / "GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h"
PINS = {
    CPP: "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b",
    HEADER: "beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06",
}


def function(text, signature):
    start = text.index(signature)
    opening = text.index("{", start)
    depth = 0
    for i in range(opening, len(text)):
        depth += (text[i] == "{") - (text[i] == "}")
        if depth == 0:
            return text[start : i + 1]
    raise ValueError(signature)


def main():
    for path, pin in PINS.items():
        if hashlib.sha256(path.read_bytes()).hexdigest() != pin:
            raise SystemExit(f"source pin mismatch: {path}")
    cpp = CPP.read_text()
    header = HEADER.read_text()
    operation = function(cpp, "Path *Pathfinder::getMoveAwayFromPath(")
    start = operation.index("ICoord2D startCellNdx;")
    end = operation.index("worldToCell(&startPos, &startCellNdx);", start)
    start_code = operation[start : end + len("worldToCell(&startPos, &startCellNdx);")]
    source = r'''
#include <cassert>
#include <cmath>
#include <iostream>
using Bool = bool;
using Real = float;
using Int = int;
using UnsignedShort = unsigned short;
#define DEBUG_ASSERTCRASH(condition, message) assert(condition)
#define REAL_TO_INT_FLOOR(value) static_cast<int>(std::floor(value))
constexpr int INVALID_ID = 0;
constexpr int PATHFIND_CELL_SIZE = 10;
constexpr float PATHFIND_CELL_SIZE_F = 10.0f;
struct ICoord2D { int x = 0; int y = 0; };
struct Coord3D { float x; float y; float z; };
struct Geometry { Real radius; Real getBoundingCircleRadius() const { return radius; } };
struct Object {
    Coord3D pos; Geometry geometry;
    const Coord3D *getPosition() const { return &pos; }
    const Geometry &getGeometryInfo() const { return geometry; }
};
struct PathfindCell;
struct PathfindCellInfo {
    static PathfindCellInfo *s_firstFree;
    static PathfindCellInfo *getACellInfo(PathfindCell*, const ICoord2D&);
    bool m_isFree = true;
    PathfindCellInfo *m_pathParent = nullptr, *m_nextOpen = nullptr, *m_prevOpen = nullptr;
    PathfindCell *m_cell = nullptr;
    ICoord2D m_pos;
    int m_costSoFar = 0, m_totalCost = 0, m_open = 0, m_closed = 0;
    int m_obstacleID = 0, m_goalUnitID = 0, m_posUnitID = 0, m_goalAircraftID = 0;
    bool m_obstacleIsFence = false, m_obstacleIsTransparent = false, m_blockedByAlly = false;
};
PathfindCellInfo *PathfindCellInfo::s_firstFree = nullptr;
struct PathfindCell {
    PathfindCellInfo *m_info = nullptr;
    Bool allocateInfo(const ICoord2D&);
__GET_X__
__GET_Y__
};
struct Extent { ICoord2D lo, hi; };
struct Pathfinder {
    Extent m_extent{{0,0}, {39,39}};
    Bool worldToCell(const Coord3D *, ICoord2D *);
    void getRadiusAndCenter(const Object *, Int &, Bool &);
    void run(const char *label, PathfindCell &parentCell, Real radius,
             Coord3D position = {100.0f,100.0f,0.0f}) {
        Object object{position, {radius}}; Object *obj = &object;
        Int cellRadius; Bool centerInCell;
        getRadiusAndCenter(obj, cellRadius, centerInCell);
__START__
        parentCell.allocateInfo(startCellNdx);
        std::cout << label << ' ' << startCellNdx.x << ' ' << startCellNdx.y << ' '
                  << parentCell.getXIndex() << ' ' << parentCell.getYIndex() << '\n';
    }
};
__GET_INFO__
__RADIUS__
__ALLOCATE__
__WORLD_TO_CELL__
int main() {
    PathfindCellInfo pool[4];
    pool[0].m_pathParent = &pool[1]; pool[1].m_pathParent = &pool[2];
    pool[2].m_pathParent = &pool[3];
    PathfindCellInfo::s_firstFree = &pool[0];
    Pathfinder finder;
    PathfindCell fresh;
    finder.run("fresh-even", fresh, 10.0f);
    PathfindCell existing;
    existing.allocateInfo(ICoord2D{10,10});
    finder.run("existing-even", existing, 10.0f);
    PathfindCell odd;
    finder.run("fresh-odd", odd, 2.0f);
    PathfindCell clipped;
    finder.run("clipped-even", clipped, 10.0f, {-25.0f,100.0f,0.0f});
}
'''
    source = source.replace("__GET_X__", function(header, "inline UnsignedShort getXIndex(void) const"))
    source = source.replace("__GET_Y__", function(header, "inline UnsignedShort getYIndex(void) const"))
    source = source.replace("__START__", start_code)
    source = source.replace("__GET_INFO__", function(cpp, "PathfindCellInfo *PathfindCellInfo::getACellInfo("))
    source = source.replace("__RADIUS__", function(cpp, "void Pathfinder::getRadiusAndCenter("))
    source = source.replace("__ALLOCATE__", function(cpp, "Bool PathfindCell::allocateInfo("))
    source = source.replace("__WORLD_TO_CELL__", function(header, "inline Bool Pathfinder::worldToCell("))
    with tempfile.TemporaryDirectory(prefix="move-away-start-oracle-") as directory:
        source_path = Path(directory) / "oracle.cpp"
        executable = Path(directory) / "oracle"
        source_path.write_text(source)
        subprocess.run(["c++", "-std=c++17", str(source_path), "-o", str(executable)], check=True)
        actual = subprocess.check_output([str(executable)], text=True)
    expected = "fresh-even 11 10 11 10\nexisting-even 11 10 10 10\nfresh-odd 10 10 10 10\nclipped-even 0 10 0 10\n"
    if actual != expected:
        raise SystemExit(f"C++ component mismatch:\n{actual}")
    print(actual, end="")


if __name__ == "__main__":
    main()
