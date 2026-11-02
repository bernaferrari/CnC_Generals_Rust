//! The immediate, bounded zone repair in `updateZonesForModify`.
use super::{UNINITIALIZED_ZONE, ZONE_BLOCK_SIZE};
use game_pathfinding::{AStarPathfinder, GridCoord};

/// Repair only uninitialized cells inside the original per-block bounds.
pub(crate) fn update_modified_zones(
    zones: &mut [Vec<u16>],
    cells: &AStarPathfinder,
    global: (GridCoord, GridCoord),
    structure: (GridCoord, GridCoord),
    mut clear_bridge: impl FnMut(usize, usize),
) {
    let (global_lo, global_hi) = global;
    let (lo, mut hi) = structure;
    hi.x = hi.x.saturating_add(1).min(global_hi.x);
    hi.y = hi.y.saturating_add(1).min(global_hi.y);
    let blocks_x = (global_hi.x - global_lo.x + ZONE_BLOCK_SIZE) / ZONE_BLOCK_SIZE;
    let blocks_y = (global_hi.y - global_lo.y + ZONE_BLOCK_SIZE) / ZONE_BLOCK_SIZE;

    for block_x in 0..blocks_x {
        for block_y in 0..blocks_y {
            let origin_x = global_lo.x + block_x * ZONE_BLOCK_SIZE;
            let origin_y = global_lo.y + block_y * ZONE_BLOCK_SIZE;
            let x0 = origin_x.max(lo.x);
            let y0 = origin_y.max(lo.y);
            let x1 = (origin_x + ZONE_BLOCK_SIZE - 1).min(hi.x);
            let y1 = (origin_y + ZONE_BLOCK_SIZE - 1).min(hi.y);
            if x0 > x1 || y0 > y1 {
                continue;
            }
            clear_bridge(block_x as usize, block_y as usize);

            for y in y0..=y1 {
                for x in x0..=x1 {
                    let cell = GridCoord::new(x, y);
                    if zone(zones, cell) != UNINITIALIZED_ZONE {
                        continue;
                    }
                    if x > x0 && copy_matching_zone(zones, cells, cell, GridCoord::new(x - 1, y)) {
                        continue;
                    }
                    if y > y0 {
                        if copy_matching_zone(zones, cells, cell, GridCoord::new(x, y - 1)) {
                            continue;
                        }
                        if x < x1
                            && same_type(cells, cell, GridCoord::new(x + 1, y))
                            && copy_matching_zone(zones, cells, cell, GridCoord::new(x + 1, y - 1))
                        {
                            continue;
                        }
                    }
                }
            }

            for y in (y0..=y1).rev() {
                for x in (x0..=x1).rev() {
                    let cell = GridCoord::new(x, y);
                    if zone(zones, cell) != UNINITIALIZED_ZONE {
                        continue;
                    }
                    if x < x1 && copy_matching_zone(zones, cells, cell, GridCoord::new(x + 1, y)) {
                        continue;
                    }
                    if y < y1 {
                        if copy_matching_zone(zones, cells, cell, GridCoord::new(x, y + 1)) {
                            continue;
                        }
                        if x < x1
                            && same_type(cells, cell, GridCoord::new(x + 1, y))
                            && copy_matching_zone(zones, cells, cell, GridCoord::new(x + 1, y + 1))
                        {
                            continue;
                        }
                    }
                }
            }
        }
    }
}

fn zone(zones: &[Vec<u16>], cell: GridCoord) -> u16 {
    zones[cell.x as usize][cell.y as usize]
}

fn same_type(cells: &AStarPathfinder, a: GridCoord, b: GridCoord) -> bool {
    matches!((cells.get_cell_type(a), cells.get_cell_type(b)), (Some(a), Some(b)) if a == b)
}

fn copy_matching_zone(
    zones: &mut [Vec<u16>],
    cells: &AStarPathfinder,
    target: GridCoord,
    source: GridCoord,
) -> bool {
    let source_zone = zone(zones, source);
    if source_zone == UNINITIALIZED_ZONE || !same_type(cells, target, source) {
        return false;
    }
    zones[target.x as usize][target.y as usize] = source_zone;
    true
}
