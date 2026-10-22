//! Receiving GameData -> Map.ini -> Solo.ini -> actual map cells.
//! This tests grid admission, not hq-99crq COI history/visitation parity.
use super::*;
use crate::presentation_frame::PresentationFrame;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use game_engine::common::ini::ini_game_data::CameraMovementRules;
use gamelogic::common::Coord3D;
use gamelogic::system::shroud_manager::ShroudState;
use std::path::{Path, PathBuf};

const PROBE: &str = "ReceivingPartitionGridProbe542";
const LOOKER: &str = "ReceivingPartitionGridLooker542";
const CONTENT: &str = r#"Object ReceivingPartitionGridProbe542
  KindOf = INERT IMMOBILE
  ShroudClearingRange = 0
  Geometry = CYLINDER
  GeometryMajorRadius = 1
  GeometryHeight = 2
  Body = ActiveBody ProbeBody
    MaxHealth = 100
    InitialHealth = 100
  End
End
Object ReceivingPartitionGridLooker542
  KindOf = INERT IMMOBILE
  ShroudClearingRange = 1
  Geometry = CYLINDER
  GeometryMajorRadius = 1
  GeometryHeight = 2
  Body = ActiveBody LookerBody
    MaxHealth = 100
    InitialHealth = 100
  End
End
"#;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn map(
    dir: &Path,
    origin: [f32; 2],
    span: f32,
    map_cell: Option<f32>,
    solo_cell: Option<f32>,
) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("ReceivingGrid.map");
    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 0, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    // Exercise the existing admitted map-bounds metadata with nonzero XZ.
    out.open_data_chunk("WaypointsList", 1);
    out.write_int(2);
    for point in [
        [origin[0], 0., origin[1]],
        [origin[0] + span, 0., origin[1] + span],
    ] {
        for value in point {
            out.write_real(value);
        }
    }
    out.close_data_chunk();
    out.open_data_chunk("SidesList", 2);
    out.write_int(2);
    let mut neutral = game_engine::common::dict::Dict::new();
    neutral.set_ascii_string(game_engine::common::well_known_keys::key_player_name(), "");
    neutral.set_ascii_string(
        game_engine::common::well_known_keys::key_player_faction(),
        "FactionCivilian",
    );
    let mut viewer = game_engine::common::dict::Dict::new();
    viewer.set_ascii_string(
        game_engine::common::well_known_keys::key_player_name(),
        "GridViewer542",
    );
    viewer.set_ascii_string(
        game_engine::common::well_known_keys::key_player_faction(),
        "FactionAmerica",
    );
    viewer.set_bool(
        game_engine::common::well_known_keys::key_player_is_human(),
        true,
    );
    for side in [neutral, viewer] {
        out.write_dict(&side);
        out.write_int(0);
    }
    out.write_int(0);
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.open_data_chunk("Object", 3);
    let offset = 110_f32.min(span * 0.5);
    // Object placement is original Z-up XY, unlike the bounds helper above.
    for value in [origin[0] + offset, origin[1] + offset, 0., 0.] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string(PROBE);
    out.close_data_chunk();
    out.close_data_chunk();
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    for (name, cell) in [("Map.ini", map_cell), ("Solo.ini", solo_cell)] {
        if let Some(cell) = cell {
            std::fs::write(
                dir.join(name),
                format!("GameData\n PartitionCellSize = {cell}\nEnd\n"),
            )
            .unwrap();
        }
    }
    path
}

fn prepared(base_cell: f32) -> GameLogic {
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "grid-probe-542.ini".to_owned(),
                CONTENT.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.admit_camera_application_movement(CameraMovementRules {
        partition_cell_size: base_cell,
        ..Default::default()
    });
    world
}

fn load(world: &mut GameLogic, path: &Path) -> ObjectId {
    assert!(
        world.load_map(path.to_str().unwrap()),
        "actual selected CKMP and private companions must admit"
    );
    assert!(world.terrain.is_some(), "actual raw HeightMapData admitted");
    let objects: Vec<_> = world
        .objects
        .values()
        .filter(|object| object.get_template().name == PROBE)
        .map(|object| object.id)
        .collect();
    assert_eq!(
        objects.len(),
        1,
        "actual receiving map placement is born once"
    );
    objects[0]
}

fn viewer(world: &GameLogic) -> u32 {
    world
        .players
        .values()
        .find(|player| player.map_side.map_player_name == "GridViewer542")
        .expect("actual named map player must be admitted")
        .id
}

fn grid(world: &GameLogic) -> ((usize, usize, f32), [f32; 2]) {
    let shroud = world.world_services.shroud().lock().unwrap();
    (
        shroud.grid_dimensions().unwrap(),
        shroud.grid_world_origin().unwrap(),
    )
}

fn query_loaded_probe(world: &mut GameLogic, id: ObjectId, origin: [f32; 2]) {
    // Real loaded placement: (110,110) belongs to cell(1,1) at64 and
    // cell(2,2) at40. A corner cell0 query's nine cells reach only the former.
    // Both query and stamp wrongly using40 must fail this composed witness.
    assert_eq!(
        world.host_object(id).unwrap().get_position(),
        Vec3::new(origin[0] + 110., 0., origin[1] + 110.)
    );
    world.tick_physics_collisions_all();
    assert!(world.partition_manager.is_registered(id.0));
    assert_eq!(
        world
            .partition_manager
            .neighbor_object_ids(origin[0] + 1., origin[1] + 1.),
        vec![id.0]
    );
    assert!(
        world
            .partition_manager
            .ids_in_radius(origin[0] + 110., origin[1] + 110., 0.)
            .contains(&id.0)
    );
    assert!(
        world
            .partition_manager
            .ids_in_radius(origin[0] - 256., origin[1] + 110., 0.)
            .is_empty(),
        "off-map queries cannot sample an origin-zero fallback"
    );
}

fn actual_visible_cells(world: &GameLogic, player: u32) -> Vec<(usize, usize)> {
    let shroud = world.world_services.shroud().lock().unwrap();
    let (width, _, _) = shroud.grid_dimensions().unwrap();
    shroud
        .snapshot_grid_for_player(player)
        .unwrap()
        .into_iter()
        .enumerate()
        .filter_map(|(index, state)| {
            (state == ShroudState::Visible as u8).then_some((index % width, index / width))
        })
        .collect()
}

#[test]
fn actual_map_solo_non40_grid_drives_loaded_objects_frozen_fow_and_peer_reset() {
    isolated(
        "actual_map_solo_non40_grid_drives_loaded_objects_frozen_fow_and_peer_reset",
        || {
            let temp = tempfile::tempdir().unwrap();
            let first = map(
                &temp.path().join("First"),
                [-140., 480.],
                400.,
                Some(32.),
                Some(64.),
            );
            let second = map(
                &temp.path().join("Second"),
                [960., -400.],
                400.,
                Some(64.),
                Some(128.),
            );
            let mut owner = prepared(256.);
            let mut peer = prepared(256.);
            let old_global = game_engine::common::global_data::read().partition_cell_size;
            game_engine::common::global_data::write().partition_cell_size = 3.;
            let owner_id = load(&mut owner, &first);
            game_engine::common::global_data::write().partition_cell_size = 999.;
            let peer_id = load(&mut peer, &second);
            game_engine::common::global_data::write().partition_cell_size = old_global;
            assert_eq!(
                owner_id, peer_id,
                "equal raw IDs belong to distinct receiving worlds"
            );
            // Nonintegral, binary-exact ratios: 400/64=6.25 and400/128=3.125.
            // These witnesses do not assert nominal ceil at the original macro's
            // exact-integer rounding discontinuities.
            assert_eq!(grid(&owner), ((7, 7, 64.), [-140., 480.]));
            assert_eq!(grid(&peer), ((4, 4, 128.), [960., -400.]));
            assert_eq!(owner.camera_follow_partition_cell_size(), 64.);
            assert_eq!(peer.camera_follow_partition_cell_size(), 128.);
            query_loaded_probe(&mut owner, owner_id, [-140., 480.]);
            query_loaded_probe(&mut peer, peer_id, [960., -400.]);
            assert!(
                peer.partition_manager
                    .ids_in_radius(-136., 484., 0.)
                    .is_empty()
            );
            let local = viewer(&owner);
            let peer_local = viewer(&peer);
            let owner_looker = owner
                .create_object_for_player(LOOKER, local, Vec3::new(-136., 0., 484.))
                .unwrap();
            let peer_looker = peer
                .create_object_for_player(LOOKER, peer_local, Vec3::new(964., 0., -396.))
                .unwrap();
            assert_eq!(owner_looker, peer_looker);
            owner.update_main_crate_vision();
            peer.update_main_crate_vision();
            // Unmodified original DiscreteCircle radius1 emits a vertical span;
            // clipping at origin leaves these two literal cells (542 C++ witness).
            // Probe(110,110) is diagonal cell1
            // on64 (hidden) but remains in origin cell0 on128 (visible).
            assert_eq!(actual_visible_cells(&owner, local), vec![(0, 0), (0, 1)]);
            assert_eq!(
                actual_visible_cells(&peer, peer_local),
                vec![(0, 0), (0, 1)]
            );
            assert_eq!(
                owner
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(local, owner_id.0),
                Some(gamelogic::common::ObjectShroudStatus::Shrouded)
            );
            assert_eq!(
                peer.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(peer_local, peer_id.0),
                Some(gamelogic::common::ObjectShroudStatus::Clear)
            );
            let frozen = PresentationFrame::build_from_logic(&mut owner, local)
                .fow_grid()
                .clone();
            assert_eq!((frozen.width, frozen.height, frozen.cell_size), (7, 7, 64.));
            assert_eq!(frozen.world_origin_xy, [-140., 480.]);
            assert_eq!(
                frozen.state_at_world_xy(-30., 590.),
                crate::fow_rendering::PresentationFowGrid::CELL_HIDDEN
            );
            let peer_frozen = PresentationFrame::build_from_logic(&mut peer, peer_local)
                .fow_grid()
                .clone();
            assert_eq!(
                peer_frozen.state_at_world_xy(1070., -290.),
                crate::fow_rendering::PresentationFowGrid::CELL_VISIBLE
            );

            let peer_before = peer
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            owner.reset();
            assert!(
                owner
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .grid_dimensions()
                    .is_none()
            );
            assert_eq!(
                owner.camera_follow_partition_cell_size(),
                256.,
                "reset restores the receiving app rule, not a foreign publication"
            );
            assert_eq!(
                peer.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                peer_before
            );
            assert!(peer.partition_manager.is_registered(peer_id.0));
            assert_eq!((frozen.width, frozen.height, frozen.cell_size), (7, 7, 64.));
            // No map override survives a genuine reset plus replacement map load.
            std::fs::remove_file(first.parent().unwrap().join("Map.ini")).unwrap();
            std::fs::remove_file(first.parent().unwrap().join("Solo.ini")).unwrap();
            load(&mut owner, &first);
            assert_eq!(grid(&owner), ((2, 2, 256.), [-140., 480.]));
            assert_eq!(grid(&peer), ((4, 4, 128.), [960., -400.]));
        },
    );
}

#[test]
fn actual_non40_map_snapshot_continues_shroud_expiry_and_receiving_index() {
    isolated(
        "actual_non40_map_snapshot_continues_shroud_expiry_and_receiving_index",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = map(
                &temp.path().join("Saved"),
                [960., -400.],
                400.,
                Some(32.),
                Some(64.),
            );
            let peer_path = map(
                &temp.path().join("Peer"),
                [-140., 480.],
                400.,
                Some(128.),
                None,
            );
            let mut source = prepared(256.);
            let source_id = load(&mut source, &path);
            let mut receiver = prepared(256.);
            let receiver_id = load(&mut receiver, &path);
            let mut peer = prepared(256.);
            let peer_id = load(&mut peer, &peer_path);
            assert_eq!(source_id, receiver_id);
            assert_eq!(source_id, peer_id);
            let center = Coord3D::new(976., -384., 0.);
            let player = viewer(&source);
            {
                let mut shroud = source.world_services.shroud().lock().unwrap();
                shroud.do_shroud_reveal(&center, 80., 1 << player);
                shroud.queue_undo_shroud_reveal(&center, 80., 1 << player, 15, 0);
                assert_eq!(
                    shroud.get_shroud_state(player, &center),
                    ShroudState::Visible
                );
            }
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let peer_before = peer
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            builder
                .restore_from_snapshot(&decoded, &mut receiver)
                .unwrap();
            assert_eq!(grid(&receiver), ((7, 7, 64.), [960., -400.]));
            for frame in [15, 16] {
                for world in [&mut source, &mut receiver] {
                    world
                        .world_services
                        .shroud()
                        .lock()
                        .unwrap()
                        .process_pending_undo_shroud_reveals(frame);
                    assert_eq!(
                        world
                            .world_services
                            .shroud()
                            .lock()
                            .unwrap()
                            .get_shroud_state(player, &center),
                        if frame == 15 {
                            ShroudState::Visible
                        } else {
                            ShroudState::Explored
                        }
                    );
                    query_loaded_probe(world, source_id, [960., -400.]);
                }
                assert_eq!(
                    source
                        .world_services
                        .shroud()
                        .lock()
                        .unwrap()
                        .snapshot_state(),
                    receiver
                        .world_services
                        .shroud()
                        .lock()
                        .unwrap()
                        .snapshot_state()
                );
            }
            assert_eq!(
                peer.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                peer_before
            );
            assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.);
            // Source Xfer checks the grid already allocated for the receiving map.
            // This is the actual manager admission, not manual descriptor copying.
            let before_rejection = peer
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            let peer_pose = peer.host_object(peer_id).unwrap().get_position();
            let peer_health = peer.host_object(peer_id).unwrap().health.current;
            let peer_objects = peer.objects.len();
            let peer_camera = peer.capture_camera_continuation().unwrap();
            let source_before = source
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            let receiver_before = receiver
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            let builder_error = builder
                .restore_from_snapshot(&decoded, &mut peer)
                .unwrap_err()
                .to_string();
            assert!(
                builder_error.to_ascii_lowercase().contains("cell")
                    || builder_error.to_ascii_lowercase().contains("grid"),
                "unexpected preflight error: {builder_error}"
            );
            assert_eq!(peer.host_object(peer_id).unwrap().get_position(), peer_pose);
            assert_eq!(
                peer.host_object(peer_id).unwrap().health.current,
                peer_health
            );
            assert_eq!(peer.objects.len(), peer_objects);
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_camera);
            assert_eq!(
                peer.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                before_rejection
            );
            assert_eq!(
                source
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                source_before
            );
            assert_eq!(
                receiver
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                receiver_before
            );
            // This is geometry preflight atomicity only, not every later load callback.

            let error = peer
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .replace_state_owned(&decoded.shroud, 16)
                .unwrap_err();
            assert!(
                error.to_ascii_lowercase().contains("cell")
                    || error.to_ascii_lowercase().contains("grid"),
                "unexpected rejection: {error}"
            );
            assert_eq!(
                peer.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                before_rejection
            );
            assert_eq!(grid(&peer), ((4, 4, 128.), [-140., 480.]));
        },
    );
}

#[test]
fn actual_map_grid_clamps_subunit_definition_without_mutating_raw_camera_rule() {
    isolated(
        "actual_map_grid_clamps_subunit_definition_without_mutating_raw_camera_rule",
        || {
            let temp = tempfile::tempdir().unwrap();
            for (index, raw) in [0., -8., 0.5].into_iter().enumerate() {
                let origin = [120. + index as f32 * 32., -160.];
                let path = map(
                    &temp.path().join(format!("Clamp{index}")),
                    origin,
                    2.5,
                    None,
                    None,
                );
                let mut world = prepared(raw);
                load(&mut world, &path);
                assert_eq!(grid(&world), ((3, 3, 1.), origin));
                assert_eq!(
                    world.camera_follow_partition_cell_size(),
                    raw,
                    "C++ PM clamps its own derived size, not the raw GameData camera tether input"
                );
            }
        },
    );
}

#[test]
fn literal_non40_small_circle_and_rotated_box_footprints_use_receiving_cells() {
    use crate::game_logic::partition_coi::{
        HostPartitionFootprint, cells_touched_for_footprint_on_grid,
    };
    let origin = [-140., 480.];
    for small in [true, false] {
        let footprint = HostPartitionFootprint {
            major_radius: 1.,
            minor_radius: 1.,
            angle: 0.,
            is_small: small,
            is_box: false,
        };
        assert_eq!(
            cells_touched_for_footprint_on_grid(origin, [7, 7], 64., -30., 590., footprint)
                .unwrap(),
            vec![(1, 1)]
        );
        assert_eq!(
            cells_touched_for_footprint_on_grid(origin, [10, 10], 40., -30., 590., footprint)
                .unwrap(),
            vec![(2, 2)]
        );
    }
    // CPP doRectFill: half-cell steps from top-left, row then column;
    // major47/minor9 keeps step counts away from exact-integer ceil inputs.
    let footprint = HostPartitionFootprint {
        major_radius: 47.,
        minor_radius: 9.,
        angle: std::f32::consts::FRAC_PI_4,
        is_small: false,
        is_box: true,
    };
    assert_eq!(
        cells_touched_for_footprint_on_grid(origin, [7, 7], 64., -30., 590., footprint).unwrap(),
        vec![(1, 1), (1, 2)]
    );
    assert_eq!(
        cells_touched_for_footprint_on_grid(origin, [10, 10], 40., -30., 590., footprint).unwrap(),
        vec![(1, 2), (2, 2), (2, 3), (3, 3)]
    );
}
