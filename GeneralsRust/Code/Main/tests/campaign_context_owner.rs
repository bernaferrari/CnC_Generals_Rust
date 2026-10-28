use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, VictoryType};
use generals_main::save_load::{
    CampaignId, CampaignManager, MissionInfo, MissionObjective, ObjectiveTarget, ObjectiveType,
};

fn mission(objective_id: &str, victory_rule: &str) -> MissionInfo {
    MissionInfo {
        id: objective_id.into(),
        campaign_id: CampaignId::USACampaign,
        mission_number: 1,
        name: objective_id.into(),
        description: objective_id.into(),
        map_name: "SharedCampaignMap".into(),
        briefing_video: None,
        preview_image: None,
        required_missions: Vec::new(),
        required_rank: None,
        required_honor_points: None,
        time_limit: None,
        starting_resources: generals_main::game_logic::Resources {
            supplies: 0,
            power: 0,
        },
        starting_units: Vec::new(),
        tech_restrictions: Vec::new(),
        special_rules: Vec::new(),
        victory_rule: Some(victory_rule.into()),
        primary_objectives: vec![MissionObjective {
            id: objective_id.into(),
            description: objective_id.into(),
            objective_type: ObjectiveType::Destroy,
            target: ObjectiveTarget::Building("Target".into()),
            required_count: Some(1),
            current_count: 0,
            time_limit: None,
            reward: None,
        }],
        secondary_objectives: Vec::new(),
        bonus_objectives: Vec::new(),
    }
}

fn accepted_minimal_map(path: &std::path::Path) {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    std::fs::write(path, map.into_ckmp_bytes()).unwrap();
    std::fs::write(path.with_file_name("Map.ini"), "").unwrap();
}

#[test]
fn accepted_same_map_admissions_keep_campaign_context_with_each_session() {
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    let campaign_dir_a = root_a.path().join("Save/Campaign");
    let campaign_dir_b = root_b.path().join("Save/Campaign");
    let mut campaign_a = CampaignManager::with_campaign_directory(campaign_dir_a.clone());
    let mut campaign_b = CampaignManager::with_campaign_directory(campaign_dir_b.clone());
    campaign_a.init().unwrap();
    campaign_b.init().unwrap();
    campaign_a.mission_definitions.insert(
        "session_a".into(),
        mission("objective_from_a", "NoUnits"),
    );
    campaign_b.mission_definitions.insert(
        "session_b".into(),
        mission("objective_from_b", "NoBuildings"),
    );
    assert_ne!(campaign_dir_a, campaign_dir_b);

    let map_root = tempfile::tempdir().unwrap();
    let map_path = map_root.path().join("SharedCampaignMap.map");
    accepted_minimal_map(&map_path);
    let map = map_path.to_str().unwrap();

    let mut session_a = GameLogic::new();
    let mut session_b = GameLogic::new();
    session_a.start_new_game(GameMode::SinglePlayer);
    session_b.start_new_game(GameMode::SinglePlayer);
    assert!(session_a.load_map_with_campaign_context(
        map,
        campaign_a.map_context_for(map),
    ));
    assert!(session_b.load_map_with_campaign_context(
        map,
        campaign_b.map_context_for(map),
    ));

    assert!(session_a
        .mission_objectives()
        .iter()
        .any(|objective| objective.id.as_deref() == Some("objective_from_a")));
    assert!(session_b
        .mission_objectives()
        .iter()
        .any(|objective| objective.id.as_deref() == Some("objective_from_b")));
    assert_eq!(session_a.victory_type(), VictoryType::NO_UNITS);
    assert_eq!(session_b.victory_type(), VictoryType::NO_BUILDINGS);
}
