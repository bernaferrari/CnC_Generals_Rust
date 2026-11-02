#!/usr/bin/env python3
"""Exercise the original examineCellsCallback with controlled cells."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()
signature = '/*static*/ Int Pathfinder::examineCellsCallback('
start = source.index(signature)
opening = source.index('{', start)
depth = 0
for index in range(opening, len(source)):
    depth += (source[index] == '{') - (source[index] == '}')
    if depth == 0:
        method = source[start:index + 1]
        break
else:
    raise ValueError('unclosed method')
program = r'''
#include <iostream>
using Int=int; using UnsignedInt=unsigned int; using Bool=bool;
#define FALSE false
struct ICoord2D { int x, y; };
struct Region { ICoord2D lo{0,0}, hi{99,99}; };
enum PathfindLayerEnum { LAYER_GROUND=1 };
struct Object { int getCrusherLevel() const { return 0; } };
struct LocomotorSet { int getValidSurfaces() const { return 1; } };
struct TCheckMovementInfo { ICoord2D cell; PathfindLayerEnum layer; bool centerInCell; int radius; bool considerTransient; int acceptableSurfaces; int allyFixedCount=0; bool enemyFixed=false; };
struct PathfindCell {
 enum CellType { CELL_CLEAR, CELL_CLIFF };
 bool info=false, open=false, closed=false, pinched=false; int cost=0, total=0; PathfindCell *parent=nullptr;
 PathfindLayerEnum getLayer() { return LAYER_GROUND; }
 bool hasInfo() { return info; } bool getOpen() { return open; } bool getClosed() { return closed; }
 bool getPinched() { return pinched; } CellType getType() { return CELL_CLEAR; }
 int getCostSoFar() { return cost; } bool allocateInfo(ICoord2D) { info=true; return true; }
 void setBlockedByAlly(bool) {} int costToGoal(PathfindCell *) { return 40; }
 void setCostSoFar(int c) { cost=c; } void setParentCell(PathfindCell *p) { parent=p; } void setTotalCost(int c) { total=c; }
 PathfindCell *removeFromClosedList(PathfindCell *p) { closed=false; return p; }
 PathfindCell *removeFromOpenList(PathfindCell *p) { open=false; return p; }
 PathfindCell *putOnSortedOpenList(PathfindCell *) { open=true; return this; }
};
struct ZoneManager { bool isPassable(int,int) { return true; } };
struct Pathfinder {
 bool m_isTunneling=false; ZoneManager m_zoneManager; Region m_logicalExtent;
 PathfindCell *m_closedList=nullptr, *m_openList=nullptr;
 bool validMovementPosition(bool,int,PathfindCell *,PathfindCell *) { return true; }
 bool checkForMovement(const Object *,TCheckMovementInfo &) { return true; }
 static Int examineCellsCallback(Pathfinder *, PathfindCell *, PathfindCell *, Int, Int, void *);
};
struct ExamineCellsStruct { Pathfinder *thePathfinder; const LocomotorSet *theLoco; Bool centerInCell; Bool isHuman; Int radius; const Object *obj; PathfindCell *goalCell; };
const int COST_ORTHOGONAL=10;
__METHOD__
void test(const char *name, bool open, bool closed, int old_cost, bool pinched) {
 Pathfinder p; LocomotorSet loco; Object o; PathfindCell from, to, goal;
 from.cost=10; to.info=open||closed; to.open=open; to.closed=closed; to.cost=old_cost; to.total=90; to.pinched=pinched;
 ExamineCellsStruct info{&p,&loco,true,true,0,&o,&goal};
 int result=Pathfinder::examineCellsCallback(&p,&from,&to,4,5,&info);
 std::cout << name << " " << result << " " << to.cost << " " << to.total << " " << to.open << " " << to.closed << " " << (to.parent==&from) << "\n";
}
int main() {
 test("fresh",false,false,0,false); test("better-open",true,false,12,false);
 test("worse-open",true,false,50,false); test("worse-closed",false,true,50,false);
 test("pinched",false,false,0,true);
}
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='closest-seed-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ('fresh 0 15 55 1 0 1\nbetter-open 0 12 90 1 0 0\n'
                'worse-open 0 15 55 1 0 1\nworse-closed 0 15 55 1 0 1\n'
                'pinched 1 0 90 0 0 0\n')
    if actual != expected:
        raise SystemExit(f'unexpected original result: {actual!r}')
    print(actual, end='')
