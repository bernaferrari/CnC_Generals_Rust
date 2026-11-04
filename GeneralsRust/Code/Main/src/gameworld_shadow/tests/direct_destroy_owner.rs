//! A presentation-health copy must not resurrect an owner-issued deletion.
use super::*;

/// Binds CommonServices + WorldServices + MainServices the shadow world needs.
fn bind_three_services() {
    if game_engine::common::common_services::try_common_services().is_none() {
        game_engine::common::common_services::install_common_services(std::sync::Arc::new(
            game_engine::common::common_services::CommonServices::new(),
        ));
    }
    if gamelogic::system::engine_stores::try_active_services().is_none() {
        gamelogic::system::engine_stores::install_services(
            gamelogic::system::engine_stores::new_world_services(),
        );
    }
    if crate::main_services::try_main_services().is_none() {
        crate::main_services::install_main_services(std::sync::Arc::new(
            crate::main_services::MainServices::new(),
        ));
    }
}

#[test]
fn coupled_direct_destroy_preserves_owner_body_through_health_writeback() {
    bind_three_services();
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

/// hq-2qedt.25: C++ TensileFormationUpdate.cpp:243 setDamageState(BODY_RUBBLE)
/// -> ActiveBody::internalChangeHealth (ActiveBody.cpp:1188-1228) zeroes HP and
/// marks effectivelyDead without onDie/destroyObject. The shadow boundary must
/// not turn that zero-HP rubble owner into a destroyed/deleted object.
#[test]
fn coupled_tensile_rubble_zero_hp_owner_is_not_destroyed_by_shadow() {
    bind_three_services();
    let mut logic = GameLogic::new();
    ensure_template(&mut logic, "TensileRubbleBoundary", 100.0);
    let id = logic
        .create_object("TensileRubbleBoundary", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut shadow = GameWorldShadow::new(16);
    shadow.sync_from_host(&logic);
    logic.health_events.clear_damage();
    {
        let object = logic.host_object_mut(id).unwrap();
        let current = object.health.current;
        object.internal_change_health(-current);
        assert_eq!(object.health.current, 0.0);
        assert!(object.status.effectively_dead);
        assert!(!object.status.destroyed);
    }
    for _ in 0..3 {
        shadow.sync_from_host(&logic);
        let _ = observe_completed_owner(&mut shadow, &mut logic);
    }
    let object = logic
        .host_object(id)
        .expect("tensile rubble owner must survive shadow boundary");
    assert!(!object.status.destroyed, "rubble is not destroyObject");
    assert!(!object.status.on_die_started, "rubble does not run onDie");
    assert!(!logic.has_pending_destroy_work());
    assert!(shadow.entity_for_host(id).is_some());
}
