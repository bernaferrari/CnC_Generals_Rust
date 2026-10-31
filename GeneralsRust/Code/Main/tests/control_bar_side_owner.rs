use game_engine::common::{ini::INILoadType, rts::player_template::PlayerTemplateStore};
use generals_main::game_logic::{GameLogic, Player, Team};
use generals_main::gameplay_layout::control_bar_local_player_side;
use generals_main::skirmish_config::{
    GameRulesSnapshot, SkirmishMatchConfig, SkirmishPlayerTemplateSelection, SkirmishSlotConfig,
    apply_skirmish_config,
};

#[test]
fn control_bar_uses_cpp_side_names_and_the_receiving_general_template() {
    let mut base = GameLogic::new();
    base.add_player(Player::new(0, Team::USA, "USA", true));
    assert!(base.select_local_player(Some(0)));
    assert_eq!(control_bar_local_player_side(&base), "America");

    let mut peer = GameLogic::new();
    let mut templates = PlayerTemplateStore::new_owned();
    templates
        .apply_source(
            "PlayerTemplate FactionTestUSA\n Side = AmericaLaserGeneral\n BaseSide = America\n StartingBuilding = AmericaCommandCenter\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    peer.admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    peer.admit_player_template_definitions(templates).unwrap();
    let config = SkirmishMatchConfig {
        source_game_info: None,
        map: "Maps/Probe.map".into(),
        random_seed: 31,
        old_factions_only: false,
        rules: GameRulesSnapshot::default_rules(),
        slots: vec![SkirmishSlotConfig {
            slot_index: 0,
            is_human: true,
            is_active: true,
            player_template: SkirmishPlayerTemplateSelection::Exact {
                template_name: "FactionTestUSA".into(),
                template_index: 0,
            },
            faction: "USA".into(),
            color_rgb: (0, 0, 200),
            team: 0,
            start_position: 0,
            player_name: "Owner".into(),
            ai_difficulty: None,
        }],
    };
    apply_skirmish_config(&mut peer, &config).unwrap();
    assert_eq!(control_bar_local_player_side(&peer), "AmericaLaserGeneral");
    assert_eq!(control_bar_local_player_side(&base), "America");
}
