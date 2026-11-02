#!/usr/bin/env python3
"""Execute source-pinned C++ path-cell and object-layer lookups.

This isolates getCell fallback; it does not execute the full Pathfinder search.
"""
import hashlib
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[6]
CPP = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
HEADER = ROOT / "GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h"
OBJECT = ROOT / "GeneralsMD/Code/GameEngine/Include/GameLogic/Object.h"
PINS = {
    CPP: "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b",
    HEADER: "beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06",
    OBJECT: "69cb651f57b858dc4f230b1f719a5c4e7695a2a712b97bf1ca3828c260a8b72b",
}


def function(text, signature):
    start = text.index(signature)
    opening = text.index("{", start)
    depth = 0
    for i in range(opening, len(text)):
        depth += (text[i] == "{") - (text[i] == "}")
        if depth == 0:
            return text[start : i + 1]
    raise ValueError(signature)


def main():
    for path, pin in PINS.items():
        if hashlib.sha256(path.read_bytes()).hexdigest() != pin:
            raise SystemExit(f"source pin mismatch: {path}")
    cpp, header, object_header = (p.read_text() for p in (CPP, HEADER, OBJECT))
    source = r'''
#include <cassert>
#include <iostream>
using Int = int;
#define DEBUG_ASSERTCRASH(condition, message) assert(condition)
enum PathfindLayerEnum { LAYER_INVALID = 0, LAYER_GROUND = 1, LAYER_LAST = 15 };
struct Point { int x, y; };
struct Extent { Point lo, hi; };
struct PathfindCell {
    enum CellType { CELL_CLEAR = 0, CELL_BRIDGE_IMPASSABLE = 5, CELL_IMPASSABLE = 6 };
    int m_type = CELL_CLEAR, m_layer = LAYER_GROUND;
__CELL_TYPE__
__CELL_LAYER__
};
struct PathfindLayer {
    PathfindCell **m_layerCells = nullptr;
    Int m_xOrigin = 1, m_yOrigin = 1, m_width = 1, m_height = 1;
    PathfindCell *getCell(Int, Int);
};
struct Object {
    PathfindLayerEnum m_layer = LAYER_GROUND;
__OBJECT_LAYER__
};
struct Pathfinder {
    Extent m_extent{{0,0},{2,2}};
    PathfindLayer m_layers[LAYER_LAST + 1];
    PathfindCell **m_map;
    PathfindCell *getCell(PathfindLayerEnum, Int, Int);
};
__LAYER_GET_CELL__
__PATHFINDER_GET_CELL__
int main() {
    PathfindCell ground[3][3];
    PathfindCell *ground_columns[] = {ground[0], ground[1], ground[2]};
    PathfindCell deck[1]; deck[0].m_layer = 2;
    PathfindCell *deck_columns[] = {deck};
    Pathfinder finder; finder.m_map = ground_columns;
    finder.m_layers[2].m_layerCells = deck_columns;
    auto bridge = static_cast<PathfindLayerEnum>(2);
    auto report = [&](const char *label, int x, int y) {
        PathfindCell *cell = finder.getCell(bridge, x, y);
        std::cout << label << ' ' << cell->getLayer() << ' ' << cell->getType() << '\n';
    };
    report("clear", 1, 1);
    deck[0].m_type = PathfindCell::CELL_IMPASSABLE;
    report("impassable", 1, 1);
    deck[0].m_type = PathfindCell::CELL_BRIDGE_IMPASSABLE;
    report("bridge-impassable", 1, 1);
    report("missing", 0, 1);
    Object ground_object;
    Object bridge_object; bridge_object.m_layer = bridge;
    std::cout << "object-layers " << ground_object.getLayer() << ' ' << bridge_object.getLayer() << '\n';
}
'''
    source = source.replace("__CELL_TYPE__", function(header, "CellType getType( void ) const"))
    cell_header = header[header.index("class PathfindCell\n"):]
    source = source.replace("__CELL_LAYER__", function(cell_header, "PathfindLayerEnum getLayer( void ) const"))
    source = source.replace("__OBJECT_LAYER__", function(object_header, "PathfindLayerEnum getLayer() const"))
    source = source.replace("__LAYER_GET_CELL__", function(cpp, "PathfindCell *PathfindLayer::getCell("))
    source = source.replace("__PATHFINDER_GET_CELL__", function(header, "inline PathfindCell *Pathfinder::getCell( PathfindLayerEnum layer, Int x, Int y )"))
    with tempfile.TemporaryDirectory(prefix="path-cell-layer-oracle-") as directory:
        source_path = Path(directory) / "oracle.cpp"
        executable = Path(directory) / "oracle"
        source_path.write_text(source)
        subprocess.run(["c++", "-std=c++17", str(source_path), "-o", str(executable)], check=True)
        actual = subprocess.check_output([str(executable)], text=True)
    expected = ("clear 2 0\nimpassable 1 0\nbridge-impassable 2 5\n"
                "missing 1 0\nobject-layers 1 2\n")
    if actual != expected:
        raise SystemExit(f"C++ component mismatch:\n{actual}")
    print(actual, end="")


if __name__ == "__main__":
    main()
