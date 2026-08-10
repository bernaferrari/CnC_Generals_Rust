//! Shared UnitAIUpdate helpers: rappel state, container kills, xfer codecs.

#![allow(unused_imports)]

use super::imports::*;
use super::registry::dual_world_registry_unavailable;

#[derive(Debug, Clone)]
pub(super) struct RappelState {
    pub(super) rappel_rate: Real,
    pub(super) dest_z: Real,
    pub(super) target_is_bldg: bool,
    pub(super) target_id: Option<ObjectID>,
}

pub(super) fn find_enemy_in_container(
    killer_id: ObjectID,
    container_id: ObjectID,
) -> Option<ObjectID> {
    // Wave 258: empty dual-world → None.

    if dual_world_registry_unavailable() {
        return None;
    }

    let contained_ids = crate::object::registry::OBJECT_REGISTRY
        .with_object(container_id, |guard| {
            let contain = guard.get_contain()?;
            let contain_guard = contain.lock().ok()?;
            Some(contain_guard.get_contained_objects().to_vec())
        })
        .flatten()?;

    for id in contained_ids {
        let is_enemy = crate::object::registry::OBJECT_REGISTRY
            .with_object(killer_id, |killer_guard| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(id, |enemy_guard| {
                        if enemy_guard.is_effectively_dead() {
                            return false;
                        }
                        killer_guard.relationship_to(enemy_guard) == Relationship::Enemies
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if is_enemy {
            return Some(id);
        }
    }
    None
}

pub(super) fn kill_enemies_in_container(
    killer_id: ObjectID,
    container_id: ObjectID,
    max_to_kill: i32,
) -> i32 {
    // Wave 258: empty dual-world → zero.

    if dual_world_registry_unavailable() {
        return 0;
    }

    let mut num_killed = 0;
    while num_killed < max_to_kill {
        let Some(enemy_id) = find_enemy_in_container(killer_id, container_id) else {
            break;
        };

        let Some(()) =
            crate::object::registry::OBJECT_REGISTRY.with_object_mut(enemy_id, |enemy_guard| {
                if let Some(contained_by_id) = enemy_guard.get_contained_by() {
                    if let Some(contain) = crate::object::registry::OBJECT_REGISTRY
                        .with_object(contained_by_id, |container_guard| {
                            container_guard.get_contain()
                        })
                        .flatten()
                    {
                        if let Ok(mut contain_guard) = contain.lock() {
                            let _ = contain_guard.release_object(enemy_id);
                        }
                    }
                }

                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                    killer_id,
                    |killer_guard| {
                        killer_guard.score_the_kill(enemy_guard);
                    },
                );
                enemy_guard.kill(None, None);
            })
        else {
            break;
        };
        num_killed += 1;
    }

    num_killed
}

pub(super) fn to_locomotor_body_damage_type(
    value: crate::common::BodyDamageType,
) -> BodyDamageType {
    match value {
        crate::common::BodyDamageType::Pristine => BodyDamageType::Pristine,
        crate::common::BodyDamageType::Damaged => BodyDamageType::Damaged,
        crate::common::BodyDamageType::ReallyDamaged => BodyDamageType::ReallyDamaged,
        crate::common::BodyDamageType::Rubble => BodyDamageType::Rubble,
    }
}

pub(super) fn xfer_unit_coord3d(xfer: &mut dyn Xfer, coord: &mut Coord3D) -> Result<(), String> {
    xfer.xfer_real(&mut coord.x).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut coord.y).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut coord.z).map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn xfer_unit_icoord2d(xfer: &mut dyn Xfer, coord: &mut ICoord2D) -> Result<(), String> {
    xfer.xfer_int(&mut coord.x).map_err(|e| e.to_string())?;
    xfer.xfer_int(&mut coord.y).map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn guard_target_type_from_u32(value: u32) -> Result<GuardTargetType, String> {
    match value {
        0 => Ok(GuardTargetType::Location),
        1 => Ok(GuardTargetType::Object),
        2 => Ok(GuardTargetType::Area),
        3 => Ok(GuardTargetType::None_),
        _ => Err(format!("Invalid AIUpdate guard target type {value}")),
    }
}

pub(super) fn xfer_guard_target_type(
    xfer: &mut dyn Xfer,
    guard_target_type: &mut GuardTargetType,
) -> Result<(), String> {
    let mut value = *guard_target_type as u32;
    xfer.xfer_unsigned_int(&mut value)
        .map_err(|e| e.to_string())?;
    *guard_target_type = guard_target_type_from_u32(value)?;
    Ok(())
}

pub(super) fn locomotor_set_type_from_i32(value: i32) -> Result<LocomotorSetType, String> {
    match value {
        -1 => Ok(LocomotorSetType::Invalid),
        0 => Ok(LocomotorSetType::Normal),
        1 => Ok(LocomotorSetType::NormalUpgraded),
        2 => Ok(LocomotorSetType::Freefall),
        3 => Ok(LocomotorSetType::Wander),
        4 => Ok(LocomotorSetType::Panic),
        5 => Ok(LocomotorSetType::Taxiing),
        6 => Ok(LocomotorSetType::Supersonic),
        7 => Ok(LocomotorSetType::Sluggish),
        _ => Err(format!("Invalid AIUpdate locomotor set type {value}")),
    }
}
