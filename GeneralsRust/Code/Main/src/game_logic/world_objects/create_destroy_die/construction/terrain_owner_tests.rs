//! Real dozer creation, admitted map bytes and encoded continuation.
use super::*;
use crate::save_load::{
    GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, SnapshotBuilder,
};

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn ramp() -> Vec<u8> {
    (0..12)
        .flat_map(|y| (0..12).map(move |x| 20 + x * 5 + y * 7))
        .collect()
}

fn authored_owner() -> game_engine::common::dict::Dict {
    use game_engine::common::well_known_keys::{
        key_player_faction, key_player_is_human, key_player_name,
    };
    let mut owner = game_engine::common::dict::Dict::new();
    owner.set_ascii_string(key_player_name(), "RustPlayer0");
    owner.set_ascii_string(key_player_faction(), "FactionAmerica");
    owner.set_bool(key_player_is_human(), true);
    owner
}

fn authored_owner_id(world: &GameLogic) -> u32 {
    world
        .get_players()
        .iter()
        .find_map(|(&id, player)| (player.script_identity() == Some("RustPlayer0")).then_some(id))
        .expect("exact map-admitted RustPlayer0 controller")
}

fn map_bytes(raw: &[u8]) -> Vec<u8> {
    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [12, 12, 2, 144] {
        out.write_int(value);
    }
    for value in raw {
        out.write_byte(*value);
    }
    out.close_data_chunk();
    // This is authored side ordinal 0, not Player ID 0. C++ reserves Player 0
    // for Neutral and admits this named controller separately.
    out.open_data_chunk("SidesList", 2);
    out.write_int(1);
    out.write_dict(&authored_owner());
    out.write_int(0); // build list
    out.write_int(0); // authored teams; source admission adds default singletons
    out.close_data_chunk();
    out.into_ckmp_bytes()
}

fn world(path: &std::path::Path, raw: &[u8]) -> (GameLogic, ObjectId) {
    world_with_scaffold_geometry(path, raw, DEFAULT_SCAFFOLD_GEOMETRY)
}

fn world_with_scaffold_geometry(
    path: &std::path::Path,
    raw: &[u8],
    scaffold_geometry: &str,
) -> (GameLogic, ObjectId) {
    std::fs::write(path, map_bytes(raw)).unwrap();
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut logic = GameLogic::new();
    admit_construction_content(&mut logic, scaffold_geometry);
    assert!(logic.load_map(path.to_str().unwrap()));
    let owner = authored_owner_id(&logic);
    // Fund the admitted named controller, preserving its default Team.
    logic
        .get_player_mut(owner)
        .expect("map-admitted named controller")
        .resources
        .supplies = 1000;
    let constructor = logic
        .create_object_for_player("OwnedHeightDozer", owner, Vec3::ZERO)
        .unwrap();
    (logic, constructor)
}

fn construct(world: &mut GameLogic, builder: ObjectId, position: Vec3) -> ObjectId {
    let owner = authored_owner_id(world);
    world
        .create_dozer_scaffold(
            "OwnedHeightScaffold",
            owner,
            builder,
            position,
            0.,
            Resources {
                supplies: 300,
                power: 0,
            },
        )
        .unwrap()
}

fn bytes(world: &GameLogic) -> Vec<u8> {
    world
        .world_services
        .terrain()
        .read()
        .unwrap()
        .logic_height_map_bytes()
        .to_vec()
}

#[test]
fn actual_dozer_creation_lowers_receiving_map_and_preserves_same_id_peer() {
    isolated(
        "actual_dozer_creation_lowers_receiving_map_and_preserves_same_id_peer",
        || {
            let temp = tempfile::tempdir().unwrap();
            let (mut a, builder_a) = world(&temp.path().join("A.map"), &ramp());
            let (mut b, builder_b) = world(&temp.path().join("B.map"), &[150; 144]);
            assert_eq!(builder_a, builder_b);
            let before_b = bytes(&b);
            let id = construct(&mut a, builder_a, Vec3::new(30., 10., 30.));
            let mut expected = ramp();
            // Executed original portable box_ramp case: physical (bordered) indices.
            for (x, y) in [(4, 6), (5, 6), (6, 5), (6, 6), (7, 5), (7, 6), (7, 4)] {
                expected[y * 12 + x] = 80;
            }
            assert_eq!(bytes(&a), expected);
            assert_eq!(
                a.host_object(id).unwrap().get_position(),
                Vec3::new(30., 50., 30.)
            );
            assert_eq!(a.host_object(id).unwrap().builder_id, Some(builder_a));
            assert_eq!(a.host_object(id).unwrap().health.current, 1.);
            assert_eq!(
                a.get_player(authored_owner_id(&a))
                    .unwrap()
                    .effective_supplies(),
                700
            );
            assert_eq!(bytes(&b), before_b);
            assert_eq!(
                b.get_player(authored_owner_id(&b))
                    .unwrap()
                    .effective_supplies(),
                1000
            );
            let peer_id = construct(&mut b, builder_b, Vec3::new(30., 10., 30.));
            assert_eq!(peer_id, id);
            assert_eq!(b.host_object(peer_id).unwrap().get_position().y, 93.75);
            b.reset();
            assert_eq!(bytes(&a), expected);
            assert_eq!(
                a.terrain_ground_height_at(Vec3::new(40., 0., 30.)),
                Some(50.)
            );
            #[cfg(feature = "game_client")]
            {
                let snapshot = a.terrain_heightmap_snapshot().unwrap();
                assert_eq!((snapshot.heights[6 * 12 + 6] * 255.).round() as u8, 80);
                let frame = crate::presentation_frame::PresentationWorldEnv::from_logic_with_runtime_heightmap(&a, None);
                assert!(
                    frame
                        .runtime_heightmap
                        .as_ref()
                        .unwrap()
                        .source_byte_heights
                );
                assert_eq!(frame.sample_gameplay_terrain_height(40., 30.), Some(50.));
            }
        },
    );
}

#[test]
fn authored_small_and_zero_radius_skip_edits_but_still_snap_at_original_phase() {
    isolated(
        "authored_small_and_zero_radius_skip_edits_but_still_snap_at_original_phase",
        || {
            for small in [true, false] {
                let temp = tempfile::tempdir().unwrap();
                // Author each exact geometry before either native or host
                // template is admitted, rather than editing one projection.
                let geometry = if small {
                    "Geometry = BOX\nGeometryIsSmall = Yes\nGeometryHeight = 1\nGeometryMajorRadius = 13\nGeometryMinorRadius = 9\n"
                } else {
                    "Geometry = CYLINDER\nGeometryIsSmall = No\nGeometryHeight = 1\nGeometryMajorRadius = 0\nGeometryMinorRadius = 9\n"
                };
                let (mut logic, builder) =
                    world_with_scaffold_geometry(&temp.path().join("Skip.map"), &ramp(), geometry);
                let before = bytes(&logic);
                let id = construct(&mut logic, builder, Vec3::new(30., -90., 30.));
                assert_eq!(bytes(&logic), before);
                assert_eq!(logic.host_object(id).unwrap().get_position().y, 50.);
                assert_eq!(
                    logic
                        .get_player(authored_owner_id(&logic))
                        .unwrap()
                        .effective_supplies(),
                    700
                );
            }
        },
    );
}

#[test]
fn constructed_raw_height_survives_native_encoded_restore_and_next_creation() {
    isolated(
        "constructed_raw_height_survives_native_encoded_restore_and_next_creation",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("Saved.map");
            let (mut source, builder) = world(&path, &ramp());
            let member = admit_group_member(&mut source);
            let id = construct(&mut source, builder, Vec3::new(30., 10., 30.));
            let request = Vec3::new(50., 52., 30.);
            let adjusted = source.adjust_group_member_goal(member, request, request);
            assert_eq!((adjusted.x, adjusted.y, adjusted.z), (50., 50., 30.));
            let accepted = bytes(&source);
            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves
                .save_game(
                    "height_owner",
                    &source,
                    &SaveGameInfo {
                        filename: "height_owner".into(),
                        display_name: "Height owner".into(),
                        description: "receiving construction continuation".into(),
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
            let decoded = saves.load_game_snapshot("height_owner").unwrap();
            let mut restored = GameLogic::new();
            admit_construction_content(&mut restored, DEFAULT_SCAFFOLD_GEOMETRY);
            assert!(restored.load_map(&decoded.save_info.map_name));
            SnapshotBuilder::new()
                .restore_from_snapshot(&decoded.snapshot, &mut restored)
                .unwrap();
            assert_eq!(bytes(&restored), accepted);
            assert!(
                crate::game_logic::PathfindingGrid::is_doing_ground_movement(
                    restored.host_object(member).unwrap()
                )
            );
            assert_eq!(
                restored.adjust_group_member_goal(member, request, request),
                adjusted,
                "encoded restore must expose restored receiving raw bytes to the group, before any next creation"
            );
            assert_eq!(
                restored.host_object(id).unwrap().get_position(),
                source.host_object(id).unwrap().get_position()
            );
            let next_position = Vec3::new(40., 10., 30.);
            let next_source = construct(&mut source, builder, next_position);
            let next_restored = construct(&mut restored, builder, next_position);
            assert_eq!(next_source, next_restored);
            assert_eq!(bytes(&restored), bytes(&source));
            assert_eq!(
                restored.host_object(next_restored).unwrap().get_position(),
                source.host_object(next_source).unwrap().get_position()
            );
            assert_eq!(
                restored
                    .get_player(authored_owner_id(&restored))
                    .unwrap()
                    .effective_supplies(),
                source
                    .get_player(authored_owner_id(&source))
                    .unwrap()
                    .effective_supplies()
            );
            assert_eq!(
                restored.terrain_ground_height_at(next_position),
                source.terrain_ground_height_at(next_position)
            );
        },
    );
}

const DEFAULT_SCAFFOLD_GEOMETRY: &str = "Geometry = BOX\nGeometryIsSmall = No\nGeometryHeight = 1\nGeometryMajorRadius = 13\nGeometryMinorRadius = 9\n";

fn admit_construction_content(world: &mut GameLogic, scaffold_geometry: &str) {
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "owned_flatten_group.ini".to_owned(),
                format!(
                    "{GROUP_MEMBER_SOURCE}\nObject OwnedHeightDozer\nKindOf = DOZER\nBody = ActiveBody DozerBody\nMaxHealth = 200\nInitialHealth = 200\nEnd\nEnd\nObject OwnedHeightScaffold\nKindOf = STRUCTURE\nBuildCost = 300\n{scaffold_geometry}Body = ActiveBody ScaffoldBody\nMaxHealth = 900\nInitialHealth = 900\nEnd\nEnd\n"
                ),
            )])
            .expect("paired receiving construction and group definitions"),
        )
        .expect("complete receiving construction content before birth/restore");
}

const GROUP_MEMBER_SOURCE: &str = "Object OwnedFlattenGroupMember\n  KindOf = VEHICLE SELECTABLE\n  Geometry = CYLINDER\n  GeometryMajorRadius = 1\n  Body = ActiveBody ModuleTag_Body\n    MaxHealth = 100\n    InitialHealth = 100\n  End\n  Behavior = AIUpdateInterface ModuleTag_AI\n  End\n  Locomotor = SET_NORMAL OwnedFlattenGroupLocomotor\nEnd\n";

fn admit_group_member(world: &mut GameLogic) -> ObjectId {
    game_engine::common::ini::ini_locomotor::load_locomotors_from_str(
        "Locomotor OwnedFlattenGroupLocomotor\n  Surfaces = GROUND\n  Speed = 30\n  Appearance = TREADS\nEnd\n",
    ).unwrap();
    let owner = authored_owner_id(world);
    let id = world
        .create_object_for_player("OwnedFlattenGroupMember", owner, Vec3::new(60., 0., 30.))
        .unwrap();
    let member = world.host_object(id).unwrap();
    assert!(
        member.is_alive(),
        "the actual receiving group member must be live"
    );
    assert_eq!(member.health.current, 100.0);
    assert_eq!(member.health.maximum, 100.0);
    assert!(
        crate::game_logic::PathfindingGrid::is_doing_ground_movement(
            world.host_object(id).unwrap()
        )
    );
    id
}

fn borrowed_group_layer(world: &GameLogic, destination: Vec3) -> u8 {
    let owner = world.world_services.terrain();
    let terrain = owner.read().unwrap();
    world.pathfinding_system.grid.layer_for_destination(
        destination,
        &crate::game_logic::pathfinding::PathfindingGround::receiving(&terrain),
    ) as u8
}

#[test]
fn actual_construction_changes_receiving_group_ground_and_layer_without_peer_writeback() {
    isolated(
        "actual_construction_changes_receiving_group_ground_and_layer_without_peer_writeback",
        || {
            let temp = tempfile::tempdir().unwrap();
            let (mut a, builder_a) = world(&temp.path().join("GroupA.map"), &ramp());
            let (mut b, builder_b) = world(&temp.path().join("GroupB.map"), &[150; 144]);
            let member_a = admit_group_member(&mut a);
            let member_b = admit_group_member(&mut b);
            assert_eq!((builder_a, member_a), (builder_b, member_b));
            let request = Vec3::new(50., 52., 30.);
            let bridge = |world: &mut GameLogic| {
                world.pathfinding_system.grid.reserve_bridge_layer(
                    Vec3::new(40., 55., 20.),
                    Vec3::new(70., 55., 20.),
                    Vec3::new(40., 55., 40.),
                    Vec3::new(70., 55., 40.),
                )
            };
            let layer_a = bridge(&mut a);
            let layer_b = bridge(&mut b);
            assert_eq!(layer_a, layer_b);
            // TerrainLogic.cpp:1668 compares actual raw ground before bridge planes.
            assert_eq!(borrowed_group_layer(&a, request), layer_a);
            let peer_before = bytes(&b);
            construct(&mut a, builder_a, Vec3::new(30., 10., 30.));
            assert_eq!(
                borrowed_group_layer(&a, request),
                gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground as u8
            );
            assert_eq!(borrowed_group_layer(&b, request), layer_b);
            let adjusted = a.adjust_group_member_goal(member_a, request, request);
            assert_eq!((adjusted.x, adjusted.z), (50., 30.));
            assert_eq!(
                adjusted.y, 50.,
                "AIPathfind.cpp:8945 must sample lowered final XY, not the admitted TerrainData clone"
            );
            assert_eq!(bytes(&b), peer_before);
            b.reset();
            assert_eq!(
                a.adjust_group_member_goal(member_a, request, request),
                adjusted
            );
            assert_eq!(
                borrowed_group_layer(&a, request),
                gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground as u8
            );
        },
    );
}
