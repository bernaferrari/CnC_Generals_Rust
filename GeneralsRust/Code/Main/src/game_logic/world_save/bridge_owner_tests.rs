//! Production map, layer, body and native reconstruction witnesses.
use super::*;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{
    GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, SnapshotBuilder,
};
use gamelogic::ai::pathfind_astar::PathfindLayerEnum;
use glam::Vec3;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, || {
        run();
    });
}

// Real CKMP geometry: raw terrain plus two source-ordered bridge endpoint pairs.
fn map_bytes(row: f32, decks: &[f32]) -> Vec<u8> {
    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        out.write_int(value);
    }
    for _ in 0..441 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    for (index, deck) in decks.iter().enumerate() {
        for (x, flags) in [(20.0, 0x10), (80.0, 0x20)] {
            out.open_data_chunk("Object", 3);
            for value in [x, row, *deck, 0.0] {
                out.write_real(value);
            }
            out.write_int(flags);
            out.write_ascii_string(&format!("OwnedBridge{index}"));
            out.close_data_chunk();
        }
    }
    out.close_data_chunk();
    out.into_ckmp_bytes()
}

fn load(path: &Path) -> GameLogic {
    let mut world = GameLogic::new();
    assert!(world.load_map(path.to_str().unwrap()));
    assert!(
        world.terrain.is_some(),
        "map raw height samples are admitted"
    );
    world
}

fn bridges(world: &GameLogic) -> Vec<(u8, u32, f32)> {
    let owner = world.world_services.terrain().clone();
    let mut rows = Vec::new();
    owner.read().unwrap().for_each_bridge(|bridge| {
        let info = bridge.get_bridge_info();
        rows.push((
            bridge.get_layer() as u8,
            info.bridge_object_id,
            info.from_left.z,
        ));
    });
    rows
}

#[test]
fn actual_map_admission_keeps_source_layers_across_bounds_and_foreign_ai() {
    isolated(
        "actual_map_admission_keeps_source_layers_across_bounds_and_foreign_ai",
        || {
            let temp = tempfile::tempdir().unwrap();
            let first_path = temp.path().join("First.map");
            let second_path = temp.path().join("Second.map");
            std::fs::write(&first_path, map_bytes(50.0, &[21.0, 55.0])).unwrap();
            std::fs::write(&second_path, map_bytes(120.0, &[9.0, 43.0])).unwrap();
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let ai = foreign.ai();
            let held = ai.write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let mut first = load(&first_path);
                let first_rows = bridges(&first);
                assert_eq!(
                    first_rows.iter().map(|r| (r.0, r.2)).collect::<Vec<_>>(),
                    vec![(3, 55.0), (2, 21.0)]
                );
                assert!(first_rows.iter().all(|r| r.1 != 0));
                let mut second = load(&second_path);
                let second_rows = bridges(&second);
                assert_eq!(
                    first_rows.iter().map(|r| r.1).collect::<Vec<_>>(),
                    second_rows.iter().map(|r| r.1).collect::<Vec<_>>(),
                    "same IDs belong to different worlds"
                );
                let tied = Vec3::new(50.0, 38.0, 50.0);
                assert_eq!(
                    ({
                        let ground_owner = first.world_services.terrain();
                        let ground_loan = ground_owner
                            .read()
                            .unwrap_or_else(|error| error.into_inner());
                        let ground_source =
                            crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                                &ground_loan,
                                first.map_loaded,
                                first.terrain.as_ref(),
                                first.pathfinding_height_samples.as_ref(),
                                &first.pathfinding_system.grid,
                            );
                        first
                            .pathfinding_system
                            .grid
                            .layer_for_destination(tied, &ground_source)
                    }) as u8,
                    3,
                    "strict tie keeps prepended source bridge"
                );
                assert_eq!(
                    ({
                        let ground_owner = second.world_services.terrain();
                        let ground_loan = ground_owner
                            .read()
                            .unwrap_or_else(|error| error.into_inner());
                        let ground_source =
                            crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                                &ground_loan,
                                second.map_loaded,
                                second.terrain.as_ref(),
                                second.pathfinding_height_samples.as_ref(),
                                &second.pathfinding_system.grid,
                            );
                        second
                            .pathfinding_system
                            .grid
                            .layer_for_destination(tied, &ground_source)
                    }),
                    PathfindLayerEnum::Ground
                );
                second.reset();
                let _inert = GameLogic::new();
                assert_eq!(bridges(&first), first_rows);
                first.override_world_size(400.0, 400.0);
                first.stamp_live_bridge_decks_and_zones();
                assert_eq!(bridges(&first), first_rows);
                assert_eq!(
                    ({
                        let ground_owner = first.world_services.terrain();
                        let ground_loan = ground_owner
                            .read()
                            .unwrap_or_else(|error| error.into_inner());
                        let ground_source =
                            crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                                &ground_loan,
                                first.map_loaded,
                                first.terrain.as_ref(),
                                first.pathfinding_height_samples.as_ref(),
                                &first.pathfinding_system.grid,
                            );
                        first
                            .pathfinding_system
                            .grid
                            .layer_for_destination(tied, &ground_source)
                    }) as u8,
                    3
                );
            });
            drop(held);
        },
    );
}

#[test]
fn canonical_body_scaffold_repair_and_delete_ignore_foreign_same_id() {
    isolated(
        "canonical_body_scaffold_repair_and_delete_ignore_foreign_same_id",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("Repair.map");
            std::fs::write(&path, map_bytes(50.0, &[21.0])).unwrap();
            let mut world = load(&path);
            let (layer, id, _) = bridges(&world)[0];
            let id = ObjectId(id);
            let foreign_object = std::sync::Arc::new(std::sync::RwLock::new(
                gamelogic::object::Object::new_for_xfer_load(id.0, 917.0),
            ));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign_object);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let ai = foreign.ai();
            let held = ai.write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                world.objects.get_mut(&id).unwrap().health.current = 0.0;
                world.refresh_owned_bridge_pathfinder_states();
                let pos = world
                    .pathfinding_system
                    .grid
                    .world_to_grid(Vec3::new(50.0, 21.0, 50.0));
                assert_eq!(
                    world.pathfinding_system.grid.layer_cell_type(layer, pos),
                    Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
                );
                world.objects.get_mut(&id).unwrap().health.current = 300.0;
                assert!(world.bridge_behavior.create_scaffolding(id));
                let motion = world
                    .bridge_behavior
                    .span(id)
                    .unwrap()
                    .scaffold_motion_frames;
                world.refresh_owned_bridge_pathfinder_states();
                assert_eq!(
                    world
                        .bridge_behavior
                        .span(id)
                        .unwrap()
                        .scaffold_motion_frames,
                    motion,
                    "terrain observer never advances behavior"
                );
                assert_eq!(
                    world.pathfinding_system.grid.layer_cell_type(layer, pos),
                    Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
                );
                world.bridge_behavior.span_mut(id).unwrap().scaffold_present = false;
                world.refresh_owned_bridge_pathfinder_states();
                assert_ne!(
                    world.pathfinding_system.grid.layer_cell_type(layer, pos),
                    Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
                );
                assert!(world.delete_owned_bridge_at(Vec3::new(50.0, 21.0, 50.0)));
                assert!(bridges(&world).is_empty(), "unlink precedes destruction");
                assert!(world.bridge_behavior.span(id).is_none());
                world.refresh_owned_bridge_pathfinder_states();
                assert_eq!(
                    ({
                        let ground_owner = world.world_services.terrain();
                        let ground_loan = ground_owner
                            .read()
                            .unwrap_or_else(|error| error.into_inner());
                        let ground_source =
                            crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                                &ground_loan,
                                world.map_loaded,
                                world.terrain.as_ref(),
                                world.pathfinding_height_samples.as_ref(),
                                &world.pathfinding_system.grid,
                            );
                        world
                            .pathfinding_system
                            .grid
                            .layer_for_destination(Vec3::new(50.0, 21.0, 50.0), &ground_source)
                    }),
                    PathfindLayerEnum::Ground,
                    "unlinked metadata cannot answer terrain queries or become healthy next frame"
                );
                assert!(
                    world.objects_to_destroy.iter().any(|event| event.id == id),
                    "canonical destruction admitted synchronously"
                );
                assert_eq!(
                    world.pathfinding_system.grid.layer_cell_type(layer, pos),
                    Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
                );
            });
            drop(held);
            assert_eq!(foreign_object.read().unwrap().get_health(), 917.0);
            assert!(
                gamelogic::object::registry::OBJECT_REGISTRY
                    .get_object(id.0)
                    .is_some()
            );
        },
    );
}

#[test]
fn native_saved_map_reconstructs_layers_and_keeps_scaffold_continuation() {
    isolated(
        "native_saved_map_reconstructs_layers_and_keeps_scaffold_continuation",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("Saved.map");
            std::fs::write(&path, map_bytes(50.0, &[21.0, 55.0])).unwrap();
            let mut source = load(&path);
            let rows = bridges(&source);
            let id = ObjectId(rows[0].1);
            assert!(source.bridge_behavior.create_scaffolding(id));
            source.refresh_owned_bridge_pathfinder_states();
            let mut saves = SaveFileManager::with_save_directory(temp.path().join("save"));
            saves.init().unwrap();
            saves
                .save_game(
                    "bridge_owner",
                    &source,
                    &SaveGameInfo {
                        filename: "bridge_owner".into(),
                        display_name: "Bridge owner".into(),
                        description: "native reconstruction".into(),
                        map_name: path.to_string_lossy().into_owned(),
                        pristine_map_name: source.map_definition_source.clone(),
                        campaign_side: None,
                        mission_number: None,
                        save_date: std::time::SystemTime::now(),
                        game_version: env!("CARGO_PKG_VERSION").into(),
                        play_time: std::time::Duration::ZERO,
                        difficulty: GameDifficulty::Medium,
                        save_type: SaveFileType::Normal,
                    },
                )
                .unwrap();
            std::fs::remove_file(&path).unwrap();
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let ai = foreign.ai();
            let held = ai.write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let crate::save_load::DecodedGameSave {
                    snapshot,
                    save_info: info,
                    ..
                } = saves.load_game_snapshot("bridge_owner").unwrap();
                assert_eq!(
                    std::fs::read(&info.map_name).unwrap(),
                    map_bytes(50.0, &[21.0, 55.0])
                );
                let mut restored = load(Path::new(&info.map_name));
                restored.templates = source.templates.clone();
                SnapshotBuilder::new()
                    .restore_from_snapshot(&snapshot, &mut restored)
                    .unwrap();
                assert_eq!(
                    bridges(&restored),
                    rows,
                    "native map reserves the same canonical source slots"
                );
                assert_eq!(
                    restored
                        .bridge_behavior
                        .span(id)
                        .unwrap()
                        .scaffold_motion_frames,
                    source
                        .bridge_behavior
                        .span(id)
                        .unwrap()
                        .scaffold_motion_frames
                );
                assert!(restored.bridge_behavior.is_scaffold_present(id));
                let pos = restored
                    .pathfinding_system
                    .grid
                    .world_to_grid(Vec3::new(50.0, 55.0, 50.0));
                assert_eq!(
                    restored.pathfinding_system.grid.layer_cell_type(3, pos),
                    Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
                );
                source.sync_host_bridge_rubble_and_scaffolds();
                restored.sync_host_bridge_rubble_and_scaffolds();
                assert_eq!(
                    restored
                        .bridge_behavior
                        .span(id)
                        .unwrap()
                        .scaffold_motion_frames,
                    source
                        .bridge_behavior
                        .span(id)
                        .unwrap()
                        .scaffold_motion_frames
                );
                assert_eq!(
                    ({
                        let ground_owner = restored.world_services.terrain();
                        let ground_loan = ground_owner
                            .read()
                            .unwrap_or_else(|error| error.into_inner());
                        let ground_source =
                            crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                                &ground_loan,
                                restored.map_loaded,
                                restored.terrain.as_ref(),
                                restored.pathfinding_height_samples.as_ref(),
                                &restored.pathfinding_system.grid,
                            );
                        restored
                            .pathfinding_system
                            .grid
                            .layer_for_destination(Vec3::new(50.0, 38.0, 50.0), &ground_source)
                    }),
                    ({
                        let ground_owner = source.world_services.terrain();
                        let ground_loan = ground_owner
                            .read()
                            .unwrap_or_else(|error| error.into_inner());
                        let ground_source =
                            crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                                &ground_loan,
                                source.map_loaded,
                                source.terrain.as_ref(),
                                source.pathfinding_height_samples.as_ref(),
                                &source.pathfinding_system.grid,
                            );
                        source
                            .pathfinding_system
                            .grid
                            .layer_for_destination(Vec3::new(50.0, 38.0, 50.0), &ground_source)
                    })
                );
            });
            drop(held);
        },
    );
}

#[test]
fn saved_coincident_overflow_bridges_keep_source_bindings_after_ids_change() {
    isolated("saved_coincident_overflow_bridges_keep_source_bindings_after_ids_change", || {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("Coincident.map");
        std::fs::write(&path, map_bytes(50.0, &[21.0; 16])).unwrap();
        let mut source = load(&path);
        let peer = load(&path);
        assert_eq!(bridges(&source), bridges(&peer));
        let ground_observation = |world: &GameLogic| {
            let grid = &world.pathfinding_system.grid;
            [Vec3::new(50.0, 0.0, 50.0), Vec3::new(10.0, 0.0, 10.0)]
                .map(|position| {
                    let cell = grid.world_to_grid(position);
                    (grid.cell_type(cell), grid.is_static_blocked(cell))
                })
        };
        let original_ground = ground_observation(&peer);
        assert_eq!(ground_observation(&source), original_ground);
        assert!(source.delete_owned_bridge_at(Vec3::new(50.0, 21.0, 50.0)));
        assert_eq!(ground_observation(&source), original_ground);
        let capture = |world: &GameLogic| {
            let mut rows = Vec::new();
            world.world_services.terrain().read().unwrap().for_each_bridge(|bridge| {
                let info = bridge.get_bridge_info();
                rows.push((info.bridge_index, bridge.get_layer() as u8, info.bridge_object_id));
            });
            rows
        };
        let expected = capture(&source);
        assert_eq!(expected.len(), 15);
        assert_eq!(expected.iter().filter(|row| row.1 == 1).count(), 2);
        let overflow_ids: Vec<_> = expected.iter().filter(|row| row.1 == 1).map(|row| row.2).collect();
        assert_ne!(overflow_ids[0], overflow_ids[1]);
        let scaffold_id = ObjectId(expected.iter().find(|row| row.1 == 3).unwrap().2);
        assert!(source.bridge_behavior.create_scaffolding(scaffold_id));
        source.refresh_owned_bridge_pathfinder_states();
        let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
        saves.init().unwrap();
        saves.save_game("coincident", &source, &SaveGameInfo {
            filename: "coincident".into(),
            display_name: "Coincident".into(),
            description: "bridge source identity".into(),
            map_name: path.to_string_lossy().into_owned(),
            pristine_map_name: source.map_definition_source.clone(),
            campaign_side: None,
            mission_number: None,
            save_date: std::time::SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").into(),
            play_time: std::time::Duration::ZERO,
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        }).unwrap();
        let decoded = saves.load_game_snapshot("coincident").unwrap();
        let mut restored = GameLogic::new();
        restored.set_next_object_id_for_restore(ObjectId(1000));
        assert!(restored.load_map(&decoded.save_info.map_name));
        let reconstructed = capture(&restored);
        assert_ne!(reconstructed[0].2, expected[0].2);
        restored.templates = source.templates.clone();
        SnapshotBuilder::new().restore_from_snapshot(&decoded.snapshot, &mut restored).unwrap();
        assert_eq!(capture(&restored), expected);
        assert_eq!(capture(&peer).len(), 16);
        assert_eq!(ground_observation(&peer), original_ground);
        assert_eq!(ground_observation(&restored), ground_observation(&source));
        assert!(restored.bridge_behavior.is_scaffold_present(scaffold_id));
        let cell = restored.pathfinding_system.grid.world_to_grid(Vec3::new(50.0, 21.0, 50.0));
        assert_eq!(
            restored.pathfinding_system.grid.layer_cell_type(2, cell),
            peer.pathfinding_system.grid.layer_cell_type(2, cell),
            "removing an overflow span cannot overwrite a reserved bridge layer"
        );
        assert_eq!(
            restored.pathfinding_system.grid.layer_cell_type(3, cell),
            Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
        );
        assert_ne!(
            restored.pathfinding_system.grid.layer_cell_type(2, cell),
            Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
        );
        for (_, _, id) in expected {
            let span = restored.bridge_behavior.span(ObjectId(id)).unwrap();
            assert!(span.point_on_deck(Vec3::new(50.0, 21.0, 50.0)));
        }
    });
}

#[test]
fn placed_landmark_identity_survives_coincident_spans_and_changed_object_ids() {
    isolated("placed_landmark_identity_survives_coincident_spans_and_changed_object_ids", || {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("Landmarks.map");
        let mut out = game_engine::common::system::DataChunkOutput::new();
        out.open_data_chunk("HeightMapData", 3);
        for value in [21, 21, 1, 441] {
            out.write_int(value);
        }
        for _ in 0..441 {
            out.write_byte(0);
        }
        out.close_data_chunk();
        out.open_data_chunk("ObjectsList", 3);
        for angle in [0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_4] {
            out.open_data_chunk("Object", 3);
            for value in [50.0, 50.0, 21.0, angle] {
                out.write_real(value);
            }
            out.write_int(0);
            out.write_ascii_string("IdentityLandmark");
            out.write_dict(&game_engine::common::dict::Dict::new());
            out.close_data_chunk();
        }
        out.close_data_chunk();
        std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
        let prepare = |world: &mut GameLogic| {
            let mut template = crate::game_logic::ThingTemplate::new("IdentityLandmark");
            template.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::LandmarkBridge)
                .add_kind_of(KindOf::Bridge)
                .set_health(400.0);
            template.geometry_info.authored = false;
            template.geometry_info.geom_type = crate::game_logic::HostGeometryType::Box;
            template.geometry_info.major_radius = 25.0;
            template.geometry_info.minor_radius = 10.0;
            let mut neutral = crate::game_logic::Player::new(0, Team::Neutral, "", false);
            neutral.map_side.role = crate::game_logic::PlayerSideRole::Neutral;
            neutral.map_side.map_player_name.clear();
            world.add_player(neutral);
            world.templates.insert("IdentityLandmark".into(), template);
        };
        let mut source = GameLogic::new();
        prepare(&mut source);
        assert!(source.load_map(path.to_str().unwrap()));
        let mut peer = GameLogic::new();
        prepare(&mut peer);
        assert!(peer.load_map(path.to_str().unwrap()));
        let source_ids: Vec<_> = source.spawned_map_object_ids.iter().map(|(id, _)| *id).collect();
        let peer_ids: Vec<_> = peer.spawned_map_object_ids.iter().map(|(id, _)| *id).collect();
        assert_eq!(source_ids.len(), 3);
        assert_eq!(source_ids, peer_ids);
        assert!(source.delete_owned_bridge_at(Vec3::new(50.0, 21.0, 50.0)));
        assert!(source.bridge_behavior.create_scaffolding(source_ids[1]));
        source.refresh_owned_bridge_pathfinder_states();
        let mut saves = SaveFileManager::with_save_directory(temp.path().join("landmark_saves"));
        saves.init().unwrap();
        saves.save_game("landmarks", &source, &SaveGameInfo {
            filename: "landmarks".into(),
            display_name: "Landmarks".into(),
            description: "placed bridge source identity".into(),
            map_name: path.to_string_lossy().into_owned(),
            pristine_map_name: source.map_definition_source.clone(),
            campaign_side: None,
            mission_number: None,
            save_date: std::time::SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").into(),
            play_time: std::time::Duration::ZERO,
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        }).unwrap();
        let decoded = saves.load_game_snapshot("landmarks").unwrap();
        let mut restored = GameLogic::new();
        prepare(&mut restored);
        restored.set_next_object_id_for_restore(ObjectId(1000));
        assert!(restored.load_map(&decoded.save_info.map_name));
        let reconstructed: Vec<_> = restored.spawned_map_object_ids.iter().map(|(id, _)| *id).collect();
        assert_eq!(reconstructed.len(), 3);
        assert_ne!(reconstructed, source_ids);
        SnapshotBuilder::new().restore_from_snapshot(&decoded.snapshot, &mut restored).unwrap();
        let rows = |world: &GameLogic| {
            let mut rows = Vec::new();
            world.world_services.terrain().read().unwrap().for_each_bridge(|bridge| {
                let info = bridge.get_bridge_info();
                rows.push((info.source_map_object_index, info.bridge_object_id));
            });
            rows
        };
        assert_eq!(rows(&restored), rows(&source));
        assert_eq!(
            rows(&restored),
            vec![(Some(1), source_ids[1].0), (Some(0), source_ids[0].0)]
        );
        assert_eq!(rows(&peer).len(), 3);
        assert!(!peer.bridge_behavior.is_scaffold_present(source_ids[1]));
        assert!(restored.bridge_behavior.is_scaffold_present(source_ids[1]));
        let east = Vec3::new(70.0, 21.0, 50.0);
        let north = Vec3::new(50.0, 21.0, 70.0);
        let east_span = restored.bridge_behavior.span(source_ids[0]).unwrap();
        let north_span = restored.bridge_behavior.span(source_ids[1]).unwrap();
        assert!(east_span.point_on_deck(east));
        assert!(!east_span.point_on_deck(north));
        assert!(north_span.point_on_deck(north));
        assert!(!north_span.point_on_deck(east));
        let cell = restored.pathfinding_system.grid.world_to_grid(north);
        assert_eq!(restored.pathfinding_system.grid.layer_cell_type(3, cell),
            Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable));
    });
}

#[test]
fn native_bridge_body_callbacks_continue_through_damage_and_direct_healing() {
    isolated(
        "native_bridge_body_callbacks_continue_through_damage_and_direct_healing",
        || {
            use crate::game_logic::combat::DamageType;
            use crate::game_logic::host_usa_pilot::HostDeathType;
            use crate::game_logic::object::DamageHitContext;
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("Continuation.map");
            std::fs::write(&path, map_bytes(50.0, &[21.0])).unwrap();
            let mut source = load(&path);
            let (layer, raw_id, _) = bridges(&source)[0];
            let id = ObjectId(raw_id);
            source.frame = 23;
            source.spawn_bridge_scaffolding(id);
            let maximum = source.objects[&id].health.maximum;
            source
                .apply_owned_damage(
                    id,
                    maximum * 0.4,
                    None,
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                )
                .unwrap();
            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves
                .save_game(
                    "bridge_body",
                    &source,
                    &SaveGameInfo {
                        filename: "bridge_body".into(),
                        display_name: "BridgeBody".into(),
                        pristine_map_name: source.map_definition_source.clone(),
                        campaign_side: None,
                        mission_number: None,
                        description: "synchronous callback continuation".into(),
                        map_name: path.to_string_lossy().into_owned(),
                        save_date: std::time::SystemTime::now(),
                        game_version: env!("CARGO_PKG_VERSION").into(),
                        play_time: std::time::Duration::ZERO,
                        difficulty: GameDifficulty::Medium,
                        save_type: SaveFileType::Normal,
                    },
                )
                .unwrap();
            let crate::save_load::DecodedGameSave {
                snapshot,
                save_info: info,
                ..
            } = saves.load_game_snapshot("bridge_body").unwrap();
            let mut restored = load(Path::new(&info.map_name));
            restored.templates = source.templates.clone();
            SnapshotBuilder::new()
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();
            for (kind, amount) in [
                (DamageType::Unresistable, maximum),
                (DamageType::Healing, maximum * 0.25),
            ] {
                for owner in [&mut source, &mut restored] {
                    owner.frame = owner.frame.wrapping_add(1);
                    if kind == DamageType::Healing {
                        // C++ attemptDamage rejects dead bodies before its healing
                        // switch. Direct attemptHealing admits dead bridge bodies.
                        owner.apply_owned_healing(id, amount, None).unwrap();
                    } else {
                        owner
                            .apply_owned_damage(
                                id,
                                amount,
                                None,
                                kind,
                                HostDeathType::Normal,
                                None,
                                &DamageHitContext::default(),
                            )
                            .unwrap();
                    }
                    let terrain = owner.world_services.terrain().read().unwrap();
                    assert!(terrain.bridge_damage_states_changed());
                    if kind == DamageType::Healing {
                        assert!(terrain.is_bridge_repaired(id.0));
                    } else {
                        assert!(terrain.is_bridge_broken(id.0));
                    }
                    let cell = owner
                        .pathfinding_system
                        .grid
                        .world_to_grid(Vec3::new(50.0, 21.0, 50.0));
                    assert_eq!(
                        owner.pathfinding_system.grid.layer_cell_type(layer, cell),
                        Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
                    );
                }
                assert_eq!(
                    source.objects[&id].health.current,
                    restored.objects[&id].health.current
                );
                assert_eq!(
                    source.objects[&id].body_damage_state,
                    restored.objects[&id].body_damage_state
                );
                let a = source.bridge_behavior.span(id).unwrap();
                let b = restored.bridge_behavior.span(id).unwrap();
                assert_eq!(
                    (a.last_body_state, a.death_frame, a.scaffold_motion_frames),
                    (b.last_body_state, b.death_frame, b.scaffold_motion_frames)
                );
            }
        },
    );
}
