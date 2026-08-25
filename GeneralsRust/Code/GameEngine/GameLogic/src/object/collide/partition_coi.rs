//! C++ `PartitionData` COI stamping (`doSmallFill` / `doCircleFill` / `doRectFill`).
//!
//! `PartitionData::updateCellsTouched` (`PartitionManager.cpp:1997`) puts an object
//! into every overlapping partition cell, not just the cell that contains its center.

use super::collision_geometry::{GeometryInfo, GeometryType};
use super::partition_manager::{CellCoord, PARTITION_CELL_SIZE};

/// C++ `PartitionManager::worldToCellDist` — `ceil(world / cellSize)`.
#[inline]
pub fn world_to_cell_dist(world: f32) -> i32 {
    (world / PARTITION_CELL_SIZE).ceil() as i32
}

/// C++ `PartitionManager::worldToCell` without map-origin offset (crate cells
/// are origin-at-zero, matching `CellCoord::from_world_pos`).
#[inline]
pub fn world_to_cell(wx: f32, wy: f32) -> CellCoord {
    CellCoord {
        x: (wx / PARTITION_CELL_SIZE).floor() as i32,
        y: (wy / PARTITION_CELL_SIZE).floor() as i32,
    }
}

/// C++ `PartitionData::updateCellsTouched` cell list for one object.
pub fn cells_touched_for_geometry(
    center_x: f32,
    center_y: f32,
    geom: &GeometryInfo,
    angle: f32,
) -> Vec<CellCoord> {
    if geom.is_small() {
        do_small_fill(center_x, center_y, geom.get_major_radius())
    } else {
        match geom.get_geom_type() {
            GeometryType::Sphere | GeometryType::Cylinder => {
                do_circle_fill(center_x, center_y, geom.get_major_radius())
            }
            GeometryType::Box => do_rect_fill(
                center_x,
                center_y,
                geom.get_major_radius(),
                geom.get_minor_radius(),
                angle,
            ),
        }
    }
}

/// C++ `PartitionData::doSmallFill` — stamp the 1–4 cells covering the
/// object's bounding square. Radius is clamped to half a partition cell.
pub fn do_small_fill(center_x: f32, center_y: f32, mut radius: f32) -> Vec<CellCoord> {
    let half_cell = PARTITION_CELL_SIZE * 0.5;
    if radius > half_cell {
        radius = half_cell;
    }
    let lo = world_to_cell(center_x - radius, center_y - radius);
    let hi = world_to_cell(center_x + radius, center_y + radius);
    let mut cells = Vec::new();
    for x in lo.x.min(hi.x)..=lo.x.max(hi.x) {
        for y in lo.y.min(hi.y)..=lo.y.max(hi.y) {
            push_unique(&mut cells, CellCoord { x, y });
        }
    }
    if cells.is_empty() {
        cells.push(world_to_cell(center_x, center_y));
    }
    cells
}

/// C++ `PartitionData::doCircleFill` — Bresenham filled-circle of cells.
pub fn do_circle_fill(center_x: f32, center_y: f32, radius: f32) -> Vec<CellCoord> {
    let center = world_to_cell(center_x, center_y);
    let mut cell_radius = world_to_cell_dist(radius);
    if cell_radius < 1 {
        cell_radius = 1;
    }
    let mut cells = Vec::new();
    let mut y = cell_radius - 1;
    let mut dec = 3 - 2 * cell_radius;
    for x in 0..cell_radius {
        h_line_circle(&mut cells, center.x - x, center.x + x, center.y + y);
        h_line_circle(&mut cells, center.x - x, center.x + x, center.y - y);
        h_line_circle(&mut cells, center.x - y, center.x + y, center.y + x);
        h_line_circle(&mut cells, center.x - y, center.x + y, center.y - x);
        if dec >= 0 {
            dec += (1 - y) << 2;
            y -= 1;
        }
        dec += (x << 2) + 6;
    }
    if cells.is_empty() {
        cells.push(center);
    }
    cells
}

/// C++ `PartitionData::doRectFill` — rotated rectangle raster at half-cell step.
pub fn do_rect_fill(
    center_x: f32,
    center_y: f32,
    halfsize_x: f32,
    halfsize_y: f32,
    angle: f32,
) -> Vec<CellCoord> {
    let c = angle.cos();
    let s = angle.sin();
    let step_size = PARTITION_CELL_SIZE * 0.5;
    let ydx = s * step_size;
    let ydy = -c * step_size;
    let xdx = c * step_size;
    let xdy = s * step_size;
    let step_size_inv_times_2 = 2.0 / step_size;
    let num_steps_x = (halfsize_x * step_size_inv_times_2).ceil() as i32;
    let num_steps_y = (halfsize_y * step_size_inv_times_2).ceil() as i32;
    let mut tl_x = center_x - halfsize_x * c - halfsize_y * s;
    let mut tl_y = center_y + halfsize_y * c - halfsize_x * s;
    let mut cells = Vec::new();
    for _iy in 0..num_steps_y.max(1) {
        let mut x = tl_x;
        let mut y = tl_y;
        for _ix in 0..num_steps_x.max(1) {
            push_unique(&mut cells, world_to_cell(x, y));
            x += xdx;
            y += xdy;
        }
        tl_x += ydx;
        tl_y += ydy;
    }
    if cells.is_empty() {
        cells.push(world_to_cell(center_x, center_y));
    }
    cells
}

fn h_line_circle(cells: &mut Vec<CellCoord>, x1: i32, x2: i32, y: i32) {
    let (lo, hi) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
    for x in lo..=hi {
        push_unique(cells, CellCoord { x, y });
    }
}

fn push_unique(cells: &mut Vec<CellCoord>, cell: CellCoord) {
    if !cells.contains(&cell) {
        cells.push(cell);
    }
}

#[cfg(test)]
mod tests {
    use super::super::collision_geometry::GeometryInfo;
    use super::super::partition_manager::CellCoord;
    use super::*;

    #[test]
    fn large_box_touches_cells_beyond_center() {
        let geom = GeometryInfo::new_box(160.0, 160.0, false);
        let cells = cells_touched_for_geometry(0.0, 0.0, &geom, 0.0);
        assert!(
            cells.contains(&CellCoord { x: 2, y: 0 }),
            "80wu half-extent box must occupy cell (2,0)"
        );
        assert!(cells.contains(&CellCoord { x: 0, y: 0 }));
    }

    #[test]
    fn small_sphere_stays_near_center_cell() {
        let geom = GeometryInfo::new_sphere(5.0, true);
        let cells = cells_touched_for_geometry(80.0, 0.0, &geom, 0.0);
        assert!(cells.contains(&CellCoord { x: 2, y: 0 }));
        assert!(!cells.contains(&CellCoord { x: 0, y: 0 }));
    }
}
