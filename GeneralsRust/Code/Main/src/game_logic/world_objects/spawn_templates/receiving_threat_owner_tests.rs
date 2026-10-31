//! Actual immutable definition -> Object factory partition threat, not a name lookup.
use super::*;

const NAME: &str = "AmericaTankThreatOwner";
fn source(threat: u16) -> String {
    format!(
        "Object {NAME}\nKindOf = VEHICLE\nVisionRange = 80\nShroudClearingRange = 80\nThreatValue = {threat}\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n"
    )
}
fn receiving(threat: u16) -> GameLogic {
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "receiving-threat.ini".to_owned(),
        source(threat),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    install_player(&mut world);
    world
}
fn install_player(world: &mut GameLogic) {
    // Prepare the receiving default team before strict TeamFactory restore,
    // as SidesList::validateSides / Player::setDefaultTeam do at new game.
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "Receiving threat owner",
        true,
    ));
}
fn reset_receiving(world: &mut GameLogic) {
    world.reset();
    install_player(world);
}
fn assert_partition_stamp(world: &GameLogic, id: ObjectId, expected: u32) {
    let object = world.host_object(id).unwrap();
    let stamp = object
        .partition_last_affect
        .expect("actual factory/maintenance partition affect stamp");
    assert_eq!(
        stamp.threat, expected,
        "actual partition affect uses receiving threat"
    );
    assert_eq!(
        stamp.mask, 1,
        "the actual controlling player owns the stamp"
    );
    assert_eq!(stamp.range, 80.0);
}
fn spawn(world: &mut GameLogic, expected: u32) -> ObjectId {
    let id = world.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(
        world.host_object(id).unwrap().partition_threat_value,
        expected,
        "actual Object partition threat must come from its exact receiving template version"
    );
    assert_partition_stamp(world, id, expected);
    id
}

#[test]
fn receiving_threat_factory_stamp_ignores_same_name_foreign_factory_and_peer() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_threat_factory_stamp_ignores_same_name_foreign_factory_and_peer",
        || {
            let mut first = receiving(17);
            let mut peer = receiving(91);
            let mut zero = receiving(0);
            assert_eq!(
                game_engine::common::thing::thing_factory::load_templates_from_ini_text(
                    &source(999),
                    "foreign-threat.ini"
                ),
                1
            );
            let compatibility = ThingTemplate::new(NAME);
            assert_eq!(
                compatibility.get_threat_value(),
                999,
                "explicit None compatibility retains the actual legacy factory path"
            );
            let a = spawn(&mut first, 17);
            let b = spawn(&mut peer, 91);
            let c = spawn(&mut zero, 0);
            assert_eq!(a, b, "same object ID does not select the foreign owner");
            assert_eq!(
                a, c,
                "authored zero shares IDs but never the foreign threat"
            );
            let held = game_engine::common::thing::get_thing_factory().unwrap();
            assert_eq!(first.templates[NAME].get_threat_value(), 17);
            assert_eq!(peer.templates[NAME].get_threat_value(), 91);
            assert_eq!(zero.templates[NAME].get_threat_value(), 0);
            spawn(&mut first, 17);
            spawn(&mut zero, 0);
            drop(held);
            reset_receiving(&mut peer);
            assert_eq!(first.host_object(a).unwrap().partition_threat_value, 17);
            spawn(&mut peer, 91);
        },
    );
}

#[test]
fn receiving_threat_map_solo_zero_reset_and_encoded_continuation_use_effective_version() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_threat_map_solo_zero_reset_and_encoded_continuation_use_effective_version",
        || {
            let mut world = receiving(7);
            let mut peer = receiving(91);
            assert_eq!(
                game_engine::common::thing::thing_factory::load_templates_from_ini_text(
                    &source(999),
                    "foreign-threat-continuation.ini"
                ),
                1
            );
            spawn(&mut world, 7);
            reset_receiving(&mut world);
            let map = format!("Object {NAME}\nThreatValue = 37\nEnd\n");
            let solo = format!("Object {NAME}\nThreatValue = 0\nEnd\n");
            world
                .apply_receiving_module_overrides(std::slice::from_ref(&map))
                .unwrap();
            let nonzero = spawn(&mut world, 37);
            let builder = crate::save_load::SnapshotBuilder::new();
            let nonzero_snapshot = builder.create_world_snapshot(&world).unwrap();
            let nonzero_bytes = bincode_legacy::serialize(&nonzero_snapshot).unwrap();
            let nonzero_decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&nonzero_bytes).unwrap();
            let mut nonzero_restored = receiving(7);
            nonzero_restored
                .apply_receiving_module_overrides(std::slice::from_ref(&map))
                .unwrap();
            builder
                .restore_from_snapshot(&nonzero_decoded, &mut nonzero_restored)
                .unwrap();
            assert_eq!(
                nonzero_restored
                    .host_object(nonzero)
                    .unwrap()
                    .partition_threat_value,
                37,
                "nonzero receiving threat must be reconstructed at actual encoded restore"
            );
            nonzero_restored.maintain_owned_partition_scalars(nonzero);
            assert_partition_stamp(&nonzero_restored, nonzero, 37);
            spawn(&mut nonzero_restored, 37);
            reset_receiving(&mut world);
            world
                .apply_receiving_module_overrides(&[map.clone(), solo.clone()])
                .unwrap();
            let zero = spawn(&mut world, 0);
            assert_eq!(
                world.templates[NAME].get_threat_value(),
                0,
                "authored zero is authoritative"
            );
            spawn(&mut peer, 91);
            let snapshot = builder.create_world_snapshot(&world).unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
            let mut restored = receiving(7);
            restored
                .apply_receiving_module_overrides(&[map, solo])
                .unwrap();
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.host_object(zero).unwrap().partition_threat_value,
                0,
                "encoded restore binds the actual receiving effective template"
            );
            restored.maintain_owned_partition_scalars(zero);
            assert_partition_stamp(&restored, zero, 0);
            spawn(&mut restored, 0);
            reset_receiving(&mut world);
            spawn(&mut world, 7);
            assert_eq!(
                restored.host_object(zero).unwrap().partition_threat_value,
                0
            );
            assert_eq!(peer.templates[NAME].get_threat_value(), 91);
        },
    );
}

fn receiving_scalar_grid(threat: u16) -> GameLogic {
    let mut world = receiving(threat);
    let services = std::sync::Arc::clone(&world.world_services);
    let mut shroud = services.shroud().lock().unwrap();
    shroud
        .init_shroud_grid_at_origin_with_cell_size([0., 0.], 400., 400., 40.)
        .unwrap();
    world.partition_manager.configure_grid_from_shroud(&shroud);
    drop(shroud);
    world
}
fn threat_cell(world: &GameLogic, x: f32) -> u32 {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .host_scalar_value_at(
            &gamelogic::common::Coord3D::new(x, 0., 0.),
            0,
            gamelogic::object::collide::partition_manager::ValueOrThreat::ThreatValue,
        )
        .unwrap()
}

#[test]
fn actual_scalar_factory_dirty_phase_same_id_peer_and_drop_use_receiving_grid() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_scalar_factory_dirty_phase_same_id_peer_and_drop_use_receiving_grid",
        || {
            let mut world = receiving_scalar_grid(17);
            let id = spawn(&mut world, 17);
            assert_eq!(
                threat_cell(&world, 0.),
                0,
                "factory projection does not apply cells before original dirty phase"
            );
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            let mut peer = receiving_scalar_grid(91);
            assert_eq!(spawn(&mut peer, 91), id);
            peer.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            assert_eq!(threat_cell(&world, 0.), 17);
            assert_eq!(threat_cell(&peer, 0.), 91);
            world.set_fixture_object_position(id, Vec3::new(160., 0., 0.));
            assert_eq!(
                threat_cell(&world, 0.),
                17,
                "pose setter does not run original partition maintenance early"
            );
            assert_eq!(threat_cell(&world, 160.), 0);
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            assert_eq!(threat_cell(&world, 0.), 0);
            assert_eq!(threat_cell(&world, 160.), 17);
            assert_eq!(threat_cell(&peer, 0.), 91);
            // DESTROYED is queued retirement, not the original effectivelyDead guard.
            world.host_object_mut(id).unwrap().status.destroyed = true;
            world.maintain_owned_partition_scalars(id);
            assert_eq!(threat_cell(&world, 160.), 17);
            world.host_object_mut(id).unwrap().status.effectively_dead = true;
            world.maintain_owned_partition_scalars(id);
            assert_eq!(threat_cell(&world, 160.), 0);
            let held = std::sync::Arc::clone(peer.world_services.shroud());
            drop(peer);
            assert_eq!(
                held.lock().unwrap().host_scalar_value_at(
                    &gamelogic::common::Coord3D::new(0., 0., 0.),
                    0,
                    gamelogic::object::collide::partition_manager::ValueOrThreat::ThreatValue
                ),
                Some(0),
                "dropping a world removes its exact stamps even when a service observer outlives it"
            );
        },
    );
}

#[test]
fn actual_bincode_pending_scalar_cell_observation_continues_without_early_restamp() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_bincode_pending_scalar_cell_observation_continues_without_early_restamp",
        || {
            let mut world = receiving_scalar_grid(17);
            let id = spawn(&mut world, 17);
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            world.set_fixture_object_position(id, Vec3::new(160., 0., 0.));
            let builder = crate::save_load::SnapshotBuilder::new();
            let bytes =
                bincode_legacy::serialize(&builder.create_world_snapshot(&world).unwrap()).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let mut loaded = receiving_scalar_grid(17);
            builder
                .restore_from_snapshot(&decoded, &mut loaded)
                .unwrap();
            assert_eq!(
                threat_cell(&loaded, 0.),
                17,
                "saved previous observation owns stale scalar stamp"
            );
            assert_eq!(threat_cell(&loaded, 160.), 0);
            loaded.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            for x in [0., 40., 120., 160., 200.] {
                assert_eq!(threat_cell(&loaded, x), threat_cell(&world, x));
            }
            reset_receiving(&mut loaded);
            assert_eq!(
                threat_cell(&world, 160.),
                17,
                "peer reset cannot clear source scalar owner"
            );
        },
    );
}

#[test]
fn receiving_scalar_and_pose_operations_reject_naked_mutators_before_any_change() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_scalar_and_pose_operations_reject_naked_mutators_before_any_change",
        || {
            let mut world = receiving_scalar_grid(17);
            let id = spawn(&mut world, 17);
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            let before = world.host_object(id).unwrap().get_position();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    world
                        .host_object_mut(id)
                        .unwrap()
                        .set_position(Vec3::X * 160.);
                }))
                .is_err()
            );
            assert_eq!(world.host_object(id).unwrap().get_position(), before);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    world.objects.get_mut(&id).unwrap().set_team_and_owner(
                        Team::China,
                        Some(3),
                        &mut world.ownership_events,
                    );
                }))
                .is_err()
            );
            assert_eq!(world.host_object(id).unwrap().owner_player_id, Some(0));
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    world.host_object_mut(id).unwrap().set_script_disabled(true);
                }))
                .is_err()
            );
            assert!(!world.host_object(id).unwrap().is_script_disabled());
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    world
                        .host_object_mut(id)
                        .unwrap()
                        .handle_partition_cell_maintenance();
                }))
                .is_err()
            );
            assert_eq!(threat_cell(&world, 0.), 17);
        },
    );
}

#[test]
fn actual_nearest_enemy_scalar_query_observes_factory_phase_and_saved_stale_cells() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_nearest_enemy_scalar_query_observes_factory_phase_and_saved_stale_cells",
        || {
            let mut world = receiving_scalar_grid(17);
            world.add_player(crate::game_logic::Player::new(
                1,
                Team::China,
                "Enemy",
                false,
            ));
            world
                .apply_receiving_module_overrides(&[format!(
                    "Object {NAME}\nBuildCost = 200\nEnd\n"
                )])
                .unwrap();
            let id = world
                .create_object(NAME, Team::China, Vec3::new(160., 0., 160.))
                .unwrap();
            assert_eq!(world.host_object(id).unwrap().owner_player_id, Some(1));
            assert_eq!(
                world.fixture_nearest_enemy_scalar_group(0, Vec3::ZERO, 100),
                None,
                "actual script reader cannot infer current-pose value before original owner phase"
            );
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            let old = world
                .fixture_nearest_enemy_scalar_group(0, Vec3::ZERO, 100)
                .expect("actual receiving scalar group");
            world.set_fixture_object_position(id, Vec3::new(320., 0., 320.));
            assert_eq!(
                world.fixture_nearest_enemy_scalar_group(0, Vec3::ZERO, 100),
                Some(old)
            );
            let builder = crate::save_load::SnapshotBuilder::new();
            let bytes =
                bincode_legacy::serialize(&builder.create_world_snapshot(&world).unwrap()).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let mut loaded = receiving_scalar_grid(17);
            loaded.add_player(crate::game_logic::Player::new(
                1,
                Team::China,
                "Enemy",
                false,
            ));
            loaded
                .apply_receiving_module_overrides(&[format!(
                    "Object {NAME}\nBuildCost = 200\nEnd\n"
                )])
                .unwrap();
            builder
                .restore_from_snapshot(&decoded, &mut loaded)
                .unwrap();
            assert_eq!(
                loaded.host_object(id).unwrap().partition_cash_value,
                200,
                "the first restored maintenance must use the receiving template's live cash payload"
            );
            assert_eq!(
                loaded.fixture_nearest_enemy_scalar_group(0, Vec3::ZERO, 100),
                Some(old)
            );
            world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            loaded.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            let new = world
                .fixture_nearest_enemy_scalar_group(0, Vec3::ZERO, 100)
                .expect("moved receiving group");
            assert_ne!(old, new);
            assert_eq!(
                loaded.fixture_nearest_enemy_scalar_group(0, Vec3::ZERO, 100),
                Some(new)
            );
        },
    );
}

#[test]
fn actual_direct_delete_retains_original_scalar_residual_but_lethal_body_clears_it() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_direct_delete_retains_original_scalar_residual_but_lethal_body_clears_it",
        || {
            let mut direct = receiving_scalar_grid(17);
            let id = spawn(&mut direct, 17);
            direct.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            let mut lethal = receiving_scalar_grid(17);
            assert_eq!(spawn(&mut lethal, 17), id);
            lethal.update_owned_partition_with_effects(&mut Default::default()).unwrap();
            direct.destroy_object(id);
            assert!(!direct.host_object(id).unwrap().status.effectively_dead);
            assert_eq!(
                threat_cell(&direct, 0.),
                17,
                "onDelete maintenance does not invent effectivelyDead"
            );
            direct
                .process_destroy_list_with_effects_checked(&mut Default::default())
                .unwrap();
            assert!(direct.host_object(id).is_none());
            assert_eq!(
                threat_cell(&direct, 0.),
                17,
                "literal C++ SightingInfo destruction has no scalar undo"
            );
            lethal
                .apply_owned_damage(
                    id,
                    1000.,
                    None,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &crate::game_logic::object::DamageHitContext::default(),
                )
                .unwrap();
            assert_eq!(
                threat_cell(&lethal, 0.),
                0,
                "ordinary lethal Body performs original effectivelyDead maintenance"
            );
            let builder = crate::save_load::SnapshotBuilder::new();
            let bytes = bincode_legacy::serialize(&builder.create_world_snapshot(&direct).unwrap())
                .unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let mut loaded = receiving_scalar_grid(17);
            builder
                .restore_from_snapshot(&decoded, &mut loaded)
                .unwrap();
            assert!(loaded.host_object(id).is_none());
            assert_eq!(
                threat_cell(&loaded, 0.),
                17,
                "native grid snapshot preserves residual without a surviving actor receipt"
            );
            let held = std::sync::Arc::clone(loaded.world_services.shroud());
            drop(loaded);
            assert_eq!(
                held.lock().unwrap().host_scalar_value_at(
                    &gamelogic::common::Coord3D::new(0., 0., 0.),
                    0,
                    gamelogic::object::collide::partition_manager::ValueOrThreat::ThreatValue
                ),
                Some(0),
                "logical world teardown clears all owned scalar cells, including retired residuals"
            );
            assert_eq!(threat_cell(&direct, 0.), 17);
        },
    );
}
