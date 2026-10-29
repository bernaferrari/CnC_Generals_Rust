use super::super::*;
// Exact twenty declarations/values selected by the production startup resolver.
const DEBUG_BLOCK: &str = r#"GameData
Wireframe = No
StateMachineDebug = No
UseCameraConstraints = Yes
ShroudOn = Yes
FogOfWarOn = No
ShowCollisionExtents = No
DebugProjectileTileWidth = 5
DebugProjectileTileDuration = 90
DebugProjectileTileColor = R:0 G:0 B:100
DebugVisibilityTileCount = 64
DebugVisibilityTileWidth = 10.0
DebugVisibilityTileDuration = 90
DebugVisibilityTileTargettableColor = R:50 G:0 B:0
DebugVisibilityTileDeshroudColor = R:100 G:100 B:100
DebugVisibilityTileGapColor = R:0 G:0 B:0
DebugThreatMapTileDuration = 30
MaxDebugThreatMapValue = 5000
DebugCashValueMapTileDuration = 30
MaxDebugCashValueMapValue = 10000
VTune = No
End
"#;
fn parse_private(text: &str) -> INIResult<GlobalData> {
    let mut data = GlobalData::new();
    let mut ini = INI::new();
    ini.with_inline_source(text, |ini| {
        ini.read_line()?;
        assert_eq!(ini.get_next_token_or_null().as_deref(), Some("GameData"));
        parse_game_data_block(ini, &mut data)
    })?;
    Ok(data)
}

#[test]
fn omitted_debug_fields_match_explicit_cpp_constructor_defaults() {
    let data = parse_private("GameData\nEnd\n").unwrap();
    assert!(!data.wireframe);
    assert!(!data.state_machine_debug);
    assert!(data.use_camera_constraints);
    assert!(data.shroud_on);
    assert!(!data.fog_of_war_on);
    assert!(!data.jabber_on && !data.munkee_on);
    assert!(!data.allow_unselectable_selection);
    assert!(!data.disable_camera_fade);
    assert!(!data.disable_scripted_input_disabling);
    assert!(!data.disable_military_caption);
    assert_eq!(data.benchmark_timer, -1);
    assert!(data.check_for_leaks);
    assert!(!data.v_tune);
    assert!(!data.debug_camera && !data.debug_visibility);
    assert_eq!(data.debug_visibility_tile_count, 32);
    assert_eq!(data.debug_visibility_tile_width, 0.0);
    assert_eq!(data.debug_visibility_tile_duration, 30);
    assert_eq!(data.debug_visibility_targettable_color, RGBColor::black());
    assert_eq!(data.debug_visibility_deshroud_color, RGBColor::black());
    assert_eq!(data.debug_visibility_gap_color, RGBColor::black());
    assert_eq!(data.debug_projectile_tile_width, 10.0);
    assert_eq!(data.debug_projectile_tile_duration, 30);
    assert_eq!(data.debug_projectile_tile_color, RGBColor::black());
    assert_eq!(data.debug_threat_map_tile_duration, 30);
    assert_eq!(data.max_debug_threat, 5000);
    assert_eq!(data.debug_cash_value_map_tile_duration, 30);
    assert_eq!(data.max_debug_value, 10000);
    assert!(!data.debug_ignore_asserts && !data.debug_ignore_stack_trace);
    assert!(!data.show_collision_extents && !data.show_audio_locations);
    assert!(!data.save_stats && !data.save_all_stats);
    assert!(!data.use_local_motd);
    assert_eq!(data.base_stats_dir, ".\\");
    assert_eq!(data.motd_path, "MOTD.txt");
    assert_eq!(data.latency_average, 0);
    assert_eq!(data.latency_amplitude, 0);
    assert_eq!(data.latency_period, 0);
    assert_eq!(data.latency_noise, 0);
    assert_eq!(data.packet_loss, 0);
    assert!(!data.extra_logging);
}

#[test]
fn actual_startup_debug_declarations_populate_existing_typed_fields() {
    let data = parse_private(DEBUG_BLOCK).unwrap();
    assert!(!data.wireframe);
    assert!(!data.state_machine_debug);
    assert!(data.use_camera_constraints);
    assert!(data.shroud_on);
    assert!(!data.fog_of_war_on);
    assert!(!data.show_collision_extents);
    assert_eq!(data.debug_projectile_tile_width, 5.);
    assert_eq!(data.debug_projectile_tile_duration, 90);
    assert_eq!(
        data.debug_projectile_tile_color,
        RGBColor::new(0., 0., 100. / 255.)
    );
    assert_eq!(data.debug_visibility_tile_count, 64);
    assert_eq!(data.debug_visibility_tile_width, 10.);
    assert_eq!(data.debug_visibility_tile_duration, 90);
    assert_eq!(
        data.debug_visibility_targettable_color,
        RGBColor::new(50. / 255., 0., 0.)
    );
    assert_eq!(
        data.debug_visibility_deshroud_color,
        RGBColor::new(100. / 255., 100. / 255., 100. / 255.)
    );
    assert_eq!(data.debug_visibility_gap_color, RGBColor::new(0., 0., 0.));
    assert_eq!(data.debug_threat_map_tile_duration, 30);
    assert_eq!(data.max_debug_threat, 5000);
    assert_eq!(data.debug_cash_value_map_tile_duration, 30);
    assert_eq!(data.max_debug_value, 10000);
    assert!(!data.v_tune);
    let changed = parse_private("GameData\nWireframe=Yes\nStateMachineDebug=Yes\nUseCameraConstraints=No\nShroudOn=No\nFogOfWarOn=Yes\nShowCollisionExtents=Yes\nVTune=Yes\nEnd\n").unwrap();
    assert!(
        changed.wireframe
            && changed.state_machine_debug
            && changed.fog_of_war_on
            && changed.show_collision_extents
            && changed.v_tune
    );
    assert!(!changed.use_camera_constraints && !changed.shroud_on);
}
#[test]
fn actual_startup_debug_block_passes_all_receiving_rule_projections() {
    ProductionEnergyRules::default()
        .overlay_ini_text(DEBUG_BLOCK)
        .unwrap();
    PhysicsContentRules::from_global_data(&runtime_global_data::GlobalData::default())
        .overlay_ini_text(DEBUG_BLOCK)
        .unwrap();
    CameraContentRules::default()
        .overlay_ini_text(DEBUG_BLOCK)
        .unwrap();
    CameraClipRules::default()
        .overlay_ini_text(DEBUG_BLOCK)
        .unwrap();
    CameraShakeRules::default()
        .overlay_ini_text(DEBUG_BLOCK)
        .unwrap();
    CameraMovementRules::default()
        .overlay_ini_text(DEBUG_BLOCK)
        .unwrap();
}
#[test]
fn debug_declarations_do_not_hide_unknown_or_malformed_content() {
    for text in [
        "GameData\nWireframe=not_a_bool\nEnd\n",
        "GameData\nDebugProjectileTileWidth=not_a_number\nEnd\n",
        "GameData\nDebugVisibilityTileGapColor=R:1 G:2\nEnd\n",
        "GameData\nUnportedUnsupportedField=Yes\nEnd\n",
    ] {
        assert!(parse_private(text).is_err(), "must reject {text}");
        assert!(
            CameraContentRules::default()
                .overlay_ini_text(text)
                .is_err()
        );
    }
}
