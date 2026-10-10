//! Actual map admission, frozen presentation and native shroud continuation.
use super::*;
use crate::fow_rendering::PresentationFowGrid;
use crate::presentation_frame::PresentationFrame;
use crate::save_load::{
    GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, SnapshotBuilder,
};
use gamelogic::common::{Coord3D, ObjectShroudStatus};
use gamelogic::system::shroud_manager::ShroudState;

fn map_bytes(origin: [f32; 2]) -> Vec<u8> {
    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 0, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    // Exercise the loader's admitted bounds metadata, including negative XZ.
    out.open_data_chunk("WaypointsList", 1);
    out.write_int(2);
    for position in [
        [origin[0], 0.0, origin[1]],
        [origin[0] + 400.0, 0.0, origin[1] + 400.0],
    ] {
        for component in position {
            out.write_real(component);
        }
    }
    out.close_data_chunk();
    out.into_ckmp_bytes()
}

fn load(path: &Path) -> GameLogic {
    let mut world = GameLogic::new();
    assert!(world.load_map(path.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Viewer", true));
    world.add_player(Player::new(2, Team::China, "Enemy", false));
    world
}

fn admit_units(world: &mut GameLogic, range: f32) -> (ObjectId, ObjectId) {
    let mut looker = ThingTemplate::new("OffsetLooker");
    looker.shroud_clearing_range = range;
    world.templates.insert(looker.name.clone(), looker);
    let mut enemy = ThingTemplate::new("OffsetEnemy");
    enemy.add_kind_of(KindOf::Infantry);
    enemy.shroud_clearing_range = 0.0;
    world.templates.insert(enemy.name.clone(), enemy);
    let edge = world.world_min + Vec3::new(20.0, 0.0, 20.0);
    (
        world
            .create_object_for_player("OffsetLooker", 1, edge)
            .unwrap(),
        world
            .create_object_for_player("OffsetEnemy", 2, edge)
            .unwrap(),
    )
}

#[test]
fn offset_map_vision_and_frozen_grid_use_driving_map_cells() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "offset_map_vision_and_frozen_grid_use_driving_map_cells",
        || {
            let temp = tempfile::tempdir().unwrap();
            let a_path = temp.path().join("OffsetA.map");
            let b_path = temp.path().join("OffsetB.map");
            std::fs::write(&a_path, map_bytes([-140.0, 480.0])).unwrap();
            std::fs::write(&b_path, map_bytes([960.0, -400.0])).unwrap();
            let mut a = load(&a_path);
            let mut b = load(&b_path);
            assert_eq!(a.world_min.x, -140.0);
            assert_eq!(b.world_min.z, -400.0);
            let (a_looker, a_enemy) = admit_units(&mut a, 80.0);
            let (b_looker, b_enemy) = admit_units(&mut b, 0.0);
            assert_eq!((a_looker, a_enemy), (b_looker, b_enemy));
            let foreign = gamelogic::object::collide::partition_manager::PARTITION_MANAGER
                .write()
                .unwrap();
            a.update_main_crate_vision();
            b.update_main_crate_vision();
            assert_eq!(
                a.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(1, a_enemy.0),
                Some(ObjectShroudStatus::Clear)
            );
            assert_eq!(
                b.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(1, b_enemy.0),
                Some(ObjectShroudStatus::Shrouded)
            );
            drop(foreign);
            let a_frame = PresentationFrame::build_from_logic(&mut a, 1);
            let frozen = a_frame.fow_grid();
            assert_eq!(frozen.world_origin_xy, [-140.0, 480.0]);
            assert_eq!((frozen.width, frozen.height), (10, 10));
            assert_eq!(
                frozen.state_at_world_xy(-120.0, 500.0),
                PresentationFowGrid::CELL_VISIBLE
            );
            assert_eq!(
                frozen.state_at_world_xy(220.0, 840.0),
                PresentationFowGrid::CELL_HIDDEN
            );
            a.host_object_mut(a_looker)
                .unwrap()
                .set_position(Vec3::new(220.0, 0.0, 840.0));
            a.update_main_crate_vision();
            a.frame = 150;
            a.update_main_crate_vision();
            assert_eq!(
                a.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(1, a_enemy.0),
                Some(ObjectShroudStatus::Clear)
            );
            b.frame = 200;
            b.update_main_crate_vision();
            b.reset();
            drop(b);
            a.frame = 151;
            a.update_main_crate_vision();
            assert_eq!(
                a.world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_host_object_shroud_status(1, a_enemy.0),
                Some(ObjectShroudStatus::Shrouded)
            );
            assert_eq!(
                frozen.state_at_world_xy(-120.0, 500.0),
                PresentationFowGrid::CELL_VISIBLE,
                "a completed frame remains immutable after both worlds advance/reset"
            );
        },
    );
}

#[test]
fn native_offset_map_restores_shroud_origin_and_pending_expiry() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_offset_map_restores_shroud_origin_and_pending_expiry",
        || {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("SavedOffset.map");
            let bytes = map_bytes([960.0, -400.0]);
            std::fs::write(&path, &bytes).unwrap();
            let mut source = load(&path);
            let center = Coord3D::new(980.0, -380.0, 0.0);
            {
                let mut shroud = source.world_services.shroud().lock().unwrap();
                shroud.do_shroud_reveal(&center, 80.0, 1 << 1);
                shroud.queue_undo_shroud_reveal(&center, 80.0, 1 << 1, 150, 0);
                assert_eq!(shroud.get_shroud_state(1, &center), ShroudState::Visible);
            }
            let before = source
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
            saves.init().unwrap();
            saves
                .save_game(
                    "offset",
                    &source,
                    &SaveGameInfo {
                        filename: "offset".into(),
                        display_name: "Offset".into(),
                        description: "owned origin".into(),
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
            let (saved, info) = saves.load_game_snapshot("offset").unwrap();
            assert_eq!(std::fs::read(&info.map_name).unwrap(), bytes);
            let mut restored = load(Path::new(&info.map_name));
            SnapshotBuilder::new()
                .restore_from_snapshot(&saved, &mut restored)
                .unwrap();
            assert_eq!(
                restored
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                before
            );
            let mut invalid = restored
                .world_services
                .shroud()
                .lock()
                .unwrap()
                .snapshot_state();
            invalid.grid.as_mut().unwrap().cells.pop();
            assert!(
                restored
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .replace_state(&invalid, 0)
                    .is_err()
            );
            for frame in [150, 151] {
                for world in [&mut source, &mut restored] {
                    let mut shroud = world.world_services.shroud().lock().unwrap();
                    shroud.process_pending_undo_shroud_reveals(frame);
                    assert_eq!(
                        shroud.get_shroud_state(1, &center),
                        if frame == 150 {
                            ShroudState::Visible
                        } else {
                            ShroudState::Explored
                        }
                    );
                }
                assert_eq!(
                    source
                        .world_services
                        .shroud()
                        .lock()
                        .unwrap()
                        .snapshot_state(),
                    restored
                        .world_services
                        .shroud()
                        .lock()
                        .unwrap()
                        .snapshot_state()
                );
            }
            source.reset();
            assert_eq!(
                restored
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .get_shroud_state(1, &center),
                ShroudState::Explored
            );
        },
    );
}
