//! Real dozer creation, admitted map bytes and encoded continuation.
use super::*;
use crate::game_logic::{HostGeometryInfo, HostGeometryType};
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
    owner.set_ascii_string(key_player_name(), "USA");
    owner.set_ascii_string(key_player_faction(), "FactionAmerica");
    owner.set_bool(key_player_is_human(), true);
    owner
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
    // This map really owns USA's default team. Validation appends the reserved
    // neutral after this authored slot, preserving owner0 and reproducible IDs.
    out.open_data_chunk("SidesList", 2);
    out.write_int(1);
    out.write_dict(&authored_owner());
    out.write_int(0); // build list
    out.write_int(0); // authored teams; source admission adds default singletons
    out.close_data_chunk();
    out.into_ckmp_bytes()
}

fn world(path: &std::path::Path, raw: &[u8]) -> (GameLogic, ObjectId) {
    std::fs::write(path, map_bytes(raw)).unwrap();
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut logic = GameLogic::new();
    assert!(logic.load_map(path.to_str().unwrap()));
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.apply_map_side_dict(&authored_owner(), false, &mut logic.economy_events);
    player.resources.supplies = 1000;
    logic.add_player(player);
    let mut builder = ThingTemplate::new("OwnedHeightDozer");
    builder.add_kind_of(KindOf::Dozer).set_health(200.);
    logic.templates.insert("OwnedHeightDozer".into(), builder);
    let constructor = logic
        .create_object_for_player("OwnedHeightDozer", 0, Vec3::ZERO)
        .unwrap();
    let mut template = ThingTemplate::new("OwnedHeightScaffold");
    template
        .add_kind_of(KindOf::Structure)
        .set_health(900.)
        .set_cost(300, 0);
    template.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        authored: true,
        major_radius: 13.,
        minor_radius: 9.,
        ..Default::default()
    };
    logic
        .templates
        .insert("OwnedHeightScaffold".into(), template);
    (logic, constructor)
}

fn construct(world: &mut GameLogic, builder: ObjectId, position: Vec3) -> ObjectId {
    world
        .create_dozer_scaffold(
            "OwnedHeightScaffold",
            0,
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
            assert_eq!(a.get_player(0).unwrap().effective_supplies(), 700);
            assert_eq!(bytes(&b), before_b);
            assert_eq!(b.get_player(0).unwrap().effective_supplies(), 1000);
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
                let (mut logic, builder) = world(&temp.path().join("Skip.map"), &ramp());
                let geometry = &mut logic
                    .templates
                    .get_mut("OwnedHeightScaffold")
                    .unwrap()
                    .geometry_info;
                geometry.is_small = small;
                if !small {
                    geometry.geom_type = HostGeometryType::Cylinder;
                    geometry.major_radius = 0.;
                }
                let before = bytes(&logic);
                let id = construct(&mut logic, builder, Vec3::new(30., -90., 30.));
                assert_eq!(bytes(&logic), before);
                assert_eq!(logic.host_object(id).unwrap().get_position().y, 50.);
                assert_eq!(logic.get_player(0).unwrap().effective_supplies(), 700);
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
            let id = construct(&mut source, builder, Vec3::new(30., 10., 30.));
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
            assert!(restored.load_map(&decoded.save_info.map_name));
            restored.templates = source.templates.clone();
            SnapshotBuilder::new()
                .restore_from_snapshot(&decoded.snapshot, &mut restored)
                .unwrap();
            assert_eq!(bytes(&restored), accepted);
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
                restored.get_player(0).unwrap().effective_supplies(),
                source.get_player(0).unwrap().effective_supplies()
            );
            assert_eq!(
                restored.terrain_ground_height_at(next_position),
                source.terrain_ground_height_at(next_position)
            );
        },
    );
}
