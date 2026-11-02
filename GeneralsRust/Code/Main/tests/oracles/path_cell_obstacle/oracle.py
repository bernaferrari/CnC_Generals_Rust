#!/usr/bin/env python3
"""Execute the original C++ obstacle cell operations on small fixtures."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
source_file = root / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
source = source_file.read_bytes()
if hashlib.sha256(source).hexdigest() != "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b":
    raise SystemExit("C++ source pin mismatch")
source = source.decode()


def extract(signature):
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == "{") - (source[index] == "}")
        if depth == 0:
            return source[start : index + 1]
    raise ValueError(f"unclosed function: {signature}")


program = r'''
#include <iostream>
using Bool = bool;
constexpr int INVALID_ID = -1, BODY_RUBBLE = 1, KINDOF_CAN_SEE_THROUGH_STRUCTURE = 1;
#define DEBUG_CRASH(x) ((void)0)
struct ICoord2D { int x = 0, y = 0; };
struct Body { int state = 0; int getDamageState() const { return state; } };
struct Object {
    int id; Body *body = nullptr; bool transparent = false;
    Body *getBodyModule() { return body; }
    int getID() const { return id; }
    bool isKindOf(int) const { return transparent; }
};
struct PathfindCell;
struct PathfindCellInfo {
    int m_obstacleID = INVALID_ID;
    bool m_obstacleIsFence = false, m_obstacleIsTransparent = false;
    static PathfindCellInfo *getACellInfo(PathfindCell *, const ICoord2D &) {
        return new PathfindCellInfo;
    }
};
struct PathfindCell {
    enum CellType { CELL_CLEAR, CELL_IMPASSABLE, CELL_OBSTACLE, CELL_RUBBLE };
    CellType m_type = CELL_CLEAR;
    PathfindCellInfo *m_info = nullptr;
    void releaseInfo() { delete m_info; m_info = nullptr; }
    Bool setTypeAsObstacle(Object *, Bool, const ICoord2D &);
    Bool removeObstacle(Object *);
};
__INSERT__
__REMOVE__
int main() {
    ICoord2D pos;
    Object first{41}, other{42};
    PathfindCell cell;
    std::cout << cell.setTypeAsObstacle(&first, false, pos) << ' ';
    std::cout << cell.setTypeAsObstacle(&other, false, pos) << ' ';
    std::cout << cell.removeObstacle(&other) << ' ' << cell.m_info->m_obstacleID << ' ';
    std::cout << cell.removeObstacle(&first) << ' ' << cell.m_type << '\n';
    cell.m_type = PathfindCell::CELL_RUBBLE;
    std::cout << cell.removeObstacle(&other) << ' ' << cell.m_type << '\n';
}
'''.replace("__INSERT__", extract("Bool PathfindCell::setTypeAsObstacle("))
program = program.replace("__REMOVE__", extract("Bool PathfindCell::removeObstacle("))

with tempfile.TemporaryDirectory(prefix="path-cell-obstacle-") as directory:
    cpp = Path(directory) / "oracle.cpp"
    binary = Path(directory) / "oracle"
    cpp.write_text(program)
    subprocess.run(["c++", "-std=c++17", str(cpp), "-o", str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = "1 0 0 41 1 0\n0 0\n"
    if actual != expected:
        raise SystemExit(f"unexpected C++ result: {actual!r}")
    print(actual, end="")
