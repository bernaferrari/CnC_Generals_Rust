#!/usr/bin/env python3
"""Exercise the original closest-path distance pruning branch."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()
start = source.index('if (distSqr > closestDistScreenSqr*4)', source.index('Path *Pathfinder::findClosestPath('))
end = source.index('// If we haven\'t already found the goal cell', start)
branch = source[start:end]
program = r'''
#include <iostream>
using Bool = bool;
constexpr float PATHFIND_CELL_SIZE_F = 10.0f;
bool keep(float distSqr, float closestDistScreenSqr, bool gotHierarchicalPath, int count) {
  for (int i = 0; i < 1; ++i) {
    __BRANCH__
    return true;
  }
  return false;
}
int main() {
  std::cout << keep(4000, 1000, false, 2001) << ' '
            << keep(4001, 1000, false, 0) << ' '
            << keep(4001, 1000, true, 2000) << ' '
            << keep(4001, 1000, true, 2001) << ' '
            << keep(3997, 999, true, 0) << '\n';
}
'''.replace('__BRANCH__', branch)
with tempfile.TemporaryDirectory(prefix='closest-pruning-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '1 0 1 0 0\n':
        raise SystemExit(f'unexpected original result: {actual!r}')
    print(actual, end='')
