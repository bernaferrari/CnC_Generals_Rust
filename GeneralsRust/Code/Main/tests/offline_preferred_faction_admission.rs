use game_engine::common::ini::INILoadType;
use game_engine::common::message_stream::ClientSkirmishLaunch;
use game_engine::common::preferences::CustomMatchPreferences;
use game_engine::common::rts::player_template::{
    PlayerTemplateStore, get_player_template_store_mut,
};
use game_engine::common::system::skirmish_game_info::SkirmishGameInfo;
use generals_main::game_logic::GameLogic;
use generals_main::skirmish_config::{
    SkirmishPlayerTemplateSelection, apply_skirmish_config, config_from_skirmish_launch_in,
};

fn two_factions() -> PlayerTemplateStore {
    let mut store = PlayerTemplateStore::new_owned();
    store
        .apply_source(
            "PlayerTemplate OfflineZero\n Side = USA\n StartingBuilding = ZeroCommandCenter\nEnd\n\
         PlayerTemplate OfflineOne\n Side = China\n StartingBuilding = OneCommandCenter\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    store
}

#[test]
fn preferred_real_faction_one_reaches_offline_skirmish_admission() {
    let definitions = two_factions();
    let previous = std::mem::replace(&mut *get_player_template_store_mut(), definitions.clone());

    let mut preferences = CustomMatchPreferences::new();
    preferences.set_preferred_faction(1);
    let selected = preferences.get_preferred_faction();
    assert_eq!(selected, 1, "index 1 is a real offline faction");

    let mut game_info = SkirmishGameInfo::default();
    game_info.slots[0].state = game_client::SlotState::Player as i32;
    game_info.slots[0].player_template = selected;
    game_info.slots[0].name = "Local Player".into();
    let launch = ClientSkirmishLaunch {
        map: "OfflineSentinelTest.map".into(),
        game_info,
        local_slot: 0,
        old_factions_only: false,
        fog_of_war: true,
        game_mode_code: 2,
        max_fps: 30,
    };
    let mut config = config_from_skirmish_launch_in(&launch, &definitions).unwrap();
    assert!(matches!(
        &config.slots[0].player_template,
        SkirmishPlayerTemplateSelection::Exact { template_name, template_index: 1 }
            if template_name == "OfflineOne"
    ));

    // This fixture does not load the multiplayer palette; discard its unrelated
    // color-source metadata after exercising the production GameInfo decoder.
    config.source_game_info = None;
    let mut logic = GameLogic::new();
    logic
        .admit_player_template_definitions(definitions)
        .unwrap();
    apply_skirmish_config(&mut logic, &config).unwrap();
    let admitted = logic.player_template_identity(0).unwrap();
    assert_eq!(admitted.template_index, Some(1));
    assert_eq!(admitted.template_name, "OfflineOne");

    *get_player_template_store_mut() = previous;
}
