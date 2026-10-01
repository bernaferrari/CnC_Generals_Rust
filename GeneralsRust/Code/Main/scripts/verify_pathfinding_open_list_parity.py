#!/usr/bin/env python3
"""Compare extracted original C++ and production Rust A* open-list behavior.

The C++ harness compiles the original PathfindCell insertion/removal method
bodies verbatim. The Rust harness compiles the production AStarNode/OpenSet
definitions verbatim. Fixtures compare equal-cost FIFO and a cheaper same-cell
requeue (CPP remove/change/reinsert vs Rust fresh generation + stale rejection).
This is queue evidence, not a full Pathfinder search comparison.
"""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
CPP_SOURCE = Path("GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp")
CPP_DRIVER = r"""
#include <iostream>
#include <string>
#define DEBUG_ASSERTCRASH(...) ((void)0)
using Bool = bool;
class PathfindCell;
struct PathfindCellInfo {
    int m_totalCost;
    Bool m_open;
    Bool m_closed;
    PathfindCellInfo *m_prevOpen;
    PathfindCellInfo *m_nextOpen;
    PathfindCell *owner;
};
class PathfindCell {
public:
    PathfindCellInfo info;
    PathfindCellInfo *m_info;
    std::string name;

    PathfindCell(std::string label, int total)
        : info{total, false, false, nullptr, nullptr, this}, m_info(&info), name(label) {}
    PathfindCell *getNextOpen() {
        return m_info->m_nextOpen ? m_info->m_nextOpen->owner : nullptr;
    }
    PathfindCell *putOnSortedOpenList(PathfindCell *list) {
__PUT_BODY__
    }
    PathfindCell *removeFromOpenList(PathfindCell *list) {
__REMOVE_BODY__
    }
};

static void emit(const char *label, PathfindCell *head) {
    std::cout << label;
    for (PathfindCell *cell = head; cell; cell = cell->getNextOpen()) {
        std::cout << ' ' << cell->name;
    }
    std::cout << '\n';
}

int main() {
    PathfindCell a("A", 20), b("B", 20), c("C", 20);
    PathfindCell *list = nullptr;
    list = a.putOnSortedOpenList(list);
    list = b.putOnSortedOpenList(list);
    list = c.putOnSortedOpenList(list);
    emit("equal", list);

    PathfindCell first("A", 20), peer("B", 20), changed("T", 30);
    PathfindCell *reordered = nullptr;
    reordered = first.putOnSortedOpenList(reordered);
    reordered = peer.putOnSortedOpenList(reordered);
    reordered = changed.putOnSortedOpenList(reordered);
    reordered = changed.removeFromOpenList(reordered);
    changed.m_info->m_totalCost = 20;
    reordered = changed.putOnSortedOpenList(reordered);
    emit("reposition", reordered);
}
"""

RUST_DRIVER = r"""
#![allow(dead_code)]
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GridCoord { x: i32, y: i32 }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PathfindLayerEnum { Ground }
type SearchKey = (GridCoord, PathfindLayerEnum);
__QUEUE_SOURCE__

fn node(label: GridCoord, g_score: u32, f_score: u32) -> AStarNode {
    AStarNode { coord: label, layer: PathfindLayerEnum::Ground, g_score,
                f_score, parent: None, enqueue_order: 0 }
}
fn emit(label: &str, nodes: impl IntoIterator<Item = GridCoord>) {
    print!("{label}");
    for coord in nodes {
        let name = match (coord.x, coord.y) {
            (1, 0) => "A", (0, 1) => "B", (2, 0) => "C", (3, 0) => "T",
            _ => panic!("unexpected fixture coordinate: {coord:?}"),
        };
        print!(" {name}");
    }
    println!();
}
fn main() {
    let mut equal = OpenSet::new();
    equal.push(node(GridCoord { x: 1, y: 0 }, 10, 20));
    equal.push(node(GridCoord { x: 0, y: 1 }, 10, 20));
    equal.push(node(GridCoord { x: 2, y: 0 }, 10, 20));
    let mut order = Vec::new();
    while let Some(entry) = equal.pop_live(|_| true) { order.push(entry.coord); }
    emit("equal", order);

    let a = GridCoord { x: 1, y: 0 };
    let b = GridCoord { x: 0, y: 1 };
    let t = GridCoord { x: 3, y: 0 };
    let key = |coord| (coord, PathfindLayerEnum::Ground);
    let mut reposition = OpenSet::new();
    let mut members = HashSet::from([key(a), key(b), key(t)]);
    let costs = HashMap::from([(key(a), 20), (key(b), 20), (key(t), 20)]);
    reposition.push(node(a, 20, 20));
    reposition.push(node(b, 20, 20));
    reposition.push(node(t, 30, 30));
    // C++ removes the old T before inserting its cheaper generation; Rust
    // leaves that heap entry stale and filters it against current g-score.
    reposition.push(node(t, 20, 20));
    let mut order = Vec::new();
    while let Some(entry) = reposition.pop_live(|entry| {
        let k = key(entry.coord);
        members.contains(&k) && costs.get(&k).map(|&best| entry.g_score <= best).unwrap_or(true)
    }) {
        members.remove(&key(entry.coord));
        order.push(entry.coord);
    }
    emit("reposition", order);
}
"""


def extract_body(source: str, signature: str) -> str:
    start = source.index(signature)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1:index]
    raise ValueError(f"unterminated C++ method body: {signature}")


def extract_rust_queue_source(source: str) -> str:
    start_marker = "/// A* node for priority queue"
    end_marker = "/// Pathfinding cell data"
    start = source.index(start_marker)
    end = source.index(end_marker, start)
    return source[start:end]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--rustc", default="rustc")
    args = parser.parse_args()
    repo = args.repo_root.resolve()
    source = (repo / CPP_SOURCE).read_text()
    put_body = extract_body(
        source,
        "PathfindCell *PathfindCell::putOnSortedOpenList( PathfindCell *list )",
    )
    remove_body = extract_body(
        source,
        "PathfindCell *PathfindCell::removeFromOpenList( PathfindCell *list )",
    )
    driver = CPP_DRIVER.replace("__PUT_BODY__", put_body).replace("__REMOVE_BODY__", remove_body)
    rust_source = (repo / "GeneralsRust/Code/GameEngine/Pathfinding/src/lib.rs").read_text()
    rust_driver = RUST_DRIVER.replace("__QUEUE_SOURCE__", extract_rust_queue_source(rust_source))

    with tempfile.TemporaryDirectory(prefix="generals-pathfinding-open-list-") as temp:
        source_path = Path(temp) / "open_list.cpp"
        binary_path = Path(temp) / "open_list"
        rust_path = Path(temp) / "open_set.rs"
        rust_binary = Path(temp) / "open_set"
        source_path.write_text(driver)
        rust_path.write_text(rust_driver)
        subprocess.run(
            [args.cxx, "-std=c++17", "-O0", str(source_path), "-o", str(binary_path)],
            check=True,
            timeout=60,
        )
        output = subprocess.run(
            [str(binary_path)], text=True, capture_output=True, check=True, timeout=10
        ).stdout
        subprocess.run(
            [args.rustc, "--edition=2024", str(rust_path), "-o", str(rust_binary)],
            check=True,
            timeout=60,
        )
        rust_output = subprocess.run(
            [str(rust_binary)], text=True, capture_output=True, check=True, timeout=10
        ).stdout

    expected = "equal A B C\nreposition A B T\n"
    if output != rust_output or output != expected:
        print("FAIL: extracted C++ and production Rust queue event sequences differ")
        print(f"expected: {expected!r}")
        print(f"C++:      {output!r}")
        print(f"Rust:     {rust_output!r}")
        return 1
    print("PASS: extracted CPP methods and production Rust queue emit matching FIFO/reposition events")
    print(output, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
