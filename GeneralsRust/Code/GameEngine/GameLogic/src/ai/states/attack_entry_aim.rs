//! C++ AttackStateMachine synchronous aim operations.

use super::*;

pub(in super::super) fn aim_enter(
    state: &mut AIAttackAimAtTargetState,
    ai: &mut dyn AiStateRuntime,
) -> Result<StateReturnType, String> {
    let owner = owner(&state.base, ai)?;
    let target = victim(&state.base, ai)?;
    let target_pos = state.base.get_machine_goal_position();
    state.can_turn_in_place = false;
    ai.with_cur_locomotor(&mut |loco| state.can_turn_in_place = loco.template.min_speed == 0.0);

    let (source_id, container) = {
        let source = owner.read().map_err(|_| "attack aim owner poisoned")?;
        if source.get_current_weapon().is_none() {
            return Ok(StateReturnType::Failure);
        }
        let enclosing = if let Some(id) = source.get_contained_by() {
            let container = ai
                .world()
                .ok_or("attack aim world missing")?
                .object(id)
                .ok_or("attack aim containing object missing")?;
            let container = container
                .read()
                .map_err(|_| "attack aim container poisoned")?;
            if let Some(contain) = container.get_contain() {
                contain
                    .lock()
                    .map_err(|_| "attack aim contain poisoned")?
                    .is_enclosing_container_for(&source)
                    .then_some(id)
            } else {
                None
            }
        } else {
            None
        };
        (source.get_id(), enclosing)
    };
    let target_id = target
        .as_ref()
        .map(|target| {
            target
                .read()
                .map(|target| target.get_id())
                .map_err(|_| "attack aim victim poisoned")
        })
        .transpose()?;
    // A garrison may synchronously reposition the source. No Object guard is
    // retained over that callback; subsequent observations read the new position.
    let in_range = if let Some(container) = container {
        ai.world_mut()
            .ok_or("attack aim world missing")?
            .attempt_best_fire_point(container, source_id, target_id, target_pos.as_ref())?
    } else {
        let source = owner.read().map_err(|_| "attack aim owner poisoned")?;
        let weapon = source
            .get_current_weapon()
            .ok_or("attack aim weapon disappeared")?
            .0;
        match target.as_ref() {
            Some(target) => {
                let target = target.read().map_err(|_| "attack aim victim poisoned")?;
                ai.world()
                    .ok_or("attack aim world missing")?
                    .weapon_in_attack_range(&source, weapon, Some(&target), None)?
            }
            None => ai
                .world()
                .ok_or("attack aim world missing")?
                .weapon_in_attack_range(&source, weapon, None, target_pos.as_ref())?,
        }
    };

    // AIStates.cpp:4925: register the targeter before any preventing query.
    if let Some(target) = target.as_ref() {
        if Arc::ptr_eq(target, &owner) {
            ai.add_targeter(source_id, true);
        } else {
            let target_ai = target
                .read()
                .map_err(|_| "attack aim victim poisoned")?
                .get_ai_update_interface();
            if let Some(target_ai) = target_ai {
                target_ai
                    .lock()
                    .map_err(|_| "attack aim victim AI poisoned")?
                    .add_targeter(source_id, true);
            }
        }
    }
    let turrets: &[TurretType] = if ai.are_turrets_linked() {
        &[TurretType::Primary, TurretType::Secondary]
    } else {
        match ai.get_which_turret_for_cur_weapon() {
            TurretType::Primary => &[TurretType::Primary],
            TurretType::Secondary => &[TurretType::Secondary],
            TurretType::Invalid => &[],
        }
    };
    for &turret in turrets {
        if state.attacking_object {
            ai.set_turret_target_object(turret, target_id, state.force_attacking);
        } else {
            ai.set_turret_target_position(
                turret,
                &target_pos.ok_or("attack aim position missing")?,
            );
        }
    }
    if turrets.is_empty() {
        let preventing = if let Some(target) = target.as_ref() {
            if Arc::ptr_eq(target, &owner) {
                ai.is_temporarily_preventing_aim_success()
            } else {
                let target_ai = target
                    .read()
                    .map_err(|_| "attack aim victim poisoned")?
                    .get_ai_update_interface();
                match target_ai {
                    Some(target_ai) => target_ai
                        .lock()
                        .map_err(|_| "attack aim victim AI poisoned")?
                        .is_temporarily_preventing_aim_success(),
                    None => false,
                }
            }
        } else {
            false
        };
        let source = owner.read().map_err(|_| "attack aim owner poisoned")?;
        if source
            .get_current_weapon()
            .is_some_and(|(weapon, _)| weapon.is_contact_weapon())
            && in_range
            && !preventing
        {
            return Ok(StateReturnType::Success);
        }
    }
    state.set_locomotor = false;
    owner
        .write()
        .map_err(|_| "attack aim owner poisoned")?
        .set_status(ObjectStatusMaskType::IS_AIMING_WEAPON, true);
    Ok(StateReturnType::Continue)
}

pub(in super::super) fn aim_exit(
    state: &mut AIAttackAimAtTargetState,
    _: StateExitType,
    ai: &mut dyn AiStateRuntime,
) -> Result<(), String> {
    owner(&state.base, ai)?
        .write()
        .map_err(|_| "attack aim owner poisoned")?
        .set_status(ObjectStatusMaskType::IS_AIMING_WEAPON, false);
    if state.can_turn_in_place && state.set_locomotor {
        ai.set_locomotor_goal_none();
    }
    Ok(())
}

pub(in super::super) fn fire_enter(
    state: &mut AIAttackFireWeaponState,
    ai: &mut dyn AiStateRuntime,
) -> Result<StateReturnType, String> {
    if ai.get_mood_matrix_action_adjustment(MoodMatrixAction::Attack)
        & mood_matrix_adjustment::ACTION_OK
        == 0
    {
        return Ok(StateReturnType::Failure);
    }
    let owner = owner(&state.base, ai)?;
    let target = victim(&state.base, ai)?;
    let target_id = target
        .as_ref()
        .map(|target| {
            target
                .read()
                .map(|target| target.get_id())
                .map_err(|_| "attack fire victim poisoned")
        })
        .transpose()?;
    let mut source = owner.write().map_err(|_| "attack fire owner poisoned")?;
    if let Some(id) = target_id {
        if let Some(team) = source.get_team() {
            let mut team = team.write().map_err(|_| "attack fire team poisoned")?;
            seed_team_target_if_attack_common(&mut team, id);
        }
    }
    source.set_status(
        ObjectStatusMaskType::from_status(ObjectStatusTypes::IsFiringWeapon),
        true,
    );
    ai.world_mut()
        .ok_or("attack fire world missing")?
        .pre_fire_current_weapon(&mut source, target_id)?;
    Ok(StateReturnType::Continue)
}

pub(in super::super) fn fire_exit(
    state: &mut AIAttackFireWeaponState,
    _: StateExitType,
    ai: &mut dyn AiStateRuntime,
) -> Result<(), String> {
    let owner = owner(&state.base, ai)?;
    let mut source = owner.write().map_err(|_| "attack fire owner poisoned")?;
    // AIStates.cpp:5297-5311: flags are cleared before the weapon status test.
    source.clear_status(ObjectStatusMaskType::from_status(
        ObjectStatusTypes::IsFiringWeapon,
    ));
    source.clear_status(ObjectStatusMaskType::from_status(
        ObjectStatusTypes::IgnoringStealth,
    ));
    if let Some((weapon, _)) = source.get_current_weapon() {
        let frame = ai.world().ok_or("attack fire world missing")?.frame();
        if weapon.get_status_at(frame) == WeaponStatus::PreAttack {
            source.cancel_pre_attack_for_current_weapon();
        }
    }
    Ok(())
}
