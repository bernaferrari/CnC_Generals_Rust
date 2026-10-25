//! Owner-local continuation of the existing host AttackStateMachine.
//! CPP AIStates.cpp:221 preserves the current state; :2503 preserves the
//! approach clock and previous victim position. Object status saves AIM/FIRE
//! flags. Loading copies these values without replaying state-entry effects.
use super::AIUpdateModuleSnapshot;
use crate::game_logic::{AIState, AttackSubState, Object};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::collections::HashMap;

const KEY: &str = "AttackStateMachine";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttackSaveState {
    version: u8,
    parent: AIState,
    state: AttackSubState,
    aiming: bool,
    firing: bool,
    approach_timestamp: u32,
    previous_victim: Option<Vec3>,
}

#[cfg(test)]
pub(super) fn capture(object: &Object) -> SaveLoadResult<Option<AIUpdateModuleSnapshot>> {
    if object.attack_substate() == AttackSubState::AimAtTarget
        && !object.status.is_aiming_weapon
        && !object.status.is_firing_weapon
        && object.approach_timestamp() == 0
        && object.prev_victim_pos().is_none()
    {
        return Ok(None);
    }
    let state = AttackSaveState {
        version: 1,
        parent: object.ai_state().clone(),
        state: object.attack_substate(),
        aiming: object.status.is_aiming_weapon,
        firing: object.status.is_firing_weapon,
        approach_timestamp: object.approach_timestamp(),
        previous_victim: object.prev_victim_pos(),
    };
    validate(&state)?;
    let encoded = serde_json::to_string(&state).map_err(|e| corrupted(e.to_string()))?;
    Ok(Some(AIUpdateModuleSnapshot {
        current_state: KEY.into(),
        state_machine_data: HashMap::from([(KEY.into(), encoded)]),
        target_object: object.target,
        current_task: None,
        task_queue: Vec::new(),
    }))
}

pub(super) fn restore(
    snapshot: &AIUpdateModuleSnapshot,
    object: &mut Object,
) -> SaveLoadResult<bool> {
    if snapshot.current_state != KEY {
        return Ok(false);
    }
    let encoded = snapshot
        .state_machine_data
        .get(KEY)
        .ok_or_else(|| corrupted("missing attack continuation".into()))?;
    let state: AttackSaveState =
        serde_json::from_str(encoded).map_err(|e| corrupted(e.to_string()))?;
    validate(&state)?;
    if state.parent != object.ai_state() || snapshot.target_object != object.target {
        return Err(corrupted(
            "attack continuation disagrees with its owner".into(),
        ));
    }
    if object.attack_substate() != state.state
        || object.status.is_aiming_weapon != state.aiming
        || object.status.is_firing_weapon != state.firing
        || object.approach_timestamp() != state.approach_timestamp
        || object.prev_victim_pos() != state.previous_victim
    {
        return Err(corrupted(
            "legacy attack projection disagrees with native14 owner".into(),
        ));
    }
    Ok(true)
}

fn validate(state: &AttackSaveState) -> SaveLoadResult<()> {
    if state.version != 1 || state.previous_victim.is_some_and(|p| !p.is_finite()) {
        return Err(corrupted(
            "invalid attack continuation version or position".into(),
        ));
    }
    Ok(())
}

fn corrupted(message: String) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("AttackStateMachine: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, Team, ThingTemplate};

    #[test]
    fn restores_substate_without_replaying_entry_or_touching_peer() {
        let mut source = GameLogic::new();
        let mut receiver = GameLogic::new();
        for world in [&mut source, &mut receiver] {
            world
                .templates
                .insert("Actor".into(), ThingTemplate::new("Actor"));
        }
        let id = source
            .create_object("Actor", Team::USA, Vec3::ZERO)
            .unwrap();
        assert_eq!(
            receiver.create_object("Actor", Team::USA, Vec3::ZERO),
            Some(id)
        );
        let actor = source.host_object_mut(id).unwrap();
        {
            let mut owner = actor.capture_base_ai_continuation();
            owner.attack_substate = AttackSubState::FireWeapon;
            actor.restore_base_ai_continuation(owner).unwrap();
        }
        actor.status.is_firing_weapon = true;
        {
            let mut owner = actor.capture_base_ai_continuation();
            owner.approach_timestamp = u32::MAX - 7;
            actor.restore_base_ai_continuation(owner).unwrap();
        }
        {
            let mut owner = actor.capture_base_ai_continuation();
            owner.prev_victim_pos = Some(Vec3::new(3.0, 4.0, 5.0));
            actor.restore_base_ai_continuation(owner).unwrap();
        }
        let saved = capture(actor).unwrap().unwrap();
        let target = receiver.host_object_mut(id).unwrap();
        target
            .restore_base_ai_continuation(actor.capture_base_ai_continuation())
            .unwrap();
        target.status.is_aiming_weapon = actor.status.is_aiming_weapon;
        target.status.is_firing_weapon = actor.status.is_firing_weapon;
        assert!(restore(&saved, target).unwrap());
        assert_eq!(target.attack_substate(), AttackSubState::FireWeapon);
        assert!(target.status.is_firing_weapon);
        assert_eq!(target.approach_timestamp(), u32::MAX - 7);
        assert_eq!(target.prev_victim_pos(), Some(Vec3::new(3.0, 4.0, 5.0)));
        assert_eq!(target.fire_intent_count, 0);
        assert_eq!(source.host_object(id).unwrap().fire_intent_count, 0);
        let mut malformed = saved;
        let value = malformed.state_machine_data.get_mut(KEY).unwrap();
        *value = value.replace("\"version\":1", "\"version\":2");
        assert!(restore(&malformed, target).is_err());
        assert_eq!(target.attack_substate(), AttackSubState::FireWeapon);
        assert_eq!(target.approach_timestamp(), u32::MAX - 7);
    }
}
