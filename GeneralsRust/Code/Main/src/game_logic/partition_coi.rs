//! Host-side C++ `PartitionData` COI stamping (XZ world, Y-up).
//!
//! Real maps supply their admitted origin, dimensions and cell size.
//! Forty-unit wrappers serve the existing standalone component fixtures.

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
    world_to_cell_at_size([0.0, 0.0], PARTITION_CELL_SIZE_RESIDUAL, x, z)
}

#[inline]
fn world_to_cell_at_size(origin: [f32; 2], s: f32, x: f32, z: f32) -> (i32, i32) {
    (
        ((x - origin[0]) * (1.0_f64 / f64::from(s)) as f32).floor() as i32,
        ((z - origin[1]) * (1.0_f64 / f64::from(s)) as f32).floor() as i32,
    )
}

#[inline]
pub fn world_to_cell_dist(world: f32) -> i32 {
    (world / PARTITION_CELL_SIZE_RESIDUAL).ceil() as i32
}

/// C++ `PartitionData::updateCellsTouched` for a host object.
pub fn cells_touched_for_footprint(x: f32, z: f32, fp: HostPartitionFootprint) -> Vec<(i32, i32)> {
    cells_touched_for_footprint_at_origin([0.0, 0.0], x, z, fp)
}

/// C++ worldToCell subtracts the driving map origin at each conversion, after
/// the footprint's world-space corner/rotation arithmetic has completed.
pub fn cells_touched_for_footprint_at_origin(
    origin: [f32; 2],
    x: f32,
    z: f32,
    fp: HostPartitionFootprint,
) -> Vec<(i32, i32)> {
    if fp.is_small {
        do_small_fill_at_origin(origin, x, z, fp.major_radius)
    } else if fp.is_box {
        do_rect_fill_at_origin(origin, x, z, fp.major_radius, fp.minor_radius, fp.angle)
    } else {
        do_circle_fill_at_origin(origin, x, z, fp.major_radius)
    }
}

/// Default when no geometry has been cached yet: C++ `doSmallFill` at half-cell.
pub fn cells_touched_default(x: f32, z: f32) -> Vec<(i32, i32)> {
    do_small_fill(x, z, PARTITION_CELL_SIZE_RESIDUAL * 0.5)
}

pub fn do_small_fill(center_x: f32, center_z: f32, radius: f32) -> Vec<(i32, i32)> {
    do_small_fill_at_origin([0.0, 0.0], center_x, center_z, radius)
}

fn do_small_fill_at_origin(
    origin: [f32; 2],
    center_x: f32,
    center_z: f32,
    radius: f32,
) -> Vec<(i32, i32)> {
    collect_footprint(
        origin,
        center_x,
        center_z,
        HostPartitionFootprint::small_circle(radius),
    )
}

pub fn do_circle_fill(center_x: f32, center_z: f32, radius: f32) -> Vec<(i32, i32)> {
    do_circle_fill_at_origin([0.0, 0.0], center_x, center_z, radius)
}

fn do_circle_fill_at_origin(
    origin: [f32; 2],
    center_x: f32,
    center_z: f32,
    radius: f32,
) -> Vec<(i32, i32)> {
    collect_footprint(
        origin,
        center_x,
        center_z,
        HostPartitionFootprint {
            major_radius: radius,
            minor_radius: radius,
            angle: 0.0,
            is_small: false,
            is_box: false,
        },
    )
}

pub fn do_rect_fill(
    center_x: f32,
    center_z: f32,
    halfsize_x: f32,
    halfsize_z: f32,
    angle: f32,
) -> Vec<(i32, i32)> {
    do_rect_fill_at_origin(
        [0.0, 0.0],
        center_x,
        center_z,
        halfsize_x,
        halfsize_z,
        angle,
    )
}

fn do_rect_fill_at_origin(
    origin: [f32; 2],
    center_x: f32,
    center_z: f32,
    halfsize_x: f32,
    halfsize_z: f32,
    angle: f32,
) -> Vec<(i32, i32)> {
    collect_footprint(
        origin,
        center_x,
        center_z,
        HostPartitionFootprint {
            major_radius: halfsize_x,
            minor_radius: halfsize_z,
            angle,
            is_small: false,
            is_box: true,
        },
    )
}

trait FootprintSink {
    fn cell(&mut self, x: i64, z: i64);
    fn span(&mut self, lo: i64, hi: i64, z: i64) {
        for x in lo..=hi {
            self.cell(x, z);
        }
    }
}

#[derive(Default)]
struct FootprintCells {
    cells: Vec<(i32, i32)>,
    seen: std::collections::HashSet<(i32, i32)>,
}

impl FootprintSink for FootprintCells {
    fn cell(&mut self, x: i64, z: i64) {
        // Original cell indices are signed Int. Avoid overflow for invalid
        // off-map inputs; valid original indices retain first-visit order.
        if let (Ok(x), Ok(z)) = (i32::try_from(x), i32::try_from(z)) {
            if self.seen.insert((x, z)) {
                self.cells.push((x, z));
            }
        }
    }
}

fn collect_footprint(
    origin: [f32; 2],
    x: f32,
    z: f32,
    fp: HostPartitionFootprint,
) -> Vec<(i32, i32)> {
    let mut cells = FootprintCells::default();
    stamp_footprint(origin, PARTITION_CELL_SIZE_RESIDUAL, x, z, fp, &mut cells);
    cells.cells
}

/// Shared original sampler. The sink performs getCellAt admission; circle
/// scanlines can therefore be clipped before enumerating off-map coordinates.
fn stamp_footprint(
    origin: [f32; 2],
    cell_size: f32,
    x: f32,
    z: f32,
    fp: HostPartitionFootprint,
    cells: &mut impl FootprintSink,
) {
    if fp.is_small {
        let radius = fp.major_radius.min(cell_size * 0.5);
        let (x1, z1) = world_to_cell_at_size(origin, cell_size, x - radius, z - radius);
        let (x2, z2) = world_to_cell_at_size(origin, cell_size, x + radius, z + radius);
        for ix in x1 as i64..=x2 as i64 {
            for iz in z1 as i64..=z2 as i64 {
                cells.cell(ix, iz);
            }
        }
    } else if fp.is_box {
        let c = fp.angle.cos();
        let s = fp.angle.sin();
        let step_size = cell_size * 0.5;
        let ydx = s * step_size;
        let ydy = -c * step_size;
        let xdx = c * step_size;
        let xdy = s * step_size;
        let inv_times_2 = 2.0 / step_size;
        let nx = (fp.major_radius * inv_times_2).ceil() as i32;
        let nz = (fp.minor_radius * inv_times_2).ceil() as i32;
        if nx <= 0 || nz <= 0 {
            return;
        }
        let mut tl_x = x - fp.major_radius * c - fp.minor_radius * s;
        let mut tl_z = z + fp.minor_radius * c - fp.major_radius * s;
        // CPP PartitionManager.cpp:1781-1799: zero steps produce no COIs.
        for _iz in 0..nz {
            let mut sx = tl_x;
            let mut sz = tl_z;
            for _ix in 0..nx {
                let (ix, iz) = world_to_cell_at_size(origin, cell_size, sx, sz);
                cells.cell(ix as i64, iz as i64);
                sx += xdx;
                sz += xdy;
            }
            tl_x += ydx;
            tl_z += ydy;
        }
    } else {
        let (cx, cz) = world_to_cell_at_size(origin, cell_size, x, z);
        let (cx, cz) = (cx as i64, cz as i64);
        let radius = i64::from(
            ((fp.major_radius * ((1.0_f64 / f64::from(cell_size)) as f32)).ceil() as i32).max(1),
        );
        let mut y = radius - 1;
        let mut dec = 3 - 2 * radius;
        for x in 0..radius {
            cells.span(cx - x, cx + x, cz + y);
            cells.span(cx - x, cx + x, cz - y);
            cells.span(cx - y, cx + y, cz + x);
            cells.span(cx - y, cx + y, cz - x);
            if dec >= 0 {
                dec += (1 - y) << 2;
                y -= 1;
            }
            dec += (x << 2) + 6;
        }
    }
}

mod clipped;
pub(crate) use clipped::{
    cells_touched_for_footprint_clipped, cells_touched_for_footprint_on_grid, footprint_sample_cost,
};

#[cfg(test)]
mod footprint_tests;

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
    fn map_origin_is_applied_at_small_circle_and_rotated_box_conversions() {
        use super::*;
        let origin = [160.0, -320.0];
        for (small, boxed) in [(true, false), (false, false), (false, true)] {
            let footprint = HostPartitionFootprint {
                major_radius: 25.0,
                minor_radius: 12.0,
                angle: 0.25,
                is_small: small,
                is_box: boxed,
            };
            assert_eq!(
                cells_touched_for_footprint_at_origin(origin, 180.0, -300.0, footprint),
                cells_touched_for_footprint(20.0, 20.0, footprint)
            );
        }
    }

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
