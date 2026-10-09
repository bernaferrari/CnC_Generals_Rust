//! A presentation-health copy must not resurrect an owner-issued deletion.
use super::*;

#[test]
fn coupled_direct_destroy_preserves_owner_body_through_health_writeback() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .set("GENERALS_GAMEWORLD_DEFERRED_DESTROY", "0");
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
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_destroy_ready_log::clear();
    let _couple = ShadowCoupleGuard::enter();
    with_coupled_shadow(&mut shadow, || logic.destroy_object(id));
    assert!(logic.host_object(id).unwrap().status.destroyed);
    shadow.writeback_health_to_host(&mut logic);
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
    assert_eq!(
        crate::game_logic::host_destroy_ready_log::pending_count(),
        0
    );
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    let object = logic.host_object(id).unwrap();
    assert!(object.status.destroyed);
    assert!(!object.status.on_die_started);
    assert_eq!(
        (object.health.current, object.previous_health),
        (40.0, 50.0)
    );
    logic.process_destroy_list();
    assert!(logic.host_object(id).is_none());
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_destroy_ready_log::clear();
}
