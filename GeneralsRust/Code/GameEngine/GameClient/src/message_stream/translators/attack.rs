use super::*;

pub(super) fn selection_can_override_special_power_destination(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    special_power_type: u32,
) -> bool {
    for &id in selection {
        let can_override = OBJECT_REGISTRY.with_object(id, |sel_guard| {
        let is_mine = local_player
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if !is_mine || sel_guard.is_effectively_dead() {
            return false;
        }

        let mut matches_power = special_power_type == SPECIAL_POWER_INVALID;
        if !matches_power {
            for behavior_arc in sel_guard.get_behavior_modules() {
                let Ok(behavior_lock) = behavior_arc.lock() else {
                    continue;
                };
                let Some(sp_module) = behavior_lock.get_special_power_module_interface_const()
                else {
                    continue;
                };
                let Some(template) = sp_module.get_special_power_template_full() else {
                    continue;
                };
                if template.get_special_power_type() as u32 == special_power_type {
                    matches_power = true;
                }
                if matches_power {
                    break;
                }
            }
        }
        if !matches_power {
            return false;
        }

        let mut can_override = false;
        for behavior_arc in sel_guard.get_behavior_modules() {
            let Ok(mut behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            let Some(update) = behavior_lock.get_special_power_update_interface() else {
                continue;
            };
            if update.does_special_power_have_overridable_destination_active()
                || update.does_special_power_have_overridable_destination()
            {
                can_override = true;
            }
            if can_override {
                break;
            }
        }

        can_override
        }).unwrap_or(false);
        if can_override {
            return true;
        }
    }

    false
}

pub(super) fn selection_attack_result(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> CanAttackResult {
    // Wave 975/1043/1049: host empty dual-world → presentation catalog residual.
    // Attack legality uses apparent team for disguised targets.
    // Wave 1049: fail-closed on illegal target status/stealth and dead local sources.
    if dual_world_registry_unavailable() {
        let Some(target) = translator_catalog_entry(target_id) else {
            return CanAttackResult::NotPossible;
        };
        if !dual_target_status_ok(&target) {
            return CanAttackResult::NotPossible;
        }
        if target.effectively_stealthed && !translator_entry_is_local(&target) {
            return CanAttackResult::NotPossible;
        }
        // Wave 1064: FOW fogged/black non-local targets fail-closed for attack residual.
        if target.shroud_status >= 2 && !translator_entry_is_local(&target) {
            return CanAttackResult::NotPossible;
        }
        // Enemy/neutral residual → Possible; ally → NotPossible.
        let local = translator_local_team_name();
        if local.is_empty() {
            return CanAttackResult::NotPossible;
        }
        let apparent = translator_entry_apparent_team(&target);
        if apparent == local {
            return CanAttackResult::NotPossible;
        }
        let mut any_local = false;
        for &id in selection {
            if let Some(sel) = translator_catalog_entry(id) {
                // Wave 1067: under-construction local source residual fail-closed.
                // Wave 1111: masked/unselectable local source residual fail-closed.
                if translator_entry_is_local(&sel)
                    && !sel.destroyed
                    && !sel.sold
                    && !sel.disabled
                    && !sel.under_construction
                    && !sel.masked
                    && !sel.unselectable
                {
                    any_local = true;
                    break;
                }
            }
        }
        return if any_local {
            CanAttackResult::Possible
        } else {
            CanAttackResult::NotPossible
        };
    }

    let outcome = OBJECT_REGISTRY.with_object(target_id, |target_guard| {
    let mut saw_invalid_shot = false;
    let mut saw_possible_after_moving = false;

    for &id in selection {
        let step = if id == target_id {
            attack_step_for_sel(local_player, target_guard, target_guard)
        } else {
            OBJECT_REGISTRY
                .with_object(id, |sel_guard| {
                    attack_step_for_sel(local_player, sel_guard, target_guard)
                })
                .unwrap_or(CanAttackResult::NotPossible)
        };
        match step {
            CanAttackResult::Possible => return CanAttackResult::Possible,
            CanAttackResult::PossibleAfterMoving => saw_possible_after_moving = true,
            CanAttackResult::InvalidShot => saw_invalid_shot = true,
            CanAttackResult::NotPossible => {}
        }
    }

    if saw_possible_after_moving {
        CanAttackResult::PossibleAfterMoving
    } else if saw_invalid_shot {
        CanAttackResult::InvalidShot
    } else {
        CanAttackResult::NotPossible
    }
    });
    return outcome.unwrap_or(CanAttackResult::NotPossible);
}

fn attack_step_for_sel(
    local_player: Option<u32>,
    sel_guard: &gamelogic::object::Object,
    target_guard: &gamelogic::object::Object,
) -> CanAttackResult {
    let is_mine = local_player
        .and_then(|pid| {
            sel_guard
                .get_controlling_player_id()
                .map(|owner| owner == pid)
        })
        .unwrap_or(false);
    if !is_mine || !sel_guard.is_able_to_attack() {
        return CanAttackResult::NotPossible;
    }
    sel_guard.get_able_to_attack_specific_object(
        AbleToAttackType::NewTarget,
        target_guard,
        CommandSourceType::FromPlayer,
    )
}

pub(super) fn selection_force_attack_object_result(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> CanAttackResult {
    let outcome = OBJECT_REGISTRY.with_object(target_id, |target_guard| {
    let mut saw_invalid_shot = false;
    let mut saw_possible_after_moving = false;

    for &id in selection {
        let step = if id == target_id {
            let is_mine = local_player
                .and_then(|pid| {
                    target_guard
                        .get_controlling_player_id()
                        .map(|owner| owner == pid)
                })
                .unwrap_or(false);
            if !is_mine || !target_guard.is_able_to_attack() {
                CanAttackResult::NotPossible
            } else {
                force_attack_object_result_for_attacker(target_guard, target_guard)
            }
        } else {
            OBJECT_REGISTRY
                .with_object(id, |sel_guard| {
                    let is_mine = local_player
                        .and_then(|pid| {
                            sel_guard
                                .get_controlling_player_id()
                                .map(|owner| owner == pid)
                        })
                        .unwrap_or(false);
                    if !is_mine || !sel_guard.is_able_to_attack() {
                        CanAttackResult::NotPossible
                    } else {
                        force_attack_object_result_for_attacker(sel_guard, target_guard)
                    }
                })
                .unwrap_or(CanAttackResult::NotPossible)
        };
        match step {
            CanAttackResult::Possible => return CanAttackResult::Possible,
            CanAttackResult::PossibleAfterMoving => saw_possible_after_moving = true,
            CanAttackResult::InvalidShot => saw_invalid_shot = true,
            CanAttackResult::NotPossible => {}
        }
    }

    if saw_possible_after_moving {
        CanAttackResult::PossibleAfterMoving
    } else if saw_invalid_shot {
        CanAttackResult::InvalidShot
    } else {
        CanAttackResult::NotPossible
    }
    });
    outcome.unwrap_or(CanAttackResult::NotPossible)
}

pub(super) fn closest_spawn_slave_id_for_position(
    owner: &gamelogic::object::Object,
    pos: &LogicCoord3D,
) -> Option<ObjectID> {
    for module in owner.behavior_modules() {
        let closest = module.with_module(|module| {
            module
                .get_spawn_control_interface()
                .and_then(|spawn| spawn.closest_slave_id_for_position([pos.x, pos.y, pos.z]))
        });
        if closest.is_some() {
            return closest;
        }
    }

    None
}

pub(super) fn closest_contained_rider_id_for_position(
    owner: &gamelogic::object::Object,
    pos: &LogicCoord3D,
) -> Option<ObjectID> {
    let contain = owner.get_contain()?;
    let rider_ids = {
        let contain_guard = contain.lock().ok()?;
        contain_guard.get_contained_objects().to_vec()
    };

    let mut closest = None;
    let mut closest_dist_sq = f32::INFINITY;

    for rider_id in rider_ids {
        let dist_sq = OBJECT_REGISTRY.with_object(rider_id, |rider_guard| {
            if rider_guard.is_effectively_dead() {
                return None;
            }
            let rider_pos = rider_guard.get_position();
            let dx = rider_pos.x - pos.x;
            let dy = rider_pos.y - pos.y;
            Some(dx * dx + dy * dy)
        });
        let Some(Some(dist_sq)) = dist_sq else {
            continue;
        };
        if dist_sq < closest_dist_sq {
            closest_dist_sq = dist_sq;
            closest = Some(rider_id);
        }
    }

    closest
}

pub(super) fn force_attack_object_result_for_attacker(
    attacker: &gamelogic::object::Object,
    target: &gamelogic::object::Object,
) -> CanAttackResult {
    let mut result = ActionManager::get_can_attack_object(
        attacker,
        target,
        CommandSourceType::FromPlayer,
        AbleToAttackType::NewTargetForced,
    );

    if !attacker.is_kind_of(KindOf::SpawnsAreTheWeapons) {
        return result;
    }

    let target_pos = target.get_position();

    if !matches!(
        result,
        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
    ) {
        if let Some(slave_id) = closest_spawn_slave_id_for_position(attacker, target_pos) {
            let slave_result = OBJECT_REGISTRY.with_object(slave_id, |slave_guard| {
                slave_guard.get_able_to_attack_specific_object(
                    AbleToAttackType::NewTargetForced,
                    target,
                    CommandSourceType::FromPlayer,
                )
            });
            if let Some(slave_result) = slave_result {
                result = slave_result;
            }
        }
    } else if let Some(rider_id) = closest_contained_rider_id_for_position(attacker, target_pos) {
        let rider_result = OBJECT_REGISTRY.with_object(rider_id, |rider_guard| {
            rider_guard.get_able_to_attack_specific_object(
                AbleToAttackType::NewTargetForced,
                target,
                CommandSourceType::FromPlayer,
            )
        });
        if let Some(rider_result) = rider_result {
            if rider_result != CanAttackResult::NotPossible {
                return rider_result;
            }
        }
    }

    result
}

pub(super) fn force_attack_position_result_for_attacker(
    attacker: &gamelogic::object::Object,
    pos: &LogicCoord3D,
) -> CanAttackResult {
    let mut test_attacker = attacker.get_id();

    if attacker.is_kind_of(KindOf::Immobile) || attacker.is_kind_of(KindOf::SpawnsAreTheWeapons) {
        if let Some(slave_id) = closest_spawn_slave_id_for_position(attacker, pos) {
            test_attacker = slave_id;
        } else {
            let result = attacker.get_able_to_use_weapon_against_position(
                AbleToAttackType::NewTarget,
                pos,
                CommandSourceType::FromPlayer,
            );
            if result != CanAttackResult::Possible {
                if let Some(rider_id) = closest_contained_rider_id_for_position(attacker, pos) {
                    test_attacker = rider_id;
                }
            }
        }
    }

    if test_attacker == attacker.get_id() {
        return attacker.get_able_to_use_weapon_against_position(
            AbleToAttackType::NewTarget,
            pos,
            CommandSourceType::FromPlayer,
        );
    }

    OBJECT_REGISTRY
        .with_object(test_attacker, |test_guard| {
            test_guard.get_able_to_use_weapon_against_position(
                AbleToAttackType::NewTarget,
                pos,
                CommandSourceType::FromPlayer,
            )
        })
        .unwrap_or(CanAttackResult::NotPossible)
}

pub(super) fn selection_force_attack_position_result(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    world: &Coord3D,
) -> CanAttackResult {
    let logic_pos = LogicCoord3D::new(world.x, world.y, world.z);
    let mut saw_invalid_shot = false;
    let mut saw_possible_after_moving = false;

    for &id in selection {
        let step = OBJECT_REGISTRY
            .with_object(id, |sel_guard| {
                let is_mine = local_player
                    .and_then(|pid| {
                        sel_guard
                            .get_controlling_player_id()
                            .map(|owner| owner == pid)
                    })
                    .unwrap_or(false);
                if !is_mine || !sel_guard.is_able_to_attack() {
                    return CanAttackResult::NotPossible;
                }
                force_attack_position_result_for_attacker(sel_guard, &logic_pos)
            })
            .unwrap_or(CanAttackResult::NotPossible);
        match step {
            CanAttackResult::Possible => return CanAttackResult::Possible,
            CanAttackResult::PossibleAfterMoving => saw_possible_after_moving = true,
            CanAttackResult::InvalidShot => saw_invalid_shot = true,
            CanAttackResult::NotPossible => {}
        }
    }

    if saw_possible_after_moving {
        CanAttackResult::PossibleAfterMoving
    } else if saw_invalid_shot {
        CanAttackResult::InvalidShot
    } else {
        CanAttackResult::NotPossible
    }
}

pub(super) fn pending_command_selection_valid(
    pending: &PendingCommand,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    match pending.command_type {
        CommandType::DoAttackObject | CommandType::DoWeaponAtObject => {
            pending_fire_weapon_can_target_object(pending, local_player, selection, target_id)
        }
        CommandType::DoSpecialPower | CommandType::DoSpecialPowerAtObject => {
            pending_special_power_can_target_object(pending, local_player, selection, target_id)
        }
        CommandType::Enter => selection_can_enter_target(local_player, selection, target_id),
        CommandType::DoRepair => selection_can_repair_target(local_player, selection, target_id),
        CommandType::GetRepaired => {
            selection_can_get_repaired_target(local_player, selection, target_id)
        }
        CommandType::GetHealed => {
            selection_can_get_healed_target(local_player, selection, target_id)
        }
        CommandType::ResumeConstruction => {
            selection_can_resume_construction_target(local_player, selection, target_id)
        }
        CommandType::Dock => selection_can_dock_at_target(local_player, selection, target_id),
        _ => true,
    }
}

pub(super) fn pending_command_position_valid(
    pending: &PendingCommand,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    position: &Coord3D,
    object_in_way: Option<ObjectID>,
) -> bool {
    match pending.command_type {
        CommandType::DoAttackObject
        | CommandType::DoWeaponAtObject
        | CommandType::DoWeaponAtLocation => pending_fire_weapon_can_target_position(
            pending,
            local_player,
            selection,
            position,
            object_in_way,
        ),
        CommandType::DoSpecialPower | CommandType::DoSpecialPowerAtLocation => {
            pending_special_power_can_target_position(
                local_player,
                selection,
                position,
                object_in_way,
            )
        }
        _ => true,
    }
}

pub(super) fn current_local_selection(local_player: i32) -> HashSet<ObjectID> {
    let mut selection_ids = HashSet::new();
    if local_player < 0 {
        return selection_ids;
    }

    let selection_manager = get_selection_manager();
    let Ok(manager) = selection_manager.read() else {
        return selection_ids;
    };
    let Some(selection) = manager.get_player_selection_ref(local_player) else {
        return selection_ids;
    };

    selection_ids.extend(selection.get_selected_objects());
    selection_ids
}
