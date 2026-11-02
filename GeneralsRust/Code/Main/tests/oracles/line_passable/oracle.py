#!/usr/bin/env python3
"""Run the original C++ line callback with independently controlled cells."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(path.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = path.read_text()
signature = '/*static*/ Int Pathfinder::linePassableCallback('
start = source.index(signature)
opening = source.index('{', start)
depth = 0
for i in range(opening, len(source)):
    depth += (source[i] == '{') - (source[i] == '}')
    if depth == 0:
        method = source[start:i + 1]
        break
else:
    raise ValueError('unclosed method')
program = r'''
#include <iostream>
using Bool=bool; using Int=int;
constexpr int LAYER_GROUND=1;
struct CellCoord { int x=0,y=0; };
struct Object { int getCrusherLevel() const { return 0; } };
struct PathfindCell {
 enum { CELL_CLEAR=0 }; int layer=1, type=0; bool pinched=false;
 int getLayer() const { return layer; } int getType() const { return type; }
 bool getPinched() const { return pinched; }
};
struct TCheckMovementInfo {
 CellCoord cell; int layer=0, radius=0, acceptableSurfaces=1, allyFixedCount=0;
 bool centerInCell=true, considerTransient=false, enemyFixed=false;
};
struct LinePassableStruct {
 Object *obj=nullptr; bool centerInCell=true, blocked=false, allowPinched=false;
 int radius=0, acceptableSurfaces=1;
};
struct Pathfinder {
 int seen=0, validations=0;
 bool checkForMovement(Object *, TCheckMovementInfo &info) { seen=info.layer; return true; }
 bool validMovementPosition(bool,int,PathfindCell *,PathfindCell *) { validations++; return true; }
 static Int linePassableCallback(Pathfinder *,PathfindCell *,PathfindCell *,Int,Int,void *);
};
__METHOD__
void run(const char *label, PathfindCell &from, PathfindCell &to) {
 Pathfinder finder; LinePassableStruct info;
 auto result=Pathfinder::linePassableCallback(&finder,&from,&to,6,7,&info);
 std::cout << label << " " << result << " " << finder.seen << " " << finder.validations << "\n";
}
int main() {
 PathfindCell deck{2,0,false}, deck_pinched{2,0,true}, ground_pinched{1,0,true}, ground{1,0,false};
 run("deck",deck,deck);
 run("deck-pinched",deck,deck_pinched);
 run("ground-fallback-pinched",deck,ground_pinched);
 run("ground",deck,ground);
}
'''.replace('__METHOD__', method)
with tempfile.TemporaryDirectory(prefix='line-passable-') as directory:
    source_file, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source_file.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source_file), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = ('deck 0 2 0\ndeck-pinched 1 2 0\nground-fallback-pinched 1 1 0\n'
                'ground 0 1 1\n')
    if actual != expected:
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
