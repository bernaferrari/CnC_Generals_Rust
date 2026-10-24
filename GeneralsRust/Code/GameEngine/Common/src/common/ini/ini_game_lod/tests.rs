use super::*;

// Parse one selected fixture block through the existing concrete owner parser.
// read_line establishes its header; get_next_value_token in that production
// parser skips the block keyword. Do not consume the name/value token here.
fn parse_owned(
    manager: &mut GameLODManager,
    source: &str,
    parser: fn(&mut GameLODManager, &mut INI) -> INIResult<()>,
) -> INIResult<()> {
    INI::new().with_inline_source(source, |ini| {
        ini.read_line()?;
        parser(manager, ini)
    })
}

#[test]
fn test_static_lod_level_conversion() {
    assert_eq!(
        StaticGameLODLevel::from_str("Low"),
        Some(StaticGameLODLevel::Low)
    );
    assert_eq!(
        StaticGameLODLevel::from_str("Medium"),
        Some(StaticGameLODLevel::Medium)
    );
    assert_eq!(
        StaticGameLODLevel::from_str("High"),
        Some(StaticGameLODLevel::High)
    );
    assert_eq!(
        StaticGameLODLevel::from_str("Custom"),
        Some(StaticGameLODLevel::Custom)
    );
    assert_eq!(StaticGameLODLevel::from_str("Invalid"), None);

    assert_eq!(StaticGameLODLevel::Low.to_str(), "Low");
    assert_eq!(StaticGameLODLevel::Medium.to_str(), "Medium");
    assert_eq!(StaticGameLODLevel::High.to_str(), "High");
    assert_eq!(StaticGameLODLevel::Custom.to_str(), "Custom");
}

#[test]
fn test_dynamic_lod_level_conversion() {
    assert_eq!(
        DynamicGameLODLevel::from_str("Low"),
        Some(DynamicGameLODLevel::Low)
    );
    assert_eq!(
        DynamicGameLODLevel::from_str("VeryHigh"),
        Some(DynamicGameLODLevel::VeryHigh)
    );
    assert_eq!(
        DynamicGameLODLevel::from_str("very high"),
        Some(DynamicGameLODLevel::VeryHigh)
    );
    assert_eq!(DynamicGameLODLevel::from_str("Invalid"), None);

    assert_eq!(DynamicGameLODLevel::VeryHigh.to_str(), "VeryHigh");
}

#[test]
fn test_particle_priority_matches_cpp_index_list() {
    assert_eq!(
        ParticlePriorityType::from_str("WEAPON_EXPLOSION"),
        Some(ParticlePriorityType::WeaponExplosion)
    );
    assert_eq!(
        ParticlePriorityType::from_str("SCORCHMARK"),
        Some(ParticlePriorityType::ScorchMark)
    );
    assert_eq!(
        ParticlePriorityType::from_str("AREA_EFFECT"),
        Some(ParticlePriorityType::AreaEffect)
    );
    assert_eq!(
        ParticlePriorityType::from_str("ALWAYS_RENDER"),
        Some(ParticlePriorityType::AlwaysRender)
    );
    assert_eq!(
        ParticlePriorityType::parse_cpp_index_list_token("12"),
        Some(ParticlePriorityType::Critical)
    );
    assert_eq!(ParticlePriorityType::ScorchMark.to_str(), "SCORCHMARK");
}

#[test]
fn test_cpu_type_conversion() {
    assert_eq!(CpuType::from_str("P3"), Some(CpuType::P3));
    assert_eq!(CpuType::from_str("P4"), Some(CpuType::P4));
    assert_eq!(CpuType::from_str("K7"), Some(CpuType::K7));
    assert_eq!(CpuType::from_str("Invalid"), None);

    assert_eq!(CpuType::P3.to_str(), "P3");
    assert_eq!(CpuType::P4.to_str(), "P4");
    assert_eq!(CpuType::K7.to_str(), "K7");
}

#[test]
fn test_chipset_type_conversion() {
    assert_eq!(ChipsetType::from_str("GF3"), Some(ChipsetType::GeForce3));
    assert_eq!(ChipsetType::from_str("GF4"), Some(ChipsetType::GeForce4));
    assert_eq!(ChipsetType::from_str("R300"), Some(ChipsetType::Radeon9700));
    assert_eq!(ChipsetType::from_str("Invalid"), None);

    assert_eq!(ChipsetType::GeForce3.to_str(), "GF3");
    assert_eq!(ChipsetType::Radeon9700.to_str(), "R300");
}

#[test]
fn test_static_lod_info_defaults() {
    let info = StaticGameLODInfo::default();
    assert_eq!(info.min_fps, 0);
    assert_eq!(info.sample_count_2d, 6);
    assert_eq!(info.sample_count_3d, 24);
    assert_eq!(info.max_particle_count, 2500);
    assert!(info.use_shadow_volumes);
    assert!(info.use_shadow_decals);
    assert_eq!(info.texture_reduction, 0);
}

#[test]
fn test_dynamic_lod_info_defaults() {
    let info = DynamicGameLODInfo::default();
    assert_eq!(info.min_fps, 0);
    assert_eq!(info.dynamic_particle_skip_mask, 0);
    assert_eq!(info.slow_death_scale, 1.0);
    assert_eq!(
        info.min_dynamic_particle_priority,
        ParticlePriorityType::WeaponExplosion
    );
}

#[test]
fn test_static_lod_bool_fields_use_cpp_yes_no_tokens() {
    let mut manager = GameLODManager::new();
    parse_owned(
        &mut manager,
        "StaticGameLOD Low\nUseShadowVolumes = No\nUseShadowDecals = Yes\nEnd\n",
        GameLODManager::parse_static_game_lod_definition,
    )
    .expect("valid C++ Yes/No bools should parse");

    let low = &manager.static_game_lod_info[StaticGameLODLevel::Low.to_index().unwrap()];
    assert!(!low.use_shadow_volumes);
    assert!(low.use_shadow_decals);
}

#[test]
fn test_static_lod_rejects_non_cpp_bool_tokens() {
    let mut manager = GameLODManager::new();
    let result = parse_owned(
        &mut manager,
        "StaticGameLOD Low\nUseShadowVolumes = true\nEnd\n",
        GameLODManager::parse_static_game_lod_definition,
    );

    assert!(result.is_err());
}

#[test]
fn test_static_lod_ignores_fields_not_in_cpp_parse_table() {
    let mut manager = GameLODManager::new();
    parse_owned(
        &mut manager,
        "StaticGameLOD Low\nUseFPSLimit = No\nEnableDynamicLOD = No\nUseTrees = No\nEnd\n",
        GameLODManager::parse_static_game_lod_definition,
    )
    .expect("unknown static LOD fields are ignored by the table parser");

    let low = &manager.static_game_lod_info[StaticGameLODLevel::Low.to_index().unwrap()];
    assert!(low.use_fps_limit);
    assert!(low.enable_dynamic_lod);
    assert!(low.use_trees);
}

#[test]
fn test_dynamic_lod_parses_cpp_priority_names_and_signed_skip_masks() {
    let mut manager = GameLODManager::new();
    parse_owned(&mut manager,
            "DynamicGameLOD Low\nParticleSkipMask = -1\nDebrisSkipMask = 7\nMinParticlePriority = SCORCHMARK\nMinParticleSkipPriority = AREA_EFFECT\nEnd\n",
            GameLODManager::parse_dynamic_game_lod_definition,
        )
        .expect("valid C++ dynamic LOD fields should parse");

    let low = &manager.dynamic_game_lod_info[DynamicGameLODLevel::Low.to_index().unwrap()];
    assert_eq!(low.dynamic_particle_skip_mask, u32::MAX);
    assert_eq!(low.dynamic_debris_skip_mask, 7);
    assert_eq!(
        low.min_dynamic_particle_priority,
        ParticlePriorityType::ScorchMark
    );
    assert_eq!(
        low.min_dynamic_particle_skip_priority,
        ParticlePriorityType::AreaEffect
    );
}

#[test]
fn test_game_lod_manager() {
    let manager = GameLODManager::new();

    assert!(manager.get_static_game_lod_index("Low").is_some());
    assert!(manager.get_static_game_lod_index("Invalid").is_none());
    assert!(manager.get_dynamic_game_lod_index("VeryHigh").is_some());
}

#[test]
fn test_add_lod_preset() {
    let mut manager = GameLODManager::new();

    // Add preset to Low level (index 0)
    let preset = manager.new_lod_preset(0);
    assert!(preset.is_some());

    let preset = preset.unwrap();
    preset.cpu_type = CpuType::P4;
    preset.mhz = 1500;
    preset.video_type = ChipsetType::GeForce3;
    preset.memory = 128;

    assert_eq!(manager.num_level_presets[0], 1);
}

#[test]
fn test_add_bench_profile() {
    let mut manager = GameLODManager::new();

    let profile = manager.new_bench_profile();
    assert!(profile.is_some());

    let profile = profile.unwrap();
    profile.cpu_type = CpuType::P4;
    profile.mhz = 2189;
    profile.int_bench_index = 6.108187;
    profile.float_bench_index = 15.113676;
    profile.mem_bench_index = 9.402223;

    assert_eq!(manager.num_bench_profiles, 1);
}

#[test]
fn test_skip_mask_zero_never_skips() {
    let mut manager = GameLODManager::new();
    assert_eq!(manager.dynamic_debris_skip_mask, 0);
    assert_eq!(manager.dynamic_particle_skip_mask, 0);
    for _ in 0..32 {
        assert!(!manager.is_debris_skipped());
        assert!(!manager.is_particle_skipped());
    }
}

#[test]
fn test_skip_mask_one_skips_every_other_call() {
    let mut manager = GameLODManager::new();
    let low = DynamicGameLODLevel::Low.to_index().unwrap();
    manager.dynamic_game_lod_info[low].dynamic_debris_skip_mask = 1;
    manager.dynamic_game_lod_info[low].dynamic_particle_skip_mask = 1;
    manager.dynamic_game_lod_info[low].slow_death_scale = 0.5;

    assert!(manager.set_dynamic_lod_level(DynamicGameLODLevel::Low));
    assert_eq!(manager.get_dynamic_lod_level(), DynamicGameLODLevel::Low);
    assert_eq!(manager.dynamic_debris_skip_mask, 1);
    assert_eq!(manager.dynamic_particle_skip_mask, 1);
    assert_eq!(manager.get_slow_death_scale(), 0.5);

    // C++: (++n & 1) != 1 → keep on odd generations, skip on even.
    assert!(!manager.is_debris_skipped());
    assert!(manager.is_debris_skipped());
    assert!(!manager.is_debris_skipped());
    assert!(manager.is_debris_skipped());

    assert!(!manager.is_particle_skipped());
    assert!(manager.is_particle_skipped());
    assert!(!manager.is_particle_skipped());
    assert!(manager.is_particle_skipped());
}

#[test]
fn test_set_dynamic_lod_level_applies_masks_and_resets_counters() {
    let mut manager = GameLODManager::new();
    let low = DynamicGameLODLevel::Low.to_index().unwrap();
    let medium = DynamicGameLODLevel::Medium.to_index().unwrap();
    manager.dynamic_game_lod_info[low].dynamic_debris_skip_mask = 1;
    manager.dynamic_game_lod_info[medium].dynamic_debris_skip_mask = 0;

    assert!(manager.set_dynamic_lod_level(DynamicGameLODLevel::Low));
    assert!(!manager.is_debris_skipped());
    assert!(manager.is_debris_skipped());

    // Same level is a no-op (C++ returns FALSE) and does not reset the counter.
    assert!(!manager.set_dynamic_lod_level(DynamicGameLODLevel::Low));
    assert!(!manager.is_debris_skipped());

    // Changing level copies the new mask and resets generation counters.
    assert!(manager.set_dynamic_lod_level(DynamicGameLODLevel::Medium));
    assert_eq!(manager.dynamic_debris_skip_mask, 0);
    for _ in 0..8 {
        assert!(!manager.is_debris_skipped());
    }
    assert!(!manager.set_dynamic_lod_level(DynamicGameLODLevel::Unknown));
}

#[test]
fn dynamic_lod_block_parses_minimum_fps() {
    let mut manager = GameLODManager::new();
    parse_owned(
        &mut manager,
        "DynamicGameLOD VeryHigh\nMinimumFPS=37\nEnd",
        GameLODManager::parse_dynamic_game_lod_definition,
    )
    .expect("synthetic DynamicGameLOD block");
    let very_high = DynamicGameLODLevel::VeryHigh.to_index().unwrap();
    assert_eq!(manager.dynamic_game_lod_info[very_high].min_fps, 37);
}

#[test]
#[ignore = "requires locally supplied retail INIZH/Data/INI/GameLOD.ini"]
fn retail_gamelod_ini_parses() {
    match crate::test_process::run_bounded(module_path!(), "retail_gamelod_ini_parses") {
        crate::test_process::TestProcess::ParentVerified => return,
        crate::test_process::TestProcess::Child => {}
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../windows_game/extracted_big_files_v2/INIZH/Data/INI/GameLOD.ini");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read retail {}: {error}", path.display()));
    init_game_lod_manager();
    let mut ini = INI::new();
    ini.with_inline_source(&src, |ini| ini.parse_current_file())
        .expect("retail GameLOD.ini");
    let manager = get_game_lod_manager();
    let very_high = DynamicGameLODLevel::VeryHigh.to_index().unwrap();
    assert_eq!(manager.dynamic_game_lod_info[very_high].min_fps, 25);
}

#[test]
fn bench_profile_reads_successive_header_tokens() {
    let mut manager = GameLODManager::new();
    parse_owned(
        &mut manager,
        "BenchProfile = P4 2189 6.1 15.1 9.4\n",
        GameLODManager::parse_bench_profile,
    )
    .expect("bench profile header");
    let profile = manager.bench_profiles[0].as_ref().expect("profile stored");
    assert_eq!(profile.mhz, 2189);
    assert!((profile.mem_bench_index - 9.4).abs() < 0.001);
}
