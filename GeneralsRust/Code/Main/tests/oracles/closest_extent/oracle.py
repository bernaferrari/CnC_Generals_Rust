#!/usr/bin/env python3
"""Execute original C++ neighbor-extent filtering, not the full search."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

source_path = Path(__file__).resolve().parents[6] / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
source = source_path.read_text()
if hashlib.sha256(source_path.read_bytes()).hexdigest() != "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b":
    raise SystemExit("C++ source pin mismatch")
start = source.index("if (isHuman) {", source.index("Pathfinder::examineNeighboringCells("))
opening = source.index("{", start)
depth = 0
for i in range(opening, len(source)):
    depth += (source[i] == "{") - (source[i] == "}")
    if depth == 0:
        original = source[start:i + 1]
        break
else:
    raise SystemExit("extent check not found")
program = r'''
#include <iostream>
struct Coord { int x, y; };
struct Extent { Coord lo, hi; } m_logicalExtent{{0, 0}, {9, 19}};
int main() {
    Coord neighbors[] = {{9, 5}, {10, 5}, {-1, 5}};
    for (bool isHuman : {true, false}) {
        for (Coord newCellCoord : neighbors) {
__CHECK__
            std::cout << (isHuman ? "human " : "computer ") << newCellCoord.x << '\n';
        }
    }
}
'''.replace("__CHECK__", original)
with tempfile.TemporaryDirectory(prefix="closest-extent-") as directory:
    cpp = Path(directory) / "oracle.cpp"
    binary = Path(directory) / "oracle"
    cpp.write_text(program)
    subprocess.run(["c++", "-std=c++17", str(cpp), "-o", str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
expected = "human 9\ncomputer 9\ncomputer 10\ncomputer -1\n"
if actual != expected:
    raise SystemExit(f"original extent check returned {actual!r}")
print(actual, end="")
