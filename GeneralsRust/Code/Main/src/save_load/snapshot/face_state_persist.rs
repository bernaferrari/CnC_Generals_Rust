//! Owned Face continuation in the existing AIUpdate module record.
//!
//! C++ `AIFaceState::xfer` (`AIStates.cpp:7379`) preserves the captured
//! `m_canTurnInPlace`; `StateMachine::xfer` (`StateMachine.cpp:863-864`)
//! preserves its object/position goal. Both load-post-process methods are
//! empty. Restore must therefore copy saved state, without reissuing a Face
//! command or recapturing the current locomotor's minimum speed.
//!
//! The host also needs its active flag and last locomotor frame to continue
//! the existing split update without integrating twice in the saved frame.
//! ObjectSnapshot's target remains canonical; `target_object` here is a
//! checked projection, never an independently writable target. Locomotor
//! goals already belong to the movement/TMAI records. Older snapshots with
//! no Face module retain their previous behavior; absent continuation cannot
//! establish faithful save/load continuation.

use super::AIUpdateModuleSnapshot;
use crate::game_logic::{AIState, Object};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const FACE_VERSION: u8 = 1;
const FACE_STATE_KEY: &str = "AIFaceState";
const FACE_OBJECT: &str = "AI_FACE_OBJECT";
const FACE_POSITION: &str = "AI_FACE_POSITION";
const FACE_OVERLAY: &str = "AI_FACE_OVERLAY";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FaceSaveState {
    version: u8,
    parent_state: AIState,
    active: bool,
    can_turn_in_place: bool,
    goal_position: Option<Vec3>,
    loco_frame: u32,
}

impl FaceSaveState {
    fn module_state(&self) -> Option<&'static str> {
        match self.parent_state {
            AIState::FacingObject => Some(FACE_OBJECT),
            AIState::FacingPosition => Some(FACE_POSITION),
            _ if self.active => Some(FACE_OVERLAY),
            _ => None,
        }
    }

    fn validate(&self, module_state: &str) -> SaveLoadResult<()> {
        if self.version != FACE_VERSION {
            return Err(corrupted(format!(
                "Unsupported Face continuation version {}",
                self.version
            )));
        }
        if self.module_state() != Some(module_state) {
            return Err(corrupted(
                "Face continuation mode disagrees with its parent state",
            ));
        }
        if self
            .goal_position
            .is_some_and(|position| !position.is_finite())
        {
            return Err(corrupted("Face continuation has a non-finite position"));
        }
        match self.parent_state {
            AIState::FacingObject if self.goal_position.is_some() => {
                Err(corrupted("AI_FACE_OBJECT has an explicit position goal"))
            }
            AIState::FacingPosition if self.goal_position.is_none() => {
                Err(corrupted("AI_FACE_POSITION is missing its position goal"))
            }
            _ => Ok(()),
        }
    }
}

/// Capture a Face state or an active Face overlay without changing the object.
/// The current host helper can arm facing while retaining SpecialAbility or
/// Capturing; the saved parent state makes these continuations unambiguous.
pub(super) fn capture(object: &Object) -> SaveLoadResult<Option<AIUpdateModuleSnapshot>> {
    let state = FaceSaveState {
        version: FACE_VERSION,
        parent_state: object.ai_state.clone(),
        active: object.face_active,
        can_turn_in_place: object.face_can_turn_in_place,
        goal_position: object.face_goal_pos,
        loco_frame: object.face_loco_frame,
    };
    let Some(module_state) = state.module_state() else {
        return Ok(None);
    };
    state.validate(module_state)?;
    let encoded = serde_json::to_string(&state)
        .map_err(|error| corrupted(format!("Could not encode Face continuation: {error}")))?;
    Ok(Some(AIUpdateModuleSnapshot {
        current_state: module_state.to_owned(),
        state_machine_data: HashMap::from([(FACE_STATE_KEY.to_owned(), encoded)]),
        target_object: object.target,
        current_task: None,
        task_queue: Vec::new(),
    }))
}

/// Restore only the four Face fields after validating the whole continuation.
/// A missing object goal is valid saved state: the next owner-bound AI update
/// performs the original lookup/failure transition after all objects exist.
/// `false` means this is another AIUpdate module, not an absent Face module.
pub(super) fn restore(
    snapshot: &AIUpdateModuleSnapshot,
    object: &mut Object,
) -> SaveLoadResult<bool> {
    let face_mode = matches!(
        snapshot.current_state.as_str(),
        FACE_OBJECT | FACE_POSITION | FACE_OVERLAY
    );
    let encoded = snapshot.state_machine_data.get(FACE_STATE_KEY);
    if !face_mode && encoded.is_none() {
        return Ok(false);
    }
    if !face_mode {
        return Err(corrupted(
            "Face continuation is attached to a non-Face module",
        ));
    }
    let encoded = encoded.ok_or_else(|| corrupted("Face module is missing its continuation"))?;
    let state: FaceSaveState = serde_json::from_str(encoded)
        .map_err(|error| corrupted(format!("Invalid Face continuation: {error}")))?;
    state.validate(&snapshot.current_state)?;
    if state.parent_state != object.ai_state {
        return Err(corrupted("Face continuation disagrees with object status"));
    }
    if snapshot.target_object != object.target {
        return Err(corrupted(
            "Face goal projection disagrees with the saved object target",
        ));
    }

    // No live setters, callbacks, target resolution, or state entry here.
    object.face_active = state.active;
    object.face_can_turn_in_place = state.can_turn_in_place;
    object.face_goal_pos = state.goal_position;
    object.face_loco_frame = state.loco_frame;
    Ok(true)
}

fn corrupted(message: impl Into<String>) -> SaveLoadError {
    SaveLoadError::Corrupted(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{LocoGoalType, ObjectId, Team, ThingTemplate};

    fn object() -> Object {
        Object::new(
            ThingTemplate::new("FaceSnapshotUnit"),
            ObjectId(1),
            Team::USA,
        )
    }

    fn facing_object() -> Object {
        let mut object = object();
        object.ai_state = AIState::FacingObject;
        object.target = Some(ObjectId(77));
        object.face_active = true;
        // Deliberately differs from current min_speed: load must not onEnter.
        object.min_speed = 0.0;
        object.face_can_turn_in_place = false;
        object.face_loco_frame = u32::MAX;
        object
    }

    fn clear_face_fields(object: &mut Object) {
        object.face_active = false;
        object.face_can_turn_in_place = true;
        object.face_goal_pos = Some(Vec3::new(1.0, 2.0, 3.0));
        object.face_loco_frame = 4;
    }

    fn assert_face_fields(left: &Object, right: &Object) {
        assert_eq!(left.face_active, right.face_active);
        assert_eq!(left.face_can_turn_in_place, right.face_can_turn_in_place);
        assert_eq!(left.face_goal_pos, right.face_goal_pos);
        assert_eq!(left.face_loco_frame, right.face_loco_frame);
        assert_eq!(left.ai_state, right.ai_state);
        assert_eq!(left.target, right.target);
    }

    fn edit_state(snapshot: &mut AIUpdateModuleSnapshot, edit: impl FnOnce(&mut FaceSaveState)) {
        let mut state: FaceSaveState =
            serde_json::from_str(&snapshot.state_machine_data[FACE_STATE_KEY]).unwrap();
        edit(&mut state);
        snapshot.state_machine_data.insert(
            FACE_STATE_KEY.to_owned(),
            serde_json::to_string(&state).unwrap(),
        );
    }

    fn assert_rejected_without_mutation(snapshot: &AIUpdateModuleSnapshot, object: &mut Object) {
        let before = object.clone();
        assert!(restore(snapshot, object).is_err());
        assert_face_fields(object, &before);
        assert_eq!(object.get_orientation(), before.get_orientation());
        assert_eq!(object.locomotor_goal_type, before.locomotor_goal_type);
        assert_eq!(object.locomotor_goal_angle, before.locomotor_goal_angle);
    }

    #[test]
    fn object_face_restores_captured_fields_without_reentering() {
        let source = facing_object();
        let snapshot = capture(&source).unwrap().unwrap();
        assert_eq!(snapshot.current_state, FACE_OBJECT);
        assert_eq!(snapshot.target_object, Some(ObjectId(77)));
        let mut restored = source.clone();
        clear_face_fields(&mut restored);
        restored.locomotor_goal_type = LocoGoalType::Angle;
        restored.locomotor_goal_angle = 0.75;
        let orientation = restored.get_orientation();
        assert!(restore(&snapshot, &mut restored).unwrap());
        assert_face_fields(&restored, &source);
        assert_eq!(restored.get_orientation(), orientation);
        assert_eq!(restored.locomotor_goal_type, LocoGoalType::Angle);
        assert_eq!(restored.locomotor_goal_angle, 0.75);
    }

    #[test]
    fn position_face_restores_explicit_goal() {
        let mut source = facing_object();
        source.ai_state = AIState::FacingPosition;
        source.target = None;
        source.face_goal_pos = Some(Vec3::new(9.0, 2.0, 13.0));
        let snapshot = capture(&source).unwrap().unwrap();
        assert_eq!(snapshot.current_state, FACE_POSITION);
        let mut restored = source.clone();
        clear_face_fields(&mut restored);
        assert!(restore(&snapshot, &mut restored).unwrap());
        assert_face_fields(&restored, &source);
    }

    #[test]
    fn inactive_face_state_and_missing_object_goal_are_preserved() {
        let mut source = facing_object();
        source.target = None;
        source.face_active = false;
        let snapshot = capture(&source).unwrap().unwrap();
        let mut restored = source.clone();
        restored.face_active = true;
        assert!(restore(&snapshot, &mut restored).unwrap());
        assert_face_fields(&restored, &source);
    }

    #[test]
    fn active_overlays_preserve_parent_and_both_goal_modes() {
        for parent in [AIState::SpecialAbility, AIState::Capturing] {
            for goal in [None, Some(Vec3::new(6.0, 0.0, 7.0))] {
                let mut source = facing_object();
                source.ai_state = parent.clone();
                source.face_goal_pos = goal;
                let snapshot = capture(&source).unwrap().unwrap();
                assert_eq!(snapshot.current_state, FACE_OVERLAY);
                let mut restored = source.clone();
                clear_face_fields(&mut restored);
                assert!(restore(&snapshot, &mut restored).unwrap());
                assert_face_fields(&restored, &source);
            }
        }
    }

    #[test]
    fn unrelated_objects_and_modules_are_ignored_without_mutation() {
        let mut object = object();
        assert!(capture(&object).unwrap().is_none());
        let snapshot = AIUpdateModuleSnapshot {
            current_state: "AI_PANIC".to_owned(),
            state_machine_data: HashMap::new(),
            target_object: Some(ObjectId(23)),
            current_task: None,
            task_queue: Vec::new(),
        };
        let before = object.clone();
        assert!(!restore(&snapshot, &mut object).unwrap());
        assert_face_fields(&object, &before);
    }

    #[test]
    fn unsupported_version_and_malformed_payload_do_not_mutate() {
        let mut object = facing_object();
        let valid = capture(&object).unwrap().unwrap();
        let mut invalid = valid.clone();
        edit_state(&mut invalid, |state| state.version = FACE_VERSION + 1);
        assert_rejected_without_mutation(&invalid, &mut object);
        for encoded in ["{", "{}", "null"] {
            let mut invalid = valid.clone();
            invalid
                .state_machine_data
                .insert(FACE_STATE_KEY.to_owned(), encoded.to_owned());
            assert_rejected_without_mutation(&invalid, &mut object);
        }
    }

    #[test]
    fn missing_payload_and_wrong_module_mode_do_not_mutate() {
        let mut object = facing_object();
        let valid = capture(&object).unwrap().unwrap();
        let mut invalid = valid.clone();
        invalid.state_machine_data.clear();
        assert_rejected_without_mutation(&invalid, &mut object);
        for mode in [FACE_POSITION, FACE_OVERLAY, "AI_IDLE"] {
            let mut invalid = valid.clone();
            invalid.current_state = mode.to_owned();
            assert_rejected_without_mutation(&invalid, &mut object);
        }
    }

    #[test]
    fn parent_and_target_disagreement_do_not_mutate() {
        let mut object = facing_object();
        let snapshot = capture(&object).unwrap().unwrap();
        object.ai_state = AIState::SpecialAbility;
        assert_rejected_without_mutation(&snapshot, &mut object);
        object.ai_state = AIState::FacingObject;
        object.target = Some(ObjectId(78));
        assert_rejected_without_mutation(&snapshot, &mut object);
    }

    #[test]
    fn object_mode_with_position_and_position_mode_without_goal_are_rejected() {
        let mut object = facing_object();
        let mut snapshot = capture(&object).unwrap().unwrap();
        edit_state(&mut snapshot, |state| {
            state.goal_position = Some(Vec3::ZERO)
        });
        assert_rejected_without_mutation(&snapshot, &mut object);
        object.ai_state = AIState::FacingPosition;
        object.face_goal_pos = Some(Vec3::ZERO);
        let mut snapshot = capture(&object).unwrap().unwrap();
        edit_state(&mut snapshot, |state| state.goal_position = None);
        assert_rejected_without_mutation(&snapshot, &mut object);
    }

    #[test]
    fn inactive_overlay_is_rejected_without_mutation() {
        let mut object = facing_object();
        object.ai_state = AIState::Capturing;
        let mut snapshot = capture(&object).unwrap().unwrap();
        edit_state(&mut snapshot, |state| state.active = false);
        assert_rejected_without_mutation(&snapshot, &mut object);
    }

    #[test]
    fn non_finite_position_is_rejected_before_capture() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut object = facing_object();
            object.ai_state = AIState::FacingPosition;
            object.face_goal_pos = Some(Vec3::new(1.0, bad, 3.0));
            assert!(capture(&object).is_err());
        }
    }

    #[test]
    fn missing_schema_field_and_unknown_field_do_not_mutate() {
        let mut object = facing_object();
        let valid = capture(&object).unwrap().unwrap();
        for remove_field in [
            "version",
            "parent_state",
            "active",
            "can_turn_in_place",
            "loco_frame",
        ] {
            let mut value: serde_json::Value =
                serde_json::from_str(&valid.state_machine_data[FACE_STATE_KEY]).unwrap();
            value.as_object_mut().unwrap().remove(remove_field);
            let mut invalid = valid.clone();
            invalid
                .state_machine_data
                .insert(FACE_STATE_KEY.to_owned(), value.to_string());
            assert_rejected_without_mutation(&invalid, &mut object);
        }
        let mut value: serde_json::Value =
            serde_json::from_str(&valid.state_machine_data[FACE_STATE_KEY]).unwrap();
        value["unexpected"] = serde_json::json!(true);
        let mut invalid = valid;
        invalid
            .state_machine_data
            .insert(FACE_STATE_KEY.to_owned(), value.to_string());
        assert_rejected_without_mutation(&invalid, &mut object);
    }
}
