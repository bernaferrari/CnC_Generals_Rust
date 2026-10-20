//! Real named archives retain the native camera independently of the C++ View sidecar.
use super::*;
use crate::save_load::snapshot::persist_v18::{
    CameraPersist, TacticalViewPersist, capture_persist_v18, parse_tactical_view_block,
};

#[test]
#[cfg(feature = "game_client")]
fn camera_owned_save_manager_interleaved_archives_preserve_full_native_pose() {
    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    let a = GameLogic::new();
    let b = GameLogic::new();
    let mut client_a = game_client::core::game_client::GameClient::new().unwrap();
    let mut client_b = game_client::core::game_client::GameClient::new().unwrap();
    let camera_a = CameraPersist {
        angle: 0.25,
        position: [1000.0, 2000.0, 3000.0],
        target: [10.0, 30.0, 20.0],
        zoom: 0.75,
    };
    let camera_b = CameraPersist {
        angle: 1.5,
        position: [-100.0, 400.0, 900.0],
        target: [-10.0, 50.0, 70.0],
        zoom: 1.25,
    };
    let info = fixture_save_info();
    for (name, world, client, camera) in [
        ("camera_a", &a, &mut client_a, camera_a),
        ("camera_b", &b, &mut client_b, camera_b),
    ] {
        saves
            .save_game_with_client_state_and_camera(
                name,
                world,
                ClientDrawableWorldSnapshot::default(),
                client,
                &info,
                camera,
            )
            .unwrap();
    }
    let crate::save_load::DecodedGameSave {
        snapshot: loaded_b, ..
    } = saves.load_game_snapshot("camera_b").unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: loaded_a, ..
    } = saves.load_game_snapshot("camera_a").unwrap();
    for (loaded, expected, source_position) in [
        (&loaded_a, camera_a, [10.0, 20.0, 30.0]),
        (&loaded_b, camera_b, [-10.0, 70.0, 50.0]),
    ] {
        assert_eq!(loaded.persist_v18.camera(), Some(expected));
        assert_eq!(
            loaded.persist_v18.tactical_view_camera,
            Some(TacticalViewPersist {
                angle: expected.angle,
                position: source_position,
            })
        );
        let mut restored = GameLogic::new();
        saves.restore_game_snapshot(loaded, &mut restored).unwrap();
        assert_eq!(capture_persist_v18(&restored).camera(), None);
        let native = bincode_legacy::serialize(loaded).unwrap();
        let decoded: WorldSnapshot = bincode_legacy::deserialize(&native).unwrap();
        assert_eq!(decoded.persist_v18.camera(), Some(expected));
        assert_eq!(decoded.persist_v18.tactical_view_camera, None);
    }
    // Invalid detached input is rejected before snapshot/client capture or any
    // archive write. It cannot change either previously accepted camera.
    for invalid in [
        CameraPersist {
            angle: f32::NAN,
            ..camera_a
        },
        CameraPersist {
            position: [f32::INFINITY, 0.0, 0.0],
            ..camera_a
        },
        CameraPersist {
            target: [0.0, f32::NAN, 0.0],
            ..camera_a
        },
        CameraPersist {
            zoom: 0.0,
            ..camera_a
        },
    ] {
        assert!(
            saves
                .save_game_with_client_state_and_camera(
                    "invalid_camera",
                    &a,
                    ClientDrawableWorldSnapshot::default(),
                    &mut client_a,
                    &info,
                    invalid,
                )
                .is_err()
        );
        assert!(!saves.get_save_path("invalid_camera").exists());
        assert_eq!(
            saves
                .load_game_snapshot("camera_a")
                .unwrap()
                .snapshot
                .persist_v18
                .camera(),
            Some(camera_a)
        );
        assert_eq!(
            saves
                .load_game_snapshot("camera_b")
                .unwrap()
                .snapshot
                .persist_v18
                .camera(),
            Some(camera_b)
        );
    }
    // Reading a peer's source chunk is a pure parse, never a pending camera write.
    let archive_b = std::fs::read(saves.get_save_path("camera_b")).unwrap();
    let blocks = walk_named_chunks(&archive_b).unwrap();
    let view = blocks
        .iter()
        .find(|(name, _)| name == CHUNK_TACTICAL_VIEW)
        .unwrap();
    assert_eq!(
        parse_tactical_view_block(&view.1).unwrap().position,
        [-10.0, 70.0, 50.0]
    );
    assert_eq!(capture_persist_v18(&a).camera(), None);
    assert_eq!(loaded_a.persist_v18.camera(), Some(camera_a));
}
