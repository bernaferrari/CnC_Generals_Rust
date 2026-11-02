#!/usr/bin/env python3
"""Execute C++ closest-search terrain and occupancy surcharge branches."""
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
start = source.index('if (info.allyMoving && dx<10 && dy<10)', function)
end = source.index('Int costRemaining = 0;', start)
surcharges = source[start:end]
start_obstacle = source.index('if (newCell->getType()==PathfindCell::CELL_OBSTACLE)', end)
open_brace = source.index('{', start_obstacle)
depth = 0
for index in range(open_brace, len(source)):
    depth += (source[index] == '{') - (source[index] == '}')
    if depth == 0:
        obstacle = source[start_obstacle:index + 1]
        break
else:
    raise SystemExit('unclosed obstacle branch')
program = r'''
#include <cmath>
#include <iostream>
using Int=int; using Real=float;
constexpr int COST_ORTHOGONAL=10, COST_DIAGONAL=14, NO_ATTACK=0;
constexpr float PATHFIND_CELL_SIZE_F=10.0f;
struct Coord3D { float x,y,z; };
struct PathfindCell {
    enum { CELL_CLEAR, CELL_CLIFF, CELL_OBSTACLE };
    int type=CELL_CLEAR; bool pinched=false, blocked=false; int x=1,y=0;
    int getType() const { return type; } bool getPinched() const { return pinched; }
    int getXIndex() const { return x; } int getYIndex() const { return y; }
    void setBlockedByAlly(bool b) { blocked=b; }
};
struct Terrain { float nextHeight=0; float getGroundHeight(float x, float) const { return x==0 ? 0 : nextHeight; } } terrain, *TheTerrainLogic=&terrain;
struct Info { bool allyMoving=false; int allyFixedCount=0; };
int run(PathfindCell &cell, Info info, float height) {
    PathfindCell parent; parent.x=0; PathfindCell *newCell=&cell, *parentCell=&parent;
    struct { int x,y; } newCellCoord{1,0};
    int dx=1,dy=0,newCostSoFar=10,attackDistance=NO_ATTACK; bool canPathThroughUnits=false;
    terrain.nextHeight=height;
    __SURCHARGES__
    __OBSTACLE__
    return newCostSoFar;
}
int main() {
    PathfindCell cell; Info moving{true,1};
    std::cout << run(cell,moving,0) << ' ' << cell.blocked << '\n';
    cell.pinched=true; std::cout << run(cell,{},0) << '\n';
    cell.pinched=false; cell.type=PathfindCell::CELL_CLIFF;
    std::cout << run(cell,{},5) << ' ' << run(cell,{},10) << '\n';
    cell.type=PathfindCell::CELL_OBSTACLE; std::cout << run(cell,{},0) << '\n';
}
'''.replace('__SURCHARGES__', surcharges).replace('__OBSTACLE__', obstacle)
with tempfile.TemporaryDirectory(prefix='closest-surcharges-') as directory:
    cpp, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    cpp.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(cpp), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '94 1\n20\n108 10\n1010\n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
