//! C++ `DistanceCalculationType` / `theDistCalcProcs` (`PartitionManager.cpp:738-925`).

use super::Coord3D;
use super::collision_geometry::GeometryInfo;

/// C++ `HUGE_DIST` (`PartitionManager.h:45`).
pub const HUGE_DIST: f32 = 1_000_000.0;

/// C++ `HUGE_DIST_SQR`.
pub const HUGE_DIST_SQR: f32 = HUGE_DIST * HUGE_DIST;

/// C++ `DistanceCalculationType` (`PartitionManager.h:121-127`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DistanceCalculationType {
    /// `FROM_CENTER_2D` — object center, XY only.
    FromCenter2D = 0,
    /// `FROM_CENTER_3D` — object center, XYZ.
    FromCenter3D = 1,
    /// `FROM_BOUNDINGSPHERE_2D` — subtract 2D bounding-circle radii.
    FromBoundingSphere2D = 2,
    /// `FROM_BOUNDINGSPHERE_3D` — subtract 3D bounding-sphere radii.
    FromBoundingSphere3D = 3,
}

impl Default for DistanceCalculationType {
    fn default() -> Self {
        Self::FromCenter2D
    }
}

/// Distance from `center` to a candidate (no source object).
///
/// Matches `theDistCalcProcs[dc](pos, NULL, otherPos, otherObj, ...)`.
pub fn distance_from_position(
    center: &Coord3D,
    other_pos: &Coord3D,
    other_geom: &GeometryInfo,
    dc: DistanceCalculationType,
) -> f32 {
    match dc {
        DistanceCalculationType::FromCenter2D => {
            let dx = other_pos.x - center.x;
            let dy = other_pos.y - center.y;
            (dx * dx + dy * dy).sqrt()
        }
        DistanceCalculationType::FromCenter3D => {
            let dx = other_pos.x - center.x;
            let dy = other_pos.y - center.y;
            let dz = other_pos.z - center.z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        }
        DistanceCalculationType::FromBoundingSphere2D => {
            let dx = other_pos.x - center.x;
            let dy = other_pos.y - center.y;
            let actual = (dx * dx + dy * dy).sqrt();
            (actual - other_geom.get_bounding_circle_radius()).max(0.0)
        }
        DistanceCalculationType::FromBoundingSphere3D => {
            let z_delta = other_geom.get_z_delta_to_center_position();
            let dx = other_pos.x - center.x;
            let dy = other_pos.y - center.y;
            let dz = (other_pos.z + z_delta) - center.z;
            let actual = (dx * dx + dy * dy + dz * dz).sqrt();
            (actual - other_geom.get_bounding_sphere_radius()).max(0.0)
        }
    }
}

/// True when the computed distance is `<= radius` (inclusive, matching
/// C++ `abDistSqr < maxDistSqr` for the open interval is not used by
/// `find_objects_in_radius` which historically accepted `dist_sqr <= radius_sqr`).
pub fn within_radius(
    center: &Coord3D,
    other_pos: &Coord3D,
    other_geom: &GeometryInfo,
    radius: f32,
    dc: DistanceCalculationType,
) -> bool {
    if radius >= HUGE_DIST {
        return true;
    }
    distance_from_position(center, other_pos, other_geom, dc) <= radius
}
