//! Owned flight transition and save/restore contracts; no movement simulation.

use super::{HostChinookAI, HostChinookAIState, HostChinookEvacuation, HostChinookFlightStatus};

#[test]
fn evacuation_entry_waits_for_actual_owner_removal_before_takeoff() {
    for (landing_state, effect, takeoff_state) in [
        (
            HostChinookAIState::LandAndEvac,
            HostChinookEvacuation::Takeoff,
            HostChinookAIState::TakingOff,
        ),
        (
            HostChinookAIState::LandAndEvacAndExit,
            HostChinookEvacuation::TakeoffAndExit,
            HostChinookAIState::TakeoffAndExit,
        ),
    ] {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 0.0]);
        ai.state = landing_state;
        ai.flight_status = HostChinookFlightStatus::Landing;
        ai.parent_idle = false;
        ai.contained_count = 2;
        assert_eq!(ai.update_from_observed_pose(), Some(effect));
        assert_eq!(ai.flight_status, HostChinookFlightStatus::Landed);
        assert_eq!(
            ai.contained_count, 2,
            "entering the phase cannot pretend the actual owner list is empty"
        );
        let pending_state = ai.state;
        let wrong_effect = if effect == HostChinookEvacuation::Takeoff {
            HostChinookEvacuation::TakeoffAndExit
        } else {
            HostChinookEvacuation::Takeoff
        };
        assert!(!ai.complete_evacuation(wrong_effect, 0));
        assert_eq!(ai.state, pending_state);
        assert!(!ai.complete_evacuation(effect, 1));
        assert_eq!(ai.state, pending_state);
        assert_eq!(ai.flight_status, HostChinookFlightStatus::Landed);
        assert_eq!(ai.update_from_observed_pose(), Some(effect));
        assert!(ai.complete_evacuation(effect, 0));
        assert_eq!(ai.state, takeoff_state);
        assert_eq!(ai.flight_status, HostChinookFlightStatus::TakingOff);
    }
}

#[test]
fn takeoff_arrival_defers_pending_command_until_next_callback() {
    let mut ai = HostChinookAI::new_combat([0.0, 0.0, 0.0]);
    ai.flight_status = HostChinookFlightStatus::Landed;
    let destination = [80.0, 0.0, 0.0];
    ai.command_evac(destination, false);
    assert_eq!(ai.state, HostChinookAIState::TakingOff);
    ai.parent_idle = false;
    ai.pos = ai.dest; // observation of the completed locomotor goal
    assert!(ai.update_from_observed_pose().is_none());
    assert_eq!(ai.state, HostChinookAIState::Idle);
    assert_eq!(ai.flight_status, HostChinookFlightStatus::Flying);
    assert_eq!(ai.pending_evac_dest, Some(destination));

    ai.parent_idle = true;
    ai.wanting_enter_or_exit = true;
    assert!(ai.update_from_observed_pose().is_none());
    assert_eq!(ai.state, HostChinookAIState::MoveToAndEvac);
    assert_eq!(ai.dest, destination);
    assert!(ai.pending_evac_dest.is_none());
}

#[test]
fn restored_idle_pending_command_precedes_auto_landing() {
    let mut ai = HostChinookAI::new_combat([0.0, 0.0, 100.0]);
    let destination = [80.0, 0.0, 0.0];
    ai.pending_evac_dest = Some(destination);
    ai.pending_evac_and_exit = true;
    ai.wanting_enter_or_exit = true;
    let saved = serde_json::to_string(&ai).expect("save owned flight state");
    let mut loaded: HostChinookAI =
        serde_json::from_str(&saved).expect("restore owned flight state");
    assert_eq!(loaded.state, HostChinookAIState::Idle);
    assert!(loaded.parent_idle);
    assert!(loaded.update_from_observed_pose().is_none());
    assert_eq!(loaded.state, HostChinookAIState::MoveToAndEvacAndExit);
    assert_eq!(loaded.dest, destination);
    assert!(loaded.pending_evac_dest.is_none());
    assert!(!loaded.pending_evac_and_exit);
    assert_eq!(loaded.flight_status, HostChinookFlightStatus::Flying);
}
