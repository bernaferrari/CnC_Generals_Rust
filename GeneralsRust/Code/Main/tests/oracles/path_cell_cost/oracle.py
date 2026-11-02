#!/usr/bin/env python3
"""Execute original PathfindCell::costSoFar for terrain and turn cases."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()
signature = 'UnsignedInt PathfindCell::costSoFar('
start = source.index(signature)
opening = source.index('{', start)
depth = 0
for i in range(opening, len(source)):
    depth += (source[i] == '{') - (source[i] == '}')
    if depth == 0:
        method = source[start:i + 1]
        break
else:
    raise ValueError('unclosed method')
program = r'''
#include <iostream>
using Int=int; using UnsignedInt=unsigned int;
constexpr int COST_ORTHOGONAL=10, COST_DIAGONAL=14;
#define DEBUG_ASSERTCRASH(condition,x) ((void)0)
struct ICoord2D { int x=0,y=0; };
struct CellInfo { ICoord2D m_pos; };
struct PathfindCell {
 CellInfo *m_info; unsigned int cost=0; bool pinched=false; PathfindCell *parent=nullptr;
 int type=0;
 int getXIndex() const { return m_info->m_pos.x; }
 int getYIndex() const { return m_info->m_pos.y; }
 unsigned int getCostSoFar() const { return cost; }
 bool getPinched() const { return pinched; }
 PathfindCell *getParentCell() const { return parent; }
 UnsignedInt costSoFar(PathfindCell *);
};
__METHOD__
int main() {
 CellInfo start_info{{1,1}}, orth_info{{2,1}}, diag_info{{2,2}}, prev_info{{0,1}}, turn_info{{1,2}};
 PathfindCell previous{&prev_info}, start{&start_info,100}, orth{&orth_info}, diagonal{&diag_info}, turn{&turn_info};
 for (int type=0; type<3; ++type) {
  orth.type=type; diagonal.type=type;
  std::cout << "terrain " << type << " " << orth.costSoFar(&start) << " " << diagonal.costSoFar(&start) << "\n";
 }
 orth.pinched=true; diagonal.pinched=true;
 std::cout << "pinched " << orth.costSoFar(&start) << " " << diagonal.costSoFar(&start) << "\n";
 start.parent=&previous;
 std::cout << "turn " << turn.costSoFar(&start) << "\n";
}
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='path-cell-cost-') as directory:
    source_file, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source_file.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source_file), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ('terrain 0 110 114\nterrain 1 110 114\nterrain 2 110 114\n'
                'pinched 124 128\nturn 118\n')
    if actual != expected:
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
