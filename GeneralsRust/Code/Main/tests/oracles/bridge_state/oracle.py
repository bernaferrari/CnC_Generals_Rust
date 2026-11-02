#!/usr/bin/env python3
"""Execute original bridge-state transition and zone invalidation methods."""
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
using Bool=bool; using PathfindLayerEnum=int;
struct PathfindLayer {
 bool m_destroyed=false, unused=false;
 bool isUnused() const { return unused; }
 void classifyCells() { std::cout << "classify " << m_destroyed << "\n"; }
 Bool setDestroyed(Bool);
};
struct ZoneManager { void markZonesDirty(bool repaired) { std::cout << "dirty " << repaired << "\n"; } };
struct Pathfinder {
 PathfindLayer m_layers[2]; ZoneManager m_zoneManager;
 void changeBridgeState(PathfindLayerEnum,Bool);
};
__METHODS__
int main() {
 Pathfinder finder;
 finder.changeBridgeState(0,true);
 finder.changeBridgeState(0,false);
 finder.changeBridgeState(0,false);
 finder.changeBridgeState(0,true);
 finder.m_layers[1].unused=true;
 finder.changeBridgeState(1,false);
}
'''.replace('__METHODS__', '\n'.join(method(s) for s in [
    'Bool PathfindLayer::setDestroyed(',
    'void Pathfinder::changeBridgeState(',
]))
with tempfile.TemporaryDirectory(prefix='bridge-state-') as directory:
    source_file, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source_file.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source_file), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = 'classify 1\ndirty 0\nclassify 0\ndirty 1\n'
    if actual != expected:
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
