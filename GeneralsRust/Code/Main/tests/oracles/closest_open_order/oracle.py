#!/usr/bin/env python3
"""Execute original C++ sorted-open insertion and removal for equal costs."""
import hashlib
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[6]
path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp'
raw = path.read_bytes()
if hashlib.sha256(raw).hexdigest() != '0e25cf977f60f59c856969adfff1ac551d543501a35acc6a4e3094210ba9485b':
    raise SystemExit('C++ source pin mismatch')
source = raw.decode()
def method(signature):
    start = source.index(signature)
    opening = source.index('{', start)
    depth = 0
    for i in range(opening, len(source)):
        depth += (source[i] == '{') - (source[i] == '}')
        if depth == 0:
            return source[start:i + 1]
    raise ValueError(signature)
program = r'''
#include <iostream>
#define DEBUG_ASSERTCRASH(a,b) ((void)0)
#define FALSE false
struct PathfindCell;
struct Info { PathfindCell *m_cell; Info *m_prevOpen=nullptr, *m_nextOpen=nullptr;
    int m_totalCost=0; bool m_closed=false, m_open=false; };
struct PathfindCell {
    Info *m_info; int label;
    PathfindCell *getNextOpen() { return m_info->m_nextOpen ? m_info->m_nextOpen->m_cell : nullptr; }
    PathfindCell *putOnSortedOpenList(PathfindCell *);
    PathfindCell *removeFromOpenList(PathfindCell *);
};
__INSERT__
__REMOVE__
int main() {
    Info a{nullptr,nullptr,nullptr,10}, b{nullptr,nullptr,nullptr,10}, c{nullptr,nullptr,nullptr,9}, d{nullptr,nullptr,nullptr,10};
    PathfindCell aa{&a,1}, bb{&b,2}, cc{&c,3}, dd{&d,4};
    a.m_cell=&aa; b.m_cell=&bb; c.m_cell=&cc; d.m_cell=&dd;
    PathfindCell *open=nullptr;
    for (auto node : {&aa,&bb,&cc,&dd}) open=node->putOnSortedOpenList(open);
    while (open) { auto node=open; std::cout << node->label << ' '; open=node->removeFromOpenList(open); }
    std::cout << '\n';
}
'''.replace('__INSERT__', method('PathfindCell *PathfindCell::putOnSortedOpenList(')).replace('__REMOVE__', method('PathfindCell *PathfindCell::removeFromOpenList('))
with tempfile.TemporaryDirectory(prefix='closest-open-order-') as directory:
    cpp, binary = Path(directory) / 'oracle.cpp', Path(directory) / 'oracle'
    cpp.write_text(program)
    subprocess.run(['c++', '-std=c++17', str(cpp), '-o', str(binary)], check=True)
    actual = subprocess.check_output([str(binary)], text=True)
    if actual != '3 1 2 4 \n':
        raise SystemExit(f'unexpected C++ result: {actual!r}')
    print(actual, end='')
