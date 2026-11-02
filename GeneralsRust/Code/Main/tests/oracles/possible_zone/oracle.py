#!/usr/bin/env python3
"""Run C++ zone mapping and possible-destination candidate checks."""
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
using Bool=bool; using Int=int; using zoneStorageType=int; using LocomotorSurfaceTypeMask=int;
using PathfindLayerEnum=int;
constexpr int LOCOMOTORSURFACE_GROUND=1, LOCOMOTORSURFACE_WATER=2, LOCOMOTORSURFACE_CLIFF=4, LOCOMOTORSURFACE_AIR=8, LOCOMOTORSURFACE_RUBBLE=16;
#define DEBUG_CRASH(x) ((void)0)
#define DEBUG_ASSERTCRASH(condition,x) ((void)0)
#define IS_IMPASSABLE(x) ((x) != 0)
struct Coord3D { int x=0,y=0; };
struct LocomotorSet { int surfaces=LOCOMOTORSURFACE_GROUND; int getValidSurfaces() const { return surfaces; } };
struct PathfindCell { int zone, type=0; int getZone() const { return zone; } int getType() const { return type; } };
struct PathfindZoneManager {
 enum { UNINITIALIZED_ZONE=0 };
 int m_maxZone=4;
 int m_hierarchicalZones[5]={0,1,2,3,4}, m_terrainZones[5]={0,1,1,3,4};
 int m_crusherZones[5]={0,1,2,3,4}, m_groundCliffZones[5]={0,1,2,3,4};
 int m_groundWaterZones[5]={0,1,2,3,4}, m_groundRubbleZones[5]={0,1,2,3,4};
 zoneStorageType getEffectiveTerrainZone(zoneStorageType) const;
 zoneStorageType getEffectiveZone(LocomotorSurfaceTypeMask,Bool,zoneStorageType) const;
};
struct ZoneBlock {
 int m_numZones=0, m_firstZone=0;
 int m_crusherZones[5]={0,1,2,3,4}, m_groundCliffZones[5]={0,1,2,3,4};
 int m_groundWaterZones[5]={0,1,2,3,4}, m_groundRubbleZones[5]={0,1,2,3,4};
 zoneStorageType getEffectiveZone(LocomotorSurfaceTypeMask,Bool,zoneStorageType) const;
};
struct Pathfinder {
 PathfindZoneManager m_zoneManager; PathfindCell cell{2};
 PathfindCell *getCell(PathfindLayerEnum,int,int) { return &cell; }
 void adjustCoordToCell(int x,int y,bool,Coord3D &dest,PathfindLayerEnum) { dest.x=x; dest.y=y; }
 Bool checkForPossible(Bool,Int,Bool,const LocomotorSet&,Int,Int,PathfindLayerEnum,Coord3D*,Bool);
};
__METHODS__
int main() {
 Pathfinder finder; ZoneBlock block; LocomotorSet locomotor; Coord3D dest;
 for (int surfaces : {LOCOMOTORSURFACE_AIR, LOCOMOTORSURFACE_GROUND | LOCOMOTORSURFACE_WATER | LOCOMOTORSURFACE_CLIFF, LOCOMOTORSURFACE_GROUND}) {
  std::cout << "zero " << surfaces << " " << finder.m_zoneManager.getEffectiveZone(surfaces,false,0) << "\n";
  std::cout << "block " << surfaces << " " << block.getEffectiveZone(surfaces,false,0) << "\n";
  finder.cell.zone=0; locomotor.surfaces=surfaces; dest={0,0};
  auto accepted=finder.checkForPossible(false,1,true,locomotor,6,7,1,&dest,false);
  std::cout << "candidate " << surfaces << " " << accepted << " " << dest.x << "," << dest.y << "\n";
 }
 finder.cell.zone=2; locomotor.surfaces=LOCOMOTORSURFACE_GROUND;
 for (bool obstacle : {false,true}) {
  dest={0,0};
  auto accepted=finder.checkForPossible(false,1,true,locomotor,6,7,1,&dest,obstacle);
  std::cout << obstacle << " " << accepted << " " << dest.x << "," << dest.y << "\n";
 }
}
'''.replace('__METHODS__', '\n'.join(method(s) for s in [
    'zoneStorageType PathfindZoneManager::getEffectiveTerrainZone(',
    'zoneStorageType PathfindZoneManager::getEffectiveZone(',
    'zoneStorageType ZoneBlock::getEffectiveZone(',
    'Bool Pathfinder::checkForPossible(',
]))
with tempfile.TemporaryDirectory(prefix='possible-zone-') as directory:
    source_file, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source_file.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source_file), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ('zero 8 1\nblock 8 0\ncandidate 8 1 6,7\n'
                'zero 7 1\nblock 7 0\ncandidate 7 1 6,7\n'
                'zero 1 0\nblock 1 0\ncandidate 1 0 0,0\n0 0 0,0\n1 1 6,7\n')
    if actual != expected:
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
