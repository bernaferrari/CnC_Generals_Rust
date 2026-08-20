//! Host pair-loop helpers: GeometryInfo+angle collide test and
//! `CollideModule::onCollide` dispatch (`PartitionContactList::processContactList`).

use crate::game_logic::partition_coi::HostPartitionFootprint;
use crate::game_logic::{KindOf, Object, ObjectId};

/// Convert a host object into C++ `CollideInfo` inputs (XY = host XZ, Z = host Y).
pub fn host_object_collide_geom(obj: &Object) -> (gamelogic::object::collide::GeometryInfo, f32) {
    use gamelogic::object::collide::GeometryInfo;
    let g = &obj.thing.geometry;
    let half_x = ((g.bounds_max.x - g.bounds_min.x).abs() * 0.5).max(0.0);
    let half_z = ((g.bounds_max.z - g.bounds_min.z).abs() * 0.5).max(0.0);
    let height = (g.bounds_max.y - g.bounds_min.y).abs().max(g.radius);
    let major = g.radius.max(obj.selection_radius).max(half_x).max(1.0);
    let minor = half_z.max(g.radius).max(1.0);
    let angle = obj.get_orientation();
    let is_structure = obj.is_kind_of(KindOf::Structure);
    let geom = if is_structure && half_x > 0.0 && half_z > 0.0 {
        GeometryInfo::new_box(major * 2.0, minor * 2.0, false)
    } else {
        let small = major <= 20.0;
        GeometryInfo::new_cylinder(major, height.max(1.0), small)
    };
    (geom, angle)
}

pub fn host_object_footprint(obj: &Object) -> HostPartitionFootprint {
    let g = &obj.thing.geometry;
    let half_x = ((g.bounds_max.x - g.bounds_min.x).abs() * 0.5).max(0.0);
    let half_z = ((g.bounds_max.z - g.bounds_min.z).abs() * 0.5).max(0.0);
    let major = g.radius.max(obj.selection_radius).max(half_x).max(1.0);
    let minor = half_z.max(g.radius).max(1.0);
    let is_structure = obj.is_kind_of(KindOf::Structure);
    HostPartitionFootprint {
        major_radius: major,
        minor_radius: minor,
        angle: obj.get_orientation(),
        is_small: major <= 20.0 && !is_structure,
        is_box: is_structure && half_x > 0.0 && half_z > 0.0,
    }
}

/// C++ `PartitionData::collidesWith` using host pose (Y-up → collide Z-up).
pub fn host_geom_collides(a: &Object, b: &Object) -> Option<(glam::Vec3, glam::Vec3)> {
    use gamelogic::object::collide::{
        collide_test_dispatch, CollideInfo, CollideLocAndNormal, Coord3D,
    };
    let (geom_a, angle_a) = host_object_collide_geom(a);
    let (geom_b, angle_b) = host_object_collide_geom(b);
    let pa = a.get_position();
    let pb = b.get_position();
    let info_a = CollideInfo::new(Coord3D::new(pa.x, pa.z, pa.y), geom_a, angle_a);
    let info_b = CollideInfo::new(Coord3D::new(pb.x, pb.z, pb.y), geom_b, angle_b);
    let this_top = info_a.position.z + info_a.geom.get_max_height_above_position();
    let this_bot = info_a.position.z - info_a.geom.get_max_height_below_position();
    let that_top = info_b.position.z + info_b.geom.get_max_height_above_position();
    let that_bot = info_b.position.z - info_b.geom.get_max_height_below_position();
    if this_top < that_bot || this_bot > that_top {
        return None;
    }
    let mut cinfo =
        CollideLocAndNormal::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0));
    if !collide_test_dispatch(
        geom_a.get_geom_type(),
        geom_b.get_geom_type(),
        &info_a,
        &info_b,
        Some(&mut cinfo),
    ) {
        return None;
    }
    Some((
        glam::Vec3::new(cinfo.loc.x, cinfo.loc.z, cinfo.loc.y),
        glam::Vec3::new(cinfo.normal.x, cinfo.normal.z, cinfo.normal.y),
    ))
}

/// C++ `Object::onCollide` / `COLLISION_MANAGER.handle_collision` on both sides.
///
/// `wouldLikeToCollideWith` is **not** a gate (C++ `processContactList`).
pub fn dispatch_collide_modules(
    a_id: ObjectId,
    b_id: ObjectId,
    loc: glam::Vec3,
    normal: glam::Vec3,
) {
    use gamelogic::object::collide::{Coord3D, GameObject, COLLISION_MANAGER};
    use gamelogic::object::registry::OBJECT_REGISTRY;

    let collide_loc = Coord3D::new(loc.x, loc.z, loc.y);
    let collide_n = Coord3D::new(normal.x, normal.z, normal.y);
    let inv_n = Coord3D::new(-collide_n.x, -collide_n.y, -collide_n.z);

    let other_b = OBJECT_REGISTRY
        .get_object(b_id.0)
        .or_else(|| gamelogic::helpers::TheGameLogic::find_object_by_id(b_id.0));
    let other_a = OBJECT_REGISTRY
        .get_object(a_id.0)
        .or_else(|| gamelogic::helpers::TheGameLogic::find_object_by_id(a_id.0));

    if let Some(handle) = &other_b {
        let _ = COLLISION_MANAGER.handle_collision(
            a_id.0,
            Some(handle as &dyn GameObject),
            &collide_loc,
            &collide_n,
        );
    } else {
        let _ = COLLISION_MANAGER.handle_collision(a_id.0, None, &collide_loc, &collide_n);
    }

    let a_dead = OBJECT_REGISTRY
        .with_object(a_id.0, |o| o.is_destroyed() || o.is_effectively_dead())
        .unwrap_or(false);
    let b_dead = OBJECT_REGISTRY
        .with_object(b_id.0, |o| o.is_destroyed() || o.is_effectively_dead())
        .unwrap_or(false);
    if a_dead || b_dead {
        return;
    }

    if let Some(handle) = &other_a {
        let _ = COLLISION_MANAGER.handle_collision(
            b_id.0,
            Some(handle as &dyn GameObject),
            &collide_loc,
            &inv_n,
        );
    } else {
        let _ = COLLISION_MANAGER.handle_collision(b_id.0, None, &collide_loc, &inv_n);
    }
}
