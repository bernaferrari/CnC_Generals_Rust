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
        crate::gameworld_shadow::with_gameworld_authority(
            crate::game_logic::game_logic::GameWorldAuthority::DEFAULT_OFF,
            run,
        );
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
                    first.pathfinding_system.grid.layer_for_destination(tied) as u8,
                    3,
                    "strict tie keeps prepended source bridge"
                );
                assert_eq!(
                    second.pathfinding_system.grid.layer_for_destination(tied),
                    PathfindLayerEnum::Ground
                );
                second.reset();
                let _inert = GameLogic::new();
                assert_eq!(bridges(&first), first_rows);
                first.override_world_size(400.0, 400.0);
                first.stamp_live_bridge_decks_and_zones();
                assert_eq!(bridges(&first), first_rows);
                assert_eq!(
                    first.pathfinding_system.grid.layer_for_destination(tied) as u8,
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
                    world
                        .pathfinding_system
                        .grid
                        .layer_for_destination(Vec3::new(50.0, 21.0, 50.0)),
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
                let (snapshot, info) = saves.load_game_snapshot("bridge_owner").unwrap();
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
                    restored
                        .pathfinding_system
                        .grid
                        .layer_for_destination(Vec3::new(50.0, 38.0, 50.0)),
                    source
                        .pathfinding_system
                        .grid
                        .layer_for_destination(Vec3::new(50.0, 38.0, 50.0))
                );
            });
            drop(held);
        },
    );
}

#[test]
fn native_bridge_body_callbacks_continue_through_damage_and_scaffold_repair() {
    isolated(
        "native_bridge_body_callbacks_continue_through_damage_and_scaffold_repair",
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
            let (snapshot, info) = saves.load_game_snapshot("bridge_body").unwrap();
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
