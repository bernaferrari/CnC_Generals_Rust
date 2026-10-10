//! CPP AIHuntState::xfer preserves the parent independently of its nested
//! attack/idle state. The existing module map retains that distinction without
//! altering positional snapshot schemas; scan timing remains in owned OXOB.
use super::AIUpdateModuleSnapshot;
use crate::game_logic::Object;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::collections::HashMap;

pub(super) fn capture(object: &Object) -> Option<AIUpdateModuleSnapshot> {
    object.hunting.then(|| AIUpdateModuleSnapshot {
        current_state: "AI_HUNT".into(),
        state_machine_data: HashMap::from([(
            "AutoAcquireWhenIdle".into(),
            object.auto_acquire_when_idle.to_string(),
        )]),
        target_object: None,
        current_task: None,
        task_queue: Vec::new(),
    })
}

pub(super) fn restore(
    snapshot: &AIUpdateModuleSnapshot,
    object: &mut Object,
) -> SaveLoadResult<bool> {
    if snapshot.current_state != "AI_HUNT" {
        return Ok(false);
    }
    let acquire = snapshot
        .state_machine_data
        .get("AutoAcquireWhenIdle")
        .and_then(|value| value.parse::<bool>().ok())
        .ok_or_else(|| {
            SaveLoadError::Corrupted(
                "AI_HUNT snapshot has invalid auto-acquire continuation".into(),
            )
        })?;
    object.hunting = true;
    object.auto_acquire_when_idle = acquire;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{ObjectId, Team, ThingTemplate};
    #[test]
    fn malformed_hunt_continuation_is_rejected_before_mutating_object() {
        let mut object = Object::new(ThingTemplate::new("SavedHunter"), ObjectId(1), Team::USA);
        object.hunting = true;
        let mut snapshot = capture(&object).unwrap();
        object.hunting = false;
        snapshot
            .state_machine_data
            .insert("AutoAcquireWhenIdle".into(), "maybe".into());
        assert!(restore(&snapshot, &mut object).is_err());
        assert!(!object.hunting);
    }
}
