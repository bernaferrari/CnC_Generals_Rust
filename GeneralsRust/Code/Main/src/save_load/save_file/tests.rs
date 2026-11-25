//! Actual current named-chunk save routes.

mod camera_continuation;
mod camera_ownership;
mod client_candidate;
mod control_group_ownership;
#[cfg(all(feature = "game_client", not(target_arch = "wasm32")))]
mod mission_checkpoint;
mod particle_candidate;
mod particle_capture_failure;
mod script_candidate;
mod stealth_continuation;
mod terrain_candidate;

use super::*;
use crate::game_logic::{
    HackerDisableChannelPhase, HackerDisableChannelState, KindOf, ObjectId, Player,
    SupplyTruckState, Team, ThingTemplate,
};
use crate::save_load::snapshot::CollectorRuntimeSnapshot;
use glam::Vec3;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_save_info() -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: "legacy_fixture".to_string(),
        display_name: "Legacy production fixture".to_string(),
        description: "v1 bincode production payload".to_string(),
        map_name: "LegacyMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH,
        game_version: "test".to_string(),
        play_time: std::time::Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn unique_fixture_directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "generalsrust-legacy-production-{}-{nonce}",
        std::process::id()
    ))
}

fn decode_versioned_archive(bytes: &[u8]) -> SaveArchive {
    decode_versioned_save_archive(bytes).expect("decode versioned test archive")
}

fn encode_versioned_archive(archive: &SaveArchive) -> Vec<u8> {
    encode_versioned_save_archive(&SaveArchiveRef {
        version: archive.version,
        save_info: &archive.save_info,
        pristine_map_name: archive.pristine_map_name.as_deref(),
        snapshot: &archive.snapshot,
        campaign_state: archive.campaign_state.as_ref(),
        map_admission_intent: archive.map_admission_intent,
        saved_game_mode: archive.saved_game_mode,
        game_client_xfer: archive.game_client_xfer.as_deref(),
        terrain_visual_xfer: archive.terrain_visual_xfer.as_deref(),
        ghost_object_xfer: archive.snapshot.w3d_ghost_companion.as_deref(),
        particle_system_xfer: archive.snapshot.particle_system_companion.as_deref(),
        map: archive.map.as_ref(),
    })
    .expect("encode mutated versioned test archive")
}

fn encode_snapshot_archive(snapshot: &WorldSnapshot, save_info: &SaveGameInfo) -> Vec<u8> {
    encode_versioned_save_archive(&SaveArchiveRef {
        version: VERSIONED_SAVE_ARCHIVE_VERSION,
        save_info,
        pristine_map_name: save_info.pristine_map_name.as_deref(),
        snapshot,
        campaign_state: Some(&game_engine::System::CampaignManagerXferState::default()),
        map_admission_intent: MapAdmissionIntent::Authored,
        saved_game_mode: Some(SAVED_GAME_MODE_SINGLE_PLAYER),
        game_client_xfer: None,
        terrain_visual_xfer: None,
        ghost_object_xfer: snapshot.w3d_ghost_companion.as_deref(),
        particle_system_xfer: snapshot.particle_system_companion.as_deref(),
        map: None,
    })
    .expect("encode snapshot test archive")
}

#[test]
fn test_save_file_paths() {
    let manager = SaveFileManager::new();

    let save_path = manager.get_save_path("test_save");
    assert!(save_path.to_string_lossy().contains("test_save"));
    assert!(save_path
        .to_string_lossy()
        .ends_with(&format!(".{}", SAVE_EXTENSION)));

    let temp_path = manager.get_temp_path("test_temp");
    assert!(temp_path.to_string_lossy().contains("test_temp"));
    assert!(temp_path.to_string_lossy().ends_with(".tmp"));
}

#[test]
fn default_save_directory_is_user_data_save_like_popup() {
    let host = SaveLoadManager::default_save_directory();
    let popup = crate::subsystem_manager::resolve_save_directory();
    assert_eq!(
        host, popup,
        "host SaveFileManager and Popup TheGameState must share UserData/Save"
    );
    assert_eq!(host.file_name().and_then(|s| s.to_str()), Some("Save"));
    let manager = SaveFileManager::new();
    assert_eq!(manager.save_directory(), host.as_path());
}

#[test]
fn host_save_kind_survives_listing_and_versioned_archive_decode() {
    let directory = tempfile::tempdir().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    let world = GameLogic::new();
    for (filename, save_type) in [
        ("ordinary", SaveFileType::Normal),
        ("quick", SaveFileType::QuickSave),
        ("automatic", SaveFileType::AutoSave),
    ] {
        let info = SaveGameInfo {
            save_type,
            ..fixture_save_info()
        };
        manager.save_game(filename, &world, &info).unwrap();

        let data = std::fs::read(manager.get_save_path(filename)).unwrap();
        assert_eq!(
            &data[..VERSIONED_SAVE_MAGIC.len()],
            VERSIONED_SAVE_MAGIC,
            "production saves use the versioned serde container"
        );

        let reported = manager
            .list_saves()
            .unwrap()
            .into_iter()
            .find(|save| save.filename == filename)
            .unwrap();
        assert_eq!(reported.save_info.save_type, save_type);
        let loaded = manager.load_game_snapshot(filename).unwrap();
        assert_eq!(loaded.save_info.save_type, save_type);
    }
}

#[test]
fn mission_save_round_trip_does_not_restore_or_require_gameplay_cash() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mission_save_round_trip_does_not_restore_or_require_gameplay_cash",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let source = GameLogic::new();
            let mut info = fixture_save_info();
            info.save_type = SaveFileType::Mission;
            info.filename = "cash_mission".into();
            manager.save_game("cash_mission", &source, &info).unwrap();
            let bytes = std::fs::read(manager.get_save_path("cash_mission")).unwrap();
            assert_eq!(
                &bytes[..VERSIONED_SAVE_MAGIC.len()],
                VERSIONED_SAVE_MAGIC,
                "mission checkpoints use the same versioned archive container"
            );
            let mut receiving = GameLogic::new();
            let mut player = Player::new(3, Team::USA, "Existing match", true);
            player.resources.supplies = 987;
            receiving.add_player(player);
            let restored = manager.load_game("cash_mission", &mut receiving).unwrap();
            assert_eq!(restored.save_type, SaveFileType::Mission);
            assert_eq!(receiving.get_players().len(), 1);
            assert_eq!(receiving.get_player(3).unwrap().resources.supplies, 987);
        },
    );
}
