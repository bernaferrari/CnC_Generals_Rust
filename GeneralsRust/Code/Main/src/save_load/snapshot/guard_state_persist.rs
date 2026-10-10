//! Owned Return/Idle continuation of C++ AIGuardMachine.
//!
//! AIGuard.cpp:298 transfers the nested machine; Return/Idle xfer methods
//! at 584/660 preserve their scan deadlines. The host's captured return goal,
//! observed guardee anchor and update stamp preserve its split execution.
//! Path goal, wander width and destination adjustment project host fields at save
//! time; they are not extra live state or original Guard Xfer fields. C++
//! restores width through the current LocomotorTemplate; the host's generic
//! binding restore does not yet cover it (hq-vif25).
//! Restoration is inert. OXOB retains an active-deadline projection, checked
//! against this immutable module record after the lifecycle tail is applied.
//! Older snapshots without this record retain their previous behavior; the
//! missing nested phase cannot establish exact continuation.

use super::{AIUpdateModuleSnapshot, ModuleSnapshot, WorldSnapshot};
use crate::game_logic::object::unit_ai_runtime::GuardPhase;
use crate::game_logic::{AIState, GameLogic, Object};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const GUARD_VERSION: u8 = 1;
const GUARD_KEY: &str = "AIGuardState";
const RETURN: &str = "AI_GUARD_RETURN";
const IDLE: &str = "AI_GUARD_IDLE";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GuardSaveState {
    version: u8,
    parent_state: AIState,
    return_goal: Option<Vec3>,
    path_goal: Option<Vec3>,
    wander_width_factor: f32,
    #[serde(default = "adjust_destinations_default")]
    adjust_destinations: bool,
    scan_deadline: u32,
    updated_frame: Option<u32>,
    anchor: Option<Vec3>,
}

fn adjust_destinations_default() -> bool {
    true
}

impl GuardSaveState {
    fn phase(&self, mode: &str) -> SaveLoadResult<GuardPhase> {
        if self.version != GUARD_VERSION {
            return Err(corrupted("Unsupported Guard continuation version"));
        }
        if !self.wander_width_factor.is_finite() {
            return Err(corrupted(
                "Guard continuation has a non-finite wander width",
            ));
        }
        if self.path_goal.is_some_and(|point| !point.is_finite()) {
            return Err(corrupted("Guard continuation has a non-finite path goal"));
        }
        if self.anchor.is_some_and(|point| !point.is_finite()) {
            return Err(corrupted("Guard continuation has a non-finite anchor"));
        }
        match (mode, self.return_goal) {
            (RETURN, Some(goal)) if goal.is_finite() => Ok(GuardPhase::Return { goal }),
            (IDLE, None) => Ok(GuardPhase::Idle),
            _ => Err(corrupted("Guard continuation mode disagrees with its goal")),
        }
    }
}

fn decode(snapshot: &AIUpdateModuleSnapshot) -> SaveLoadResult<Option<GuardSaveState>> {
    let selected = matches!(snapshot.current_state.as_str(), RETURN | IDLE);
    let encoded = snapshot.state_machine_data.get(GUARD_KEY);
    if !selected && encoded.is_none() {
        return Ok(None);
    }
    if !selected {
        return Err(corrupted(
            "Guard continuation is attached to a non-Guard module",
        ));
    }
    let encoded = encoded.ok_or_else(|| corrupted("Guard module is missing its continuation"))?;
    let state: GuardSaveState = serde_json::from_str(encoded)
        .map_err(|error| corrupted(format!("Invalid Guard continuation: {error}")))?;
    state.phase(&snapshot.current_state)?;
    Ok(Some(state))
}

pub(super) fn capture(object: &Object) -> SaveLoadResult<Option<AIUpdateModuleSnapshot>> {
    let runtime = &object.unit_ai_runtime;
    let Some(phase) = runtime.guard_phase() else {
        return Ok(None);
    };
    let (mode, return_goal) = match phase {
        GuardPhase::Return { goal } => (RETURN, Some(goal)),
        GuardPhase::Idle => (IDLE, None),
    };
    let state = GuardSaveState {
        version: GUARD_VERSION,
        // Temporary movement overlays may retain the nested Guard phase.
        // Preserve their actual parent rather than reentering Guard on load.
        parent_state: object.ai_state.clone(),
        return_goal,
        path_goal: object.path_goal_position,
        wander_width_factor: object.wander_width_factor,
        adjust_destinations: object.adjust_destinations,
        scan_deadline: runtime
            .guard_scan_deadline()
            .ok_or_else(|| corrupted("Active Guard phase is missing its scan deadline"))?,
        updated_frame: runtime.guard_updated_frame(),
        anchor: runtime.guard_anchor(),
    };
    state.phase(mode)?;
    let encoded = serde_json::to_string(&state)
        .map_err(|error| corrupted(format!("Could not encode Guard continuation: {error}")))?;
    Ok(Some(AIUpdateModuleSnapshot {
        current_state: mode.into(),
        state_machine_data: HashMap::from([(GUARD_KEY.into(), encoded)]),
        target_object: None,
        current_task: None,
        task_queue: Vec::new(),
    }))
}

/// Parse and validate before copying any fields; no callbacks or phase entry.
pub(super) fn restore(
    snapshot: &AIUpdateModuleSnapshot,
    object: &mut Object,
) -> SaveLoadResult<bool> {
    let Some(state) = decode(snapshot)? else {
        return Ok(false);
    };
    if state.parent_state != object.ai_state {
        return Err(corrupted("Guard continuation disagrees with object status"));
    }
    let phase = state.phase(&snapshot.current_state)?;
    let runtime = &mut object.unit_ai_runtime;
    runtime.set_guard_phase(Some(phase));
    runtime.set_guard_scan_deadline(Some(state.scan_deadline));
    runtime.set_guard_updated_frame(state.updated_frame);
    runtime.set_guard_anchor(state.anchor);
    object.path_goal_position = state.path_goal;
    object.wander_width_factor = state.wander_width_factor;
    object.adjust_destinations = state.adjust_destinations;
    Ok(true)
}

fn validate_deadline(state: &GuardSaveState, object: &Object) -> SaveLoadResult<()> {
    if object.unit_ai_runtime.guard_scan_deadline() != Some(state.scan_deadline) {
        return Err(corrupted(
            "Guard continuation deadline disagrees with lifecycle tail",
        ));
    }
    Ok(())
}

/// Read saved expectations after OXOB has restored the active deadline. This
/// retains one mutable deadline in UnitAiRuntime, not another runtime mirror.
pub(super) fn validate_after_lifecycle_tail(
    snapshot: &WorldSnapshot,
    world: &GameLogic,
) -> SaveLoadResult<()> {
    for saved in snapshot.objects.values() {
        let mut found = false;
        for module in saved.modules.values() {
            let ModuleSnapshot::AIUpdate(module) = module else {
                continue;
            };
            let Some(state) = decode(module)? else {
                continue;
            };
            if found {
                return Err(corrupted("Object has duplicate Guard continuations"));
            }
            found = true;
            let object = world
                .host_object(saved.id)
                .ok_or_else(|| corrupted("Saved Guard object is absent after restoration"))?;
            validate_deadline(&state, object)?;
        }
    }
    Ok(())
}

fn corrupted(message: impl Into<String>) -> SaveLoadError {
    SaveLoadError::Corrupted(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{ObjectId, Team, ThingTemplate};

    fn object(phase: GuardPhase) -> Object {
        let mut object = Object::new(ThingTemplate::new("SavedGuard"), ObjectId(1), Team::USA);
        object.ai_state = AIState::GuardingArea;
        object.unit_ai_runtime.set_guard_phase(Some(phase));
        object
            .unit_ai_runtime
            .set_guard_scan_deadline(Some(u32::MAX));
        object.unit_ai_runtime.set_guard_updated_frame(Some(19));
        object
            .unit_ai_runtime
            .set_guard_anchor(Some(Vec3::new(4.0, 5.0, 6.0)));
        object
    }

    fn assert_runtime(left: &Object, right: &Object) {
        let left = &left.unit_ai_runtime;
        let right = &right.unit_ai_runtime;
        assert_eq!(left.guard_phase(), right.guard_phase());
        assert_eq!(left.guard_scan_deadline(), right.guard_scan_deadline());
        assert_eq!(left.guard_updated_frame(), right.guard_updated_frame());
        assert_eq!(left.guard_anchor(), right.guard_anchor());
    }

    fn rewrite(snapshot: &mut AIUpdateModuleSnapshot, edit: impl FnOnce(&mut GuardSaveState)) {
        let mut state = decode(snapshot).unwrap().unwrap();
        edit(&mut state);
        snapshot
            .state_machine_data
            .insert(GUARD_KEY.into(), serde_json::to_string(&state).unwrap());
    }

    fn reject_without_mutation(snapshot: &AIUpdateModuleSnapshot, target: &mut Object) {
        let before = target.clone();
        assert!(restore(snapshot, target).is_err());
        assert_runtime(target, &before);
        assert_eq!(target.ai_state, before.ai_state);
        assert_eq!(target.movement.path, before.movement.path);
        assert_eq!(target.waiting_for_path, before.waiting_for_path);
    }

    #[test]
    fn return_and_idle_roundtrip_without_reentering_or_requesting_paths() {
        for phase in [
            GuardPhase::Return {
                goal: Vec3::new(1.0, 2.0, 3.0),
            },
            GuardPhase::Idle,
        ] {
            let source = object(phase);
            let snapshot = capture(&source).unwrap().unwrap();
            let mut target = source.clone();
            target.unit_ai_runtime.clear_guard();
            target.movement.path = vec![Vec3::splat(20.0)];
            target.waiting_for_path = true;
            assert!(restore(&snapshot, &mut target).unwrap());
            assert_runtime(&source, &target);
            assert_eq!(target.movement.path, vec![Vec3::splat(20.0)]);
            assert!(target.waiting_for_path);
        }
    }

    #[test]
    fn destination_adjustment_projection_roundtrips_and_accepts_older_capsules() {
        let mut source = object(GuardPhase::Idle);
        source.ai_state = AIState::Moving;
        source.adjust_destinations = false;
        let mut saved = capture(&source).unwrap().unwrap();
        let mut target = source.clone();
        target.adjust_destinations = true;
        assert!(restore(&saved, &mut target).unwrap());
        assert!(!target.adjust_destinations);
        let encoded = saved.state_machine_data.get_mut(GUARD_KEY).unwrap();
        let mut older: serde_json::Value = serde_json::from_str(encoded).unwrap();
        older.as_object_mut().unwrap().remove("adjust_destinations");
        *encoded = serde_json::to_string(&older).unwrap();
        assert!(restore(&saved, &mut target).unwrap());
        assert!(target.adjust_destinations);
        assert_runtime(&source, &target);
    }

    #[test]
    fn overlay_parent_is_preserved_without_forcing_guard_entry() {
        let mut source = object(GuardPhase::Idle);
        source.ai_state = AIState::Moving;
        let snapshot = capture(&source).unwrap().unwrap();
        let mut target = source.clone();
        target.unit_ai_runtime.clear_guard();
        assert!(restore(&snapshot, &mut target).unwrap());
        assert_eq!(target.ai_state, AIState::Moving);
        assert_runtime(&source, &target);
    }

    #[test]
    fn absent_phase_and_unrelated_module_are_inert() {
        let mut target = object(GuardPhase::Idle);
        let mut unrelated = capture(&target).unwrap().unwrap();
        unrelated.current_state = "AI_HUNT".into();
        unrelated.state_machine_data.clear();
        let before = target.clone();
        assert!(!restore(&unrelated, &mut target).unwrap());
        assert_runtime(&target, &before);
        target.unit_ai_runtime.clear_guard();
        assert!(capture(&target).unwrap().is_none());
    }

    #[test]
    fn malformed_missing_and_unknown_fields_are_rejected_inertly() {
        let mut target = object(GuardPhase::Idle);
        for encoded in [Some("{"), Some("{\"unknown\":true}"), None] {
            let mut snapshot = capture(&target).unwrap().unwrap();
            snapshot.state_machine_data.clear();
            if let Some(encoded) = encoded {
                snapshot
                    .state_machine_data
                    .insert(GUARD_KEY.into(), encoded.into());
            }
            reject_without_mutation(&snapshot, &mut target);
        }
        // Exercise a complete record plus an unknown field, rather than a
        // record already invalid because its required fields are absent.
        let mut snapshot = capture(&target).unwrap().unwrap();
        let mut encoded: serde_json::Value =
            serde_json::from_str(&snapshot.state_machine_data[GUARD_KEY]).unwrap();
        encoded["unknown"] = serde_json::Value::Bool(true);
        snapshot
            .state_machine_data
            .insert(GUARD_KEY.into(), serde_json::to_string(&encoded).unwrap());
        reject_without_mutation(&snapshot, &mut target);
    }

    #[test]
    fn version_goal_mode_and_parent_disagreement_are_rejected_inertly() {
        let mut target = object(GuardPhase::Idle);
        for edit in [
            (|s: &mut GuardSaveState| s.version = 2) as fn(&mut GuardSaveState),
            |s| s.return_goal = Some(Vec3::ZERO),
            |s| s.parent_state = AIState::Idle,
        ] {
            let mut snapshot = capture(&target).unwrap().unwrap();
            rewrite(&mut snapshot, edit);
            reject_without_mutation(&snapshot, &mut target);
        }
        let mut snapshot = capture(&target).unwrap().unwrap();
        snapshot.current_state = RETURN.into();
        reject_without_mutation(&snapshot, &mut target);
        snapshot.current_state = "AI_HUNT".into();
        reject_without_mutation(&snapshot, &mut target);
    }

    #[test]
    fn active_phase_requires_deadline_and_finite_geometry() {
        let mut source = object(GuardPhase::Idle);
        source.unit_ai_runtime.set_guard_scan_deadline(None);
        assert!(capture(&source).is_err());
        source.unit_ai_runtime.set_guard_scan_deadline(Some(0));
        source
            .unit_ai_runtime
            .set_guard_anchor(Some(Vec3::splat(f32::NAN)));
        assert!(capture(&source).is_err());
        source.unit_ai_runtime.set_guard_anchor(None);
        source
            .unit_ai_runtime
            .set_guard_phase(Some(GuardPhase::Return {
                goal: Vec3::splat(f32::INFINITY),
            }));
        assert!(capture(&source).is_err());
        source
            .unit_ai_runtime
            .set_guard_phase(Some(GuardPhase::Idle));
        assert!(capture(&source).is_ok());
        source.wander_width_factor = f32::INFINITY;
        assert!(capture(&source).is_err());
    }

    #[test]
    fn lifecycle_deadline_agreement_is_read_only_and_mismatch_fails() {
        let mut target = object(GuardPhase::Idle);
        let snapshot = capture(&target).unwrap().unwrap();
        let state = decode(&snapshot).unwrap().unwrap();
        assert!(validate_deadline(&state, &target).is_ok());
        target.unit_ai_runtime.set_guard_scan_deadline(Some(0));
        let before = target.clone();
        assert!(validate_deadline(&state, &target).is_err());
        assert_runtime(&target, &before);
        target.unit_ai_runtime.set_guard_scan_deadline(None);
        assert!(validate_deadline(&state, &target).is_err());
    }
}
