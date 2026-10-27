use game_engine::common::ini::ini_game_data::CombatBonusRules;

#[test]
fn ordered_game_data_and_map_overrides_keep_separate_baselines() {
    let base = CombatBonusRules::default()
        .overlay_ini_text(
            "GameData\n HumanSoloPlayerHealthBonus_Normal = 125%\n HealthBonus_Veteran = 150%\n WeaponBonus = SOLO_HUMAN_NORMAL DAMAGE 110%\nEnd\n",
        )
        .unwrap();
    let first = base
        .clone()
        .overlay_ini_text(
            "GameData\n HumanSoloPlayerHealthBonus_Normal = 175%\n WeaponBonus = SOLO_HUMAN_NORMAL DAMAGE 140%\nEnd\n",
        )
        .unwrap();
    let peer = base
        .clone()
        .overlay_ini_text("GameData\n HealthBonus_Veteran = 225%\nEnd\n")
        .unwrap();

    assert_eq!(base.solo_health[0][1], 1.25);
    assert_eq!(first.solo_health[0][1], 1.75);
    assert_eq!(peer.solo_health[0][1], 1.25);
    assert_eq!(first.veterancy_health[1], 1.5);
    assert_eq!(peer.veterancy_health[1], 2.25);
    assert_eq!(base.weapon_entries.len(), 1);
    assert_eq!(first.weapon_entries.len(), 2);
    assert_eq!(first.weapon_entries.last().unwrap().value, 1.4);
    assert!(
        base.clone()
            .overlay_ini_text("GameData\n HealthBonus_Veteran = invalid\nEnd\n")
            .is_err()
    );
    assert_eq!(base.veterancy_health[1], 1.5);
}
