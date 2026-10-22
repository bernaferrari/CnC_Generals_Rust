//! C++ PartitionFilterWouldCollide geometry for the owning status callback.
use crate::game_logic::{HostGeometryInfo, HostGeometryType};
use gamelogic::object::collide::{
    CollideInfo, Coord3D, GeometryInfo as CollisionGeometry, collide_test_dispatch,
};
use glam::Vec3;

/// Matches PartitionManager::iteratePotentialCollisions' per-object broadphase
/// and PartitionFilterWouldCollide exact narrow phase. This does not reproduce
/// partition-cell candidate omissions or iterator ordering; see README.md.
pub(super) fn object_would_collide(
    query_position: Vec3,
    query_geometry: &HostGeometryInfo,
    query_orientation: f32,
    candidate_position: Vec3,
    candidate_geometry: &HostGeometryInfo,
    candidate_orientation: f32,
) -> bool {
    // C++ `iteratePotentialCollisions`: maxDist = query bounding-sphere radius
    // * 1.1, then FROM_BOUNDINGSPHERE_3D. In the query overload objA is null,
    // so only the candidate object's sphere is subtracted, and its center is
    // offset by height/2 for non-spheres. Host world Y is C++ Z (vertical).
    if !broadphase_candidate(
        query_position,
        query_geometry,
        candidate_position,
        candidate_geometry,
    ) {
        return false;
    }

    let query = collision_info(query_position, query_geometry, query_orientation);
    let candidate = collision_info(
        candidate_position,
        candidate_geometry,
        candidate_orientation,
    );

    // C++ PartitionFilterWouldCollide does this vertical interval test before
    // dispatching the oriented geometry pair. Keep raw authored height here;
    // do not use selection radii or XZ circles.
    let query_top = query.position.z + max_height_above(query_geometry);
    let candidate_top = candidate.position.z + max_height_above(candidate_geometry);
    if !(query_top >= candidate.position.z && query.position.z <= candidate_top) {
        return false;
    }

    // `collide_test_dispatch` matches C++'s type-pair collision routine table.
    // The raw vertical gate above is intentional; use dispatch rather than the
    // generic `collision_test` wrapper, which performs its own geometry-height
    // gate from the adapter's constructed GeometryInfo.
    collide_test_dispatch(
        query.geom.get_geom_type(),
        candidate.geom.get_geom_type(),
        &query,
        &candidate,
        None,
    )
}

fn broadphase_candidate(
    query_position: Vec3,
    query_geometry: &HostGeometryInfo,
    candidate_position: Vec3,
    candidate_geometry: &HostGeometryInfo,
) -> bool {
    let max_dist = query_geometry.bounding_sphere_radius() * 1.1;
    let candidate_center_height = z_delta_to_center(candidate_geometry);
    let dx = candidate_position.x - query_position.x;
    let dy = candidate_position.z - query_position.z;
    let dz = candidate_position.y + candidate_center_height - query_position.y;
    let center_distance_squared = dx * dx + dy * dy + dz * dz;
    let radius = candidate_geometry.bounding_sphere_radius();
    let distance_squared = if radius > 0.0 {
        let distance = center_distance_squared.sqrt() - radius;
        if distance <= 0.0 {
            0.0
        } else {
            distance * distance
        }
    } else {
        center_distance_squared
    };
    distance_squared < max_dist * max_dist
}

fn collision_info(position: Vec3, host: &HostGeometryInfo, orientation: f32) -> CollideInfo {
    let geometry = match host.geom_type {
        HostGeometryType::Sphere => CollisionGeometry::new_sphere(host.major_radius, host.is_small),
        HostGeometryType::Cylinder => {
            CollisionGeometry::new_cylinder(host.major_radius, host.height, host.is_small)
        }
        HostGeometryType::Box => CollisionGeometry::new_box(
            host.major_radius * 2.0,
            host.minor_radius * 2.0,
            host.is_small,
        )
        .with_z_height(host.height),
    };
    // C++ GeometryInfo uses X/Y as ground plane and Z as vertical. Main stores
    // positions as X/Y(vertical)/Z, matching the established RTB adapter.
    CollideInfo::new(
        Coord3D::new(position.x, position.z, position.y),
        geometry,
        orientation,
    )
}

fn z_delta_to_center(geometry: &HostGeometryInfo) -> f32 {
    match geometry.geom_type {
        HostGeometryType::Sphere => 0.0,
        HostGeometryType::Cylinder | HostGeometryType::Box => geometry.height * 0.5,
    }
}

fn max_height_above(geometry: &HostGeometryInfo) -> f32 {
    match geometry.geom_type {
        HostGeometryType::Sphere => geometry.major_radius,
        HostGeometryType::Cylinder | HostGeometryType::Box => geometry.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(radius: f32) -> HostGeometryInfo {
        HostGeometryInfo {
            geom_type: HostGeometryType::Sphere,
            is_small: false,
            height: radius,
            major_radius: radius,
            minor_radius: radius,
            authored: true,
        }
    }

    fn box_geom(half_x: f32, half_z: f32, height: f32) -> HostGeometryInfo {
        HostGeometryInfo {
            geom_type: HostGeometryType::Box,
            is_small: false,
            height,
            major_radius: half_x,
            minor_radius: half_z,
            authored: true,
        }
    }

    fn cylinder(radius: f32, height: f32) -> HostGeometryInfo {
        HostGeometryInfo {
            geom_type: HostGeometryType::Cylinder,
            is_small: false,
            height,
            major_radius: radius,
            minor_radius: radius,
            authored: true,
        }
    }

    #[test]
    fn vertical_disjoint_shapes_are_rejected_after_broadphase() {
        // The large query box keeps the object in the 1.1x bounding-sphere
        // broadphase, while exact vertical intervals [0,5] and [6,7] do not touch.
        let query = box_geom(5.0, 5.0, 5.0);
        let candidate = sphere(1.0);
        let candidate_pos = Vec3::new(0.0, 6.0, 0.0);
        assert!(broadphase_candidate(
            Vec3::ZERO,
            &query,
            candidate_pos,
            &candidate
        ));
        assert!(!object_would_collide(
            Vec3::ZERO,
            &query,
            0.0,
            candidate_pos,
            &candidate,
            0.0,
        ));
    }

    #[test]
    fn oriented_box_uses_dispatch_shape_not_its_bounding_circle() {
        let elongated = box_geom(4.0, 0.5, 4.0);
        let small = box_geom(0.4, 0.4, 4.0);
        // At orientation zero, the small box at ground Z=3.0 lies beyond the
        // elongated box's narrow side. Bounding circles overlap, shapes do not.
        assert!(!object_would_collide(
            Vec3::ZERO,
            &elongated,
            0.0,
            Vec3::new(0.0, 0.0, 3.0),
            &small,
            0.0,
        ));
        // Rotate long axis onto ground Z. Same centers now overlap by shape.
        assert!(object_would_collide(
            Vec3::ZERO,
            &elongated,
            std::f32::consts::FRAC_PI_2,
            Vec3::new(0.0, 0.0, 3.0),
            &small,
            0.0,
        ));
    }

    #[test]
    fn sphere_and_cylinder_use_authored_height_interval_and_shape() {
        // Sphere [−2,2], cylinder [1.75,5.75], same XZ: overlap at the cap.
        assert!(object_would_collide(
            Vec3::ZERO,
            &sphere(2.0),
            0.0,
            Vec3::new(0.0, 1.75, 0.0),
            &cylinder(0.5, 4.0),
            0.0,
        ));
        // Bottom 2.1 is above the sphere top 2.0; broadphase still sees it
        // because its remaining boundary distance is 2.1 < 2.2.
        let just_separated = Vec3::new(0.0, 2.1, 0.0);
        assert!(broadphase_candidate(
            Vec3::ZERO,
            &sphere(2.0),
            just_separated,
            &cylinder(0.5, 4.0),
        ));
        assert!(!object_would_collide(
            Vec3::ZERO,
            &sphere(2.0),
            0.0,
            just_separated,
            &cylinder(0.5, 4.0),
            0.0,
        ));
    }

    #[test]
    fn broadphase_uses_candidate_sphere_and_strict_1_1_query_radius() {
        let query = sphere(1.0);
        let candidate = sphere(1.0);
        // Surface distance 0.999 is within query maxDist 1.1 and shapes overlap.
        assert!(object_would_collide(
            Vec3::ZERO,
            &query,
            0.0,
            Vec3::new(0.0, 0.0, 1.999),
            &candidate,
            0.0,
        ));
        // Use an exactly representable boundary: 10 * 1.1 = 11 and
        // 12 - candidate radius 1 = 11. The rounded f32 value 2.1 - 1
        // is slightly BELOW 1.1, so it is not an equality witness.
        let boundary_query = sphere(10.0);
        assert!(broadphase_candidate(
            Vec3::ZERO,
            &boundary_query,
            Vec3::new(0.0, 0.0, f32::from_bits(12.0_f32.to_bits() - 1)),
            &candidate
        ));
        assert!(!broadphase_candidate(
            Vec3::ZERO,
            &boundary_query,
            Vec3::new(0.0, 0.0, 12.0),
            &candidate
        ));
        assert!(!broadphase_candidate(
            Vec3::ZERO,
            &boundary_query,
            Vec3::new(0.0, 0.0, f32::from_bits(12.0_f32.to_bits() + 1)),
            &candidate
        ));
    }
}
