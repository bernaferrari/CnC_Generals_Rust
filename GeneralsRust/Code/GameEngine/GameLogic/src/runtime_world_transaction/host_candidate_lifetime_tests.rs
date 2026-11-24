use super::*;
use crate::player::Player;
use std::sync::RwLock;

#[test]
fn save_stage_marker_belongs_to_receiving_factory_even_with_foreign_world_published() {
    let receiving = Arc::new(WorldServices::new_for_world());
    let foreign = Arc::new(WorldServices::new_for_world());

    engine_stores::with_world_services(&foreign, || {
        let stage = HostRuntimeWorldStage::begin(&receiving);
        assert!(
            !world_runtime_staging_active(),
            "foreign world must remain unstaged"
        );
        engine_stores::with_world_services(&receiving, || {
            assert!(
                world_runtime_staging_active(),
                "receiving world must be staged"
            );
        });
        drop(stage);
        assert!(!world_runtime_staging_active());
        engine_stores::with_world_services(&receiving, || {
            assert!(
                !world_runtime_staging_active(),
                "rollback clears the receiving stage"
            );
        });
    });
}

fn seed(services: &WorldServices, marker: u32) {
    services
        .players()
        .write()
        .unwrap()
        .add_player(Arc::new(RwLock::new(Player::new(marker as i32))));
    services
        .team_factory()
        .lock()
        .unwrap()
        .set_next_team_id(marker);
}

fn expect_marker(services: &WorldServices, marker: u32) {
    assert!(
        services
            .players()
            .read()
            .unwrap()
            .get_player(marker as i32)
            .is_some()
    );
    assert_eq!(
        services.team_factory().lock().unwrap().get_next_team_id(),
        marker
    );
}

#[test]
fn independent_candidates_keep_their_metadata_and_rollback_clears_only_its_stage() {
    let live = Arc::new(WorldServices::new_for_world());
    let a = Arc::new(WorldServices::new_for_world());
    let b = Arc::new(WorldServices::new_for_world());
    seed(&live, 1);

    engine_stores::with_world_services(&live, || {
        let a_stage = HostRuntimeWorldStage::begin(&a);
        seed(&a, 2);
        let b_stage = HostRuntimeWorldStage::begin(&b);
        seed(&b, 3);
        expect_marker(&live, 1);
        assert!(engine_stores::is_services_active(&live));

        let a_candidate = a_stage.finish();
        assert!(!world_runtime_staging_active_in(&a));
        assert!(world_runtime_staging_active_in(&b));
        drop(b_stage);
        assert!(!world_runtime_staging_active_in(&b));
        expect_marker(&a, 2);
        expect_marker(&live, 1);
        assert!(engine_stores::is_services_active(&live));

        a_candidate.preflight_native(&a).unwrap().publish_services();
        assert!(engine_stores::is_services_active(&a));
        expect_marker(&a, 2);
        assert!(engine_stores::uninstall_services(&a));
        assert!(engine_stores::is_services_active(&live));
    });
}

#[test]
fn save_preflight_rejects_a_candidate_for_a_different_owner_before_publication() {
    let live = Arc::new(WorldServices::new_for_world());
    let candidate = Arc::new(WorldServices::new_for_world());
    engine_stores::with_world_services(&live, || {
        let staged = HostRuntimeWorldStage::begin(&candidate).finish();
        let result = staged.preflight_native(&live);
        assert_eq!(
            result.err().as_deref(),
            Some("native save stage belongs to a different world")
        );
        assert!(engine_stores::is_services_active(&live));
    });
}
