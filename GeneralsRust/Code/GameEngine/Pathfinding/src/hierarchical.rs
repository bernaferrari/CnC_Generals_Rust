// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// The hierarchical block path finder: coarse 10x10 block A* over zone blocks
// (C++ internal_findHierarchicalPath) plus the Pathfinder::findPath
// clearPassableFlags / corridor-marking dance.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::astar::AStarPathfinder;
use crate::cell::{GridCoord, PathfindLayerEnum, SURFACE_AIR};

impl AStarPathfinder {
    /// C++ `Pathfinder::findPath` hierarchical passable dance (AIPathfind.cpp:6375-6381).
    ///
    /// `clearPassableFlags` → coarse `ZONE_BLOCK_SIZE` search (plus bridge
    /// start/end jumps) → mark corridor + start box, or `setAllPassable`.
    pub fn apply_hierarchical_zone_prune(
        &mut self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        bridge_jumps: &[(GridCoord, GridCoord)],
    ) -> bool {
        if (surfaces & SURFACE_AIR) != 0 {
            self.clear_zone_passable_flags();
            return true;
        }
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            self.clear_zone_passable_flags();
            return false;
        }
        self.mark_all_zone_blocks_impassable();
        if let Some(path) =
            self.find_hierarchical_block_path(start, goal, surfaces, is_crusher, bridge_jumps)
        {
            for c in path {
                self.set_zone_passable(c, true);
            }
            let half = Self::ZONE_BLOCK_SIZE;
            for dx in -half..=half {
                for dy in -half..=half {
                    let c = GridCoord::new(start.x + dx, start.y + dy);
                    if self.in_bounds(c) {
                        self.set_zone_passable(c, true);
                    }
                }
            }
            true
        } else {
            self.clear_zone_passable_flags();
            false
        }
    }

    #[inline]
    fn zone_block_index(coord: GridCoord) -> (i32, i32) {
        (
            coord.x.div_euclid(Self::ZONE_BLOCK_SIZE),
            coord.y.div_euclid(Self::ZONE_BLOCK_SIZE),
        )
    }

    fn collect_connect_layer_jumps(&self) -> Vec<(GridCoord, GridCoord)> {
        let mut by_layer: HashMap<u8, Vec<GridCoord>> = HashMap::new();
        for x in 0..self.width as i32 {
            for y in 0..self.height as i32 {
                let c = GridCoord::new(x, y);
                if let Some(cl) = self.get_cell_connect_layer(c) {
                    if cl != PathfindLayerEnum::Invalid && cl != PathfindLayerEnum::Ground {
                        by_layer.entry(cl as u8).or_default().push(c);
                    }
                }
            }
        }
        let mut pairs = Vec::new();
        for cells in by_layer.values() {
            if cells.len() < 2 {
                continue;
            }
            let mut lo = cells[0];
            let mut hi = cells[0];
            for &c in cells {
                if c.x + c.y < lo.x + lo.y {
                    lo = c;
                }
                if c.x + c.y > hi.x + hi.y {
                    hi = c;
                }
            }
            if lo != hi {
                pairs.push((lo, hi));
            }
        }
        pairs
    }

    fn zone_blocks_share_passable_edge(
        &self,
        a: (i32, i32),
        b: (i32, i32),
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        let size = Self::ZONE_BLOCK_SIZE;
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        if dx.abs() + dy.abs() != 1 {
            return false;
        }
        let w = self.width as i32;
        let h = self.height as i32;
        if dx != 0 {
            let x0 = if dx > 0 {
                a.0 * size + size - 1
            } else {
                a.0 * size
            };
            let x1 = x0 + dx;
            let y0 = (a.1 * size).max(0);
            let y1 = ((a.1 + 1) * size).min(h);
            if x0 < 0 || x1 < 0 || x0 >= w || x1 >= w {
                return false;
            }
            for y in y0..y1 {
                if self.is_passable(GridCoord::new(x0, y), surfaces, is_crusher)
                    && self.is_passable(GridCoord::new(x1, y), surfaces, is_crusher)
                {
                    return true;
                }
            }
        } else {
            let y0 = if dy > 0 {
                a.1 * size + size - 1
            } else {
                a.1 * size
            };
            let y1 = y0 + dy;
            let x0 = (a.0 * size).max(0);
            let x1 = ((a.0 + 1) * size).min(w);
            if y0 < 0 || y1 < 0 || y0 >= h || y1 >= h {
                return false;
            }
            for x in x0..x1 {
                if self.is_passable(GridCoord::new(x, y0), surfaces, is_crusher)
                    && self.is_passable(GridCoord::new(x, y1), surfaces, is_crusher)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Coarse 10×10 block A* (C++ `internal_findHierarchicalPath`).
    fn find_hierarchical_block_path(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        bridge_jumps: &[(GridCoord, GridCoord)],
    ) -> Option<Vec<GridCoord>> {
        let start_b = Self::zone_block_index(start);
        let goal_b = Self::zone_block_index(goal);
        if start_b == goal_b {
            return Some(vec![start, goal]);
        }
        let bx_max = (self.width as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        let by_max = (self.height as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        let mut jumps = self.collect_connect_layer_jumps();
        jumps.extend_from_slice(bridge_jumps);

        let hier_h = |b: (i32, i32)| (b.0 - goal_b.0).abs() + (b.1 - goal_b.1).abs();
        let mut open: BinaryHeap<Reverse<(i32, i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push(Reverse((hier_h(start_b), 0, start_b.0, start_b.1)));
        g_score.insert(start_b, 0);

        let mut reached = false;
        while let Some(Reverse((_f, g, bx, by))) = open.pop() {
            if !closed.insert((bx, by)) {
                continue;
            }
            if (bx, by) == goal_b {
                reached = true;
                break;
            }
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = bx + dx;
                let ny = by + dy;
                if nx < 0 || ny < 0 || nx >= bx_max || ny >= by_max {
                    continue;
                }
                if closed.contains(&(nx, ny)) {
                    continue;
                }
                if !self.zone_blocks_share_passable_edge((bx, by), (nx, ny), surfaces, is_crusher) {
                    continue;
                }
                let ng = g + 1;
                if g_score.get(&(nx, ny)).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert((nx, ny), ng);
                came_from.insert((nx, ny), (bx, by));
                open.push(Reverse((ng + hier_h((nx, ny)), ng, nx, ny)));
            }
            for &(near, far) in &jumps {
                let ends = [(near, far), (far, near)];
                for (from, to) in ends {
                    if Self::zone_block_index(from) != (bx, by) {
                        continue;
                    }
                    let dest_b = Self::zone_block_index(to);
                    if dest_b.0 < 0
                        || dest_b.1 < 0
                        || dest_b.0 >= bx_max
                        || dest_b.1 >= by_max
                        || closed.contains(&dest_b)
                    {
                        continue;
                    }
                    let ng = g + 1;
                    if g_score.get(&dest_b).is_some_and(|&og| ng >= og) {
                        continue;
                    }
                    g_score.insert(dest_b, ng);
                    came_from.insert(dest_b, (bx, by));
                    open.push(Reverse((ng + hier_h(dest_b), ng, dest_b.0, dest_b.1)));
                }
            }
        }
        if !reached {
            return None;
        }
        let mut blocks = vec![goal_b];
        let mut cur = goal_b;
        while cur != start_b {
            cur = *came_from.get(&cur)?;
            blocks.push(cur);
        }
        blocks.reverse();
        let mut cells: Vec<GridCoord> = blocks
            .iter()
            .map(|&(bx, by)| GridCoord::new(bx * Self::ZONE_BLOCK_SIZE, by * Self::ZONE_BLOCK_SIZE))
            .collect();
        cells.push(start);
        cells.push(goal);
        Some(cells)
    }
}
