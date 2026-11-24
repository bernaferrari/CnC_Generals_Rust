use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use std::time::{Duration, UNIX_EPOCH};

fn save_info(filename: &str) -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: filename.to_string(),
        display_name: "Save Fixture".to_string(),
        description: "Deterministic save/load fixture".to_string(),
        map_name: "RestoreApplicationMapless".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(42),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

#[test]
fn staged_logical_restore_keeps_the_receiving_application_channels() {
    use game_engine::common::common_services::CommonServicesScope;
    use gamelogic::system::engine_stores::{
        active_services, new_world_services, with_world_services,
    };
    use std::sync::Arc;

    let (mut live, receiving, foreign) = {
        let application = CommonServicesScope::new();
        let mut live = GameLogic::new();
        live.admit_object_ini_sources([(
            "RestoreApplication.ini".into(),
            "Object RestoreApplicationActor
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
"
            .into(),
        )])
        .unwrap();
        live.start_new_game(GameMode::SinglePlayer).unwrap();
        live.clear_all_players();
        live.add_player(Player::new(0, Team::USA, "ReceivingApplication", true));
        live.create_object_for_player("RestoreApplicationActor", 0, glam::Vec3::ZERO)
            .unwrap();
        let receiving = live.with_own_services(active_services);
        let foreign = {
            let _foreign_application = CommonServicesScope::new();
            new_world_services()
        };
        assert!(Arc::ptr_eq(
            &receiving.common().application,
            &application.services().application,
        ));
        (live, receiving, foreign)
    };
    assert!(!Arc::ptr_eq(
        &receiving.common().application,
        &foreign.common().application,
    ));
    live.set_current_frame(77);
    let directory = tempfile::tempdir().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    let name = "ApplicationChannelOwner";
    manager.save_game(name, &live, &save_info(name)).unwrap();

    with_world_services(&foreign, || {
        manager.load_game(name, &mut live).unwrap();
        assert_eq!(live.get_current_frame(), 77);
        let restored = live.with_own_services(active_services);
        assert!(!Arc::ptr_eq(&receiving, &restored));
        assert!(Arc::ptr_eq(
            &receiving.common().application,
            &restored.common().application,
        ));
    });
}
