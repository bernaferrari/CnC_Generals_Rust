//! Host pair-loop helpers: GeometryInfo+angle collide test and
//! `CollideModule::onCollide` dispatch (`PartitionContactList::processContactList`).

use crate::game_logic::partition_coi::HostPartitionFootprint;
use crate::game_logic::{KindOf, Object, ObjectId};

/// Convert a host object into C++ `CollideInfo` inputs (XY = host XZ, Z = host Y).
pub fn host_object_collide_geom(obj: &Object) -> (gamelogic::object::collide::GeometryInfo, f32) {
    use crate::game_logic::HostGeometryType;
    use gamelogic::object::collide::GeometryInfo;
    let angle = obj.get_orientation();
    let authored = &obj.thing.template.geometry_info;
    if authored.authored {
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
    let g = &obj.thing.geometry;
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
    let authored = &obj.thing.template.geometry_info;
    if authored.authored {
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

/// C++ `Object::onCollide` / `COLLISION_MANAGER.handle_collision` on both sides.
///
/// `wouldLikeToCollideWith` is **not** a gate (C++ `processContactList`).
pub fn dispatch_collide_modules(
    a_id: ObjectId,
    b_id: ObjectId,
    loc: glam::Vec3,
    normal: glam::Vec3,
) {
    use gamelogic::object::collide::{COLLISION_MANAGER, Coord3D, GameObject};
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

/// C++ `FireWeaponCollideModuleData` residual on a live host object.
#[derive(Debug, Clone)]
pub struct HostFireWeaponCollideSpec {
    pub weapon_name: String,
    pub fire_once: bool,
    pub required_status: u64,
    pub forbidden_status: u64,
}

/// C++ `FireWeaponCollide::shouldFireWeapon` (`FireWeaponCollide.cpp:71-88`).
/// `m_everFired` is never set in C++ — pass `false` to match.
pub fn host_should_fire_weapon_collide(
    status_bits: u64,
    spec: &HostFireWeaponCollideSpec,
    ever_fired: bool,
) -> bool {
    if spec.required_status != 0 && (status_bits & spec.required_status) != spec.required_status {
        return false;
    }
    if spec.forbidden_status != 0 && (status_bits & spec.forbidden_status) != 0 {
        return false;
    }
    if ever_fired && spec.fire_once {
        return false;
    }
    true
}

/// Resolve a live-host FireWeaponCollide module.
///
/// C++ INI: `CollideWeapon` + optional `RequiredStatus` / `ForbiddenStatus` /
/// `FireOnce`. Only objects that author FireWeaponCollide shoot — a burning
/// tank without the module must not invent Flame damage. Host objects are not
/// in crate `OBJECT_REGISTRY`, so this is the live path used by
/// [`super::collide_modules`].
pub fn host_fire_weapon_collide_spec(obj: &Object) -> Option<HostFireWeaponCollideSpec> {
    if let Some(spec) = fire_weapon_collide_spec_from_definition(&obj.template_name) {
        return Some(spec);
    }
    // Residual leftover firestorm / fire-field objects spawned by name when
    // the asset catalog has not yet bound the module (host_create_object_die).
    let n = obj.template_name.to_ascii_lowercase();
    let leftover = n.contains("firestorm")
        || n.contains("firefield")
        || n.contains("firewallsegment")
        || n.contains("fireweaponcollide");
    if !leftover {
        return None;
    }
    let weapon_name = obj
        .thing
        .template
        .primary_weapon_name
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "FirestormSmall".to_string());
    Some(HostFireWeaponCollideSpec {
        weapon_name,
        fire_once: false,
        required_status: 0,
        forbidden_status: 0,
    })
}

fn fire_weapon_collide_spec_from_definition(
    template_name: &str,
) -> Option<HostFireWeaponCollideSpec> {
    use crate::game_logic::host_status_bits_upgrade::object_status_bit;
    let manager = crate::assets::get_asset_manager()?;
    let guard = manager.lock().ok()?;
    let definition = guard.get_object_definition(template_name)?;
    let module = definition
        .behavior_modules
        .iter()
        .find(|module| module.class_name.eq_ignore_ascii_case("FireWeaponCollide"))?;
    let weapon_name = module
        .attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("CollideWeapon"))
        .map(|(_, value)| value.trim().to_string())
        .filter(|name| !name.is_empty())?;
    let fire_once = module
        .attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("FireOnce"))
        .is_some_and(|(_, value)| parse_ini_bool(value));
    let required_status = parse_status_mask(
        module
            .attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("RequiredStatus"))
            .map(|(_, value)| value.as_str()),
        object_status_bit,
    );
    let forbidden_status = parse_status_mask(
        module
            .attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("ForbiddenStatus"))
            .map(|(_, value)| value.as_str()),
        object_status_bit,
    );
    Some(HostFireWeaponCollideSpec {
        weapon_name,
        fire_once,
        required_status,
        forbidden_status,
    })
}

fn parse_ini_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "yes" | "true" | "1" | "on"
    )
}

fn parse_status_mask(raw: Option<&str>, bit_of: fn(&str) -> Option<u32>) -> u64 {
    let Some(raw) = raw else {
        return 0;
    };
    raw.split(|c: char| c.is_ascii_whitespace() || matches!(c, ',' | '|'))
        .filter_map(|token| bit_of(token.trim()))
        .fold(0u64, |acc, idx| acc | (1u64 << idx))
}

/// Primary damage for a collide weapon (WeaponStore residual; fail-closed 0).
pub fn host_fire_weapon_collide_damage(weapon_name: &str) -> f32 {
    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    gamelogic::weapon::with_weapon_store(|store| {
        store
            .find_weapon_template(weapon_name)
            .map(|template| template.primary_damage.max(0.0))
            .unwrap_or(0.0)
    })
    .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_fire_matches_cpp_status_gates() {
        // C++ FireWeaponCollide.cpp:71-88 — testForAll / testForAny / fireOnce.
        let spec = HostFireWeaponCollideSpec {
            weapon_name: "FirestormSmall".into(),
            fire_once: false,
            required_status: 0b0010,
            forbidden_status: 0b1000,
        };
        assert!(!host_should_fire_weapon_collide(0, &spec, false));
        assert!(host_should_fire_weapon_collide(0b0010, &spec, false));
        assert!(!host_should_fire_weapon_collide(0b1010, &spec, false));
        let once = HostFireWeaponCollideSpec {
            fire_once: true,
            ..spec
        };
        // C++ never sets m_everFired — ever_fired=false still fires.
        assert!(host_should_fire_weapon_collide(0b0010, &once, false));
        assert!(!host_should_fire_weapon_collide(0b0010, &once, true));
    }

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
