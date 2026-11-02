#!/usr/bin/env python3
"""Execute the original safe and closest controller selection, not full searches."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

cpp = Path(__file__).resolve().parents[6] / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(cpp.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = cpp.read_text()
program = r'''
#include <cassert>
#include <iostream>
using Bool = bool;
enum PlayerType { PLAYER_HUMAN, PLAYER_COMPUTER };
struct Player { PlayerType type; PlayerType getPlayerType() const { return type; } };
struct Object { Player *player; Player *getControllingPlayer() const { return player; } };
bool decide(Object *obj) {
__BLOCK__
    return isHuman;
}
int main() {
    Player human{PLAYER_HUMAN}, computer{PLAYER_COMPUTER};
    Object h{&human}, c{&computer}, none{nullptr};
    assert(decide(&h)); assert(!decide(&c)); assert(decide(&none)); assert(decide(nullptr));
    std::cout << "human human\ncomputer computer\nmissing-controller human\nmissing-object human\n";
}
'''
with tempfile.TemporaryDirectory(prefix='safe-controller-') as directory:
    src = Path(directory) / 'oracle.cpp'
    binary = Path(directory) / 'oracle'
    expected = 'human human\ncomputer computer\nmissing-controller human\nmissing-object human\n'
    for method in ('findSafePath', 'findClosestPath'):
        start = source.index('Bool isHuman = true;', source.index(f'Path *Pathfinder::{method}('))
        end = source.index('\n\n', start)
        src.write_text(program.replace('__BLOCK__', source[start:end]))
        subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
        actual = subprocess.check_output([str(binary)], text=True)
        if actual != expected:
            raise SystemExit(f'{method}: unexpected result: {actual!r}')
        print(f'{method}: {actual.replace(chr(10), ", ").rstrip(", ")}')
