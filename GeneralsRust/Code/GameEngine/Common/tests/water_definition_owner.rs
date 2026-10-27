use game_engine::common::ini::INILoadType;
use game_engine::common::ini::ini_water::{TimeOfDay, WaterDefinitions};

#[test]
fn water_definitions_retain_ordered_values_and_isolate_overrides() {
    let base = WaterDefinitions::default()
        .overlay_ini_text(
            "WaterSet Morning\nWaterTexture = First.tga\nWaterRepeatCount = 4\nEnd\nWaterTransparency\nTransparentWaterDepth = 3\nSkyboxTextureN = BaseN.tga\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap()
        .overlay_ini_text(
            "WaterSet Morning\nWaterRepeatCount = 7\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    assert_eq!(
        base.setting(TimeOfDay::Morning).texture_name.as_str(),
        "First.tga"
    );
    assert_eq!(base.setting(TimeOfDay::Morning).water_repeat_count, 7);

    let first = base
        .overlay_ini_text(
            "WaterTransparency\nTransparentWaterDepth = 8\nSkyboxTextureN = FirstN.tga\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    let peer = base
        .overlay_ini_text(
            "WaterTransparency\nTransparentWaterDepth = 12\nSkyboxTextureN = PeerN.tga\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    assert_eq!(first.transparency().transparent_water_depth, 8.0);
    assert_eq!(peer.transparency().transparent_water_depth, 12.0);
    assert_eq!(base.transparency().transparent_water_depth, 3.0);
    assert_eq!(
        first.presentation().original_skybox_textures[0],
        "BaseN.tga"
    );
    assert_eq!(first.presentation().skybox_textures[0], "FirstN.tga");
    let mut lighting = game_engine::common::ini::ini_game_data::LightingContentRules::default();
    lighting.water_definitions = first.clone();
    let lighting = lighting
        .overlay_ini_text("GameData\n WaterPositionZ = 14\nEnd\n")
        .unwrap();
    assert_eq!(
        lighting
            .water_definitions
            .transparency()
            .skybox_texture_n
            .as_str(),
        "FirstN.tga"
    );

    // Original overrides inherit the non-overloaded base, not the prior override.
    let replaced = first
        .overlay_ini_text(
            "WaterTransparency\nTransparentWaterDepth = 4\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    assert_eq!(
        replaced.transparency().skybox_texture_n.as_str(),
        "BaseN.tga"
    );
    assert_eq!(first.transparency().skybox_texture_n.as_str(), "FirstN.tga");
    assert!(
        first
            .overlay_ini_text(
                "WaterTransparency\nTransparentWaterDepth = invalid\nEnd\n",
                INILoadType::CreateOverrides,
            )
            .is_err()
    );
    assert_eq!(first.transparency().transparent_water_depth, 8.0);
}

#[test]
fn water_none_is_a_valid_index_and_unrelated_assignments_are_ignored() {
    assert_eq!(
        WaterDefinitions::default().setting(TimeOfDay::Morning).surface_color,
        (0.0, 0.0, 0.0, 0.0)
    );
    let rules = WaterDefinitions::default()
        .overlay_ini_text(
            "Object Example\nWaterTransparency = unrelated\nEnd\nWaterSet NONE\nWaterRepeatCount = 11\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    assert_eq!(rules.setting(TimeOfDay::Invalid).water_repeat_count, 11);
    assert_eq!(rules.transparency().transparent_water_depth, 3.0);
}
