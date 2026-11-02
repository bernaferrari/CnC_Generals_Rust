#!/usr/bin/env python3
"""Execute C++ updateZonesForModify with a pair of initialized blocks."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
cpp_source = root / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
header = root / "GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h"
source_bytes = cpp_source.read_bytes()
if hashlib.sha256(source_bytes).hexdigest() != "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b":
    raise SystemExit("C++ source pin mismatch")
header_bytes = header.read_bytes()
if hashlib.sha256(header_bytes).hexdigest() != "beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06":
    raise SystemExit("C++ header pin mismatch")
if "enum {UNINITIALIZED_ZONE = 0};" not in header_bytes.decode():
    raise SystemExit("unexpected original uninitialized-zone value")
source = source_bytes.decode()


def extract(signature):
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        depth += (source[index] == "{") - (source[index] == "}")
        if depth == 0:
            return source[start : index + 1]
    raise ValueError(f"unclosed method: {signature}")


method = extract("void PathfindZoneManager::updateZonesForModify(")
clear_passable = extract("void PathfindZoneManager::clearPassableFlags(")
set_passable = extract("void PathfindZoneManager::setAllPassable(")

program = r'''
#include <iostream>
using Int = int;
struct ICoord2D { int x, y; };
struct IRegion2D { ICoord2D lo, hi; };
struct PathfindLayer {};
struct PathfindCell {
    int type = 0, zone = 0;
    int getType() const { return type; }
    int getZone() const { return zone; }
    void setZone(int value) { zone = value; }
};
bool typesMatch(const PathfindCell &a, const PathfindCell &b) { return a.type == b.type; }
struct ZoneBlock {
    bool bridge = true, passable = true;
    void setInteractsWithBridge(bool value) { bridge = value; }
    void setPassable(bool value) { passable = value; }
};
struct PathfindZoneManager {
    enum { ZONE_BLOCK_SIZE = 10, UNINITIALIZED_ZONE = 0 };
    ICoord2D m_zoneBlockExtent{2, 1};
    ZoneBlock *m_zoneBlocks[2];
    unsigned int m_nextFrameToCalculateZones = 123;
    void updateZonesForModify(PathfindCell **, PathfindLayer[], const IRegion2D &, const IRegion2D &);
    void clearPassableFlags();
    void setAllPassable();
};
__METHOD__
__CLEAR_PASSABLE__
__SET_PASSABLE__
int main() {
    PathfindCell storage[20][10]; PathfindCell *map[20];
    for (int x = 0; x < 20; ++x) {
        map[x] = storage[x];
        for (int y = 0; y < 10; ++y) map[x][y].zone = x < 10 ? 1 : 2;
    }
    ZoneBlock blocks[2]; PathfindZoneManager manager;
    manager.m_zoneBlocks[0] = &blocks[0]; manager.m_zoneBlocks[1] = &blocks[1];
    PathfindLayer layers[1]; IRegion2D global{{0,0},{19,9}};
    map[4][4].type = 1; map[4][4].zone = 0;
    map[5][4].type = 1; map[5][4].zone = 0;
    manager.updateZonesForModify(map, layers, {{4,4},{5,4}}, global);
    std::cout << map[4][4].zone << ' ' << map[5][4].zone << ' ' << map[6][4].zone << ' '
              << blocks[0].bridge << ' ' << blocks[1].bridge << '\n';
    map[4][4].type = 0; map[4][4].zone = 0;
    manager.updateZonesForModify(map, layers, {{4,4},{5,4}}, global);
    std::cout << map[4][4].zone << ' ' << map[5][4].zone << '\n';
    map[9][5].zone = 0; map[10][5].zone = 0;
    manager.updateZonesForModify(map, layers, {{9,5},{10,5}}, global);
    std::cout << map[9][5].zone << ' ' << map[10][5].zone << ' '
              << blocks[0].bridge << ' ' << blocks[1].bridge << '\n';
    for (int x = 0; x < 20; ++x) for (int y = 0; y < 10; ++y) {
        map[x][y].type = 1; map[x][y].zone = 3;
    }
    map[4][4].type = 0; map[4][4].zone = 0;
    map[5][4].type = 0; map[5][4].zone = 0;
    map[5][3].type = 0; map[5][3].zone = 1;
    manager.updateZonesForModify(map, layers, {{3,3},{5,5}}, global);
    std::cout << map[4][4].zone << ' ' << map[5][4].zone << '\n';
    map[4][4].zone = 0; map[5][4].type = 1; map[5][4].zone = 3;
    manager.updateZonesForModify(map, layers, {{3,3},{5,5}}, global);
    std::cout << map[4][4].zone << '\n';
    manager.clearPassableFlags();
    std::cout << manager.m_nextFrameToCalculateZones << ' ' << blocks[0].passable << ' ' << blocks[1].passable << '\n';
    manager.setAllPassable();
    std::cout << manager.m_nextFrameToCalculateZones << ' ' << blocks[0].passable << ' ' << blocks[1].passable << '\n';
}
'''.replace("__METHOD__", method).replace("__CLEAR_PASSABLE__", clear_passable).replace("__SET_PASSABLE__", set_passable)
with tempfile.TemporaryDirectory(prefix="path-zone-update-") as directory:
    cpp, binary = Path(directory) / "oracle.cpp", Path(directory) / "oracle"
    cpp.write_text(program)
    subprocess.run(["c++", "-std=c++17", str(cpp), "-o", str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = "0 0 1 0 1\n1 0\n1 2 0 0\n1 1\n0\n123 0 0\n123 1 1\n"
    if actual != expected:
        raise SystemExit(f"unexpected original zones: {actual!r}")
    print(actual, end="")
