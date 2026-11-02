#!/usr/bin/env python3
"""Execute the original append decision with instrumented state-machine calls."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

cpp = Path(__file__).resolve().parents[6] / 'GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp'
if hashlib.sha256(cpp.read_bytes()).hexdigest() != '6ed6ea940b05acd368b4fd490733335007e44bfb4d281a89d228941aa50dba2c':
    raise SystemExit('C++ source pin mismatch')
source = cpp.read_text()
start = source.index('void AIUpdateInterface::privateFollowPathAppend(')
end = source.index('\n}\n', start) + 2
program = r'''
#include <cassert>
#include <cstddef>
#include <iostream>
#include <vector>
using Bool = bool;
using CommandSourceType = int;
struct Coord3D { int x; };
constexpr int AI_FOLLOW_PATH = 1;
struct Machine {
    std::vector<Coord3D> goals;
    std::size_t getGoalPathSize() const { return goals.size(); }
    void addToGoalPath(const Coord3D *p) { goals.push_back(*p); }
};
struct AIUpdateInterface {
    bool moving = false, waiting = false, called = false;
    int state = 0;
    Coord3D goal{7};
    Machine machine;
    std::vector<Coord3D> replacement;
    bool isMoving() const { return moving; }
    bool isWaitingForPath() const { return waiting; }
    int getAIStateType() const { return state; }
    Machine *getStateMachine() { return &machine; }
    const Coord3D *getGoalPosition() const { return &goal; }
    void privateFollowPath(const std::vector<Coord3D> *p, void *, int, bool) {
        called = true; replacement = *p;
    }
    void privateFollowPathAppend(const Coord3D *, CommandSourceType);
};
__ORIGINAL__
int main() {
    const Coord3D next{9};
    AIUpdateInterface follow;
    follow.state = AI_FOLLOW_PATH; follow.moving = true; follow.machine.goals.push_back({5});
    follow.privateFollowPathAppend(&next, 0);
    assert(!follow.called && follow.machine.goals.size() == 2 && follow.machine.goals[1].x == 9);
    AIUpdateInterface other;
    other.waiting = true;
    other.privateFollowPathAppend(&next, 0);
    assert(other.called && other.replacement.size() == 2 && other.replacement[0].x == 7 && other.replacement[1].x == 9);
    AIUpdateInterface idle;
    idle.privateFollowPathAppend(&next, 0);
    assert(idle.called && idle.replacement.size() == 1 && idle.replacement[0].x == 9);
    std::cout << "active follow appends goal\nwaiting move preserves goal\nidle starts new goal\n";
}
'''.replace('__ORIGINAL__', source[start:end])
with tempfile.TemporaryDirectory(prefix='follow-path-append-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    print(subprocess.check_output([str(binary)], text=True), end='')
