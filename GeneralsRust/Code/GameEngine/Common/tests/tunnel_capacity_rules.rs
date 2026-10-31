use game_engine::common::ini::ini_game_data::TunnelCapacityRules;

#[test]
fn ordered_sources_preserve_defaults_and_override_without_global_state() {
    let base = TunnelCapacityRules::default();
    assert_eq!(base.max_capacity, 0);
    let application = base
        .overlay_ini_text("GameData\n MaxTunnelCapacity = 8\nEnd\n")
        .unwrap();
    let map = application
        .overlay_ini_text("GameData\n MaxTunnelCapacity = 2\nEnd\n")
        .unwrap();
    assert_eq!(application.max_capacity, 8);
    assert_eq!(map.max_capacity, 2);
    assert!(
        application
            .overlay_ini_text("GameData\n MaxTunnelCapacity = invalid\nEnd\n")
            .is_err()
    );
}
