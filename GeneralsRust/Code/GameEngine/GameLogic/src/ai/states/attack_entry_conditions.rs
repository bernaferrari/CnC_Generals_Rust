//! C++ AttackStateMachine synchronous conditions operations.

use super::*;

pub(super) fn out_of_range(
    base: &State,
    object: bool,
    ai: &mut dyn AiStateRuntime,
) -> Result<bool, String> {
    let owner = owner(base, ai)?;
    let owner = owner
        .read()
        .map_err(|_| "attack condition owner poisoned")?;
    let Some((weapon, _)) = owner.get_current_weapon() else {
        return Ok(false);
    };
    if object {
        let Some(target) = victim(base, ai)? else {
            return Ok(false);
        };
        let target = target
            .read()
            .map_err(|_| "attack condition victim poisoned")?;
        // AIStates.cpp:1087-1130: view observation precedes the leech/range checks.
        let blocked = !weapon.is_contact_weapon()
            && on_ground(&owner, ai)?
            && !ai
                .world()
                .ok_or("attack condition world missing")?
                .is_significantly_above_terrain(&target)?
            && view_blocked(&owner, Some(&target), target.get_position(), ai)?;
        if weapon.has_leech_range() {
            return Ok(false);
        }
        Ok(blocked
            || !ai
                .world()
                .ok_or("attack condition world missing")?
                .weapon_in_attack_range(&owner, weapon, Some(&target), None)?)
    } else {
        let Some(position) = base.get_machine_goal_position() else {
            return Ok(false);
        };
        if on_ground(&owner, ai)? && view_blocked(&owner, None, &position, ai)? {
            return Ok(true);
        }
        Ok(!ai
            .world()
            .ok_or("attack condition world missing")?
            .weapon_in_attack_range(&owner, weapon, None, Some(&position))?)
    }
}

fn squish(base: &State, ai: &mut dyn AiStateRuntime) -> Result<bool, String> {
    let owner = owner(base, ai)?;
    let Some(target) = victim(base, ai)? else {
        return Ok(false);
    };
    let owner = owner
        .read()
        .map_err(|_| "attack condition owner poisoned")?;
    let target = target
        .read()
        .map_err(|_| "attack condition victim poisoned")?;
    if target.get_contained_by().is_some()
        || ai.get_which_turret_for_cur_weapon() == TurretType::Invalid
    {
        return Ok(false);
    }
    Ok(ai
        .world()
        .ok_or("attack condition world missing")?
        .ai_crushes_infantry()?
        && player_type(&owner, ai)? == Some(PlayerType::Computer)
        && can_crush(&owner, &target, ai)?
        && !owner.is_kind_of(KindOf::DontAutoCrushInfantry))
}

fn impossible(
    base: &State,
    data: &StateTransitionUserData,
    ai: &mut dyn AiStateRuntime,
) -> Result<bool, String> {
    let owner = owner(base, ai)?;
    let Some(target) = victim(base, ai)? else {
        return Ok(false);
    };
    let owner = owner
        .read()
        .map_err(|_| "attack condition owner poisoned")?;
    let target = target
        .read()
        .map_err(|_| "attack condition victim poisoned")?;
    if !owner.is_able_to_attack_with_world(ai.world().ok_or("attack condition world missing")?)? {
        return Ok(true);
    }
    let continuation = data
        .data
        .as_ref()
        .and_then(|value| value.downcast_ref::<AttackContinuationData>())
        .ok_or("attack condition continuation missing")?;
    let result = ai
        .world()
        .ok_or("attack condition world missing")?
        .can_attack(
            &owner,
            &target,
            ai.get_last_command_source(),
            continuation.attack_type,
        )?;
    Ok(!matches!(
        result,
        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
    ))
}

macro_rules! conditions {
    ($state:ty, $object:ident, $position:ident, $squish:ident, $impossible:ident) => {
        pub(in super::super) fn $object(
            state: &$state,
            _: &StateTransitionUserData,
            ai: &mut dyn AiStateRuntime,
        ) -> Result<bool, String> {
            out_of_range(state.base_state(), true, ai)
        }
        pub(in super::super) fn $position(
            state: &$state,
            _: &StateTransitionUserData,
            ai: &mut dyn AiStateRuntime,
        ) -> Result<bool, String> {
            out_of_range(state.base_state(), false, ai)
        }
        pub(in super::super) fn $squish(
            state: &$state,
            _: &StateTransitionUserData,
            ai: &mut dyn AiStateRuntime,
        ) -> Result<bool, String> {
            squish(state.base_state(), ai)
        }
        pub(in super::super) fn $impossible(
            state: &$state,
            data: &StateTransitionUserData,
            ai: &mut dyn AiStateRuntime,
        ) -> Result<bool, String> {
            impossible(state.base_state(), data, ai)
        }
    };
}
conditions!(
    AIAttackAimAtTargetState,
    out_of_weapon_range_object_aim,
    out_of_weapon_range_position_aim,
    want_to_squish_target_aim,
    cannot_possibly_attack_object_aim
);
conditions!(
    AIAttackFireWeaponState,
    out_of_weapon_range_object_fire,
    out_of_weapon_range_position_fire,
    want_to_squish_target_fire,
    cannot_possibly_attack_object_fire
);
