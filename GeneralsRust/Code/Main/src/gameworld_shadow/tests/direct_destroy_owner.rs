//! A presentation-health copy must not resurrect an owner-issued deletion.
use super::*;

#[test]
fn coupled_direct_destroy_preserves_owner_body_through_health_writeback() {
    let mut logic = GameLogic::new();
    ensure_template(&mut logic, "DirectDestroyBoundary", 100.0);
    let id = logic
        .create_object("DirectDestroyBoundary", Team::USA, Vec3::ZERO)
        .unwrap();
    let object = logic.host_object_mut(id).unwrap();
    object.health.current = 40.0;
    object.previous_health = 50.0;
    let mut shadow = GameWorldShadow::new(16);
    shadow.sync_from_host(&logic);
    logic.health_events.clear_damage();

    logic.destroy_object(id);
    assert!(logic.host_object(id).unwrap().status.destroyed);
    shadow.sync_from_host(&logic);
    let object = logic.host_object(id).unwrap();
    assert!(
        object.status.destroyed,
        "positive HP does not cancel destroyObject"
    );
    assert!(!object.status.on_die_started);
    assert_eq!(
        (object.health.current, object.previous_health),
        (40.0, 50.0)
    );
    assert!(logic.has_pending_destroy_work());
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    let object = logic
        .host_object(id)
        .expect("observer retains pending owner");
    assert!(object.status.destroyed);
    assert!(!object.status.on_die_started);
    assert_eq!(
        (object.health.current, object.previous_health),
        (40.0, 50.0)
    );
    assert!(logic.objects.ordered_ids().contains(&id));
    assert!(shadow.entity_for_host(id).is_some());
    assert!(logic.has_pending_destroy_work());
    logic
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert!(logic.host_object(id).is_none());
    assert!(shadow.entity_for_host(id).is_none());
    logic.process_destroy_list();
    assert!(logic.host_object(id).is_none());
    logic.health_events.clear_damage();
}
