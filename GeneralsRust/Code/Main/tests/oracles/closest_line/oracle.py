#!/usr/bin/env python3
"""Execute the original cell-line iterator and retain its exact visitation."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()
signature = 'Int Pathfinder::iterateCellsAlongLine( const ICoord2D &start'
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
#include <cstdlib>
#include <iostream>
using Int=int; using Bool=bool;
struct ICoord2D { int x, y; };
enum PathfindLayerEnum { LAYER_GROUND=1 };
struct PathfindCell { int x, y; };
struct Pathfinder;
using CellAlongLineProc = Int (*)(Pathfinder *, PathfindCell *, PathfindCell *, Int, Int, void *);
struct Pathfinder {
    PathfindCell cells[32][32]{};
    PathfindCell *getCell(PathfindLayerEnum, Int x, Int y) {
        if (x<0 || y<0 || x>=32 || y>=32) return nullptr;
        cells[x][y]={x,y}; return &cells[x][y];
    }
    Int iterateCellsAlongLine(const ICoord2D &, const ICoord2D &, PathfindLayerEnum, CellAlongLineProc, void *);
};
__METHOD__
Int record(Pathfinder *, PathfindCell *from, PathfindCell *to, Int, Int, void *) {
    if (from) std::cout << " " << from->x << "," << from->y << ">" << to->x << "," << to->y;
    return 0;
}
void test(const char *name, ICoord2D from, ICoord2D to) {
    Pathfinder p; std::cout << name; p.iterateCellsAlongLine(from,to,LAYER_GROUND,record,nullptr); std::cout << "\n";
}
int main() {
    test("shallow", {2,2}, {6,4});
    test("steep", {2,2}, {4,6});
    test("diagonal", {2,2}, {4,4});
    test("reverse", {6,4}, {2,2});
    test("horizontal", {2,2}, {5,2});
    test("same", {2,2}, {2,2});
}
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='closest-line-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ("shallow 2,2>2,3 2,3>3,3 3,3>4,3 4,3>4,4 4,4>5,4 5,4>6,4 6,4>6,5\n"
                "steep 2,2>3,2 3,2>3,3 3,3>3,4 3,4>4,4 4,4>4,5 4,5>4,6 4,6>5,6\n"
                "diagonal 2,2>2,3 2,3>3,3 3,3>3,4 3,4>4,4 4,4>4,5\n"
                "reverse 6,4>6,3 6,3>5,3 5,3>4,3 4,3>4,2 4,2>3,2 3,2>2,2 2,2>2,1\n"
                "horizontal 2,2>3,2 3,2>4,2 4,2>5,2\n"
                "same 2,2>2,3\n")
    if actual != expected:
        raise SystemExit(f'unexpected original result: {actual!r}')
    print(actual, end='')
