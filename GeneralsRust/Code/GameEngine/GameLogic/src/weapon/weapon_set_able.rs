//! C++ `WeaponSet` targeting: getVictimAntiMask, getAbleToAttackSpecificObject,
//! getAbleToUseWeaponAgainstTarget (WeaponSet.cpp:347-760).

use crate::attack::{AbleToAttackType, CanAttackResult, is_forced_attack};
use crate::common::{CommandSourceType, Coord3D, KindOf, ObjectID, Relationship};
use crate::object::ObjectScriptStatusBit;
use crate::player::player_list;

use super::super::helpers::dual_world_registry_unavailable;
use super::super::masks_enums::{WeaponAntiMask, WeaponSlotType};
use super::WeaponSet;
use crate::damage::DamageType;

/// Exclusive victim anti-mask. Do not reuse `Object::get_anti_mask`.
pub fn get_victim_anti_mask(victim_id: ObjectID) -> u32 {
    crate::object::registry::OBJECT_REGISTRY
        .with_object(victim_id, |victim| {
            if victim.is_kind_of(KindOf::SmallMissile) {
                WeaponAntiMask::SMALL_MISSILE
            } else if victim.is_kind_of(KindOf::BallisticMissile) {
                WeaponAntiMask::BALLISTIC_MISSILE
            } else if victim.is_kind_of(KindOf::Projectile) {
                WeaponAntiMask::PROJECTILE
            } else if victim.is_kind_of(KindOf::Mine) || victim.is_kind_of(KindOf::Demotrap) {
                WeaponAntiMask::MINE | WeaponAntiMask::GROUND
            } else if victim.is_airborne_target() {
                if victim.is_kind_of(KindOf::Vehicle) {
                    WeaponAntiMask::AIRBORNE_VEHICLE
                } else if victim.is_kind_of(KindOf::Infantry) {
                    WeaponAntiMask::AIRBORNE_INFANTRY
                } else if victim.is_kind_of(KindOf::Parachute) {
                    WeaponAntiMask::PARACHUTE
                } else {
                    0
                }
            } else {
                WeaponAntiMask::GROUND
            }
        })
        .unwrap_or(WeaponAntiMask::GROUND)
}

impl WeaponSet {
    pub fn get_able_to_attack_specific_object(
        &self,
        attack_type: AbleToAttackType,
        source_obj: ObjectID,
        target_obj: ObjectID,
        command_source: CommandSourceType,
        specific_slot: Option<WeaponSlotType>,
    ) -> CanAttackResult {
        if dual_world_registry_unavailable() {
            return CanAttackResult::NotPossible;
        }
        if source_obj == 0 || target_obj == 0 || source_obj == target_obj {
            return CanAttackResult::NotPossible;
        }

        let Some(legality) =
            crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |source| {
                crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |victim| {
                    attack_object_legality(source, victim, attack_type, command_source)
                })
            })
        else {
            return CanAttackResult::NotPossible;
        };
        let Some(legality) = legality else {
            return CanAttackResult::NotPossible;
        };
        if legality != CanAttackResult::Possible {
            return legality;
        }

        let victim_pos =
            crate::object::registry::OBJECT_REGISTRY.with_object(target_obj, |v| *v.get_position());
        self.get_able_to_use_weapon_against_target(
            attack_type,
            source_obj,
            Some(target_obj),
            victim_pos.as_ref(),
            command_source,
            specific_slot,
        )
    }

    pub fn get_able_to_use_weapon_against_target(
        &self,
        attack_type: AbleToAttackType,
        source_obj: ObjectID,
        target_obj: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        command_source: CommandSourceType,
        specific_slot: Option<WeaponSlotType>,
    ) -> CanAttackResult {
        if dual_world_registry_unavailable() {
            return CanAttackResult::NotPossible;
        }

        let (target_anti_mask, resolved_pos) = if let Some(target_id) = target_obj {
            let pos = crate::object::registry::OBJECT_REGISTRY
                .with_object(target_id, |obj| *obj.get_position())
                .or_else(|| target_pos.copied());
            (get_victim_anti_mask(target_id), pos)
        } else {
            (WeaponAntiMask::GROUND, target_pos.copied())
        };
        let Some(resolved_pos) = resolved_pos else {
            return CanAttackResult::NotPossible;
        };

        let contained_by = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj, |src| src.get_contained_by())
            .flatten();

        let mut within_attack_range = false;
        let mut has_a_weapon = false;
        let mut has_a_weapon_in_range = false;

        if let Some(weapon) = self.get_weapon_in_slot(self.current_weapon) {
            has_a_weapon = true;
            if (self.total_anti_mask & target_anti_mask) != 0 {
                let garrison_goal = contained_by.and_then(|container_id| {
                    garrison_fire_goal(container_id, source_obj, &resolved_pos)
                });
                within_attack_range = if let Some(goal) = garrison_goal {
                    weapon.is_source_object_with_goal_position_within_attack_range(
                        source_obj,
                        &goal,
                        target_obj,
                        Some(&resolved_pos),
                    )
                } else {
                    weapon.is_within_attack_range(source_obj, target_obj, Some(&resolved_pos))
                };
                if within_attack_range {
                    has_a_weapon_in_range = true;
                }
            }
        }

        let immobile_or_spawn_or_contained = crate::object::registry::OBJECT_REGISTRY
            .with_object(source_obj, |src| {
                src.is_kind_of(KindOf::Immobile)
                    || src.is_kind_of(KindOf::SpawnsAreTheWeapons)
                    || src.get_contained_by().is_some()
            })
            .unwrap_or(false);
        if immobile_or_spawn_or_contained
            && has_a_weapon
            && !has_a_weapon_in_range
            && attack_type != AbleToAttackType::TunnelNetworkGuard
        {
            return CanAttackResult::InvalidShot;
        }

        let mut ok_result = if within_attack_range {
            CanAttackResult::Possible
        } else {
            CanAttackResult::PossibleAfterMoving
        };

        if self.has_any_damage_weapon() {
            if (self.total_anti_mask & target_anti_mask) == 0 {
                return CanAttackResult::InvalidShot;
            }
            if target_obj.is_none() {
                return ok_result;
            }
            let victim_id = target_obj.unwrap();
            if !self.is_any_within_target_pitch(source_obj, victim_id) {
                return CanAttackResult::InvalidShot;
            }

            let (first, last) = if self.is_current_weapon_locked() {
                let cur = self.current_weapon as usize;
                (cur, cur)
            } else if let Some(slot) = specific_slot {
                let idx = slot as usize;
                (idx, idx)
            } else {
                (2, 0)
            };

            let mut i = first as i32;
            let last_i = last as i32;
            while i >= last_i {
                let slot = match i {
                    0 => WeaponSlotType::Primary,
                    1 => WeaponSlotType::Secondary,
                    _ => WeaponSlotType::Tertiary,
                };
                if let Some(weapon) = self.get_weapon_in_slot(slot) {
                    let damage =
                        weapon.estimate_weapon_damage(source_obj, target_obj, Some(&resolved_pos));
                    if damage > 0.0 {
                        if weapon.get_damage_type() == DamageType::KillPilot
                            && crate::object::registry::OBJECT_REGISTRY
                                .with_object(source_obj, |src| src.is_kind_of(KindOf::Hero))
                                .unwrap_or(false)
                            && self.current_weapon == WeaponSlotType::Primary
                            && specific_slot.is_none()
                        {
                            i -= 1;
                            continue;
                        }
                        return ok_result;
                    }
                }
                i -= 1;
            }
        }

        if let Some(passenger_result) = passenger_fire_result(
            source_obj,
            attack_type,
            target_obj,
            &resolved_pos,
            command_source,
        ) {
            return passenger_result;
        }

        if let Some(slave_result) = spawn_slave_result(
            source_obj,
            attack_type,
            target_obj,
            &resolved_pos,
            command_source,
        ) {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(source_obj, |src| {
                    src.is_kind_of(KindOf::Immobile)
                        && src.is_kind_of(KindOf::SpawnsAreTheWeapons)
                        && ok_result == CanAttackResult::PossibleAfterMoving
                })
                .unwrap_or(false)
            {
                ok_result = CanAttackResult::Possible;
            }
            if matches!(
                slave_result,
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
            ) {
                return ok_result;
            }
        }

        CanAttackResult::InvalidShot
    }
}

fn attack_object_legality(
    source: &crate::object::Object,
    victim: &crate::object::Object,
    attack_type: AbleToAttackType,
    command_source: CommandSourceType,
) -> CanAttackResult {
    use crate::common::ObjectStatusTypes;

    if source.is_effectively_dead()
        || victim.is_effectively_dead()
        || source.is_destroyed()
        || victim.is_destroyed()
        || source.get_id() == victim.get_id()
    {
        return CanAttackResult::NotPossible;
    }

    let same_owner_force = source.get_controlling_player_id() == victim.get_controlling_player_id()
        && is_forced_attack(attack_type);

    if victim.test_status(ObjectStatusTypes::Masked) {
        return CanAttackResult::NotPossible;
    }
    if victim.is_kind_of(KindOf::Unattackable) {
        return CanAttackResult::NotPossible;
    }
    if victim.test_status(ObjectStatusTypes::NoAttackFromAi)
        && command_source == CommandSourceType::FromAi
    {
        return CanAttackResult::NotPossible;
    }

    let mut allow_stealth = true;
    if source.test_status(ObjectStatusTypes::IgnoringStealth) || same_owner_force {
        allow_stealth = false;
    }
    if is_forced_attack(attack_type)
        && victim.is_kind_of(KindOf::Disguiser)
        && victim.test_status(ObjectStatusTypes::Disguised)
    {
        allow_stealth = false;
    }

    if allow_stealth
        && victim.test_status(ObjectStatusTypes::Stealthed)
        && !victim.test_status(ObjectStatusTypes::Detected)
    {
        if !victim.is_kind_of(KindOf::Disguiser) {
            return CanAttackResult::NotPossible;
        }
        if disguised_as_non_enemy(source, victim) {
            return CanAttackResult::NotPossible;
        }
    }

    let r = source.relationship_to(victim);
    if r != Relationship::Enemies
        && !is_forced_attack(attack_type)
        && !(victim.is_kind_of(KindOf::Mine) && r != Relationship::Allies)
    {
        if command_source == CommandSourceType::FromPlayer
            && (!victim.test_script_status_bit(ObjectScriptStatusBit::ScriptTargetable)
                || r == Relationship::Allies)
        {
            return CanAttackResult::NotPossible;
        }
    }

    if let Some(container_id) = victim.get_contained_by() {
        if container_encloses(container_id, victim) {
            return CanAttackResult::NotPossible;
        }
    }

    if !is_forced_attack(attack_type) {
        if object_apparent_controller_blocks_player(source, victim, r, command_source) {
            return CanAttackResult::NotPossible;
        }
    }

    CanAttackResult::Possible
}

fn disguised_as_non_enemy(source: &crate::object::Object, victim: &crate::object::Object) -> bool {
    let Some(stealth) = victim.get_stealth() else {
        return false;
    };
    let Ok(stealth_guard) = stealth.lock() else {
        return false;
    };
    if !stealth_guard.is_disguised() {
        return false;
    }
    drop(stealth_guard);

    let mut disguised_index = None;
    for behavior in victim.get_behavior_modules() {
        let Ok(mut guard) = behavior.lock() else {
            continue;
        };
        if let Some(idx) = guard.get_disguised_player_index() {
            disguised_index = Some(idx);
            break;
        }
    }
    let Some(idx) = disguised_index else {
        return false;
    };
    let Some(our_player) = source.get_controlling_player() else {
        return false;
    };
    let Ok(our_guard) = our_player.read() else {
        return false;
    };
    let Ok(list) = player_list().read() else {
        return false;
    };
    let Some(other_arc) = list.get_player(idx) else {
        return false;
    };
    let Ok(other_guard) = other_arc.read() else {
        return false;
    };
    let Some(other_team) = other_guard.get_default_team() else {
        return false;
    };
    let Ok(team_guard) = other_team.read() else {
        return false;
    };
    our_guard.get_relationship_with_team(&team_guard) != Relationship::Enemies
}

fn container_encloses(container_id: ObjectID, victim: &crate::object::Object) -> bool {
    crate::object::registry::OBJECT_REGISTRY
        .with_object(container_id, |container| {
            let Some(contain) = container.get_contain() else {
                return false;
            };
            let Ok(contain_guard) = contain.try_lock() else {
                return false;
            };
            contain_guard.is_enclosing_container_for(victim)
        })
        .unwrap_or(false)
}

/// C++ `WeaponSet.cpp:552-571` — non-force FROM_PLAYER attack is
/// `NOT_POSSIBLE` when the victim contain's apparent controller is not
/// ENEMIES to the source team, unless `SCRIPT_TARGETABLE` and not ALLIES.
#[inline]
pub fn apparent_controller_blocks_player(
    apparent_controller_present: bool,
    source_team_to_apparent_default: Relationship,
    from_player: bool,
    script_targetable: bool,
    source_to_victim: Relationship,
) -> bool {
    if !apparent_controller_present {
        return false;
    }
    if source_team_to_apparent_default == Relationship::Enemies {
        return false;
    }
    from_player && (!script_targetable || source_to_victim == Relationship::Allies)
}

fn object_apparent_controller_blocks_player(
    source: &crate::object::Object,
    victim: &crate::object::Object,
    r: Relationship,
    command_source: CommandSourceType,
) -> bool {
    let Some(contain) = victim.get_contain() else {
        return false;
    };
    let Ok(contain_guard) = contain.try_lock() else {
        return false;
    };
    let Some(source_player) = source.get_controlling_player() else {
        return false;
    };
    let Ok(source_player_guard) = source_player.read() else {
        return false;
    };
    let Some(apparent) = contain_guard.get_apparent_controlling_player(Some(&source_player_guard))
    else {
        return false;
    };
    let Ok(apparent_guard) = apparent.read() else {
        return false;
    };
    let Some(apparent_team) = apparent_guard.get_default_team() else {
        return false;
    };
    let Ok(apparent_team_guard) = apparent_team.read() else {
        return false;
    };
    let Some(source_team) = source.get_team() else {
        return false;
    };
    let Ok(source_team_guard) = source_team.read() else {
        return false;
    };
    apparent_controller_blocks_player(
        true,
        source_team_guard.get_relationship(&apparent_team_guard),
        command_source == CommandSourceType::FromPlayer,
        victim.test_script_status_bit(ObjectScriptStatusBit::ScriptTargetable),
        r,
    )
}

fn garrison_fire_goal(
    container_id: ObjectID,
    source_id: ObjectID,
    target_pos: &Coord3D,
) -> Option<Coord3D> {
    crate::object::registry::OBJECT_REGISTRY
        .with_object(container_id, |container| {
            let Some(contain) = container.get_contain() else {
                return None;
            };
            let Ok(contain_guard) = contain.try_lock() else {
                return None;
            };
            if !contain_guard.is_garrisonable() {
                return None;
            }
            let enclosing = crate::object::registry::OBJECT_REGISTRY
                .with_object(source_id, |source| {
                    contain_guard.is_enclosing_container_for(source)
                })
                .unwrap_or(true);
            if !enclosing {
                return None;
            }
            // C++ WeaponSet.cpp:636-641 — fire-goal is calcBestGarrisonPosition,
            // not the container origin.
            let mut goal = *container.get_position();
            if contain_guard.calc_best_garrison_position(&mut goal, target_pos) {
                Some(goal)
            } else {
                None
            }
        })
        .flatten()
}
fn passenger_fire_result(
    source_obj: ObjectID,
    attack_type: AbleToAttackType,
    target_obj: Option<ObjectID>,
    pos: &Coord3D,
    command_source: CommandSourceType,
) -> Option<CanAttackResult> {
    let members = crate::object::registry::OBJECT_REGISTRY.with_object(source_obj, |src| {
        let contain = src.get_contain()?;
        let contain_guard = contain.try_lock().ok()?;
        if !contain_guard.is_passenger_allowed_to_fire(None) {
            return None;
        }
        Some(contain_guard.get_contained_objects().to_vec())
    })??;

    for member_id in members {
        let result = crate::object::registry::OBJECT_REGISTRY.with_object(member_id, |member| {
            if !member.is_able_to_attack() {
                return CanAttackResult::NotPossible;
            }
            member.weapon_set.get_able_to_use_weapon_against_target(
                attack_type,
                member_id,
                target_obj,
                Some(pos),
                command_source,
                None,
            )
        });
        if matches!(
            result,
            Some(CanAttackResult::Possible) | Some(CanAttackResult::PossibleAfterMoving)
        ) {
            return result;
        }
    }
    None
}

fn spawn_slave_result(
    source_obj: ObjectID,
    attack_type: AbleToAttackType,
    target_obj: Option<ObjectID>,
    pos: &Coord3D,
    command_source: CommandSourceType,
) -> Option<CanAttackResult> {
    let source_arc = crate::helpers::TheGameLogic::find_object_by_id(source_obj)?;
    let source_guard = source_arc.read().ok()?;
    let spawn_mod = source_guard.get_spawn_behavior_interface_public()?;
    drop(source_guard);
    let mut spawn_guard = spawn_mod.lock().ok()?;

    // C++ WeaponSet.cpp:741-744 + SpawnBehavior.cpp:424-432: victim may be
    // NULL for ground attacks; slaves still run getAbleToUseWeaponAgainstTarget.
    let Some(target_id) = target_obj else {
        let spawn = spawn_guard.get_spawn_behavior_interface()?;
        let ids: Vec<ObjectID> = (0..spawn.get_spawn_count())
            .filter_map(|i| spawn.get_spawn_object(i))
            .collect();
        drop(spawn_guard);
        return spawn_slave_ground_result(ids, attack_type, pos, command_source);
    };

    let victim_arc = crate::helpers::TheGameLogic::find_object_by_id(target_id)?;
    let spawn = spawn_guard.get_spawn_behavior_full_interface()?;
    let victim_guard = victim_arc.read().ok()?;
    Some(spawn.get_can_any_slaves_use_weapon_against_target(
        attack_type,
        &victim_guard,
        pos,
        command_source,
    ))
}

fn spawn_slave_ground_result(
    spawn_ids: Vec<ObjectID>,
    attack_type: AbleToAttackType,
    pos: &Coord3D,
    command_source: CommandSourceType,
) -> Option<CanAttackResult> {
    let mut invalid_shot = false;
    for spawn_id in spawn_ids {
        let result = crate::object::registry::OBJECT_REGISTRY.with_object(spawn_id, |member| {
            member.weapon_set.get_able_to_use_weapon_against_target(
                attack_type,
                spawn_id,
                None,
                Some(pos),
                command_source,
                None,
            )
        });
        match result {
            Some(CanAttackResult::Possible) | Some(CanAttackResult::PossibleAfterMoving) => {
                return result;
            }
            Some(CanAttackResult::InvalidShot) => invalid_shot = true,
            _ => {}
        }
    }
    Some(if invalid_shot {
        CanAttackResult::InvalidShot
    } else {
        CanAttackResult::NotPossible
    })
}

#[cfg(test)]
mod apparent_controller_blocks_player_tests {
    use super::*;

    #[test]
    fn from_player_blocks_non_enemy_apparent_without_script_targetable() {
        assert!(apparent_controller_blocks_player(
            true,
            Relationship::Neutral,
            true,
            false,
            Relationship::Enemies,
        ));
        assert!(apparent_controller_blocks_player(
            true,
            Relationship::Allies,
            true,
            false,
            Relationship::Enemies,
        ));
        assert!(!apparent_controller_blocks_player(
            true,
            Relationship::Enemies,
            true,
            false,
            Relationship::Enemies,
        ));
        assert!(!apparent_controller_blocks_player(
            false,
            Relationship::Neutral,
            true,
            false,
            Relationship::Enemies,
        ));
        assert!(!apparent_controller_blocks_player(
            true,
            Relationship::Neutral,
            false,
            false,
            Relationship::Enemies,
        ));
        assert!(!apparent_controller_blocks_player(
            true,
            Relationship::Neutral,
            true,
            true,
            Relationship::Enemies,
        ));
        assert!(apparent_controller_blocks_player(
            true,
            Relationship::Neutral,
            true,
            true,
            Relationship::Allies,
        ));
    }
}
