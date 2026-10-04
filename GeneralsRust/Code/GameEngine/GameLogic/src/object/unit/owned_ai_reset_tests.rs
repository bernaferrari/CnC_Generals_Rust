//! The registered legacy adapter is still a distinct execution boundary.
//! CPP AIUpdate.cpp:4443–4448 resets the AI-owned clock with forceIdle rules.
use super::*;
use crate::common::DefaultThingTemplate;
use crate::object::unit::Unit;

#[test]
fn registered_legacy_unit_reset_updates_owned_timer_and_preserves_scan_stamp() {
    if !child(concat!(
        module_path!(),
        "::registered_legacy_unit_reset_updates_owned_timer_and_preserves_scan_stamp"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = RestoreAmbientFrame::set(97);
    let idle_frames = crate::ai::the_ai()
        .read()
        .unwrap()
        .get_ai_data()
        .force_idle_frames_count;
    let id = 0xA1_F0_17;
    assert!(crate::object::unit::registry::get_unit_arc(id).is_none());
    assert!(
        crate::object::registry::OBJECT_REGISTRY
            .get_object(id)
            .is_none()
    );
    let owner = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    // A genuine standalone Unit exercises the existing registered adapter.
    // It is not a replacement or added registration for a factory-owned Unit.
    let unit = Arc::new(RwLock::new(
        Unit::new(owner, &DefaultThingTemplate::new("LegacyMoodUnit".into())).unwrap(),
    ));
    unit.write().unwrap().last_target_scan_frame = 11;
    let mut ai = runtime(id);
    crate::object::unit::register_unit(id, &unit);
    struct Unregister(ObjectID);
    impl Drop for Unregister {
        fn drop(&mut self) {
            crate::object::unit::unregister_unit(self.0);
            crate::object::registry::OBJECT_REGISTRY.unregister_object(self.0);
            crate::ai::object_registry::unregister_legacy_object(self.0);
        }
    }
    let _registration = Unregister(id);
    ai.set_next_mood_check_time(241);
    ai.reset_next_mood_check_time();
    assert_eq!(
        unit.read().unwrap().last_target_scan_frame,
        97,
        "preserve the independent legacy Unit scan-kernel stamp"
    );
    assert_eq!(
        ai.get_next_mood_check_time(),
        97_u32.wrapping_add(idle_frames),
        "CPP resets owned timer with forceIdle, not the Unit scan interval"
    );
    assert!(ai.take_random_mood_offset());
    assert!(!ai.take_random_mood_offset());
}

#[test]
fn factory_reset_absent_legacy_unit_does_not_discover_ambient_clock() {
    if !child(concat!(
        module_path!(),
        "::factory_reset_absent_legacy_unit_does_not_discover_ambient_clock"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let mut ai = actual.ai.lock().unwrap();
    ai.set_next_mood_check_time(241);
    let foreign = crate::system::game_logic::get_game_logic().lock().unwrap();
    // hq-6tx39 must supply real driving frame/rules before enabling this
    // legacy adapter for an ordinary factory runtime. Do not add re-entry.
    ai.reset_next_mood_check_time();
    assert_eq!(ai.get_next_mood_check_time(), 241);
    assert!(!ai.take_random_mood_offset());
    drop(foreign);
    drop(ai);
}
