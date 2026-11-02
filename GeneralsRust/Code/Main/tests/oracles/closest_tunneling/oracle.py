#!/usr/bin/env python3
"""Execute original tunneling decisions; this is not a full pathfinder oracle."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

cpp = Path(__file__).resolve().parents[6] / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(cpp.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = cpp.read_text()

def block(needle, offset=0):
    start = source.index(needle, offset)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError('unclosed C++ block')

closest = source.index('Path *Pathfinder::findClosestPath(')
initial = block('if (validMovementPosition( isCrusher, locomotorSet.getValidSurfaces(), parentCell ) == false)', closest)
neighbor = block('if (!movementValid && !m_isTunneling)')
exit_tunnel = block('if (movementValid && !newCell->getPinched())')
goal = block('if (parentCell == goalCell)', closest)
program = r'''
#include <cassert>
#include <iostream>
using Bool = bool;
struct Cell {
    bool pinched = false;
    bool getPinched() const { return pinched; }
    int getXIndex() const { return 0; }
    int getYIndex() const { return 0; }
    int getLayer() const { return 0; }
    void releaseInfo() {}
};
struct Path {};
struct GlobalData { bool m_debugAI = false; };
static GlobalData globalData;
static GlobalData *TheGlobalData = &globalData;
struct LocomotorSet { int getValidSurfaces() const { return 2; } };
static bool validMovementPosition(bool, int, Cell *) { return false; }
struct Finder {
    bool m_isTunneling = false;
    Path result;
    bool checkDestination(void *, int, int, int, int, bool) { return true; }
    void debugShowSearch(bool) {}
    Path *buildActualPath(void *, int, void *, Cell *, bool, bool) { return &result; }
    void cleanOpenAndClosedLists() {}
    Path *accept_goal_while_tunneling() {
        Cell goal;
        Cell *parentCell = &goal, *goalCell = &goal, *closesetCell = nullptr;
        bool goalOnObstacle = false, canPathThroughUnits = false, foundGoal = false;
        bool centerInCell = true, blocked = false;
        int radius = 0;
        void *obj = nullptr, *from = nullptr;
        LocomotorSet locomotorSet;
        m_isTunneling = true;
        for (int i = 0; i < 1; ++i) {
            __GOAL__
        }
        return nullptr;
    }
    bool run(bool valid_start, bool valid_neighbor, bool pinched) {
        bool isCrusher = false;
        LocomotorSet locomotorSet;
        Cell parent, next{pinched};
        Cell *parentCell = &parent, *newCell = &next;
        if (!valid_start) {
            __INITIAL__
        }
        for (int i = 0; i < 1; ++i) {
            bool movementValid = valid_neighbor;
            __NEIGHBOR__
            __EXIT__
            return true;
        }
        return false;
    }
};
int main() {
    Finder invalid_start;
    assert(invalid_start.run(false, false, false));
    assert(invalid_start.m_isTunneling);
    Finder valid_start;
    assert(!valid_start.run(true, false, false));
    Finder escape;
    assert(escape.run(false, true, false));
    assert(!escape.m_isTunneling);
    Finder pinched;
    assert(pinched.run(false, true, true));
    assert(pinched.m_isTunneling);
    Finder reached_goal;
    assert(reached_goal.accept_goal_while_tunneling() != nullptr);
    assert(!reached_goal.m_isTunneling);
    std::cout << "invalid start tunnels\nvalid start rejects invalid cell\nvalid exit ends tunneling\npinched exit retains tunneling\ngoal is accepted while tunneling\n";
}
'''.replace('__INITIAL__', initial).replace('__NEIGHBOR__', neighbor).replace('__EXIT__', exit_tunnel).replace('__GOAL__', goal)
with tempfile.TemporaryDirectory(prefix='closest-tunnel-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    print(subprocess.check_output([str(binary)], text=True), end='')
