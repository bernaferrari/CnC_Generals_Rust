//! AIStateMachine.h:324-326: Busy update always continues.
use super::*;
use crate::ai::states::AIBusyState;
use crate::compat::legacy_state::LegacyStateAdapter;
use crate::state_machine::{StateImplementation, StateMachine, StateReturnType};

fn callback_bodies(actual: &FactoryRuntime) -> (AIBusyState, LegacyStateAdapter<AIBusyState>) {
    // Concrete callback fixtures use the factory's admitted Object. This
    // construction never replaces its cached AI or installed state machine.
    let context = StateMachine::new(Some(Arc::downgrade(&actual.owner)), "busy-callback-oracle");
    (
        AIBusyState::new(&context),
        LegacyStateAdapter::new(AIBusyState::new(&context)),
    )
}

#[test]
fn busy_update_ignores_owner_idle_mirror() {
    if !child(concat!(
        module_path!(),
        "::busy_update_ignores_owner_idle_mirror"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let (mut direct, mut registered) = callback_bodies(&actual);
    for idle in [false, true] {
        actual.owner.write().unwrap().ai_fire_is_idle = idle;
        assert_eq!(direct.update(), StateReturnType::Continue);
        assert_eq!(registered.update(), StateReturnType::Continue);
    }
}

#[test]
fn busy_callback_update_does_not_borrow_owner() {
    if !child(concat!(
        module_path!(),
        "::busy_callback_update_does_not_borrow_owner"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let (mut direct, mut registered) = callback_bodies(&actual);
    let _held = actual.owner.write().unwrap();
    // Busy has no owner-dependent update behavior in C++; the write guard
    // detects even a read that would otherwise return the same status.
    assert_eq!(direct.update(), StateReturnType::Continue);
    assert_eq!(registered.update(), StateReturnType::Continue);
}

#[test]
fn factory_busy_remains_busy_across_ordinary_updates() {
    if !child(concat!(
        module_path!(),
        "::factory_busy_remains_busy_across_ordinary_updates"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let mut ai = actual.ai.lock().unwrap();
    ai.execute_command(&crate::ai::AiCommandParams::new(
        crate::ai::AiCommandType::Busy,
        crate::ai::CommandSourceType::FromPlayer,
    ))
    .unwrap();
    for frame in [17, 18] {
        let _frame = RestoreAmbientFrame::set(frame);
        ai.update().unwrap();
        assert_eq!(
            ai.get_current_state_id(),
            Some(crate::ai::states::AIStateType::Busy as u32)
        );
        assert!(ai.is_busy());
    }
    assert!(super::super::registry::get_unit_arc(actual.id).is_none());
    assert!(Arc::ptr_eq(
        &actual.ai,
        &actual
            .owner
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap()
    ));
}
