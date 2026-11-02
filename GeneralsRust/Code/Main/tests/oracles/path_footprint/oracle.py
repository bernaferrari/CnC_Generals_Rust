#!/usr/bin/env python3
"""Execute the original box and circle footprint rasters from AIPathfind.cpp."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
source_path = root / "GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp"
source = source_path.read_bytes()
if hashlib.sha256(source).hexdigest() != "0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b":
    raise SystemExit("C++ source pin mismatch")
source = source.decode()
method = source.index("void Pathfinder::internal_classifyObjectFootprint(")
start = source.index("Real angle = obj->getOrientation();", method)
end = source.index("\n\t\t}\n\t\tbreak;", start)
box_raster = source[start:end]
start = source.index("ICoord2D topLeft, bottomRight;", end)
end = source.index("\n\t\t} // cylinder", start)
circle_raster = source[start:end]

program = r'''
#include <cmath>
#include <iostream>
#include <set>
#include <utility>
using Real = float; using Int = int;
constexpr Real PATHFIND_CELL_SIZE_F = 10.0f;
#define Cos(x) std::cos(x)
#define Sin(x) std::sin(x)
#define REAL_TO_INT_CEIL(x) static_cast<int>(std::ceil(x))
#define REAL_TO_INT_FLOOR(x) static_cast<int>(std::floor(x))
struct ICoord2D { int x = 0, y = 0; };
struct IRegion2D { ICoord2D lo, hi; };
struct Coord2D { float x = 0, y = 0; };
struct Coord3D { float x, y, z; };
struct Geometry { float getMajorRadius() const { return 10; } float getMinorRadius() const { return 10; } };
struct Object { float getOrientation() const { return 0; } Geometry getGeometryInfo() const { return {}; } };
struct PathfindZoneManager { enum { UNINITIALIZED_ZONE = 0 }; };
std::set<std::pair<int, int>> cells;
struct Cell {
    bool setTypeAsObstacle(Object *, bool, const ICoord2D &p) { cells.emplace(p.x, p.y); return true; }
    bool removeObstacle(Object *) { return false; }
    void setZone(int) {}
};
int main() {
    Object object; Object *obj = &object; Coord3D position{55, 55, 0}; Coord3D *pos = &position;
    IRegion2D cellBounds{{5, 5}, {5, 5}}, m_extent{{0, 0}, {10, 10}};
    Cell m_map[10][10]; bool insert = true;
    __RASTER__
    std::cout << "box\n";
    for (auto [x, y] : cells) std::cout << x << ',' << y << '\n';
    cells.clear();
    __CIRCLE__
    std::cout << "circle\n";
    for (auto [x, y] : cells) std::cout << x << ',' << y << '\n';
}
'''.replace("__RASTER__", box_raster).replace("__CIRCLE__", circle_raster)
with tempfile.TemporaryDirectory(prefix="path-box-footprint-") as directory:
    cpp, binary = Path(directory) / "oracle.cpp", Path(directory) / "oracle"
    cpp.write_text(program)
    subprocess.run(["c++", "-std=c++17", str(cpp), "-o", str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    expected = "box\n4,5\n4,6\n5,5\n5,6\n6,5\n6,6\ncircle\n4,5\n5,4\n5,5\n5,6\n6,5\n"
    if actual != expected:
        raise SystemExit(f"unexpected C++ raster: {actual!r}")
    print(actual, end="")
