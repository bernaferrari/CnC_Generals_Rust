//! Bound the packed Forever sentinel on pathological restored logic frames.
//! C++ uses a 30-bit absolute wake field; treating its max as never-due avoids
//! the original rebalanced-sentinel loop when a restored frame reaches the cap.
use super::sleepy_parity_tests::counting_ptr;
use super::*;
use std::sync::atomic::Ordering;

fn prepare_owner(
    logic: &mut GameLogic,
    factory: &mut crate::object::object_factory::ObjectFactory,
) -> ObjectID {
    factory
        .prepare_object(
            logic,
            Arc::new(crate::common::DefaultThingTemplate::new(
                "ForeverSchedulerOwner".into(),
            )),
            crate::common::Coord3D::ZERO,
            None,
            crate::object::object_factory::ObjectCreationFlags::NO_AI,
        )
        .unwrap()
}

#[test]
fn forever_at_or_after_packed_frame_limit_remains_registered_without_running() {
    let mut logic = GameLogic::new();
    let mut factory = crate::object::object_factory::ObjectFactory::new();
    let id = prepare_owner(&mut logic, &mut factory);
    let (module, calls) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::Forever);
    logic.register_sleepy_update_module(id, module.clone(), UPDATE_SLEEP_FOREVER_FRAMES);
    for frame in [
        UPDATE_SLEEP_FOREVER_FRAMES - 1,
        UPDATE_SLEEP_FOREVER_FRAMES,
        UPDATE_SLEEP_FOREVER_FRAMES + 1,
        u32::MAX,
    ] {
        logic.frame = frame;
        assert!(logic.next_owned_scheduled_update(frame).unwrap().is_none());
        assert_eq!(
            logic.sleepy_entry_for(&module),
            Some((id, UPDATE_SLEEP_FOREVER_FRAMES))
        );
        assert_eq!(logic.sleepy_update_count(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn earlier_due_updates_keep_wake_order_before_forever_on_late_restored_frame() {
    let mut logic = GameLogic::new();
    let mut factory = crate::object::object_factory::ObjectFactory::new();
    let id = prepare_owner(&mut logic, &mut factory);
    let (forever, forever_calls) =
        counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::Forever);
    let (earlier, earlier_calls) =
        counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::Forever);
    let (later, later_calls) = counting_ptr(DisabledMaskType::empty(), UpdateSleepTime::Forever);
    logic.register_sleepy_update_module(id, forever.clone(), UPDATE_SLEEP_FOREVER_FRAMES);
    logic.register_sleepy_update_module(id, later.clone(), 5);
    logic.register_sleepy_update_module(id, earlier.clone(), 3);
    logic.frame = u32::MAX;
    for expected in [&earlier, &later] {
        let scheduled = logic
            .next_owned_scheduled_update(u32::MAX)
            .unwrap()
            .unwrap();
        assert_eq!(scheduled.object_id, id);
        assert!(scheduled.should_process);
        assert!(Arc::ptr_eq(&scheduled.module, expected));
        let sleep = scheduled.module.write().unwrap().update().unwrap();
        logic.finish_owned_scheduled_update(u32::MAX, scheduled, sleep);
    }
    assert!(
        logic
            .next_owned_scheduled_update(u32::MAX)
            .unwrap()
            .is_none()
    );
    assert_eq!(logic.sleepy_update_count(), 3);
    assert_eq!(earlier_calls.load(Ordering::SeqCst), 1);
    assert_eq!(later_calls.load(Ordering::SeqCst), 1);
    assert_eq!(forever_calls.load(Ordering::SeqCst), 0);
    for module in [&forever, &earlier, &later] {
        assert_eq!(
            logic.sleepy_entry_for(module),
            Some((id, UPDATE_SLEEP_FOREVER_FRAMES))
        );
    }
}
