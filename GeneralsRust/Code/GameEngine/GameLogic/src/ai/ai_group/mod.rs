//! C++ `AIGroup.cpp` helpers extracted from leftover `group.rs`.
//!
//! Keep `AIGroup` methods in `group.rs` (residual include_str scans).

use crate::common::Coord3D;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::terrain::get_terrain_logic;

/// C++ `STD_WAYPOINT_CLAMP_MARGIN` (`PATHFIND_CELL_SIZE_F * 4`).
pub const STD_WAYPOINT_CLAMP_MARGIN: f32 = PATHFIND_CELL_SIZE_F * 4.0;
/// C++ `STD_AIRCRAFT_EXTRA_MARGIN` (`PATHFIND_CELL_SIZE_F * 10`).
pub const STD_AIRCRAFT_EXTRA_MARGIN: f32 = PATHFIND_CELL_SIZE_F * 10.0;

const CIRCLE: f32 = 2.0 * std::f32::consts::PI;
const ASSUMED_HELI_DIAMETER: f32 = 70.0;

/// C++ `clampWaypointPosition` (`AIGroup.cpp:1497-1521`).
pub fn clamp_waypoint_position(position: &mut Coord3D, margin: f32) {
    let Ok(terrain) = get_terrain_logic().read() else {
        return;
    };
    let mut extent = terrain.get_extent();
    extent.hi.x -= margin;
    extent.hi.y -= margin;
    extent.lo.x += margin;
    extent.lo.y += margin;
    let inside = position.x >= extent.lo.x
        && position.x <= extent.hi.x
        && position.y >= extent.lo.y
        && position.y <= extent.hi.y;
    if inside {
        return;
    }
    if position.x > extent.hi.x {
        position.x = extent.hi.x;
    } else if position.x < extent.lo.x {
        position.x = extent.lo.x;
    }
    if position.y > extent.hi.y {
        position.y = extent.hi.y;
    } else if position.y < extent.lo.y {
        position.y = extent.lo.y;
    }
    position.z = terrain.get_ground_height(position.x, position.y, None);
}

/// C++ `getHelicopterOffset` (`AIGroup.cpp:1799-1826`).
pub fn get_helicopter_offset(pos_out: &mut Coord3D, idx: i32) {
    if idx == 0 {
        return;
    }
    let mut radius = ASSUMED_HELI_DIAMETER;
    let mut circumference = radius * CIRCLE;
    let mut angle = 0.0_f32;
    let mut angle_between = ASSUMED_HELI_DIAMETER / circumference * CIRCLE;
    for _h in 1..idx {
        angle += angle_between;
        if angle > CIRCLE {
            radius += ASSUMED_HELI_DIAMETER;
            circumference = radius * CIRCLE;
            angle_between = ASSUMED_HELI_DIAMETER / circumference * CIRCLE;
            angle -= CIRCLE;
        }
    }
    let cx = pos_out.x;
    let cy = pos_out.y;
    pos_out.x = cx + angle.sin() * radius;
    pos_out.y = cy + angle.cos() * radius;
}
