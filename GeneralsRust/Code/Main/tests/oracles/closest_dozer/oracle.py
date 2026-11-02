#!/usr/bin/env python3
"""Execute C++ examineNeighboringCells dozer admission in isolation."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
raw = path.read_bytes()
if hashlib.sha256(raw).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = raw.decode()
function = source.index('Int Pathfinder::examineNeighboringCells(')
start = source.index('Bool movementValid = true;', function)
end = source.index('TCheckMovementInfo info;', start)
branch = source[start:end]
program = r'''
#include <iostream>
using Bool=bool; constexpr int KINDOF_DOZER=1, ENEMIES=2;
struct PathfindCell { enum { CELL_CLEAR, CELL_OBSTACLE }; int type=CELL_OBSTACLE, owner=41;
    int getType() const { return type; } int getObstacleID() const { return owner; }
};
struct Object { bool dozer=false; int relation=0;
    bool isKindOf(int) const { return dozer; }
    int getRelationship(Object*) const { return relation; }
};
struct Logic { Object *obstacle=nullptr; Object *findObjectByID(int) { return obstacle; } } logic, *TheGameLogic=&logic;
struct Locomotor { int getValidSurfaces() const { return 1; } };
struct Pathfinder { bool m_isTunneling=false, terrainValid=false;
    bool validMovementPosition(bool, int, PathfindCell*, PathfindCell*) { return terrainValid; }
    int examine(Object *obj) {
        PathfindCell cell, parent; PathfindCell *newCell=&cell, *parentCell=&parent;
        Locomotor locomotorSet; bool isCrusher=false; bool neighborFlags[1]={false};
        for (int i=0;i<1;i++) {
            __BRANCH__
            return 10*movementValid + 2*dozerHack + neighborFlags[i];
        }
        return -1;
    }
};
int main() {
    Object mover{true,0}, obstacle; Pathfinder p; logic.obstacle=&obstacle;
    std::cout << p.examine(&mover) << ' ';
    mover.relation=ENEMIES; std::cout << p.examine(&mover) << ' ';
    mover.relation=0; logic.obstacle=nullptr; std::cout << p.examine(&mover) << ' ';
    logic.obstacle=&obstacle; mover.dozer=false; std::cout << p.examine(&mover) << ' ';
    p.terrainValid=true; std::cout << p.examine(&mover) << ' ';
    p.terrainValid=false; p.m_isTunneling=true; std::cout << p.examine(&mover) << '\n';
}
'''.replace('__BRANCH__', branch)
with tempfile.TemporaryDirectory(prefix='closest-dozer-') as directory:
    cpp, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    cpp.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(cpp), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '12 -1 -1 -1 11 1\n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
