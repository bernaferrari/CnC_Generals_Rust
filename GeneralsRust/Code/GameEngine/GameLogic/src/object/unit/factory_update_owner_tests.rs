//! The update mirror and pending-command drain must use the factory AI's exact
//! construction owner even when a different Object with the same ID is in the
//! legacy registry.
use super::*;

#[test]
fn factory_update_mirror_and_drain_stay_on_exact_owner_with_same_id_canary() {
    if !child(concat!(
        module_path!(),
        "::factory_update_mirror_and_drain_stay_on_exact_owner_with_same_id_canary"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = FactoryRuntime::new();
    let foreign = Arc::new(RwLock::new(Object::new_test(actual.id, 200.0)));
    {
        let mut foreign = foreign.write().unwrap();
        foreign.ai_pending_state_id = Some(crate::ai::states::AIStateType::Busy as u32);
        foreign.ai_pending_desired_speed = Some(987.25);
        foreign.ai_fire_desired_speed = -987.25;
    }
    crate::object::registry::OBJECT_REGISTRY.register_object(actual.id, &foreign);
    actual.owner.write().unwrap().ai_fire_desired_speed = -42.0;

    actual.ai.lock().unwrap().update().unwrap();

    {
        let foreign = foreign.read().unwrap();
        assert_eq!(
            foreign.ai_pending_state_id,
            Some(crate::ai::states::AIStateType::Busy as u32)
        );
        assert_eq!(foreign.ai_pending_desired_speed, Some(987.25));
        assert_eq!(foreign.ai_fire_desired_speed, -987.25);
    }
    assert_ne!(actual.owner.read().unwrap().ai_fire_desired_speed, -42.0);
    crate::object::registry::OBJECT_REGISTRY.register_object(actual.id, &actual.owner);
}
