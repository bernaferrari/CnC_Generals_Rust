//! Host-side C++ `PartitionData` COI stamping (XZ world, Y-up).
//!
//! Partition cells are 40 world units (`PARTITION_CELL_SIZE_RESIDUAL`).

use super::partition_manager::PARTITION_CELL_SIZE_RESIDUAL;

/// Host geometry used to stamp COI cells. Angle is yaw about Y (radians).
#[derive(Debug, Clone, Copy)]
pub struct HostPartitionFootprint {
    pub major_radius: f32,
    pub minor_radius: f32,
    pub angle: f32,
    pub is_small: bool,
    pub is_box: bool,
}

impl HostPartitionFootprint {
    pub fn small_circle(radius: f32) -> Self {
        Self {
            major_radius: radius.max(0.0),
            minor_radius: radius.max(0.0),
            angle: 0.0,
            is_small: true,
            is_box: false,
        }
    }
}

/// C++ `worldToCell` on the host XZ plane.
#[inline]
pub fn world_to_cell(x: f32, z: f32) -> (i32, i32) {
    let s = PARTITION_CELL_SIZE_RESIDUAL;
    ((x / s).floor() as i32, (z / s).floor() as i32)
}

#[inline]
pub fn world_to_cell_dist(world: f32) -> i32 {
    (world / PARTITION_CELL_SIZE_RESIDUAL).ceil() as i32
}

/// C++ `PartitionData::updateCellsTouched` for a host object.
pub fn cells_touched_for_footprint(x: f32, z: f32, fp: HostPartitionFootprint) -> Vec<(i32, i32)> {
    if fp.is_small {
        do_small_fill(x, z, fp.major_radius)
    } else if fp.is_box {
        do_rect_fill(x, z, fp.major_radius, fp.minor_radius, fp.angle)
    } else {
        do_circle_fill(x, z, fp.major_radius)
    }
}

/// Default when no geometry has been cached yet: C++ `doSmallFill` at half-cell.
pub fn cells_touched_default(x: f32, z: f32) -> Vec<(i32, i32)> {
    do_small_fill(x, z, PARTITION_CELL_SIZE_RESIDUAL * 0.5)
}

pub fn do_small_fill(center_x: f32, center_z: f32, mut radius: f32) -> Vec<(i32, i32)> {
    let half_cell = PARTITION_CELL_SIZE_RESIDUAL * 0.5;
    if radius > half_cell {
        radius = half_cell;
    }
    let (x1, z1) = world_to_cell(center_x - radius, center_z - radius);
    let (x2, z2) = world_to_cell(center_x + radius, center_z + radius);
    let mut cells = Vec::new();
    for x in x1.min(x2)..=x1.max(x2) {
        for z in z1.min(z2)..=z1.max(z2) {
            push_unique(&mut cells, (x, z));
        }
    }
    if cells.is_empty() {
        cells.push(world_to_cell(center_x, center_z));
    }
    cells
}

pub fn do_circle_fill(center_x: f32, center_z: f32, radius: f32) -> Vec<(i32, i32)> {
    let (cx, cz) = world_to_cell(center_x, center_z);
    let mut cell_radius = world_to_cell_dist(radius);
    if cell_radius < 1 {
        cell_radius = 1;
    }
    let mut cells = Vec::new();
    let mut y = cell_radius - 1;
    let mut dec = 3 - 2 * cell_radius;
    for x in 0..cell_radius {
        h_line(&mut cells, cx - x, cx + x, cz + y);
        h_line(&mut cells, cx - x, cx + x, cz - y);
        h_line(&mut cells, cx - y, cx + y, cz + x);
        h_line(&mut cells, cx - y, cx + y, cz - x);
        if dec >= 0 {
            dec += (1 - y) << 2;
            y -= 1;
        }
        dec += (x << 2) + 6;
    }
    if cells.is_empty() {
        cells.push((cx, cz));
    }
    cells
}

pub fn do_rect_fill(
    center_x: f32,
    center_z: f32,
    halfsize_x: f32,
    halfsize_z: f32,
    angle: f32,
) -> Vec<(i32, i32)> {
    let c = angle.cos();
    let s = angle.sin();
    let step_size = PARTITION_CELL_SIZE_RESIDUAL * 0.5;
    let ydx = s * step_size;
    let ydy = -c * step_size;
    let xdx = c * step_size;
    let xdy = s * step_size;
    let step_size_inv_times_2 = 2.0 / step_size;
    let num_steps_x = (halfsize_x * step_size_inv_times_2).ceil() as i32;
    let num_steps_z = (halfsize_z * step_size_inv_times_2).ceil() as i32;
    let mut tl_x = center_x - halfsize_x * c - halfsize_z * s;
    let mut tl_z = center_z + halfsize_z * c - halfsize_x * s;
    let mut cells = Vec::new();
    for _iz in 0..num_steps_z.max(1) {
        let mut x = tl_x;
        let mut z = tl_z;
        for _ix in 0..num_steps_x.max(1) {
            push_unique(&mut cells, world_to_cell(x, z));
            x += xdx;
            z += xdy;
        }
        tl_x += ydx;
        tl_z += ydy;
    }
    if cells.is_empty() {
        cells.push(world_to_cell(center_x, center_z));
    }
    cells
}

fn h_line(cells: &mut Vec<(i32, i32)>, x1: i32, x2: i32, z: i32) {
    let (lo, hi) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
    for x in lo..=hi {
        push_unique(cells, (x, z));
    }
}

fn push_unique(cells: &mut Vec<(i32, i32)>, cell: (i32, i32)) {
    if !cells.contains(&cell) {
        cells.push(cell);
    }
}

/// C++ `PartitionData::getShroudedStatus` cell mix (`PartitionManager.cpp:1615-1674`).
/// Returns `(status, ever_seen)` after applying fogged-enemy / mine / neutral-mobile rules.
pub fn mix_object_shroud_from_cells(
    coi_count: usize,
    shrouded_cells: usize,
    fogged_cells: usize,
    relationship_neutral: bool,
    immobile: bool,
    mine: bool,
    ever_seen: bool,
) -> (gamelogic::common::types::ObjectShroudStatus, bool) {
    use gamelogic::common::types::ObjectShroudStatus;
    if coi_count == 0 || shrouded_cells == coi_count {
        return (ObjectShroudStatus::Shrouded, false);
    }
    if shrouded_cells + fogged_cells == coi_count {
        let mut fogged = ObjectShroudStatus::Fogged;
        if relationship_neutral {
            if !immobile {
                fogged = ObjectShroudStatus::Shrouded;
            }
        } else if !(immobile && ever_seen) || mine {
            fogged = ObjectShroudStatus::Shrouded;
        }
        return (fogged, ever_seen);
    }
    if shrouded_cells == 0 && fogged_cells == 0 {
        (ObjectShroudStatus::Clear, true)
    } else {
        (ObjectShroudStatus::PartialClear, true)
    }
}

#[cfg(test)]
mod mix_tests {
    use super::mix_object_shroud_from_cells;
    use gamelogic::common::types::ObjectShroudStatus;

    #[test]
    fn coi_mix_matches_partition_manager_cpp() {
        // C++ PartitionManager.cpp:1615-1674.
        let (s, seen) = mix_object_shroud_from_cells(4, 4, 0, false, true, false, true);
        assert_eq!(s, ObjectShroudStatus::Shrouded);
        assert!(!seen);

        let (s, seen) = mix_object_shroud_from_cells(4, 0, 0, false, true, false, false);
        assert_eq!(s, ObjectShroudStatus::Clear);
        assert!(seen);

        let (s, seen) = mix_object_shroud_from_cells(4, 1, 0, false, true, false, false);
        assert_eq!(s, ObjectShroudStatus::PartialClear);
        assert!(seen);

        let (s, _) = mix_object_shroud_from_cells(2, 0, 2, true, false, false, false);
        assert_eq!(
            s,
            ObjectShroudStatus::Shrouded,
            "neutral mobile fog → shroud"
        );

        let (s, _) = mix_object_shroud_from_cells(2, 0, 2, false, false, false, false);
        assert_eq!(
            s,
            ObjectShroudStatus::Shrouded,
            "unseen mobile enemy fog → shroud"
        );

        let (s, _) = mix_object_shroud_from_cells(2, 0, 2, false, true, false, true);
        assert_eq!(
            s,
            ObjectShroudStatus::Fogged,
            "seen immobile enemy stays fog ghost"
        );

        let (s, _) = mix_object_shroud_from_cells(2, 0, 2, false, true, true, true);
        assert_eq!(
            s,
            ObjectShroudStatus::Shrouded,
            "KINDOF_MINE never fog-ghosts"
        );
    }
}
