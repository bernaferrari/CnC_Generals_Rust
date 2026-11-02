#!/usr/bin/env python3
"""Run original C++ checkDestination against isolated cell fixtures."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp = (root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp').read_bytes()
header = (root / 'GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h').read_bytes()
ids = (root / 'GeneralsMD/Code/GameEngine/Include/Common/GameType.h').read_bytes()
for data, expected in ((cpp, '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b'), (header, 'beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06'), (ids, '3679c35576f92874ef4a06b598128c6fc031ec65214eb4685bbfccbf56ccc3de')):
    if hashlib.sha256(data).hexdigest() != expected:
        raise SystemExit('C++ source pin mismatch')

def method(source, signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError(signature)

program = r'''
#include <iostream>
using Bool = bool; using Int = int; using ObjectID = unsigned int; using PathfindLayerEnum = int;
constexpr ObjectID INVALID_ID = 0; constexpr int ALLIES = 1, TEST_CRUSH_OR_SQUISH = 0;
#define DEBUG_ASSERTCRASH(a, b) ((void)0)
struct CellInfo { ObjectID m_obstacleID; };
struct PathfindCell {
    enum CellType { CELL_CLEAR, CELL_WATER, CELL_OBSTACLE, CELL_IMPASSABLE, CELL_BRIDGE_IMPASSABLE };
    enum { NO_UNITS = 0, UNIT_PRESENT_FIXED = 3 };
    CellType type; CellInfo *m_info; ObjectID goal = INVALID_ID; bool aircraft = false; Int flags = NO_UNITS;
    CellType getType() const { return type; }
    Bool isObstaclePresent(ObjectID) const;
    Bool isAircraftGoal() const { return aircraft; }
    ObjectID getGoalAircraft() const { return goal; }
    Int getFlags() const { return flags; }
    ObjectID getGoalUnit() const { return goal; }
};
struct AI { ObjectID ignored; bool aircraft = false; ObjectID getIgnoredObstacleID() const { return ignored; } Bool isAircraftThatAdjustsDestination() const { return aircraft; } };
struct Object { AI ai; ObjectID id; const AI* getAIUpdateInterface() const { return &ai; } const AI* getAI() const { return &ai; } ObjectID getID() const { return id; } Int relation = 0; bool crush = false; Int getRelationship(Object*) const { return relation; } Bool canCrushOrSquish(Object*, Int) const { return crush; } };
struct Logic { Object* target = nullptr; Object* findObjectByID(ObjectID) const { return target; } } logic; Logic *TheGameLogic = &logic;
struct Pathfinder { PathfindCell *cell; PathfindCell* getCell(PathfindLayerEnum, Int, Int) { return cell; } Bool checkDestination(const Object*, Int, Int, PathfindLayerEnum, Int, Bool); };
__CELL__
__IMPASSABLE__
__DESTINATION__
int main() {
    CellInfo matching{41}, other{42}; Object unit{{41}, 100};
    PathfindCell water{PathfindCell::CELL_WATER, nullptr}, accepted{PathfindCell::CELL_OBSTACLE, &matching}, rejected{PathfindCell::CELL_OBSTACLE, &other}, blocked{PathfindCell::CELL_IMPASSABLE, nullptr};
    Pathfinder finder{nullptr};
    for (auto *cell : {&water, &accepted, &rejected, &blocked}) { finder.cell = cell; std::cout << finder.checkDestination(&unit, 0, 0, 1, 0, true) << ' '; }
    unit.ai.aircraft = true; PathfindCell ground{PathfindCell::CELL_CLEAR, nullptr, 99, true}, bridge{PathfindCell::CELL_CLEAR, nullptr};
    finder.cell = &ground; std::cout << finder.checkDestination(&unit, 0, 0, 1, 0, true) << ' ';
    finder.cell = &bridge; std::cout << finder.checkDestination(&unit, 0, 0, 2, 0, true) << ' ';
    finder.cell = &ground; std::cout << finder.checkDestination(&unit, 0, 0, 2, 0, true) << ' ';
    unit.ai.aircraft = false; Object target{{0}, 101}; logic.target = &target; PathfindCell occupied{PathfindCell::CELL_CLEAR, nullptr, 101, false, PathfindCell::UNIT_PRESENT_FIXED}; finder.cell = &occupied;
    unit.relation = ALLIES; std::cout << finder.checkDestination(&unit, 0, 0, 1, 0, true) << ' ';
    unit.relation = 0; std::cout << finder.checkDestination(&unit, 0, 0, 1, 0, true) << ' ';
    unit.crush = true; std::cout << finder.checkDestination(&unit, 0, 0, 1, 0, true) << ' ';
    unit.crush = false; occupied.flags = 1; std::cout << finder.checkDestination(&unit, 0, 0, 1, 0, true) << ' ';
    std::cout << '\n';
}
'''
program = program.replace('__CELL__', method(header.decode(), 'inline Bool PathfindCell::isObstaclePresent('))
program = program.replace('__IMPASSABLE__', method(cpp.decode(), 'static inline Bool IS_IMPASSABLE('))
program = program.replace('__DESTINATION__', method(cpp.decode(), 'Bool Pathfinder::checkDestination('))
with tempfile.TemporaryDirectory(prefix='path-destination-') as directory:
    source, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '1 1 0 0 0 1 0 0 0 1 1 \n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
