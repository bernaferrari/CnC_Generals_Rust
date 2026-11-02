#!/usr/bin/env python3
"""Execute the original closest-path goal-on-obstacle decision in isolation."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
source = path.read_bytes()
if hashlib.sha256(source).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
text = source.decode()
function = text.index('Path *Pathfinder::findClosestPath(')
start = text.index('Bool goalOnObstacle = false;', function)
end = text.index('// determine start cell', start)
branch = text[start:end]
program = r'''
#include <iostream>
#include <map>
using Bool=bool; using ObjectID=unsigned int;
constexpr ObjectID INVALID_ID=0; constexpr int KINDOF_FS_AIRFIELD=1;
struct Coord3D { int x; };
struct Object { int layer=1; Coord3D pos{0}; bool airfield=false;
    int getLayer() const { return layer; }
    const Coord3D *getPosition() const { return &pos; }
    bool isKindOf(int) const { return airfield; }
};
struct Cell { ObjectID owner=0; ObjectID getObstacleID() const { return owner; } };
using PathfindCell = Cell;
struct Logic { std::map<ObjectID,Object*> objects;
    Object *findObjectByID(ObjectID id) { auto it=objects.find(id); return it==objects.end()?nullptr:it->second; }
} logic, *TheGameLogic=&logic;
struct Pathfinder { ObjectID m_ignoreObstacleID=0; Cell parked_cell, goal;
    Cell *getClippedCell(int, const Coord3D *) { return &parked_cell; }
    bool decide() { Cell *goalCell=&goal;
        __BRANCH__
        return goalOnObstacle;
    }
};
int main() {
    Object parked, airfield; Pathfinder p;
    logic.objects[41]=&parked; logic.objects[50]=&airfield;
    p.parked_cell.owner=50; p.goal.owner=50; airfield.airfield=true; p.m_ignoreObstacleID=41;
    std::cout << p.decide() << ' ' << p.m_ignoreObstacleID << '\n';
    airfield.airfield=false; p.m_ignoreObstacleID=41;
    std::cout << p.decide() << ' ' << p.m_ignoreObstacleID << '\n';
    p.parked_cell.owner=41; p.goal.owner=41; p.m_ignoreObstacleID=41;
    std::cout << p.decide() << ' ' << p.m_ignoreObstacleID << '\n';
    p.parked_cell.owner=50; p.goal.owner=41; p.m_ignoreObstacleID=41;
    std::cout << p.decide() << ' ' << p.m_ignoreObstacleID << '\n';
    logic.objects.erase(41); p.parked_cell.owner=50; p.goal.owner=50; p.m_ignoreObstacleID=41; airfield.airfield=true;
    std::cout << p.decide() << ' ' << p.m_ignoreObstacleID << '\n';
}
'''.replace('__BRANCH__', branch)
with tempfile.TemporaryDirectory(prefix='closest-goal-obstacle-') as directory:
    source_path, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source_path.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source_path), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = '1 50\n0 41\n1 41\n0 41\n0 41\n'
    if actual != expected:
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
