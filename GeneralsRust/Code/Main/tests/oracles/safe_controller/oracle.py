#!/usr/bin/env python3
"""Execute the original safe-path controller selection, not the full search."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

cpp = Path(__file__).resolve().parents[6] / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(cpp.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = cpp.read_text()
start = source.index('Bool isHuman = true;', source.index('Path *Pathfinder::findSafePath('))
end = source.index('\n\n', start)
block = source[start:end]
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
'''.replace('__BLOCK__', block)
with tempfile.TemporaryDirectory(prefix='safe-controller-') as directory:
    src = Path(directory) / 'oracle.cpp'
    binary = Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
expected = 'human human\ncomputer computer\nmissing-controller human\nmissing-object human\n'
if actual != expected:
    raise SystemExit(f'unexpected result: {actual!r}')
print(actual, end='')
