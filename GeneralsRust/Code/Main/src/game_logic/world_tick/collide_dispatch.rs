//! Host pair-loop helpers: GeometryInfo+angle collide test and
//! `CollideModule::onCollide` dispatch (`PartitionContactList::processContactList`).

use crate::game_logic::partition_coi::HostPartitionFootprint;
use crate::game_logic::{KindOf, Object, ObjectId};
use generals_sim::object::collision_weapon::{CollisionWeapon, CollisionWeaponGate};
use generals_sim::object::ObjectStatusMaskType;

/// Convert a host object into C++ `CollideInfo` inputs (XY = host XZ, Z = host Y).
pub fn host_object_collide_geom(obj: &Object) -> (gamelogic::object::collide::GeometryInfo, f32) {
    use crate::game_logic::HostGeometryType;
    use gamelogic::object::collide::GeometryInfo;
    let angle = obj.get_orientation();
    let authored = obj.geometry_info();
    if authored.authored || obj.thing().template.receiving_object_definition.is_some() {
        let geom = match authored.geom_type {
            HostGeometryType::Sphere => {
                GeometryInfo::new_sphere(authored.major_radius, authored.is_small)
            }
            HostGeometryType::Cylinder => GeometryInfo::new_cylinder(
                authored.major_radius,
                authored.height,
                authored.is_small,
            ),
            HostGeometryType::Box => {
                // C++ major/minor are already half-extents; crate `new_box`
                // takes full XY extents and stores height in the 2nd arg.
                let mut geom = GeometryInfo::new_box(
                    authored.major_radius * 2.0,
                    authored.minor_radius * 2.0,
                    authored.is_small,
                );
                geom.set_height(authored.height);
                geom
            }
        };
        return (geom, angle);
    }
    let g = &obj.thing().geometry;
    let half_x = ((g.bounds_max.x - g.bounds_min.x).abs() * 0.5).max(0.0);
    let half_z = ((g.bounds_max.z - g.bounds_min.z).abs() * 0.5).max(0.0);
    let height = (g.bounds_max.y - g.bounds_min.y).abs().max(g.radius);
    let major = g.radius.max(obj.selection_radius).max(half_x).max(1.0);
    let minor = half_z.max(g.radius).max(1.0);
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
    use crate::game_logic::HostGeometryType;
    let authored = obj.geometry_info();
    if authored.authored || obj.thing().template.receiving_object_definition.is_some() {
        let (major, minor, is_box) = match authored.geom_type {
            HostGeometryType::Sphere | HostGeometryType::Cylinder => {
                (authored.major_radius, authored.major_radius, false)
            }
            HostGeometryType::Box => (authored.major_radius, authored.minor_radius, true),
        };
        return HostPartitionFootprint {
            major_radius: major,
            minor_radius: minor,
            angle: obj.get_orientation(),
            is_small: authored.is_small,
            is_box,
        };
    }
    let g = &obj.thing().geometry;
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
        CollideInfo, CollideLocAndNormal, Coord3D, collide_test_dispatch,
    };
    // C++ PartitionData::collidesWith (PartitionManager.cpp:1932-1933).
    if a.is_kind_of(KindOf::NoCollide) || b.is_kind_of(KindOf::NoCollide) {
        return None;
    }

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

/// Resolve collision metadata from the installed definition. Detached
/// programmatic objects can still use the legacy asset and fire-field data.
pub(super) fn collision_weapon(obj: &Object, allow_detached: bool) -> Option<CollisionWeapon> {
    let template = &obj.thing().template;
    if let Some(definition) = &template.receiving_object_definition {
        return collision_weapon_from_definition(definition);
    }
    if !allow_detached {
        return None;
    }
    if let Some(spec) = crate::assets::get_asset_manager().and_then(|manager| {
        let assets = manager.lock().ok()?;
        collision_weapon_from_definition(assets.get_object_definition(&obj.template_name)?)
    }) {
        return Some(spec);
    }
    let n = obj.template_name.to_ascii_lowercase();
    if !["firestorm", "firefield", "firewallsegment", "fireweaponcollide"]
        .iter()
        .any(|name| n.contains(name))
    {
        return None;
    }
    Some(CollisionWeapon {
        weapon_name: template
            .primary_weapon_name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "FirestormSmall".to_string()),
        gate: CollisionWeaponGate::UNRESTRICTED,
    })
}

fn collision_weapon_from_definition(
    definition: &crate::assets::ObjectDefinition,
) -> Option<CollisionWeapon> {
    let module = definition
        .behavior_modules
        .iter()
        .find(|module| module.class_name.eq_ignore_ascii_case("FireWeaponCollide"))?;
    let name = module.attribute("CollideWeapon")?.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("None") {
        return None;
    }
    let status = |field| {
        let value = module.attribute(field).unwrap_or_default();
        ObjectStatusMaskType::parse_tokens(
            value
                .split(|c: char| c.is_ascii_whitespace() || matches!(c, ',' | '|'))
                .filter(|token| !token.is_empty()),
        )
        .ok()
    };
    Some(CollisionWeapon {
        weapon_name: name.to_owned(),
        gate: CollisionWeaponGate {
            fire_once: module.attribute("FireOnce").is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "yes" | "true" | "1" | "on"
                )
            }),
            required_status: status("RequiredStatus")?,
            forbidden_status: status("ForbiddenStatus")?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_ini_geometry_drives_collide_not_kind_radii() {
        // C++ Geometry.cpp:26-58 / ThingTemplate.cpp:201-205.
        // Pre-fix construct.rs used Vehicle=15; live collide must use INI BOX 13/9/10.
        use crate::game_logic::{
            HostGeometryInfo, HostGeometryType, KindOf, Object, ObjectId, Team, ThingTemplate,
        };
        use gamelogic::object::collide::GeometryType;

        let mut template = ThingTemplate::new("AmericaTankBattleMaster");
        template.add_kind_of(KindOf::Vehicle);
        template.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Box,
            is_small: true,
            height: 10.0,
            major_radius: 13.0,
            minor_radius: 9.0,
            authored: true,
        };
        let obj = Object::new(template, ObjectId(1), Team::USA);
        let (geom, _) = host_object_collide_geom(&obj);
        assert_eq!(geom.get_geom_type(), GeometryType::Box);
        assert!((geom.get_major_radius() - 13.0).abs() < 1e-4);
        assert!((geom.get_minor_radius() - 9.0).abs() < 1e-4);
        assert!((geom.get_height() - 10.0).abs() < 1e-4);
        assert!(geom.is_small());
        assert!(
            (obj.selection_radius - 15.0).abs() > 0.5,
            "must not fall back to hardcoded Vehicle radius 15"
        );

        let fp = host_object_footprint(&obj);
        assert!((fp.major_radius - 13.0).abs() < 1e-4);
        assert!((fp.minor_radius - 9.0).abs() < 1e-4);
        assert!(fp.is_box);
        assert!(fp.is_small);
    }

    #[test]
    fn kindof_no_collide_skips_partition_collides_with() {
        // C++ PartitionData::collidesWith (PartitionManager.cpp:1932-1933).
        use crate::game_logic::{
            HostGeometryInfo, HostGeometryType, KindOf, Object, ObjectId, Team, ThingTemplate,
        };

        let mut tank = ThingTemplate::new("AmericaTankBattleMaster");
        tank.add_kind_of(KindOf::Vehicle);
        tank.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Box,
            is_small: true,
            height: 10.0,
            major_radius: 13.0,
            minor_radius: 9.0,
            authored: true,
        };
        let mut remnant = ThingTemplate::new("ParticleUplinkCannonTrailRemnant");
        remnant.add_kind_of(KindOf::NoCollide);
        remnant.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Cylinder,
            is_small: true,
            height: 8.0,
            major_radius: 8.0,
            minor_radius: 8.0,
            authored: true,
        };
        let a = Object::new(tank, ObjectId(1), Team::USA);
        let b = Object::new(remnant, ObjectId(2), Team::USA);
        assert!(host_geom_collides(&a, &b).is_none());
        assert!(host_geom_collides(&b, &a).is_none());
    }
}
