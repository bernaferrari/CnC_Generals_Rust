#!/usr/bin/env python3
"""Execute the original safe-path candidate acceptance block, not the search."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

cpp = Path(__file__).resolve().parents[6] / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
source = cpp.read_text()
if hashlib.sha256(cpp.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
fn = source.index('Path *Pathfinder::findSafePath(')
start = source.index('Bool ok = false;', fn)
end = source.index('\n\t\tif ( ok &&', start)
block = source[start:end]
program = r'''
#include <cassert>
#include <iostream>
using Bool = bool;
using Real = float;
constexpr int MAX_CELLS = 2000;
struct Coord { float x, y; };
bool accept(float x, float y, float p1x, float p1y, float p2x, float p2y,
            float repulsorDistSqr, bool open, int cellCount, float& farthestDistanceSqr) {
    Coord cellCenter{x,y}, first{p1x,p1y}, second{p2x,p2y};
    Coord *repulsorPos1 = &first, *repulsorPos2 = &second;
    void *m_openList = open ? &first : nullptr;
    Real dx = cellCenter.x-repulsorPos1->x;
    Real dy = cellCenter.y-repulsorPos1->y;
__BLOCK__
    return ok;
}
int main() {
    float farthest = 0;
    assert(!accept(5,0,0,0,0,0,100,true,1,farthest));
    farthest = 0;
    assert(accept(5,0,0,0,0,0,100,false,1,farthest));
    farthest = 0;
    assert(accept(5,0,0,0,0,0,100,true,2001,farthest));
    farthest = 36;
    assert(!accept(5,0,0,0,0,0,100,true,2001,farthest));
    farthest = 0;
    assert(accept(11,0,0,0,0,0,100,true,0,farthest));
    std::cout << "inside continue\nexhausted accept\nlimit-farthest accept\nlimit-not-farthest continue\noutside accept\n";
}
'''.replace('__BLOCK__', block)
with tempfile.TemporaryDirectory(prefix='safe-acceptance-') as directory:
    src = Path(directory) / 'oracle.cpp'
    binary = Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
expected = 'inside continue\nexhausted accept\nlimit-farthest accept\nlimit-not-farthest continue\noutside accept\n'
if actual != expected:
    raise SystemExit(f'unexpected result: {actual!r}')
print(actual, end='')
