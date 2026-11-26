//! Production map, layer, body and native reconstruction witnesses.
use super::*;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{
    GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, SnapshotBuilder,
};
use generals_sim::pathfinding::PathfindLayerEnum;
use glam::Vec3;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, || {
        run();
    });
}

use crate::fixture_content::{FixtureObject, admit_fixture_objects};

fn bridge_actor_definition() -> FixtureObject {
    FixtureObject::new("GenericBridge")
        .kind_of("STRUCTURE BRIDGE IMMOBILE")
        .health(300.0)
}

const FIXTURE_DECK_Z: f32 = 20.25;

fn attach_bridge_art(world: &mut GameLogic, path: &Path) {
    crate::fixture_content::bridge_art::attach(
        world,
        path.parent().unwrap(),
        (0..17).map(|index| format!("OwnedBridge{index}")),
        [-10.0, 10.0],
        0.0,
        1.0,
    );
}

// Real CKMP geometry: raw terrain plus two source-ordered bridge endpoint pairs.
fn map_bytes(row: f32, decks: &[f32]) -> Vec<u8> {
    map_bytes_for_spans(
        decks
            .iter()
            .map(|deck| [[20.0, row, *deck], [80.0, row, *deck]]),
    )
}

fn map_bytes_for_spans(spans: impl IntoIterator<Item = [[f32; 3]; 2]>) -> Vec<u8> {
    use game_engine::map_object::{FLAG_BRIDGE_POINT1, FLAG_BRIDGE_POINT2};

    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        out.write_int(value);
    }
    for _ in 0..441 {
        out.write_byte(32);
    }
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    for (index, [start, end]) in spans.into_iter().enumerate() {
        for ([x, y, z], flag) in [(start, FLAG_BRIDGE_POINT1), (end, FLAG_BRIDGE_POINT2)] {
            out.open_data_chunk("Object", 3);
            for value in [x, y, z, 0.0] {
                out.write_real(value);
            }
            out.write_int(flag as i32);
            out.write_ascii_string(&format!("OwnedBridge{index}"));
            out.close_data_chunk();
        }
    }
    out.close_data_chunk();
    out.into_ckmp_bytes()
}

fn load(path: &Path) -> GameLogic {
    let mut world = GameLogic::new();
    admit_fixture_objects(&mut world, &[bridge_actor_definition()]);
    attach_bridge_art(&mut world, path);
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
fn invalid_native_bridge_bindings_leave_the_receiving_world_intact() {
    use crate::save_load::SaveLoadError;
    use crate::save_load::snapshot::mutate_saved_bridge_bindings_for_test;

    isolated(
        "invalid_native_bridge_bindings_leave_the_receiving_world_intact",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("CrossingSpans.map");
            std::fs::write(
                &path,
                map_bytes_for_spans([
                    [[20.0, 50.0, 21.0], [80.0, 50.0, 21.0]],
                    [[50.0, 20.0, 21.0], [50.0, 80.0, 21.0]],
                ]),
            )
            .unwrap();
            let source = load(&path);
            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves.quick_save(&source).unwrap();
            let mut receiving = load(&path);
            let peer = load(&path);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let _foreign_ai = foreign.ai().write().unwrap();
            let original_bridges = bridges(&receiving);
            assert_eq!(original_bridges.len(), 2);
            assert_eq!(bridges(&peer), original_bridges);

            for (case, expected) in [
                ("duplicate source", "duplicate map bridge source"),
                ("duplicate actor", "duplicate map bridge actor"),
                ("unknown source", "unknown map bridge source"),
                ("missing actor", "missing map bridge actor"),
                ("crossed bindings", "map bridge geometry differs"),
                ("malformed source", "invalid map bridge source"),
            ] {
                let mut snapshot = saves.load_game_snapshot("quicksave").unwrap().snapshot;
                mutate_saved_bridge_bindings_for_test(&mut snapshot, |rows| {
                    assert_eq!(rows.len(), 2);
                    match case {
                        "duplicate source" => rows.push(rows[0]),
                        "duplicate actor" => rows[0].2 = rows[1].2,
                        "unknown source" => rows[0].0 = 999,
                        "missing actor" => rows[0].2 = u32::MAX,
                        "crossed bindings" => {
                            let first = rows[0].2;
                            rows[0].2 = rows[1].2;
                            rows[1].2 = first;
                        }
                        "malformed source" => rows[0].1 = Some(0),
                        _ => unreachable!(),
                    }
                });
                // Frame and object birth tokens expose even a clear-and-recreate
                // that leaves identical terrain rows behind.
                receiving.frame = 91;
                let id = ObjectId(original_bridges[0].1);
                let birth = std::sync::Arc::downgrade(
                    &receiving
                        .host_object(id)
                        .unwrap()
                        .visual_draw_identity_token,
                );
                let result = gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                    saves.restore_game_snapshot(&snapshot, &mut receiving)
                });
                assert!(
                    matches!(&result, Err(SaveLoadError::Corrupted(message)) if message.contains(expected)),
                    "{case}: {result:?}"
                );
                assert_eq!(receiving.frame, 91, "{case}");
                assert!(receiving.object_birth_matches(id, &birth), "{case}");
                assert_eq!(bridges(&receiving), original_bridges, "{case}");
                assert_eq!(bridges(&peer), original_bridges, "{case}");
            }
        },
    );
}

#[test]
fn save_after_bridge_actor_retirement_drops_the_orphaned_span_on_load() {
    isolated(
        "save_after_bridge_actor_retirement_drops_the_orphaned_span_on_load",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("RetiredSpan.map");
            std::fs::write(&path, map_bytes(50.0, &[21.0, 27.0])).unwrap();
            let mut source = load(&path);
            let rows = bridges(&source);
            let retired = ObjectId(rows[0].1);
            source.destroy_object(retired).unwrap();
            source.process_destroy_list();
            assert!(source.host_object(retired).is_none());
            assert_eq!(
                bridges(&source),
                rows,
                "terrain still owns the retired actor's deck"
            );

            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves.quick_save(&source).unwrap();
            let decoded = saves.load_game_snapshot("quicksave").unwrap();
            let mut restored = load(&path);
            saves
                .restore_game_snapshot(&decoded.snapshot, &mut restored)
                .unwrap();
            assert_eq!(bridges(&restored), rows[1..]);
            assert!(restored.bridge_behavior.span(retired).is_none());
            assert!(restored.bridge_behavior.span(ObjectId(rows[1].1)).is_some());
        },
    );
}

fn asymmetric_authored_bridge_map() -> Vec<u8> {
    use game_engine::common::system::DataChunkOutput;
    use game_engine::map_object::{FLAG_BRIDGE_POINT1 as START, FLAG_BRIDGE_POINT2 as END};

    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 0, 441] {
        map.write_int(value);
    }
    for y in 0..21 {
        for x in 0..21 {
            map.write_byte(2 * x + y);
        }
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    use game_engine::common::well_known_keys::{
        key_object_indestructible, key_object_max_hps, key_object_name, key_original_owner,
    };
    for (name, x, y, flag) in [
        ("MissingArt", 40.0, 85.0, START),
        ("MissingArt", 80.0, 85.0, END),
        ("FixtureSpan", 40.0, 35.0, START),
        ("FixtureSpan", 80.0, 35.0, END),
        ("FixtureSpan", 50.0, 59.0, START),
        ("OtherEnd", 70.0, 59.0, END),
        ("FixtureSpan", 40.0, 8.0, START),
        ("OrdinaryObject", 60.0, 8.0, 0),
        ("FixtureSpan", 80.0, 8.0, END),
    ] {
        map.open_data_chunk("Object", 3);
        for value in [x, y, 99.0, 0.0] {
            map.write_real(value);
        }
        map.write_int(flag as i32);
        map.write_ascii_string(name);
        let mut props = game_engine::common::dict::Dict::new();
        let authored = match (flag, y) {
            (START, 35.0) => Some(("FirstBridge", 460, true)),
            (START, 59.0) => Some(("SecondBridge", 620, false)),
            (END, _) => Some(("WrongEnd", 17, false)),
            _ => None,
        };
        if let Some((name, hp, indestructible)) = authored {
            props.set_ascii_string(key_object_name(), name);
            props.set_int(key_object_max_hps(), hp);
            props.set_bool(key_object_indestructible(), indestructible);
        }
        if name == "OrdinaryObject" {
            props.set_ascii_string(key_original_owner(), "team");
        }
        map.write_dict(&props);
        map.close_data_chunk();
    }
    map.close_data_chunk();
    map.into_ckmp_bytes()
}

#[test]
fn native_map_and_save_use_owned_model_bounds_and_sampled_bridge_heights() {
    isolated(
        "native_map_and_save_use_owned_model_bounds_and_sampled_bridge_heights",
        || {
            use generals_sim::pathfinding::PathfindCellType;
            use gamelogic::system::map_loader::{BridgeEndpoints, Coord3D};

            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("AuthoredDeck.map");
            std::fs::write(&path, asymmetric_authored_bridge_map()).unwrap();
            let prepare = |label: &str, pivot_y: f32, scale: f32, with_bridge_actor: bool| {
                let mut world = GameLogic::new();
                let mut objects = vec![
                    FixtureObject::new("OrdinaryObject")
                        .kind_of("STRUCTURE IMMOBILE")
                        .health(150.0),
                ];
                if with_bridge_actor {
                    objects.push(bridge_actor_definition());
                }
                admit_fixture_objects(&mut world, &objects);
                crate::fixture_content::bridge_art::attach(
                    &mut world,
                    &temp.path().join(label),
                    ["FixtureSpan".into()],
                    [-6.0, 6.0],
                    pivot_y,
                    scale,
                );
                let mut roads = world.terrain_road_baseline().clone();
                roads.new_bridge("MissingArt".into()).bridge_model_name =
                    "MissingFixtureModel".into();
                world.admit_terrain_road_definitions(roads).unwrap();
                world
            };

            let check = |world: &GameLogic| {
                let terrain = world.world_services.terrain().read().unwrap();
                let first = terrain
                    .find_bridge_at(&Coord3D::new(60.0, 35.0, 0.0))
                    .unwrap();
                let info = first.get_bridge_info();
                assert_eq!(info.bridge_width, 24.0);
                assert_eq!((info.from.z, info.to.z), (7.4375, 12.4375));
                assert_eq!((info.from_left.y, info.from_right.y), (53.0, 29.0));
                assert_eq!((info.from_left.z, info.to_right.z), (7.4375, 12.4375));
                assert_eq!(info.bridge_index, 0);
                let layer = first.get_layer() as u8;
                assert_eq!(layer, PathfindLayerEnum::Top as u8);
                let second = terrain
                    .find_bridge_at(&Coord3D::new(60.0, 72.0, 0.0))
                    .unwrap();
                let second_info = second.get_bridge_info();
                assert_eq!(second_info.bridge_index, 1);
                assert_eq!((second_info.from.y, second_info.to.y), (59.0, 59.0));
                assert_eq!(second.get_bridge_template_name().as_str(), "FixtureSpan");
                assert!(
                    terrain
                        .find_bridge_at(&Coord3D::new(74.0, 85.0, 0.0))
                        .is_none()
                );
                assert!(
                    terrain
                        .find_bridge_at(&Coord3D::new(60.0, 14.0, 0.0))
                        .is_none()
                );
                let grid = &world.pathfinding_system.grid;
                let inside = grid.world_to_grid(Vec3::new(65.0, 0.0, 45.0));
                let edge = grid.world_to_grid(Vec3::new(65.0, 0.0, 25.0));
                assert_eq!(
                    grid.layer_cell_type(layer, inside),
                    Some(PathfindCellType::Clear)
                );
                assert_eq!(
                    grid.layer_cell_type(layer, edge),
                    Some(PathfindCellType::BridgeImpassable)
                );
                let first_actor = world.host_object(ObjectId(info.bridge_object_id)).unwrap();
                let second_actor = world
                    .host_object(ObjectId(second_info.bridge_object_id))
                    .unwrap();
                assert_eq!(
                    (
                        first_actor.name.as_str(),
                        first_actor.health.maximum,
                        first_actor.indestructible
                    ),
                    ("FirstBridge", 460.0, true),
                );
                assert_eq!(
                    (
                        second_actor.name.as_str(),
                        second_actor.health.maximum,
                        second_actor.indestructible
                    ),
                    ("SecondBridge", 620.0, false),
                );
                let ordinary = world
                    .host_objects()
                    .registered_values()
                    .find(|object| object.template_name == "OrdinaryObject")
                    .expect("ordinary map object");
                assert!(first_actor.id.0 < second_actor.id.0 && second_actor.id.0 < ordinary.id.0);
                (info.bridge_object_id, second_info.bridge_object_id)
            };

            let mut peer = prepare("peer", 9.0, 1.0, true);
            assert!(peer.load_map(path.to_str().unwrap()));
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let _foreign_ai = foreign.ai().write().unwrap();
            let mut source = prepare("source", 3.0, 2.0, true);
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                assert!(source.load_map(path.to_str().unwrap()));
            });
            let identities = check(&source);
            assert_ne!(identities.0, identities.1);
            assert!(identities.0 != 0 && identities.1 != 0);

            let mut no_actor = prepare("no-bridge-actor", 3.0, 2.0, false);
            assert!(no_actor.load_map(path.to_str().unwrap()));
            let check_actorless = |world: &GameLogic| {
                assert!(!world.templates.contains_key("GenericBridge"));
                assert!(
                    world
                        .host_objects()
                        .registered_values()
                        .all(|object| object.template_name != "GenericBridge")
                );
                let no_actor_terrain = world.world_services.terrain().read().unwrap();
                let deck = no_actor_terrain
                    .find_bridge_at(&Coord3D::new(60.0, 35.0, 0.0))
                    .unwrap();
                assert_eq!(deck.get_bridge_info().bridge_object_id, 0);
                let grid = &world.pathfinding_system.grid;
                let clear = grid.world_to_grid(Vec3::new(65.0, 0.0, 45.0));
                assert_eq!(
                    grid.layer_cell_type(deck.get_layer() as u8, clear),
                    Some(PathfindCellType::Clear)
                );
            };
            check_actorless(&no_actor);
            let mut no_actor_saves =
                SaveFileManager::with_save_directory(temp.path().join("actorless-saves"));
            no_actor_saves.init().unwrap();
            no_actor_saves.quick_save(&no_actor).unwrap();
            let actorless = no_actor_saves.load_game_snapshot("quicksave").unwrap();
            let mut restored_actorless = prepare("restored-actorless", 3.0, 2.0, false);
            assert!(restored_actorless.load_map(&actorless.save_info.map_name));
            no_actor_saves
                .restore_game_snapshot(&actorless.snapshot, &mut restored_actorless)
                .unwrap();
            assert!(
                bridges(&restored_actorless).is_empty(),
                "TerrainLogic::loadPostProcess removes spans without an actor"
            );
            let grid = &restored_actorless.pathfinding_system.grid;
            let cell = grid.world_to_grid(Vec3::new(65.0, 0.0, 45.0));
            assert_eq!(
                grid.layer_cell_type(PathfindLayerEnum::Top as u8, cell),
                Some(PathfindCellType::BridgeImpassable)
            );

            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves.quick_save(&source).unwrap();
            std::fs::remove_file(&path).unwrap();
            let decoded = saves.load_game_snapshot("quicksave").unwrap();
            let mut restored = prepare("restored", 3.0, 2.0, true);
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                assert!(restored.load_map(&decoded.save_info.map_name));
                saves
                    .restore_game_snapshot(&decoded.snapshot, &mut restored)
                    .unwrap();
            });
            assert_eq!(check(&restored), identities);

            let peer_terrain = peer.world_services.terrain().read().unwrap();
            let peer_first = peer_terrain
                .find_bridge_at(&Coord3D::new(60.0, 44.0, 0.0))
                .unwrap();
            let peer_info = peer_first.get_bridge_info();
            assert_eq!(
                (
                    peer_info.bridge_width,
                    peer_info.from_left.y,
                    peer_info.from_right.y
                ),
                (12.0, 50.0, 38.0)
            );
            drop(peer_terrain);
            let over_capacity = (0..202)
                .map(|index| BridgeEndpoints {
                    template_name: if index == 0 {
                        "MissingArt"
                    } else {
                        "FixtureSpan"
                    }
                    .into(),
                    from: Coord3D::new(40.0, index as f32, 0.0),
                    to: Coord3D::new(80.0, index as f32, 0.0),
                    properties: Default::default(),
                })
                .collect();
            let decks = restored.resolve_authored_map_bridges(over_capacity);
            assert_eq!(decks.len(), 200);
            assert_eq!((decks[0].from.y, decks[199].from.y), (1.0, 200.0));
        },
    );
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
                    vec![(3, FIXTURE_DECK_Z), (2, FIXTURE_DECK_Z)]
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
                    Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
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
                    Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
                );
                world.bridge_behavior.span_mut(id).unwrap().scaffold_present = false;
                world.refresh_owned_bridge_pathfinder_states();
                assert_ne!(
                    world.pathfinding_system.grid.layer_cell_type(layer, pos),
                    Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
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
                    Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
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
fn native_saved_moving_bridge_actor_keeps_the_constructor_ground_claim() {
    isolated(
        "native_saved_moving_bridge_actor_keeps_the_constructor_ground_claim",
        || {
            use crate::fixture_content::{FixtureObject, admit_fixture_objects};
            use crate::game_logic::{AIState, GameMode, ObjectAudio, pathfinding::PathRoute};

            const WALKER: &str = "BridgeContinuationWalker";
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("MovingBridge.map");
            std::fs::write(&path, map_bytes(50.0, &[21.0])).unwrap();
            let load_with_walker = |path: &Path| {
                let mut world = GameLogic::new();
                attach_bridge_art(&mut world, path);
                admit_fixture_objects(
                    &mut world,
                    &[
                        FixtureObject::new("GenericBridge")
                            .kind_of("STRUCTURE BRIDGE IMMOBILE")
                            .health(300.0),
                        FixtureObject::new(WALKER)
                            .kind_of("INFANTRY SELECTABLE")
                            .health(100.0)
                            .field("Geometry = CYLINDER")
                            .field("GeometryMajorRadius = 3")
                            .field("GeometryHeight = 10")
                            .walker(),
                    ],
                );
                world.start_new_game(GameMode::SinglePlayer).unwrap();
                assert!(world.load_map(path.to_str().unwrap()));
                world
            };
            let mut source = load_with_walker(&path);
            let mut peer = load_with_walker(&path);
            let bridge = bridges(&source);
            assert_eq!(bridge.len(), 1);
            let layer_id = bridge[0].0;
            let layer = PathfindLayerEnum::from_u32(layer_id as u32);
            assert_ne!(layer, PathfindLayerEnum::Ground);
            assert!(source.pathfinding_system.map_ready);
            let pose = Vec3::new(55.0, 21.0, 50.0);
            let goal = Vec3::new(65.0, 21.0, 50.0);
            let peer_pose = Vec3::new(135.0, 0.0, 125.0);
            let id = source.create_object(WALKER, Team::Neutral, pose).unwrap();
            let peer_id = peer
                .create_object(WALKER, Team::Neutral, peer_pose)
                .unwrap();
            assert_eq!(id, peer_id);
            let pose_cell = source.pathfinding_system.grid.world_to_grid(pose);
            let goal_cell = source.pathfinding_system.grid.world_to_grid(goal);
            let peer_cell = peer.pathfinding_system.grid.world_to_grid(peer_pose);
            let grid = &source.pathfinding_system.grid;
            assert!(grid.layer_cell_type(layer_id, pose_cell).is_some());
            assert!(grid.layer_cell_type(layer_id, goal_cell).is_some());
            assert!(!grid.object_interacts_with_bridge_end(pose, 3.0, layer));

            // Use the real layer transition to establish the moving save.
            source.set_owned_pathfinder_layer(id, layer_id);
            let frame = source.frame;
            let mut nodes = PathRoute::from_coords(vec![Vec3::new(45.0, 21.0, 50.0), goal])
                .nodes()
                .to_vec();
            for node in &mut nodes {
                node.layer = Some(layer_id);
            }
            let route = PathRoute::from_nodes(nodes, false).unwrap();
            let cursor = route.cursor_from_raw(Some(0), Some(1)).unwrap();
            {
                let mover = source.host_object_mut(id).unwrap();
                mover.movement.path = route.clone();
                mover.movement.path_cursor = cursor;
                mover.movement.target_position = Some(goal);
                mover.requested_destination = Some(goal);
                mover.set_ai_state(
                    AIState::Moving,
                    frame,
                    &mut ObjectAudio::record(&mut Vec::new()),
                );
                mover.set_locomotor_goal_position_on_path();
                mover.set_status_moving(true);
            }
            source.register_path_goal_on_layer(id, goal, layer_id);
            let grid = &source.pathfinding_system.grid;
            assert_eq!(grid.position_unit_on_layer(pose_cell, layer), id.0);
            assert_eq!(grid.goal_unit_on_layer(goal_cell, layer), id.0);
            assert_eq!(source.pathfinding_ground_position(pose_cell), None);
            assert_eq!(peer.pathfinding_ground_position(peer_cell), Some(peer_id));

            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves.quick_save(&source).unwrap();
            std::fs::remove_file(&path).unwrap();
            let decoded = saves.load_game_snapshot("quicksave").unwrap();
            let mut restored = load_with_walker(Path::new(&decoded.save_info.map_name));
            saves
                .restore_game_snapshot(&decoded.snapshot, &mut restored)
                .unwrap();

            let mover = restored.host_object(id).unwrap();
            assert!(mover.host_ai_is_moving());
            assert_eq!(mover.get_position(), pose);
            assert_eq!(mover.movement.path, route);
            assert_eq!(mover.movement.path_cursor, cursor);
            assert_eq!(mover.pathfind_layer, layer_id);
            assert_eq!(mover.pathfind_cur_cell, (pose_cell.x, pose_cell.y));
            let grid = &restored.pathfinding_system.grid;
            assert!(grid.layer_cell_type(layer_id, pose_cell).is_some());
            assert_eq!(grid.goal_unit_on_layer(goal_cell, layer), id.0);
            assert_eq!(grid.position_unit_on_layer(pose_cell, layer), 0);
            // Object::xfer installs the pose while the new object is Ground.
            // The saved layer is transferred directly, without setLayer.
            assert_eq!(restored.pathfinding_ground_position(pose_cell), Some(id));
            assert_eq!(restored.pathfinding_ground_position(peer_cell), None);
            assert_eq!(peer.pathfinding_ground_position(peer_cell), Some(peer_id));
            assert_eq!(peer.pathfinding_ground_position(pose_cell), None);
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
                    .world_to_grid(Vec3::new(50.0, FIXTURE_DECK_Z, 50.0));
                assert_eq!(
                    restored.pathfinding_system.grid.layer_cell_type(3, pos),
                    Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
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
        std::fs::write(&path, map_bytes(50.0, &[21.0; 17])).unwrap();
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
        let flight_layer = |world: &GameLogic| {
            let terrain = world.world_services.terrain().read().unwrap();
            world.pathfinding_system.grid.highest_layer_at_or_below_in(
                Some(&terrain), Vec3::new(50.0, 30.0, 50.0), 0.0, false,
            )
        };
        assert_eq!(flight_layer(&source), 1, "newest coincident overflow wins the strict tie");
        assert_eq!(flight_layer(&peer), 1);
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
        assert_eq!(expected.len(), 16);
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
        admit_fixture_objects(&mut restored, &[bridge_actor_definition()]);
        attach_bridge_art(&mut restored, Path::new(&decoded.save_info.map_name));
        restored.set_next_object_id_for_restore(ObjectId(1000));
        assert!(restored.load_map(&decoded.save_info.map_name));
        let reconstructed = capture(&restored);
        assert_ne!(reconstructed[0].2, expected[0].2);
        restored.templates = source.templates.clone();
        SnapshotBuilder::new().restore_from_snapshot(&decoded.snapshot, &mut restored).unwrap();
        assert_eq!(capture(&restored), expected);
        assert_eq!(capture(&peer).len(), 17);
        assert_eq!(ground_observation(&peer), original_ground);
        assert_eq!(ground_observation(&restored), ground_observation(&source));
        assert_eq!(flight_layer(&restored), 1);
        assert_eq!(flight_layer(&peer), 1);
        assert!(restored.bridge_behavior.is_scaffold_present(scaffold_id));
        let cell = restored.pathfinding_system.grid.world_to_grid(Vec3::new(50.0, 21.0, 50.0));
        assert_eq!(
            restored.pathfinding_system.grid.layer_cell_type(2, cell),
            peer.pathfinding_system.grid.layer_cell_type(2, cell),
            "removing an overflow span cannot overwrite a reserved bridge layer"
        );
        assert_eq!(
            restored.pathfinding_system.grid.layer_cell_type(3, cell),
            Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
        );
        assert_ne!(
            restored.pathfinding_system.grid.layer_cell_type(2, cell),
            Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
        );
        for (_, _, id) in expected {
            let span = restored.bridge_behavior.span(ObjectId(id)).unwrap();
            assert!(span.point_on_deck(Vec3::new(50.0, 21.0, 50.0)));
        }
    });
}

#[test]
fn placed_landmark_identity_survives_coincident_spans_and_changed_object_ids() {
    isolated(
        "placed_landmark_identity_survives_coincident_spans_and_changed_object_ids",
        || {
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
            // Fill the original 2..=15 layer slots before admitting three
            // coincident landmarks on the shared Ground layer.
            const RESERVED_SPANS: usize = 14;
            let base = (0..RESERVED_SPANS).map(|index| {
                [
                    35.0 + (index % 5) as f32 * 30.0,
                    100.0 + (index / 5) as f32 * 30.0,
                    21.0,
                    0.0,
                ]
            });
            let overflow = [
                0.0,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::FRAC_PI_4,
            ]
            .map(|angle| [50.0, 50.0, 21.0, angle]);
            for pose in base.chain(overflow) {
                out.open_data_chunk("Object", 3);
                for value in pose {
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
                template
                    .add_kind_of(KindOf::Structure)
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
                let mut tower = crate::game_logic::ThingTemplate::new("IdentityTower");
                tower
                    .add_kind_of(KindOf::Structure)
                    .add_kind_of(KindOf::BridgeTower)
                    .set_health(100.0);
                tower.geometry_info.major_radius = 4.0;
                world.templates.insert("IdentityTower".into(), tower);
                let mut roads = world.terrain_road_baseline().clone();
                roads
                    .new_bridge("IdentityLandmark".into())
                    .tower_object_name = std::array::from_fn(|_| "IdentityTower".into());
                world.admit_terrain_road_definitions(roads).unwrap();
            };
            let mut source = GameLogic::new();
            prepare(&mut source);
            assert!(source.load_map(path.to_str().unwrap()));
            let mut peer = GameLogic::new();
            prepare(&mut peer);
            assert!(peer.load_map(path.to_str().unwrap()));
            let source_ids: Vec<_> = source
                .spawned_map_object_ids
                .iter()
                .map(|(id, _)| *id)
                .collect();
            let peer_ids: Vec<_> = peer
                .spawned_map_object_ids
                .iter()
                .map(|(id, _)| *id)
                .collect();
            assert_eq!(source_ids.len(), RESERVED_SPANS + 3);
            assert_eq!(source_ids, peer_ids);
            let [east_id, north_id, deleted_id] = source_ids[RESERVED_SPANS..].try_into().unwrap();
            assert!(source.delete_owned_bridge_at(Vec3::new(50.0, 21.0, 50.0)));
            assert!(source.bridge_behavior.span(deleted_id).is_none());
            assert!(source.bridge_behavior.create_scaffolding(north_id));
            assert!(source.bridge_behavior.create_scaffolding(source_ids[1]));
            source.refresh_owned_bridge_pathfinder_states();
            let mut saves =
                SaveFileManager::with_save_directory(temp.path().join("landmark_saves"));
            saves.init().unwrap();
            saves
                .save_game(
                    "landmarks",
                    &source,
                    &SaveGameInfo {
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
                    },
                )
                .unwrap();
            let decoded = saves.load_game_snapshot("landmarks").unwrap();
            let mut restored = GameLogic::new();
            prepare(&mut restored);
            restored.set_next_object_id_for_restore(ObjectId(1000));
            assert!(restored.load_map(&decoded.save_info.map_name));
            let reconstructed: Vec<_> = restored
                .spawned_map_object_ids
                .iter()
                .map(|(id, _)| *id)
                .collect();
            assert_eq!(reconstructed.len(), RESERVED_SPANS + 3);
            assert_ne!(reconstructed, source_ids);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let _foreign_ai = foreign.ai().write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                SnapshotBuilder::new()
                    .restore_from_snapshot(&decoded.snapshot, &mut restored)
                    .unwrap();
            });
            let rows = |world: &GameLogic| {
                let mut rows = Vec::new();
                world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .for_each_bridge(|bridge| {
                        let info = bridge.get_bridge_info();
                        rows.push((
                            info.source_map_object_index,
                            bridge.get_layer(),
                            ObjectId(info.bridge_object_id),
                            info.tower_object_id.map(ObjectId),
                        ));
                    });
                rows
            };
            let restored_rows = rows(&restored);
            assert_eq!(restored_rows, rows(&source));
            assert_eq!(rows(&peer).len(), RESERVED_SPANS + 3);
            let overflow_rows = &restored_rows[..2];
            assert_eq!(overflow_rows[0].0, Some(RESERVED_SPANS + 1));
            assert_eq!(overflow_rows[1].0, Some(RESERVED_SPANS));
            let mut tower_ids = std::collections::HashSet::new();
            for ((_, layer, id, towers), expected_id) in
                overflow_rows.iter().zip([north_id, east_id])
            {
                assert_eq!(*layer, gamelogic::path::PathfindLayerEnum::Ground);
                assert_eq!(*id, expected_id);
                assert_eq!(
                    *towers,
                    restored.bridge_behavior.span(*id).unwrap().tower_ids
                );
                for tower in towers {
                    assert!(
                        tower_ids.insert(*tower),
                        "each overflow bridge has its own towers"
                    );
                    assert_eq!(restored.bridge_behavior.span_id_for(*tower), Some(*id));
                    assert!(
                        restored
                            .host_object(*tower)
                            .unwrap()
                            .is_kind_of(KindOf::BridgeTower)
                    );
                }
            }
            assert!(restored.bridge_behavior.span(deleted_id).is_none());
            assert!(!peer.bridge_behavior.is_scaffold_present(north_id));
            assert!(restored.bridge_behavior.is_scaffold_present(north_id));
            assert!(restored.bridge_behavior.is_scaffold_present(source_ids[1]));
            let east = Vec3::new(70.0, 21.0, 50.0);
            let north = Vec3::new(50.0, 21.0, 70.0);
            let east_span = restored.bridge_behavior.span(east_id).unwrap();
            let north_span = restored.bridge_behavior.span(north_id).unwrap();
            assert!(east_span.point_on_deck(east));
            assert!(!east_span.point_on_deck(north));
            assert!(north_span.point_on_deck(north));
            assert!(!north_span.point_on_deck(east));
            let cell = restored
                .pathfinding_system
                .grid
                .world_to_grid(Vec3::new(65.0, 21.0, 100.0));
            assert_eq!(
                restored.pathfinding_system.grid.layer_cell_type(3, cell),
                Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
            );
        },
    );
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
            assert_eq!(source.host_objects().registered_len(), 1, "only the bridge span is an actor");
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
                for (receiver_index, owner) in [&mut source, &mut restored].into_iter().enumerate() {
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
                        let bridge_object = &owner.objects[&id];
                        assert!(
                            terrain.is_bridge_broken(id.0),
                            "receiver {receiver_index} bridge {id} health {} of {} state {:?}",
                            bridge_object.health.current,
                            bridge_object.health.maximum,
                            bridge_object.body_damage_state,
                        );
                    }
                    let cell = owner
                        .pathfinding_system
                        .grid
                        .world_to_grid(Vec3::new(50.0, 21.0, 50.0));
                    assert_eq!(
                        owner.pathfinding_system.grid.layer_cell_type(layer, cell),
                        Some(generals_sim::pathfinding::PathfindCellType::BridgeImpassable)
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
