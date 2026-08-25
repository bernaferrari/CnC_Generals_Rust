use super::super::super::*;

pub(super) const MAX_GARRISON_FIRE_POINTS: usize = 40;
/// C++ GameData `WeaponBonus = GARRISONED RANGE 133%`.
/// GarrisonContain / HelixContain `onContaining` set `WEAPONBONUSCONDITION_GARRISONED`.
pub(super) const GARRISONED_WEAPON_RANGE_MULT: f32 = 1.33;
/// C++ `HelixContain::redeployOccupants` (`HelixContain.cpp:115`) `firePos.z += 8`.
/// Leftover `helix_contain.rs` already matches. Host Y-up maps C++ Z → Y.
pub(super) const HELIX_OCCUPANT_FIRE_HEIGHT: f32 = 8.0;

pub(super) fn cpp_bone_to_host_local(bone: gamelogic::common::Coord3D) -> glam::Vec3 {
    // C++ Z-up (x, y, z) -> host Y-up (x, z, y).
    glam::Vec3::new(bone.x, bone.z, bone.y)
}

pub(super) fn rotate_yaw_host(origin: glam::Vec3, yaw: f32, local: glam::Vec3) -> glam::Vec3 {
    let (sin, cos) = yaw.sin_cos();
    glam::Vec3::new(
        origin.x + local.x * cos - local.z * sin,
        origin.y + local.y,
        origin.z + local.x * sin + local.z * cos,
    )
}

pub(super) fn load_prefix_bones_for_model(
    container: &Object,
    model: &str,
    prefix: &str,
    max: usize,
) -> Vec<glam::Vec3> {
    let scale = container.thing.template.asset_scale;
    let pos = container.get_position();
    let yaw = container.get_orientation();
    let mut out = Vec::new();
    for i in 1..=max {
        let name = format!("{prefix}{i:02}");
        let Some(local) =
            gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, &name)
        else {
            break;
        };
        out.push(rotate_yaw_host(pos, yaw, cpp_bone_to_host_local(local)));
    }
    out
}

pub(super) fn load_prefix_bones_world(
    container: &Object,
    prefix: &str,
    max: usize,
) -> Vec<glam::Vec3> {
    load_prefix_bones_for_model(
        container,
        container.thing.template.get_model_name(),
        prefix,
        max,
    )
}

pub(super) fn named_bone_world(container: &Object, name: &str) -> Option<glam::Vec3> {
    let model = container.thing.template.get_model_name();
    let scale = container.thing.template.asset_scale;
    let pos = container.get_position();
    let yaw = container.get_orientation();
    let local = gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, name)?;
    Some(rotate_yaw_host(pos, yaw, cpp_bone_to_host_local(local)))
}

pub(super) fn garrison_condition_index(
    state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
) -> u8 {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    match state {
        HostBodyDamageType::Damaged => 1,
        HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => 2,
        _ => 0,
    }
}

pub(super) fn garrison_points_for_condition<'a>(
    bd: &'a crate::game_logic::BuildingData,
    idx: u8,
) -> &'a [glam::Vec3] {
    match idx {
        1 if !bd.garrison_fire_points_damaged.is_empty() => &bd.garrison_fire_points_damaged,
        2 if !bd.garrison_fire_points_really_damaged.is_empty() => {
            &bd.garrison_fire_points_really_damaged
        }
        _ => &bd.garrison_fire_points,
    }
}

pub(super) fn load_garrison_condition_bone_sets(
    container: &Object,
) -> (Vec<glam::Vec3>, Vec<glam::Vec3>, Vec<glam::Vec3>) {
    let base = container.thing.template.get_model_name();
    let pristine =
        load_prefix_bones_for_model(container, base, "FIREPOINT", MAX_GARRISON_FIRE_POINTS);
    let dmg_key = crate::assets::mesh_asset_resolve::model_key_with_body_damage(base, 1, false);
    let rd_key = crate::assets::mesh_asset_resolve::model_key_with_body_damage(base, 2, false);
    let damaged = if dmg_key != base {
        load_prefix_bones_for_model(container, &dmg_key, "FIREPOINT", MAX_GARRISON_FIRE_POINTS)
    } else {
        Vec::new()
    };
    let really = if rd_key != base && rd_key != dmg_key {
        load_prefix_bones_for_model(container, &rd_key, "FIREPOINT", MAX_GARRISON_FIRE_POINTS)
    } else {
        Vec::new()
    };
    (pristine, damaged, really)
}

pub(super) fn transport_passenger_fire_origin(
    container: &Object,
    passenger_index: usize,
) -> glam::Vec3 {
    // C++ HelixContain::redeployOccupants (HelixContain.cpp:112-123): every rider
    // setPosition at Helix origin z += 8, not sequential FIREPOINT bones.
    // Humvee/Chinook/Bus keep OpenContain FIREPOINT (hq-ncs1d).
    if container.is_helix_transport {
        let mut fire_pos = container.get_position();
        fire_pos.y += HELIX_OCCUPANT_FIRE_HEIGHT;
        return fire_pos;
    }
    let bones = load_prefix_bones_world(container, "FIREPOINT", MAX_GARRISON_FIRE_POINTS);
    if bones.is_empty() {
        container.get_position()
    } else {
        bones[passenger_index % bones.len()]
    }
}

pub(super) fn open_contain_exit_path(
    container: &Object,
    which_path: u8,
    number_exits: i32,
) -> (glam::Vec3, glam::Vec3, u8) {
    let origin = container.get_position();
    let yaw = container.get_orientation();
    let geom = container.thing.template.geometry_info;
    let major = if geom.authored {
        geom.major_radius.max(8.0)
    } else {
        20.0
    };
    let fallback_end = {
        let (sin, cos) = yaw.sin_cos();
        glam::Vec3::new(origin.x + major * cos, origin.y, origin.z + major * sin)
    };
    // C++ OpenContain::exitObjectViaDoor: numberExits<=0 skips the door walk.
    if number_exits <= 0 {
        return (origin, origin, 1);
    }
    // C++ numberExits>1 uses ExitStart0N/ExitEnd0N cycling m_whichExitPath.
    if number_exits > 1 {
        let n = number_exits as u8;
        let idx = if which_path == 0 {
            1
        } else {
            ((which_path - 1) % n) + 1
        };
        let start = named_bone_world(container, &format!("ExitStart{idx:02}")).unwrap_or(origin);
        let end = named_bone_world(container, &format!("ExitEnd{idx:02}")).unwrap_or(fallback_end);
        let next = (idx % n) + 1;
        return (start, end, next);
    }
    let start = named_bone_world(container, "ExitStart").unwrap_or(origin);
    let end = named_bone_world(container, "ExitEnd").unwrap_or(fallback_end);
    (start, end, 1)
}

pub(super) fn closest_free_garrison_point(
    points: &[glam::Vec3],
    occupied: &[Option<ObjectId>],
    occupant_id: ObjectId,
    target: glam::Vec3,
    fallback: glam::Vec3,
) -> (usize, glam::Vec3) {
    if points.is_empty() {
        return (0, fallback);
    }
    let mut best_i = 0;
    let mut best_d = f32::MAX;
    let mut best = points[0];
    for (i, p) in points.iter().enumerate() {
        let taken = occupied.get(i).and_then(|id| *id);
        if taken.is_some() && taken != Some(occupant_id) {
            continue;
        }
        let d = (*p - target).length_squared();
        if d < best_d {
            best_d = d;
            best_i = i;
            best = *p;
        }
    }
    (best_i, best)
}

pub(super) fn garrison_occupant_fire_point(
    container: &Object,
    occupant_id: ObjectId,
    target_pos: glam::Vec3,
) -> (usize, glam::Vec3) {
    let fallback = container.get_position();
    let Some(bd) = container.building_data.as_ref() else {
        return (0, fallback);
    };
    // C++ WeaponSet.cpp:632-633 / GarrisonContain.cpp:662-663:
    // non-enclosing Fire Base does not use FIREPOINTs. Occupants fire from
    // their pre-assigned STATION bone (not the building center).
    if !container.is_enclosing_garrison_container() {
        return station_occupant_fire_point(bd, occupant_id, fallback);
    }
    let idx = garrison_condition_index(container.body_damage_state);
    closest_free_garrison_point(
        garrison_points_for_condition(bd, idx),
        &bd.garrison_point_occupant,
        occupant_id,
        target_pos,
        fallback,
    )
}

/// C++ `Weapon::getDamageType() == DAMAGE_POISON` (Toxin / Anthrax).
pub(super) fn occupant_weapon_is_poison(obj: &Object, slot: u8) -> bool {
    let Some(name) = obj.weapon_name_for_slot(slot) else {
        return false;
    };
    if crate::game_logic::host_poisoned_behavior::is_poison_damage_type(
        crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(name),
    ) {
        return true;
    }
    // Store-miss residual: same name peel the weapon seed uses for Poison.
    let n = name.to_ascii_lowercase();
    n.contains("toxin") || n.contains("anthrax") || n.contains("poison")
}

/// C++ `positionObjectsAtStationGarrisonPoints` / `pickAStationForMe`.
pub(super) fn station_occupant_fire_point(
    bd: &crate::game_logic::BuildingData,
    occupant_id: ObjectId,
    fallback: glam::Vec3,
) -> (usize, glam::Vec3) {
    if bd.garrison_station_points.is_empty() {
        return (0, fallback);
    }
    for (i, slot) in bd.garrison_point_occupant.iter().enumerate() {
        if *slot == Some(occupant_id) {
            if let Some(&pos) = bd.garrison_station_points.get(i) {
                return (i, pos);
            }
        }
    }
    for (i, pos) in bd.garrison_station_points.iter().enumerate() {
        let taken = bd.garrison_point_occupant.get(i).and_then(|id| *id);
        if taken.is_none() {
            return (i, *pos);
        }
    }
    (0, bd.garrison_station_points[0])
}

pub(super) fn apply_leftover_open_contain_door_pulse(
    obj: &mut Object,
    pulse: gamelogic::object::contain::open_contain::LeftoverOpenContainDoorPulse,
) {
    use crate::game_logic::host_enum_table_residual::{
        door_1_closing_model_bit, door_1_opening_model_bit,
    };
    if !pulse.set_opening && !pulse.set_closing {
        return;
    }
    let open_b = door_1_opening_model_bit();
    let close_b = door_1_closing_model_bit();
    if pulse.set_opening {
        obj.model_condition_bits &= !(1u128 << close_b);
        obj.model_condition_bits |= 1u128 << open_b;
    }
    if pulse.set_closing {
        obj.model_condition_bits &= !(1u128 << open_b);
        obj.model_condition_bits |= 1u128 << close_b;
    }
    obj.record_host_model_condition();
}
