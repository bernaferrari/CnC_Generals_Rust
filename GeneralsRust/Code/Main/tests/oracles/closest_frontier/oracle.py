#!/usr/bin/env python3
"""Run original closest-search admission and layer-transition branches."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()

def function(signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError('unclosed function')

neighbors = function('Int Pathfinder::examineNeighboringCells(')
start = neighbors.index('Bool onList = false;', neighbors.index('for( int i=0;'))
end = neighbors.index('if (i>=firstDiagonal)', start)
branch = neighbors[start:end]
program = r'''
#include <iostream>
using Int=int; using Bool=bool;
constexpr Bool false_value=false;
#define FALSE false
#define TRUE true
#define DEBUG_ASSERTCRASH(a,b) ((void)0)
struct ICoord2D { int x, y; };
enum PathfindLayerEnum { LAYER_INVALID=0, LAYER_GROUND=1, LAYER_BRIDGE=2 };
struct PathfindCell {
  bool info=false, open=false, closed=false; int cost=0, total=0; PathfindCell *parent=nullptr;
  PathfindLayerEnum connect=LAYER_INVALID;
  bool hasInfo() { return info; } bool getOpen() { return open; } bool getClosed() { return closed; }
  PathfindLayerEnum getConnectLayer() { return connect; } int getXIndex() { return 5; } int getYIndex() { return 6; }
  bool allocateInfo(ICoord2D) { info=true; return true; }
  void setParentCell(PathfindCell *p) { parent=p; }
  void setCostSoFar(int c) { cost=c; } int getCostSoFar() { return cost; }
  void setTotalCost(int c) { total=c; } int getTotalCost() { return total; }
  PathfindCell *putOnSortedOpenList(PathfindCell *) { open=true; return this; }
};
struct Pathfinder {
  PathfindCell *m_openList=nullptr, *target=nullptr;
  PathfindCell *getCell(PathfindLayerEnum, int, int) { return target; }
  void checkChangeLayers(PathfindCell *parentCell);
};
__CHANGE__
int neighbor_accepted(PathfindCell *newCell) {
  int accepted=0;
  for (int i=0;i<1;i++) {
    __BRANCH__
    accepted++;
  }
  return accepted;
}
int main() {
  PathfindCell parent; parent.connect=LAYER_BRIDGE; parent.cost=17; parent.total=57;
  PathfindCell target; Pathfinder finder; finder.target=&target;
  finder.checkChangeLayers(&parent);
  std::cout << "fresh " << target.cost << " " << target.total << " " << (target.parent==&parent) << "\n";
  target.cost=90; target.total=91; target.parent=nullptr; target.open=true; target.closed=false;
  finder.checkChangeLayers(&parent);
  std::cout << "open " << target.cost << " " << target.total << " " << (target.parent==nullptr) << "\n";
  std::cout << "neighbor-open " << neighbor_accepted(&target) << "\n";
  target.open=false; target.closed=true;
  finder.checkChangeLayers(&parent);
  std::cout << "closed " << target.cost << " " << target.total << " " << (target.parent==nullptr) << "\n";
  std::cout << "neighbor-closed " << neighbor_accepted(&target) << "\n";
  target.closed=false; target.info=false;
  std::cout << "neighbor-fresh " << neighbor_accepted(&target) << "\n";
}
'''.replace('__CHANGE__', function('void Pathfinder::checkChangeLayers(')).replace('__BRANCH__', branch)
with tempfile.TemporaryDirectory(prefix='closest-frontier-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ('fresh 17 57 1\nopen 90 91 1\nneighbor-open 0\n'
                'closed 90 91 1\nneighbor-closed 0\nneighbor-fresh 1\n')
    if actual != expected:
        raise SystemExit(f'unexpected original result: {actual!r}')
    print(actual, end='')
