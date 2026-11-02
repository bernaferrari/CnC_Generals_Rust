#!/usr/bin/env python3
"""Execute original terrain-surface and crusher movement checks."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()

def method(signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for i in range(opening, len(source)):
        depth += (source[i] == '{') - (source[i] == '}')
        if depth == 0:
            return source[start:i + 1]
    raise ValueError('unclosed method')

program = r'''
#include <iostream>
using Bool=bool; using LocomotorSurfaceTypeMask=int;
constexpr int LOCOMOTORSURFACE_GROUND=1, LOCOMOTORSURFACE_WATER=2, LOCOMOTORSURFACE_CLIFF=4, LOCOMOTORSURFACE_AIR=8, LOCOMOTORSURFACE_RUBBLE=16, NO_SURFACES=0;
struct PathfindCell {
 enum CellType { CELL_OBSTACLE, CELL_IMPASSABLE, CELL_BRIDGE_IMPASSABLE, CELL_CLEAR, CELL_WATER, CELL_RUBBLE, CELL_CLIFF };
 CellType type; bool fence=false, ignored=false;
 bool isObstaclePresent(int) const { return ignored; }
 bool isObstacleFence() const { return fence; }
 CellType getType() const { return type; }
};
struct Pathfinder {
 int m_ignoreObstacleID=42;
 static LocomotorSurfaceTypeMask validLocomotorSurfacesForCellType(PathfindCell::CellType);
 Bool validMovementPosition(Bool,LocomotorSurfaceTypeMask,PathfindCell *,PathfindCell *);
};
__METHODS__
int main() {
 Pathfinder finder; PathfindCell rubble{PathfindCell::CELL_RUBBLE};
 PathfindCell fence{PathfindCell::CELL_OBSTACLE,true};
 PathfindCell solid{PathfindCell::CELL_OBSTACLE};
 for (bool crusher : {false,true}) {
  std::cout << "ground-rubble " << crusher << " " << finder.validMovementPosition(crusher,LOCOMOTORSURFACE_GROUND,&rubble,nullptr) << "\n";
 }
 std::cout << "rubble-surface " << finder.validMovementPosition(true,LOCOMOTORSURFACE_RUBBLE,&rubble,nullptr) << "\n";
 std::cout << "air-rubble " << finder.validMovementPosition(false,LOCOMOTORSURFACE_AIR,&rubble,nullptr) << "\n";
 std::cout << "fence-crusher " << finder.validMovementPosition(true,LOCOMOTORSURFACE_GROUND,&fence,nullptr) << "\n";
 std::cout << "fence-noncrusher " << finder.validMovementPosition(false,LOCOMOTORSURFACE_GROUND,&fence,nullptr) << "\n";
 std::cout << "solid-crusher " << finder.validMovementPosition(true,LOCOMOTORSURFACE_GROUND,&solid,nullptr) << "\n";
}
'''.replace('__METHODS__', '\n'.join(method(s) for s in [
    '/*static*/ LocomotorSurfaceTypeMask Pathfinder::validLocomotorSurfacesForCellType(',
    'Bool Pathfinder::validMovementPosition(',
]))
with tempfile.TemporaryDirectory(prefix='valid-movement-') as directory:
    source_file, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source_file.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source_file), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ('ground-rubble 0 0\nground-rubble 1 0\nrubble-surface 1\nair-rubble 1\n'
                'fence-crusher 1\nfence-noncrusher 0\nsolid-crusher 0\n')
    if actual != expected:
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
