#!/usr/bin/env python3
"""Execute the original C++ path-segment iteration, not the full search."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

CPP = Path(__file__).resolve().parents[6] / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
PIN = "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b"
text = CPP.read_text()
if hashlib.sha256(CPP.read_bytes()).hexdigest() != PIN:
    raise SystemExit("C++ source pin mismatch")
start = text.index("for( node = pathToAvoid->getFirstNode();", text.index("Path *Pathfinder::getMoveAwayFromPath("))
opening = text.index("{", start)
depth = 0
for i in range(opening, len(text)):
    depth += (text[i] == "{") - (text[i] == "}")
    if depth == 0:
        loop = text[start:i + 1]
        break
else:
    raise SystemExit("original loop not found")
source = r'''
#include <cassert>
#include <iostream>
struct Coord2D { float x, y; };
struct Point { float x, y; };
struct Region2D {};
struct PathNode {
    Point pos{};
    PathNode *next = nullptr;
    PathNode *getNextOptimized() { return next; }
    Point *getPosition() { return &pos; }
};
struct Path { PathNode *first = nullptr; PathNode *getFirstNode() { return first; } };
int checks = 0;
bool LineInRegion(Coord2D *, Coord2D *, Region2D *) { ++checks; return true; }
bool overlap_for(Path *pathToAvoid) {
    PathNode *node;
    Region2D bounds;
    bool overlap = false;
__LOOP__
    return overlap;
}
int main() {
    PathNode first, second;
    Path path;
    assert(!overlap_for(&path) && checks == 0);
    path.first = &first;
    assert(!overlap_for(&path) && checks == 0);
    first.next = &second;
    assert(overlap_for(&path) && checks == 1);
    std::cout << "empty no-overlap\nsingle no-overlap\ntwo-nodes overlap\n";
}
'''.replace("__LOOP__", loop)
with tempfile.TemporaryDirectory(prefix="move-away-short-") as directory:
    source_path = Path(directory) / "oracle.cpp"
    binary = Path(directory) / "oracle"
    source_path.write_text(source)
    subprocess.run(["c++", "-std=c++17", str(source_path), "-o", str(binary)], check=True)
    result = subprocess.check_output([str(binary)], text=True)
expected = "empty no-overlap\nsingle no-overlap\ntwo-nodes overlap\n"
if result != expected:
    raise SystemExit(f"unexpected C++ result: {result}")
print(result, end="")
