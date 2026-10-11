//! Synchronous entry chain of C++ AttackStateMachine (AIStates.cpp).
//!
//! These functions borrow the installed owner's runtime and its supplied world.
//! They do not select a registry, publish a world, or retain the capability.
//! Later attack updates remain a separate migration.

use super::*;
use crate::modules::ai_state_runtime::AiStateRuntime;

fn owner(base: &State, ai: &dyn AiStateRuntime) -> Result<Arc<RwLock<Object>>, String> {
    let owner = base
        .get_machine_owner()
        .ok_or("attack entry owner missing")?;
    let id = owner
        .read()
        .map_err(|_| "attack entry owner poisoned")?
        .get_id();
    let supplied = ai
        .world()
        .ok_or("attack entry world missing")?
        .object(id)
        .ok_or("attack entry owner absent from supplied world")?;
    if !Arc::ptr_eq(&owner, &supplied) {
        return Err("attack entry owner differs from supplied world".into());
    }
    Ok(owner)
}

fn victim(base: &State, ai: &dyn AiStateRuntime) -> Result<Option<Arc<RwLock<Object>>>, String> {
    let Some(id) = base.get_machine_goal_object_id() else {
        return Ok(None);
    };
    Ok(ai.world().ok_or("attack entry world missing")?.object(id))
}

fn human(source: &Object, ai: &dyn AiStateRuntime) -> Result<bool, String> {
    ai.world()
        .ok_or("attack entry world missing")?
        .controller_is_human(source)
        .ok_or_else(|| "attack entry controlling player missing".into())
}

fn player_type(source: &Object, ai: &dyn AiStateRuntime) -> Result<Option<PlayerType>, String> {
    let Some(team) = source.get_team() else {
        return Ok(None);
    };
    let id = team
        .read()
        .map_err(|_| "attack controlling team poisoned")?
        .get_controlling_player_id();
    let Some(id) = id else {
        return Ok(None);
    };
    let Some(player) = ai
        .world()
        .ok_or("attack entry world missing")?
        .player(id as Int)
    else {
        return Ok(None);
    };
    let kind = player
        .read()
        .map_err(|_| "attack controlling player poisoned")?
        .get_player_type();
    Ok(Some(kind))
}

// Object.cpp:1076, with relationship observed through this same world's players.
fn can_crush(source: &Object, target: &Object, ai: &dyn AiStateRuntime) -> Result<bool, String> {
    if source.is_disabled_by_type(DisabledType::DisabledUnmanned) {
        return Ok(false);
    }
    let level = source.get_crusher_level();
    if matches!(
        ai.world()
            .ok_or("attack entry world missing")?
            .relationship(source, target),
        crate::object::contain::open_contain::ObjectRelationship::Ally
            | crate::object::contain::open_contain::ObjectRelationship::Self_
    ) || level == 0
    {
        return Ok(false);
    }
    Ok(target.find_module_by_name("SquishCollide").is_some()
        || level > target.get_crushable_level())
}

fn view_blocked(
    source: &Object,
    target: Option<&Object>,
    position: &Coord3D,
    ai: &mut dyn AiStateRuntime,
) -> Result<bool, String> {
    ai.world_mut()
        .ok_or("attack entry world missing")?
        .attack_view_blocked(source, target, position)
}

fn on_ground(source: &Object, ai: &dyn AiStateRuntime) -> Result<bool, String> {
    let mut ground = ai.is_doing_ground_movement_for_owner(source)?
        || source.is_kind_of(KindOf::Immobile)
        || source.is_kind_of(KindOf::SpawnsAreTheWeapons);
    if let Some(id) = source.get_contained_by() {
        let container = ai
            .world()
            .ok_or("attack entry world missing")?
            .object(id)
            .ok_or("attack entry containing object missing")?;
        let container = container
            .read()
            .map_err(|_| "attack entry container poisoned")?;
        if container.is_kind_of(KindOf::Structure) || !container.is_airborne_target() {
            ground = true;
        }
    }
    Ok(ground)
}

/// C++'s built-in ContinueState/FailureState used by this machine's chase slot.
#[derive(Debug)]
pub(super) struct StationaryChase {
    base: State,
    portable: bool,
}
impl StationaryChase {
    pub(super) fn new(machine: &StateMachine, portable: bool) -> Self {
        Self {
            base: State::new(
                machine,
                if portable {
                    "ContinueState"
                } else {
                    "FailureState"
                },
            ),
            portable,
        }
    }
}
impl CppState for StationaryChase {
    fn base_state(&self) -> &State {
        &self.base
    }
    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }
    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        Ok(if self.portable {
            StateReturnType::Continue
        } else {
            StateReturnType::Failure
        })
    }
    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        self.cpp_on_enter()
    }
    fn cpp_on_exit(&mut self, _: StateExitType) -> Result<(), String> {
        Ok(())
    }
}
pub(super) fn portable_in_range(
    state: &StationaryChase,
    _: &StateTransitionUserData,
) -> Result<bool, String> {
    in_weapon_range_object_state(state.base_state())
}
pub(super) fn portable_in_range_with_ai(
    state: &StationaryChase,
    _: &StateTransitionUserData,
    ai: &mut dyn AiStateRuntime,
) -> Result<bool, String> {
    Ok(!conditions::out_of_range(state.base_state(), true, ai)?)
}

#[path = "attack_entry_aim.rs"]
mod aim;
#[path = "attack_entry_conditions.rs"]
mod conditions;
#[path = "attack_entry_movement.rs"]
mod movement;

pub(super) use aim::{aim_enter, aim_exit, fire_enter, fire_exit};
pub(super) use conditions::{
    cannot_possibly_attack_object_aim, cannot_possibly_attack_object_fire,
    out_of_weapon_range_object_aim, out_of_weapon_range_object_fire,
    out_of_weapon_range_position_aim, out_of_weapon_range_position_fire, want_to_squish_target_aim,
    want_to_squish_target_fire,
};
pub(super) use movement::{approach_enter, approach_exit, pursue_enter, pursue_exit};
