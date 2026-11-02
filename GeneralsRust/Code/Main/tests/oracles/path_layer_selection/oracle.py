#!/usr/bin/env python3
"""Run original Pathfinder and PathfindLayer cell selection."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp = (root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp').read_bytes()
header = (root / 'GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h').read_bytes()
if hashlib.sha256(cpp).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b' or hashlib.sha256(header).hexdigest() != 'beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06':
    raise SystemExit('C++ source pin mismatch')

def method(source, signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError(signature)

program = r'''
#include <iostream>
using Int = int; using PathfindLayerEnum = int;
constexpr int LAYER_GROUND = 1, LAYER_LAST = 15;
#define DEBUG_ASSERTCRASH(a, b) ((void)0)
struct PathfindCell { enum CellType { CELL_CLEAR, CELL_IMPASSABLE }; CellType type; int goal; CellType getType() const { return type; } };
struct PathfindLayer { PathfindCell **m_layerCells = nullptr; int m_xOrigin = 0, m_yOrigin = 0, m_width = 1, m_height = 1; PathfindCell *getCell(Int, Int); };
struct Point { int x, y; }; struct Extent { Point lo, hi; };
struct Pathfinder { Extent m_extent{{0,0},{0,0}}; PathfindLayer m_layers[16]; PathfindCell **m_map; PathfindCell *getCell(PathfindLayerEnum, Int, Int); };
__LAYER__
__CELL__
int main() {
    PathfindCell ground{PathfindCell::CELL_CLEAR, 42}, bridge{PathfindCell::CELL_CLEAR, 0};
    PathfindCell *ground_row = &ground, *bridge_row = &bridge;
    Pathfinder finder; finder.m_map = &ground_row;
    std::cout << finder.getCell(2, 0, 0)->goal << ' ';
    finder.m_layers[2].m_layerCells = &bridge_row;
    std::cout << finder.getCell(2, 0, 0)->goal << ' ';
    bridge.type = PathfindCell::CELL_IMPASSABLE;
    std::cout << finder.getCell(2, 0, 0)->goal << '\n';
}
'''
program = program.replace('__LAYER__', method(cpp.decode(), 'PathfindCell *PathfindLayer::getCell(Int x, Int y)'))
program = program.replace('__CELL__', method(header.decode(), 'inline PathfindCell *Pathfinder::getCell( PathfindLayerEnum layer, Int x, Int y )'))
with tempfile.TemporaryDirectory(prefix='path-layer-selection-') as directory:
    source, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    source.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(source), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '42 0 42\n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
