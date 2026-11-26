//! Owned flight transition and save/restore contracts; no movement simulation.

use super::{ChinookAi, ChinookAiState, ChinookEvacuation, ChinookFlightStatus};

#[test]
fn evacuation_entry_waits_for_actual_owner_removal_before_takeoff() {
    for (landing_state, effect, takeoff_state) in [
        (
            ChinookAiState::LandAndEvac,
            ChinookEvacuation::Takeoff,
            ChinookAiState::TakingOff,
        ),
        (
            ChinookAiState::LandAndEvacAndExit,
            ChinookEvacuation::TakeoffAndExit,
            ChinookAiState::TakeoffAndExit,
        ),
    ] {
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 0.0]);
        ai.state = landing_state;
        ai.flight_status = ChinookFlightStatus::Landing;
        ai.parent_idle = false;
        ai.contained_count = 2;
        assert_eq!(ai.update_from_observed_pose(), Some(effect));
        assert_eq!(ai.flight_status, ChinookFlightStatus::Landed);
        assert_eq!(
            ai.contained_count, 2,
            "entering the phase cannot pretend the actual owner list is empty"
        );
        let pending_state = ai.state;
        let wrong_effect = if effect == ChinookEvacuation::Takeoff {
            ChinookEvacuation::TakeoffAndExit
        } else {
            ChinookEvacuation::Takeoff
        };
        assert!(!ai.complete_evacuation(wrong_effect, 0));
        assert_eq!(ai.state, pending_state);
        assert!(!ai.complete_evacuation(effect, 1));
        assert_eq!(ai.state, pending_state);
        assert_eq!(ai.flight_status, ChinookFlightStatus::Landed);
        assert_eq!(ai.update_from_observed_pose(), Some(effect));
        assert!(ai.complete_evacuation(effect, 0));
        assert_eq!(ai.state, takeoff_state);
        assert_eq!(ai.flight_status, ChinookFlightStatus::TakingOff);
    }
}

#[test]
fn takeoff_arrival_defers_pending_command_until_next_callback() {
    let mut ai = ChinookAi::new_combat([0.0, 0.0, 0.0]);
    ai.flight_status = ChinookFlightStatus::Landed;
    let destination = [80.0, 0.0, 0.0];
    ai.command_evac(destination, false);
    assert_eq!(ai.state, ChinookAiState::TakingOff);
    ai.parent_idle = false;
    ai.pos = ai.dest; // observation of the completed locomotor goal
    assert!(ai.update_from_observed_pose().is_none());
    assert_eq!(ai.state, ChinookAiState::Idle);
    assert_eq!(ai.flight_status, ChinookFlightStatus::Flying);
    assert_eq!(ai.pending_evac_dest, Some(destination));

    ai.parent_idle = true;
    ai.wanting_enter_or_exit = true;
    assert!(ai.update_from_observed_pose().is_none());
    assert_eq!(ai.state, ChinookAiState::MoveToAndEvac);
    assert_eq!(ai.dest, destination);
    assert!(ai.pending_evac_dest.is_none());
}

#[test]
fn restored_idle_pending_command_precedes_auto_landing() {
    let mut ai = ChinookAi::new_combat([0.0, 0.0, 100.0]);
    let destination = [80.0, 0.0, 0.0];
    ai.pending_evac_dest = Some(destination);
    ai.pending_evac_and_exit = true;
    ai.wanting_enter_or_exit = true;
    let saved = serde_json::to_string(&ai).expect("save owned flight state");
    let mut loaded: ChinookAi = serde_json::from_str(&saved).expect("restore owned flight state");
    assert_eq!(loaded.state, ChinookAiState::Idle);
    assert!(loaded.parent_idle);
    assert!(loaded.update_from_observed_pose().is_none());
    assert_eq!(loaded.state, ChinookAiState::MoveToAndEvacAndExit);
    assert_eq!(loaded.dest, destination);
    assert!(loaded.pending_evac_dest.is_none());
    assert!(!loaded.pending_evac_and_exit);
    assert_eq!(loaded.flight_status, ChinookFlightStatus::Flying);
}
