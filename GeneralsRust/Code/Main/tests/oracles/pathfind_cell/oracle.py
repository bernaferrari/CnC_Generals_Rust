#!/usr/bin/env python3
"""Run the unchanged, source-pinned C++ PathfindCell obstacle admission method."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[6]
SOURCE = ROOT / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
EXPECTED = Path(__file__).with_name('expected.txt')
PIN = '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b'

PREFIX = r'''
#include <iostream>
using Bool = bool;
constexpr int BODY_RUBBLE = 3;
constexpr int KINDOF_CAN_SEE_THROUGH_STRUCTURE = 1;
constexpr int INVALID_ID = 0;
struct ICoord2D { int x = 0, y = 0; };
struct Body { int state; int getDamageState() const { return state; } };
struct Object {
    int id; Body* body; bool transparent;
    Body* getBodyModule() { return body; }
    int getID() const { return id; }
    bool isKindOf(int) const { return transparent; }
};
struct PathfindCell;
struct PathfindCellInfo {
    int m_obstacleID = 0; bool m_obstacleIsFence = false, m_obstacleIsTransparent = false;
    static PathfindCellInfo* getACellInfo(PathfindCell*, const ICoord2D&) { return new PathfindCellInfo; }
};
#define DEBUG_CRASH(x) ((void)0)
struct PathfindCell {
    enum { CELL_CLEAR = 0, CELL_IMPASSABLE = 6, CELL_RUBBLE = 3, CELL_OBSTACLE = 4 };
    int m_type; PathfindCellInfo* m_info;
    void releaseInfo() { delete m_info; m_info = nullptr; }
    Bool setTypeAsObstacle(Object*, Bool, const ICoord2D&);
};
'''
SUFFIX = r'''
int main() {
    ICoord2D pos;
    for (int initial = 0; initial != 7; ++initial) {
        for (int body_mode = 0; body_mode != 3; ++body_mode) {
            PathfindCell cell{initial, nullptr};
            if (initial == PathfindCell::CELL_OBSTACLE) {
                cell.m_info = new PathfindCellInfo;
                cell.m_info->m_obstacleID = 77;
            }
            Body body{body_mode == 2 ? BODY_RUBBLE : 0};
            Object object{11, body_mode == 0 ? nullptr : &body, false};
            bool accepted = cell.setTypeAsObstacle(&object, false, pos);
            int owner = cell.m_info ? cell.m_info->m_obstacleID : 0;
            std::cout << initial << ' ' << body_mode << ' ' << accepted << ' ' << cell.m_type << ' ' << owner << '\n';
            if (cell.m_info) cell.releaseInfo();
        }
    }
}
'''

def original_method(source: str) -> str:
    marker = 'Bool PathfindCell::setTypeAsObstacle('
    if source.count(marker) != 1:
        raise ValueError('original method is missing or ambiguous')
    start = source.index(marker)
    opening = source.index('{', start)
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == '{':
            depth += 1
        elif source[index] == '}':
            depth -= 1
            if depth == 0:
                return source[start:index + 1]
    raise ValueError('original method has no closing brace')

def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument('--write', action='store_true', help='replace expected output with original execution')
    args = parser.parse_args()
    data = SOURCE.read_bytes()
    if hashlib.sha256(data).hexdigest() != PIN:
        raise SystemExit('original AIPathfind.cpp hash changed; review before updating the oracle')
    with tempfile.TemporaryDirectory(prefix='cnc-pathfind-cell-') as directory:
        directory = Path(directory)
        harness = directory / 'original.cpp'
        executable = directory / 'original'
        harness.write_text(PREFIX + original_method(data.decode()) + SUFFIX)
        subprocess.run(['c++', '-std=c++17', str(harness), '-o', str(executable)], check=True)
        output = subprocess.check_output([str(executable)], text=True)
    if args.write:
        EXPECTED.write_text(output)
        print(f'wrote {EXPECTED}')
    elif EXPECTED.read_text() != output:
        raise SystemExit('original execution differs from the checked-in oracle')
    else:
        print('original C++ cell admission matches all 21 oracle rows')

if __name__ == '__main__':
    main()
