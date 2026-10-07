//! AIStateMachine.h:324-326: Busy update always continues.
use super::*;
use crate::ai::states::AIBusyState;
use crate::state_machine::cpp_state::CppStateAdapter;
use crate::state_machine::{StateImplementation, StateMachine, StateReturnType};

fn callback_bodies(actual: &FactoryRuntime) -> (AIBusyState, CppStateAdapter<AIBusyState>) {
    // Concrete callback fixtures use the factory's admitted Object. This
    // construction never replaces its cached AI or installed state machine.
    let context = StateMachine::new(Some(Arc::downgrade(&actual.owner)), "busy-callback-oracle");
    (
        AIBusyState::new(&context),
        CppStateAdapter::new(AIBusyState::new(&context)),
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
        // CPP privateBusy sets the incoming source once; Busy OnEnter has
        // no effect that can replay this command during ordinary update.
        assert_eq!(
            ai.get_last_command_source(),
            crate::ai::CommandSourceType::FromPlayer,
        );
        assert!(Arc::ptr_eq(
            &actual.ai,
            &actual
                .owner
                .read()
                .unwrap()
                .get_ai_update_interface()
                .unwrap(),
        ));
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

#[test]
fn busy_callback_enter_does_not_borrow_owner() {
    if !child(concat!(
        module_path!(),
        "::busy_callback_enter_does_not_borrow_owner"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let (mut direct, mut registered) = callback_bodies(&actual);
    let _held = actual.owner.write().unwrap();
    // AIStateMachine.h:324: onEnter is constant Continue, even while the
    // driving Object is already borrowed. The bounded child catches reentry.
    assert_eq!(direct.on_enter(), StateReturnType::Continue);
    assert_eq!(registered.on_enter(), StateReturnType::Continue);
}

#[test]
fn busy_callback_enter_needs_no_owner() {
    if !child(concat!(
        module_path!(),
        "::busy_callback_enter_needs_no_owner"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let context = StateMachine::new(None::<std::sync::Weak<RwLock<Object>>>, "busy-enter-oracle");
    let mut direct = AIBusyState::new(&context);
    let mut registered = CppStateAdapter::new(AIBusyState::new(&context));
    assert_eq!(direct.on_enter(), StateReturnType::Continue);
    assert_eq!(registered.on_enter(), StateReturnType::Continue);
}
