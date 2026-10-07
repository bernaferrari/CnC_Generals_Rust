//! Common/StateMachine.h:132-136: moving and attacking are not busy.
//! AIStateMachine.h:327: only AIBusyState overrides the false base result.
use super::*;
use crate::compat::legacy_state::LegacyStateAdapter;
use crate::state_machine::{StateImplementation, StateMachine};

fn assert_not_busy<S: crate::compat::ClassicState + StateImplementation + 'static>(state: S) {
    // The direct implementation and the adapter used by native registration
    // must agree. Query concrete bodies without entering or advancing them.
    assert!(!StateImplementation::is_busy(&state));
    let registered = LegacyStateAdapter::new(state);
    assert!(!StateImplementation::is_busy(&registered));
}

macro_rules! movement_is_not_busy {
    ($name:ident, $constructor:expr) => {
        #[test]
        fn $name() {
            let machine = StateMachine::new(None, "cpp-busy-classification");
            assert_not_busy($constructor(&machine));
        }
    };
}

movement_is_not_busy!(move_to_is_not_busy, AIMoveToState::new);
movement_is_not_busy!(attack_move_to_is_not_busy, AIAttackMoveToState::new);
movement_is_not_busy!(move_and_evacuate_is_not_busy, |machine| {
    AIMoveAndEvacuateState::new(machine, "AIMoveAndEvacuate")
});
movement_is_not_busy!(
    team_waypoint_is_not_busy,
    AIFollowWaypointPathAsTeamState::new
);
movement_is_not_busy!(
    team_exact_waypoint_is_not_busy,
    AIFollowWaypointPathAsTeamExactState::new
);
movement_is_not_busy!(
    individual_waypoint_is_not_busy,
    AIFollowWaypointPathAsIndividualsState::new
);
movement_is_not_busy!(
    individual_exact_waypoint_is_not_busy,
    AIFollowWaypointPathAsIndividualsExactState::new
);

#[test]
fn attack_with_victim_is_not_busy() {
    let machine = StateMachine::new(None, "cpp-attack-busy");
    for (force, follow) in [(false, false), (true, false), (false, true)] {
        let mut state = AIAttackObjectState::new(&machine, force, follow);
        state.target_id = 123;
        assert!(crate::compat::ClassicState::classic_is_attack(&state));
        assert_not_busy(state);
    }
}

#[test]
fn only_explicit_busy_state_is_busy() {
    let machine = StateMachine::new(None, "cpp-busy-controls");
    let busy = AIBusyState::new(&machine);
    assert!(StateImplementation::is_busy(&busy));
    assert!(StateImplementation::is_busy(&LegacyStateAdapter::new(busy)));
    let idle = LegacyStateAdapter::new(AIIdleState::new(&machine, true));
    assert!(StateImplementation::is_idle(&idle));
    assert!(!StateImplementation::is_busy(&idle));
}
