#!/usr/bin/env python3
"""Execute original AIUpdateInterface::loadPostProcess position/goal policy."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '6ed6ea940b05acd368b4fd490733335007e44bfb4d281a89d228941aa50dba2c':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()
start = source.index('void AIUpdateInterface::loadPostProcess(')
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
#define FALSE false
using LocomotorSetType = int;
constexpr int LOCOMOTORSET_INVALID = -1;
constexpr float PATHFIND_CELL_SIZE_F = 10.0f;
struct Coord3D { float x=0, y=0, z=0; };
struct ICoord2D { int x, y; };
struct Object { Coord3D position{75,55,0}; Coord3D *getPosition() { return &position; } int getLayer() { return 1; } };
struct Pathfinder {
 int goals=0, positions=0; Coord3D last;
 void updateGoal(Object *, Coord3D *pos, int) { ++goals; last=*pos; }
 void updatePos(Object *, Coord3D *) { ++positions; }
};
struct AI { Pathfinder *pf; Pathfinder *pathfinder() { return pf; } };
AI *TheAI;
struct UpdateModule { void loadPostProcess() {} };
struct AIUpdateInterface : UpdateModule {
 bool m_fixLocoInPostProcess=false, moving=false; int m_curLocomotorSet=LOCOMOTORSET_INVALID;
 ICoord2D m_pathfindGoalCell{3,4}, m_pathfindCurCell{7,5}; Object object;
 bool isMoving() { return moving; } Object *getObject() { return &object; }
 void chooseLocomotorSet(int) {}
 void loadPostProcess();
};
__METHOD__
void test(const char *name, bool moving, bool has_goal) {
 Pathfinder pf; AI ai{&pf}; TheAI=&ai; AIUpdateInterface unit;
 unit.moving=moving; if (!has_goal) unit.m_pathfindGoalCell={-1,-1};
 unit.loadPostProcess();
 std::cout << name << " " << pf.goals << " " << pf.positions << " "
           << unit.m_pathfindCurCell.x << "," << unit.m_pathfindCurCell.y << " "
           << pf.last.x << "," << pf.last.y << "\n";
}
int main() { test("idle",false,true); test("moving",true,true); test("moving-no-goal",true,false); }
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='ai-load-position-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = 'idle 1 1 -1,-1 75,55\nmoving 1 0 7,5 35,45\nmoving-no-goal 0 0 7,5 0,0\n'
    if actual != expected:
        raise SystemExit(f'unexpected original result: {actual!r}')
    print(actual, end='')
