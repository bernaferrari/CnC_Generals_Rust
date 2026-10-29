use super::*;

struct RestoreGlobals {
    bundle: Arc<WorldServices>,
    original: Option<HostRuntimeGlobals>,
}
impl Drop for RestoreGlobals {
    fn drop(&mut self) {
        let _lock = world_runtime_transaction_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let current = HostRuntimeGlobals::take_from_singletons(&self.bundle);
        drop(current);
        drop(
            self.original
                .take()
                .unwrap()
                .install_into_singletons(&self.bundle),
        );
    }
}
fn seed(marker: u32) {
    player_list()
        .write()
        .unwrap()
        .add_player(Arc::new(std::sync::RwLock::new(
            crate::player::Player::new(marker as i32),
        )));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_counter("ReservationMarker", marker as i32)
        .unwrap();
    *get_script_engine().write().unwrap() = Some(engine);
    let mut dict = game_engine::common::dict::Dict::new();
    dict.set_ascii_string(
        game_engine::common::well_known_keys::key_team_on_unit_destroyed_script(),
        "NotExecutedByNativeCommit",
    );
    let mut factory = get_team_factory().lock().unwrap();
    let prototype = factory
        .init_team("ReservationTeam".into(), "".into(), false, Some(&dict))
        .unwrap();
    let team = factory
        .create_team_on_prototype_with_id(&prototype, marker + 20)
        .unwrap();
    factory.set_next_team_id(marker + 100);
    drop(factory);
    team.write().unwrap().notify_team_of_object_death();
}
fn observe(marker: u32) {
    assert!(
        player_list()
            .read()
            .unwrap()
            .get_player(marker as i32)
            .is_some()
    );
    assert_eq!(
        get_team_factory().lock().unwrap().get_next_team_id(),
        marker + 100
    );
    assert_eq!(
        get_script_engine()
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .get_counter("ReservationMarker")
            .unwrap()
            .value,
        marker as i32
    );
    let queue = take_pending_team_script_events_for_world_boundary();
    assert_eq!(queue.event_count_for_world_transaction_test(), 1);
    let replaced = replace_pending_team_script_events_for_world_boundary(queue);
    assert_eq!(replaced.event_count_for_world_transaction_test(), 0);
}
fn empty_receipt() {
    assert!(
        defer_team_factory_effects_if_staging(
            TeamFactoryDeferredEffects::empty_for_world_transaction_test()
        )
        .is_ok()
    );
}

#[test]
fn detached_a_allows_b_failed_c_drop_and_commit_interleavings_without_reserving_singletons() {
    let _serial = crate::test_sync::lock();
    let live = Arc::new(WorldServices::new_for_world());
    let original = HostRuntimeGlobals::take_from_singletons(&live);
    let _restore = RestoreGlobals {
        bundle: Arc::clone(&live),
        original: Some(original),
    };
    seed(1);
    let a_services = Arc::new(WorldServices::new_for_world());
    let a_stage = HostRuntimeWorldStage::begin(&live);
    seed(2);
    empty_receipt();
    empty_receipt();
    let a = a_stage.finish_and_restore_live(&a_services);
    assert!(
        world_runtime_transaction_lock().try_lock().is_ok(),
        "detached A must not reserve stores"
    );
    assert!(!world_runtime_staging_active());
    observe(1);
    // seed's TeamFactoryGuard drops one legitimate empty receipt, followed
    // by the two explicit empty receipts; preserve every original visit.
    assert_eq!(a.team_factory_effects.len(), 3);
    assert!(
        a.team_factory_effects
            .iter()
            .all(|e| e.pending_callback_counts() == (0, 0))
    );
    assert_eq!(
        a.globals
            .pending_team_script_events
            .event_count_for_world_transaction_test(),
        1
    );

    let b_services = Arc::new(WorldServices::new_for_world());
    let b_stage = HostRuntimeWorldStage::begin(&live);
    seed(3);
    empty_receipt();
    let b = b_stage.finish_and_restore_live(&b_services);
    observe(1);
    // One seed guard receipt followed by the explicit empty receipt.
    assert_eq!(b.team_factory_effects.len(), 2);
    assert!(
        b.team_factory_effects
            .iter()
            .all(|e| e.pending_callback_counts() == (0, 0))
    );
    // Failed C rolls back while both detached tokens remain held.
    let c_stage = HostRuntimeWorldStage::begin(&live);
    seed(4);
    assert!(
        defer_team_factory_effects_if_staging(
            TeamFactoryDeferredEffects::pending_for_world_transaction_test(true, true)
        )
        .is_ok()
    );
    drop(c_stage);
    observe(1);
    assert!(!world_runtime_staging_active());
    assert!(
        WorldRuntimeStageScope::enter().finish().is_empty(),
        "C rollback discards both seed's empty receipt and its pending callback receipt"
    );
    drop(b);
    observe(1);

    let committed = a.preflight_native().unwrap().install_globals();
    assert!(
        world_runtime_transaction_lock().try_lock().is_err(),
        "raw install retains exclusion until Main commit completes"
    );
    observe(2);
    committed.finish_after_logic_commit();
    assert!(world_runtime_transaction_lock().try_lock().is_ok());
    // A later reset-stage rollback preserves the just-committed A values.
    let reset = HostRuntimeWorldStage::begin(&a_services);
    seed(5);
    drop(reset);
    observe(2);
    engine_stores::uninstall_services(&a_services);
}

#[test]
fn native_rejection_drops_detached_receipts_without_callbacks_and_releases_reservation() {
    let _serial = crate::test_sync::lock();
    let live = Arc::new(WorldServices::new_for_world());
    let original = HostRuntimeGlobals::take_from_singletons(&live);
    let _restore = RestoreGlobals {
        bundle: Arc::clone(&live),
        original: Some(original),
    };
    seed(1);
    let stage = HostRuntimeWorldStage::begin(&live);
    seed(2);
    assert!(
        defer_team_factory_effects_if_staging(
            TeamFactoryDeferredEffects::pending_for_world_transaction_test(true, true)
        )
        .is_ok()
    );
    let candidate = stage.finish_and_restore_live(&live);
    assert!(world_runtime_transaction_lock().try_lock().is_ok());
    assert_eq!(candidate.team_factory_effects.len(), 2);
    assert_eq!(
        candidate.team_factory_effects[0].pending_callback_counts(),
        (0, 0)
    );
    assert_eq!(
        candidate.team_factory_effects[1].pending_callback_counts(),
        (1, 1)
    );
    assert!(candidate.preflight_native().is_err());
    observe(1);
    assert!(world_runtime_transaction_lock().try_lock().is_ok());
    observe(1);
}
