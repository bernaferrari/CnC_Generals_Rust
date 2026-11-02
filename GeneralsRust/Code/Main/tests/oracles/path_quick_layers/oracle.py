#!/usr/bin/env python3
"""Execute original quick-path decisions against controlled layer cells."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp = (root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp').read_bytes()
if hashlib.sha256(cpp).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = cpp.decode()
start = source.index('Bool Pathfinder::clientSafeQuickDoesPathExist( const LocomotorSet& locomotorSet,')
opening = source.index('{', start)
depth = 0
for index in range(opening, len(source)):
    depth += (source[index] == '{') - (source[index] == '}')
    if depth == 0:
        method = source[start:index + 1]
        break
else:
    raise ValueError('unclosed clientSafeQuickDoesPathExist')
program = r'''
#include <iostream>
using Int = int; using Bool = bool; using PathfindLayerEnum = int;
constexpr bool FALSE = false;
struct Coord3D { float x, y, z; }; struct LocomotorSet { int getValidSurfaces() const { return 1; } };
struct PathfindCell { enum Type { CELL_CLEAR, CELL_WATER, CELL_CLIFF, CELL_OBSTACLE }; Type type; int zone; Type getType() { return type; } int getZone() { return zone; } };
struct PathfindZoneManager { enum { UNINITIALIZED_ZONE = 0 }; int getEffectiveZone(int, bool, int z) { return z; } int getEffectiveTerrainZone(int z) { return z; } };
struct Terrain { int getLayerForDestination(const Coord3D *pos) { return pos->z > 10 ? 2 : 1; } } terrain; Terrain *TheTerrainLogic = &terrain;
struct Pathfinder {
    PathfindZoneManager m_zoneManager; PathfindCell ground{PathfindCell::CELL_WATER, 1}, bridge{PathfindCell::CELL_CLEAR, 2};
    Bool validMovementPosition(bool, int layer, const LocomotorSet&, const Coord3D*) { return layer == 2; }
    PathfindCell *getClippedCell(int layer, const Coord3D*) { return layer == 2 ? &bridge : &ground; }
    Bool clientSafeQuickDoesPathExist(const LocomotorSet&, const Coord3D*, const Coord3D*);
};
__METHOD__
int main() { Pathfinder p; LocomotorSet l; Coord3D deck{45,45,20}, ground{45,45,0}; std::cout << p.clientSafeQuickDoesPathExist(l, &deck, &deck) << ' ' << p.clientSafeQuickDoesPathExist(l, &ground, &deck) << ' ' << p.clientSafeQuickDoesPathExist(l, &deck, &ground) << '\n'; }
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='path-quick-layers-') as directory:
    filename, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    filename.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(filename), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '1 0 0\n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
