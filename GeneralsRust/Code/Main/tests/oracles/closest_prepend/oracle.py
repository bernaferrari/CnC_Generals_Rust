#!/usr/bin/env python3
"""Run original adjustCoordToCell/prependCells with a minimal fake path."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

cpp = Path(__file__).resolve().parents[6] / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
if hashlib.sha256(cpp.read_bytes()).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = cpp.read_text()

def function(signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == '{') - (source[index] == '}')
        if depth == 0:
            return source[start:index + 1]
    raise ValueError('unclosed C++ function')

program = r'''
#include <cassert>
#include <cstddef>
#include <iostream>
#include <vector>
using Int = int; using Real = float; using Bool = bool;
using PathfindLayerEnum = int;
constexpr int LAYER_GROUND = 0;
constexpr float PATHFIND_CELL_SIZE_F = 10.0f;
#define DEBUG_ASSERTCRASH(...) ((void)0)
struct Coord3D { float x = 0, y = 0, z = 0; };
struct Terrain { float getLayerHeight(float, float, int) { return 0; } };
static Terrain terrain, *TheTerrainLogic = &terrain;
struct PathfindCell {
    enum { CELL_CLIFF = 1 };
    int x, y, layer = 0, type = 0;
    PathfindCell *parent = nullptr;
    bool pinched = false;
    bool getPinched() { return pinched; }
    PathfindCell *getParentCell() { return parent; }
    int getXIndex() { return x; } int getYIndex() { return y; }
    int getLayer() { return layer; } int getType() { return type; }
    bool isBlockedByAlly() { return false; }
    void clearParentCell() { parent = nullptr; }
};
struct Node {
    Coord3D pos; int layer;
    void setLayer(int value) { layer = value; }
    void setCanOptimize(bool) {}
    const Coord3D *getPosition() { return &pos; }
};
struct Path {
    std::vector<Node> nodes;
    Node *getFirstNode() { return nodes.empty() ? nullptr : &nodes.front(); }
    void prependNode(const Coord3D *p, int layer) { nodes.insert(nodes.begin(), Node{*p, layer}); }
    void setBlockedByAlly(bool) {}
};
struct Zone { void setPassable(int, int, bool) {} };
struct Pathfinder {
    Zone m_zoneManager;
    void adjustCoordToCell(Int, Int, Bool, Coord3D &, PathfindLayerEnum);
    void prependCells(Path *, const Coord3D *, PathfindCell *, Bool);
};
__ADJUST__
__PREPEND__
PathfindCell *select_goal(PathfindCell *goalCell) {
    __PINCHED__
    return goalCell;
}
int main() {
    Pathfinder finder;
    PathfindCell cell{2, 5};
    Coord3D from{21, 52, 0};
    Path path;
    finder.prependCells(&path, &from, &cell, true);
    assert(path.nodes.size() == 2);
    assert(path.nodes[0].pos.x == 21 && path.nodes[0].pos.y == 52);
    assert(path.nodes[1].pos.x == 25 && path.nodes[1].pos.y == 55);
    Coord3D centered{25, 55, 0};
    Path centered_path;
    finder.prependCells(&centered_path, &centered, &cell, true);
    assert(centered_path.nodes.size() == 1);
    Path even_path;
    finder.prependCells(&even_path, &from, &cell, false);
    assert(even_path.nodes.size() == 2);
    assert(even_path.nodes[0].pos.x == 21 && even_path.nodes[0].pos.y == 52);
    assert(even_path.nodes[1].pos.x == 20.5f && even_path.nodes[1].pos.y == 50.5f);
    PathfindCell root{2, 5}, middle{3, 5}, last{4, 5};
    middle.parent = &root; last.parent = &middle;
    Path chain;
    finder.prependCells(&chain, &from, &last, true);
    assert(chain.nodes.size() == 3);
    assert(chain.nodes[0].pos.x == 21 && chain.nodes[0].pos.y == 52);
    assert(chain.nodes[1].pos.x == 35 && chain.nodes[1].pos.y == 55);
    assert(chain.nodes[2].pos.x == 45 && chain.nodes[2].pos.y == 55);
    PathfindCell open{2, 5}, squeezed{3, 5};
    squeezed.parent = &open; squeezed.pinched = true;
    assert(select_goal(&squeezed) == &open);
    open.pinched = true;
    assert(select_goal(&squeezed) == &squeezed);
    std::cout << "uncentered: start then cell center\ncentered: one node\neven footprint: cell inset\nchain: original start and adjusted cells\npinched goal: back off only to open parent\n";
}
'''.replace('__ADJUST__', function('void Pathfinder::adjustCoordToCell(')).replace('__PREPEND__', function('void Pathfinder::prependCells(')).replace('__PINCHED__', source[source.index('if (goalCell->getPinched() && goalCell->getParentCell() && !goalCell->getParentCell()->getPinched())', source.index('Path *Pathfinder::buildActualPath(')):].split('}', 1)[0] + '}')
with tempfile.TemporaryDirectory(prefix='closest-prepend-') as directory:
    src, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    src.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(src), '-o', str(binary)], check=True)
    print(subprocess.check_output([str(binary)], text=True), end='')
