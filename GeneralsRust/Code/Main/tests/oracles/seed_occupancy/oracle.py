#!/usr/bin/env python3
"""Execute original checkForMovement occupancy for line-seed inputs."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()
if 'info.layer = parentCell->getLayer();' not in source:
    raise SystemExit('C++ neighbor layer selection changed')
signature = 'Bool Pathfinder::checkForMovement('
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
#include <iostream>
using Int=int; using ObjectID=int; using Bool=bool;
#define INFANTRY_MOVES_THROUGH_INFANTRY
constexpr int INVALID_ID=0, ALLIES=1, ENEMIES=2, KINDOF_INFANTRY=1, TEST_CRUSH_OR_SQUISH=0;
struct Coord { int x=0,y=0; };
struct AI { int ignored=0; int getIgnoredObstacleID() const { return ignored; } };
struct Object {
 int id, relation; bool infantry=false, crush=false, has_ai=true; AI ai;
 int getID() const { return id; }
 const AI *getAIUpdateInterface() const { return has_ai ? &ai : nullptr; }
 bool isKindOf(int) const { return infantry; }
 int getRelationship(const Object *other) const { return other->relation; }
 bool canCrushOrSquish(Object *, int) const { return crush; }
};
struct PathfindCell {
 enum CellFlags { NO_UNITS, UNIT_GOAL, UNIT_GOAL_OTHER_MOVING, UNIT_PRESENT_MOVING, UNIT_PRESENT_FIXED };
 CellFlags flags; ObjectID pos;
 CellFlags getFlags() { return flags; } ObjectID getPosUnit() { return pos; }
};
struct TCheckMovementInfo {
 int allyFixedCount=0; bool allyMoving=false, allyGoal=false, enemyFixed=false;
 Coord cell; int layer=1, radius=0, acceptableSurfaces=1; bool centerInCell=true, considerTransient=false;
};
struct GameLogic { Object *unit=nullptr; Object *findObjectByID(ObjectID id) { return unit && unit->id==id ? unit : nullptr; } };
GameLogic *TheGameLogic;
struct Pathfinder {
 PathfindCell *cell; PathfindCell *deck=nullptr;
 PathfindCell *getCell(int layer,int,int) { return layer == 2 && deck ? deck : cell; }
 Bool checkForMovement(const Object *,TCheckMovementInfo &);
};
__METHOD__
void test(const char *name, PathfindCell::CellFlags flags, int relation, bool crush, bool infantry) {
 Object mover{1,ALLIES}; mover.crush=crush; mover.infantry=infantry;
 Object other{2,relation}; other.infantry=infantry;
 GameLogic logic{&other}; TheGameLogic=&logic; PathfindCell cell{flags,2}; Pathfinder p{&cell};
 TCheckMovementInfo info; bool result=p.checkForMovement(&mover,info);
 std::cout << name << " " << result << " " << info.enemyFixed << " " << info.allyFixedCount << " " << info.allyMoving << "\n";
}
void test_layers() {
 Object mover{1,ALLIES}, other{2,ENEMIES};
 GameLogic logic{&other}; TheGameLogic=&logic;
 PathfindCell occupied{PathfindCell::UNIT_PRESENT_FIXED,2};
 PathfindCell empty{PathfindCell::NO_UNITS,0};
 Pathfinder p{&occupied,&empty}; TCheckMovementInfo info; info.layer=2;
 p.checkForMovement(&mover,info);
 std::cout << "enemy-below-deck " << info.enemyFixed << "\n";
 p.cell=&empty; p.deck=&occupied; info=TCheckMovementInfo{}; info.layer=2;
 p.checkForMovement(&mover,info);
 std::cout << "enemy-on-deck " << info.enemyFixed << "\n";
}
int main() {
 test("moving-enemy",PathfindCell::UNIT_PRESENT_MOVING,ENEMIES,false,false);
 test("fixed-enemy",PathfindCell::UNIT_PRESENT_FIXED,ENEMIES,false,false);
 test("crushable-enemy",PathfindCell::UNIT_PRESENT_FIXED,ENEMIES,true,false);
 test("fixed-ally",PathfindCell::UNIT_PRESENT_FIXED,ALLIES,true,false);
 test("moving-ally",PathfindCell::UNIT_PRESENT_MOVING,ALLIES,false,false);
 test("infantry",PathfindCell::UNIT_PRESENT_FIXED,ENEMIES,false,true);
 test_layers();
}
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='seed-occupancy-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ('moving-enemy 1 0 0 0\nfixed-enemy 1 1 0 0\ncrushable-enemy 1 0 0 0\n'
                'fixed-ally 1 0 1 0\nmoving-ally 1 0 0 1\ninfantry 1 0 0 0\n'
                'enemy-below-deck 0\nenemy-on-deck 1\n')
    if actual != expected:
        raise SystemExit(f'unexpected original result: {actual!r}')
    print(actual, end='')
