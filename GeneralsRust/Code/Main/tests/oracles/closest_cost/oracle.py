#!/usr/bin/env python3
"""Execute the original C++ path-cell heuristic and incremental cost."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
source_path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(source_path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = source_path.read_text()

def function(signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError('unclosed function')

program = r'''
#include <iostream>
using Int = int;
using UnsignedInt = unsigned int;
struct ICoord2D { int x, y; };
struct PathfindCell;
struct Info { ICoord2D m_pos; PathfindCell *parent; unsigned int cost; };
struct PathfindCell {
    Info *m_info; bool pinched;
    int getXIndex() { return m_info->m_pos.x; }
    int getYIndex() { return m_info->m_pos.y; }
    unsigned int getCostSoFar() { return m_info->cost; }
    PathfindCell *getParentCell() { return m_info->parent; }
    bool getPinched() { return pinched; }
    UnsignedInt costToGoal(PathfindCell *goal);
    UnsignedInt costSoFar(PathfindCell *parent);
};
#define DEBUG_ASSERTCRASH(a,b) ((void)0)
const Int COST_ORTHOGONAL = 10;
const Int COST_DIAGONAL = 14;
__HEURISTIC__
__STEP__
int main() {
    Info goalinfo{{3,2}, nullptr, 0}; PathfindCell goal{&goalinfo, false};
    Info origininfo{{0,0}, nullptr, 0}; PathfindCell origin{&origininfo, false};
    std::cout << "heuristic " << origin.costToGoal(&goal) << "\n";
    Info parentinfo{{1,0}, &origin, 10}; PathfindCell parent{&parentinfo, false};
    Info straightinfo{{2,0}, nullptr, 0}; PathfindCell straight{&straightinfo, false};
    Info diaginfo{{2,1}, nullptr, 0}; PathfindCell diag{&diaginfo, false};
    Info rightinfo{{1,1}, nullptr, 0}; PathfindCell right{&rightinfo, false};
    Info backinfo{{0,1}, nullptr, 0}; PathfindCell back{&backinfo, false};
    std::cout << "straight " << straight.costSoFar(&parent) << "\n";
    std::cout << "turn45 " << diag.costSoFar(&parent) << "\n";
    std::cout << "turn90 " << right.costSoFar(&parent) << "\n";
    std::cout << "turn135 " << back.costSoFar(&parent) << "\n";
    straight.pinched = true;
    std::cout << "pinched " << straight.costSoFar(&parent) << "\n";
}
'''.replace('__HEURISTIC__', function('UnsignedInt PathfindCell::costToGoal(')).replace('__STEP__', function('UnsignedInt PathfindCell::costSoFar('))
with tempfile.TemporaryDirectory(prefix='closest-cost-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = 'heuristic 40\nstraight 20\nturn45 28\nturn90 28\nturn135 40\npinched 34\n'
    if actual != expected:
        raise SystemExit(f'unexpected original result: {actual!r}')
    print(actual, end='')
