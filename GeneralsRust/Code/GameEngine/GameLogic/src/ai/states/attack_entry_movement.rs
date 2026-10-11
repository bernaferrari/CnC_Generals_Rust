//! C++ AttackStateMachine synchronous movement operations.

use super::*;

fn can_pursue(
    source: &Object,
    weapon: &Weapon,
    target: &Object,
    ai: &dyn AiStateRuntime,
) -> Result<bool, String> {
    let Some(physics) = target.get_physics() else {
        return Ok(false);
    };
    if ai.get_which_turret_for_cur_weapon() == TurretType::Invalid {
        return Ok(false);
    }
    if ai
        .world()
        .ok_or("attack pursue world missing")?
        .ai_crushes_infantry()?
        && player_type(source, ai)? == Some(PlayerType::Computer)
        && can_crush(source, target, ai)?
    {
        return Ok(true);
    }
    // Weapon.cpp:2211-2238: isTooClose uses ordinary boundary distance,
    // with no bridge/contact collision special cases or registry observation.
    let minimum = weapon.get_template().get_minimum_attack_range();
    if minimum != 0.0 {
        let dx = source.get_position().x - target.get_position().x;
        let dy = source.get_position().y - target.get_position().y;
        // PartitionManager.cpp:BoundaryAndBoundary_2D combines the radii
        // before subtraction and keeps the original square when radius is zero.
        let center_squared = dx * dx + dy * dy;
        let radius = source.get_geometry_info().get_bounding_circle_radius()
            + target.get_geometry_info().get_bounding_circle_radius();
        let distance_squared = if radius > 0.0 {
            let boundary = center_squared.sqrt() - radius;
            if boundary <= 0.0 {
                0.0
            } else {
                boundary * boundary
            }
        } else {
            center_squared
        };
        if distance_squared < minimum * minimum {
            return Ok(false);
        }
    }
    let speed = ai.get_cur_locomotor_speed_for_owner(source)?;
    let target_speed = physics
        .access()
        .map_err(|_| "attack pursue physics poisoned")?
        .get_forward_speed_2d_with_object(target);
    if target_speed >= speed || target_speed < speed / 10.0 {
        return Ok(false);
    }
    let dx = target.get_position().x - source.get_position().x;
    let dy = target.get_position().y - source.get_position().y;
    let direction = target.get_unit_direction_vector_2d();
    Ok(dx * direction.0 + dy * direction.1 >= 0.0)
}

fn target_destroyed(base: &State, ai: &dyn AiStateRuntime) -> Result<bool, String> {
    Ok(base.get_machine_goal_object_id().is_some() && victim(base, ai)?.is_none())
}

pub(in super::super) fn pursue_enter(
    state: &mut AIAttackPursueTargetState,
    ai: &mut dyn AiStateRuntime,
) -> Result<StateReturnType, String> {
    let owner = owner(&state.base.base, ai)?;
    if owner
        .read()
        .map_err(|_| "attack pursue owner poisoned")?
        .is_kind_of(KindOf::Projectile)
        || target_destroyed(&state.base.base, ai)?
        || !state.attacking_object
    {
        return Ok(StateReturnType::Success);
    }
    state.base.set_adjusts_destination(false);
    let parent_state = ai
        .get_parent_state_id()
        .ok_or("attack pursue parent state missing")?;
    if parent_state != AIStateType::GuardRetaliate as u32 {
        let source = owner.read().map_err(|_| "attack pursue owner poisoned")?;
        if human(&source, ai)? && ai.get_last_command_source() == CommandSourceType::FromAi {
            return Ok(StateReturnType::Success);
        }
    }
    state.prev_victim_pos = Coord3D::new(0.0, 0.0, 0.0);
    state.approach_timestamp = 0u32.wrapping_sub(ATTACK_MIN_RECOMPUTE_TIME);
    let Some(target) = victim(&state.base.base, ai)? else {
        return Ok(StateReturnType::Success);
    };
    let target_id = {
        let source = owner.read().map_err(|_| "attack pursue owner poisoned")?;
        let target = target.read().map_err(|_| "attack pursue victim poisoned")?;
        let Some((weapon, _)) = source.get_current_weapon() else {
            return Ok(StateReturnType::Failure);
        };
        if !can_pursue(&source, weapon, &target, ai)? {
            return Ok(StateReturnType::Success);
        }
        target.get_id()
    };
    let turret = ai.get_which_turret_for_cur_weapon();
    if turret == TurretType::Invalid {
        return Ok(StateReturnType::Success);
    }
    ai.set_turret_target_object(turret, Some(target_id), state.force_attacking);
    if !pursue_path(state, ai)? {
        return Ok(StateReturnType::Success);
    }
    start_move(state, ai)
}

pub(in super::super) fn approach_enter(
    state: &mut AIAttackApproachTargetState,
    ai: &mut dyn AiStateRuntime,
) -> Result<StateReturnType, String> {
    let owner = owner(&state.base.base, ai)?;
    if owner
        .read()
        .map_err(|_| "attack approach owner poisoned")?
        .is_kind_of(KindOf::Projectile)
    {
        ai.with_cur_locomotor_mut(&mut |loco| loco.set_precise_z_pos(true));
    }
    if target_destroyed(&state.base.base, ai)? {
        return Ok(StateReturnType::Success);
    }
    state.prev_victim_pos = Coord3D::new(0.0, 0.0, 0.0);
    state.approach_timestamp = 0u32.wrapping_sub(ATTACK_MIN_RECOMPUTE_TIME);
    let target = victim(&state.base.base, ai)?;
    if let Some(target) = target.as_ref() {
        let source = owner.read().map_err(|_| "attack approach owner poisoned")?;
        let target = target
            .read()
            .map_err(|_| "attack approach victim poisoned")?;
        let Some((weapon, _)) = source.get_current_weapon() else {
            return Ok(StateReturnType::Failure);
        };
        if ai
            .world()
            .ok_or("attack approach world missing")?
            .weapon_in_attack_range(&source, weapon, Some(&target), None)?
        {
            let blocked = ai.is_doing_ground_movement_for_owner(&source)?
                && !ai
                    .world()
                    .ok_or("attack approach world missing")?
                    .is_significantly_above_terrain(&target)?
                && view_blocked(&source, Some(&target), target.get_position(), ai)?;
            if !blocked {
                return Ok(StateReturnType::Success);
            }
        }
        let parent_state = ai
            .get_parent_state_id()
            .ok_or("attack approach parent state missing")?;
        if parent_state != AIStateType::GuardRetaliate as u32 {
            if human(&source, ai)? {
                if ai.get_last_command_source() == CommandSourceType::FromAi
                    && !ai.is_allowed_to_chase()
                    && !weapon.is_contact_weapon()
                {
                    return Ok(StateReturnType::Failure);
                }
            } else if parent_state != AIStateType::Hunt as u32
                && target.is_kind_of(KindOf::Aircraft)
                && target.is_airborne_target()
            {
                return Ok(StateReturnType::Failure);
            }
        }
        if can_pursue(&source, weapon, &target, ai)? {
            return Ok(StateReturnType::Success);
        }
    } else {
        ai.destroy_path();
    }
    let turret = ai.get_which_turret_for_cur_weapon();
    if turret != TurretType::Invalid {
        if state.attacking_object {
            let id = target
                .as_ref()
                .map(|target| {
                    target
                        .read()
                        .map(|target| target.get_id())
                        .map_err(|_| "attack approach victim poisoned")
                })
                .transpose()?;
            ai.set_turret_target_object(turret, id, state.force_attacking);
        } else {
            ai.set_turret_target_position(
                turret,
                &state
                    .base
                    .base
                    .get_machine_goal_position()
                    .ok_or("attack approach position missing")?,
            );
        }
    }
    if !approach_path(state, ai)? {
        return Ok(StateReturnType::Failure);
    }
    state.base.set_adjusts_destination(false);
    let result = start_move(state, ai);
    state.base.set_adjusts_destination(true);
    result
}

pub(in super::super) fn pursue_exit(
    state: &mut AIAttackPursueTargetState,
    exit: StateExitType,
    ai: &mut dyn AiStateRuntime,
) -> Result<(), String> {
    state.base.cpp_on_exit_with_ai(exit, ai)?;
    state.is_initial_approach = false;
    Ok(())
}

pub(in super::super) fn approach_exit(
    state: &mut AIAttackApproachTargetState,
    exit: StateExitType,
    ai: &mut dyn AiStateRuntime,
) -> Result<(), String> {
    state.base.cpp_on_exit_with_ai(exit, ai)?;
    ai.ignore_obstacle(None).map_err(|err| err.to_string())?;
    let owner = owner(&state.base.base, ai)?;
    if owner
        .read()
        .map_err(|_| "attack approach owner poisoned")?
        .is_kind_of(KindOf::Projectile)
    {
        ai.with_cur_locomotor_mut(&mut |loco| loco.set_precise_z_pos(false));
    }
    let ground = {
        let source = owner.read().map_err(|_| "attack approach owner poisoned")?;
        ai.is_doing_ground_movement_for_owner(&source)?
    };
    if ground {
        let mut source = owner
            .write()
            .map_err(|_| "attack approach owner poisoned")?;
        let dx = state.base.goal_position.x - source.get_position().x;
        let dy = state.base.goal_position.y - source.get_position().y;
        if dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.125 {
            ai.world_mut()
                .ok_or("attack approach world missing")?
                .set_object_position(&mut source, &state.base.goal_position)?;
        }
    }
    state.is_initial_approach = false;
    Ok(())
}

trait AttackMovement {
    fn movement(&mut self) -> &mut AIMoveToState;
    fn compute_path(&mut self, ai: &mut dyn AiStateRuntime) -> Result<bool, String>;
}
impl AttackMovement for AIAttackPursueTargetState {
    fn movement(&mut self) -> &mut AIMoveToState {
        &mut self.base
    }
    fn compute_path(&mut self, ai: &mut dyn AiStateRuntime) -> Result<bool, String> {
        pursue_path(self, ai)
    }
}
impl AttackMovement for AIAttackApproachTargetState {
    fn movement(&mut self) -> &mut AIMoveToState {
        &mut self.base
    }
    fn compute_path(&mut self, ai: &mut dyn AiStateRuntime) -> Result<bool, String> {
        approach_path(self, ai)
    }
}

// AIInternalMoveToState::onEnter, including virtual computePath. Calling the
// ordinary AIMoveTo entry would overwrite requestAttackPath with requestPath.
fn start_move(
    state: &mut impl AttackMovement,
    ai: &mut dyn AiStateRuntime,
) -> Result<StateReturnType, String> {
    let base = state.movement();
    let owner = owner(&base.base, ai)?;
    base.ambient_playing_handle = 0;
    base.waiting_for_path = ai.is_waiting_for_path();
    if owner
        .read()
        .map_err(|_| "attack movement owner poisoned")?
        .test_status(ObjectStatusTypes::Immobile)
    {
        return Ok(StateReturnType::Failure);
    }
    let mut ultra = false;
    ai.with_cur_locomotor_mut(&mut |loco| {
        loco.start_move();
        ultra = loco.is_ultra_accurate();
    });
    if ultra {
        base.set_adjusts_destination(false);
    }
    base.try_one_more_repath = true;
    ai.friend_starting_move();
    let condition = {
        let source = owner.read().map_err(|_| "attack movement owner poisoned")?;
        if ai
            .world()
            .ok_or("attack movement world missing")?
            .physics_is_motive(&source)?
        {
            let cliff = ai
                .world()
                .ok_or("attack movement world missing")?
                .cliff_at(source.get_position())?;
            Some(if cliff {
                ModelConditionFlags::CLIMBING
            } else {
                ModelConditionFlags::MOVING
            })
        } else if source.is_kind_of(KindOf::Dozer) && source.is_kind_of(KindOf::Harvester) {
            Some(ModelConditionFlags::MOVING)
        } else {
            None
        }
    };
    if let Some(condition) = condition {
        owner
            .write()
            .map_err(|_| "attack movement owner poisoned")?
            .set_model_condition_state(condition);
    }
    let adjust_destination = base.adjust_destinations
        && !owner
            .read()
            .map_err(|_| "attack movement owner poisoned")?
            .test_status(ObjectStatusTypes::Rider8);
    if adjust_destination {
        if !ai.adjust_destination(&mut base.goal_position) {
            let source = owner.read().map_err(|_| "attack movement owner poisoned")?;
            if !ai.snap_closest_goal_position(&source, &mut base.goal_position) {
                return Err("attack movement destination service missing".into());
            }
        }
        let layer = ai
            .world()
            .ok_or("attack movement world missing")?
            .layer_for_destination(&base.goal_position)?;
        ai.update_goal_position(&base.goal_position, layer)?;
    }
    if !state.compute_path(ai)? {
        ai.friend_ending_move();
        return Ok(StateReturnType::Failure);
    }
    ai.set_path_extra_distance(0.0)
        .map_err(|err| err.to_string())?;
    ai.set_desired_speed(FAST_AS_POSSIBLE);
    let base = state.movement();
    base.start_move_sound_with_ai(&owner, ai)?;
    Ok(StateReturnType::Continue)
}

fn pursue_path(
    state: &mut AIAttackPursueTargetState,
    ai: &mut dyn AiStateRuntime,
) -> Result<bool, String> {
    let owner = owner(&state.base.base, ai)?;
    if !owner
        .read()
        .map_err(|_| "attack pursue owner poisoned")?
        .is_mobile()
        || ai.is_blocked_and_stuck()
    {
        return Ok(false);
    }
    if state.base.waiting_for_path {
        return Ok(true);
    }
    let force = ai.get_path().is_none() && !ai.is_waiting_for_path();
    let now = ai.world().ok_or("attack pursue world missing")?.frame();
    if !force && now.wrapping_sub(state.approach_timestamp) < ATTACK_MIN_RECOMPUTE_TIME {
        return Ok(true);
    }
    state.approach_timestamp = now;
    let Some(target) = victim(&state.base.base, ai)? else {
        return Ok(false);
    };
    {
        let source = owner.read().map_err(|_| "attack pursue owner poisoned")?;
        let target = target.read().map_err(|_| "attack pursue victim poisoned")?;
        if !force
            && state.base.is_same_position(
                source.get_position(),
                &state.prev_victim_pos,
                target.get_position(),
            )
        {
            return Ok(true);
        }
        let Some((weapon, _)) = source.get_current_weapon() else {
            return Ok(false);
        };
        if !can_pursue(&source, weapon, &target, ai)? {
            return Ok(false);
        }
        state.prev_victim_pos = *target.get_position();
    }
    state.base.set_adjusts_destination(true);
    state.base.goal_position = state.prev_victim_pos;
    state.base.waiting_for_path = true;
    ai.request_path(&state.base.goal_position, false)?;
    state.stop_if_in_range = false;
    Ok(true)
}

fn approach_path(
    state: &mut AIAttackApproachTargetState,
    ai: &mut dyn AiStateRuntime,
) -> Result<bool, String> {
    let owner = owner(&state.base.base, ai)?;
    if !owner
        .read()
        .map_err(|_| "attack approach owner poisoned")?
        .is_mobile()
    {
        return Ok(false);
    }
    let force = ai.is_blocked_and_stuck() || ai.get_path().is_none() && !ai.is_waiting_for_path();
    if state.base.waiting_for_path {
        return Ok(true);
    }
    let now = ai.world().ok_or("attack approach world missing")?.frame();
    if !force && now.wrapping_sub(state.approach_timestamp) < ATTACK_MIN_RECOMPUTE_TIME {
        return Ok(true);
    }
    state.approach_timestamp = now;
    if let Some(target) = victim(&state.base.base, ai)? {
        let (target_id, center, contact) = {
            let source = owner.read().map_err(|_| "attack approach owner poisoned")?;
            let target = target
                .read()
                .map_err(|_| "attack approach victim poisoned")?;
            if !force
                && state.base.is_same_position(
                    source.get_position(),
                    &state.prev_victim_pos,
                    target.get_position(),
                )
            {
                return Ok(true);
            }
            let Some((weapon, _)) = source.get_current_weapon() else {
                return Ok(false);
            };
            state.prev_victim_pos = *target.get_position();
            if can_pursue(&source, weapon, &target, ai)? {
                return Ok(false);
            }
            (
                target.get_id(),
                target
                    .get_geometry_info()
                    .get_center_position(target.get_position()),
                weapon.is_contact_weapon(),
            )
        };
        state.base.set_adjusts_destination(true);
        if contact {
            ai.ignore_obstacle(Some(target_id))
                .map_err(|err| err.to_string())?;
            state.base.set_adjusts_destination(false);
            ai.set_path_extra_distance(10.0 * PATHFIND_CELL_SIZE_F)
                .map_err(|err| err.to_string())?;
        }
        state.base.goal_position = state.prev_victim_pos;
        state.base.waiting_for_path = true;
        ai.request_attack_path(target_id, &center)?;
        state.stop_if_in_range = false;
        Ok(true)
    } else {
        state.base.set_adjusts_destination(true);
        state.stop_if_in_range = false;
        state.base.goal_position = state
            .base
            .base
            .get_machine_goal_position()
            .ok_or("attack approach position missing")?;
        if !force {
            return Ok(true);
        }
        state.base.waiting_for_path = true;
        ai.request_attack_path(INVALID_ID, &state.base.goal_position)?;
        Ok(true)
    }
}
