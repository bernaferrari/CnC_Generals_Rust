//! AIStateMachine.h:326: Busy update has no owner-dependent behavior.
use super::AIBusyState;
use crate::compat::legacy_state::LegacyStateAdapter;
use crate::state_machine::{StateImplementation, StateMachine, StateReturnType};

#[test]
fn busy_update_without_owner_continues() {
    let context = StateMachine::new(None, "busy-no-owner");
    let mut direct = AIBusyState::new(&context);
    let mut registered = LegacyStateAdapter::new(AIBusyState::new(&context));
    assert_eq!(direct.update(), StateReturnType::Continue);
    assert_eq!(registered.update(), StateReturnType::Continue);
}
