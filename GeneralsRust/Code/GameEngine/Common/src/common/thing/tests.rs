use super::*;
use crate::common::bit_flags::{ArmorSetFlags as ArmorSetBits, WeaponSetFlags as WeaponSetBits};
use crate::common::thing::module::ModuleType;
use std::sync::Arc;

#[test]
fn editor_sorting_matches_cpp_thing_sort_order() {
    assert_eq!(EditorSortingType::None as u8, 0);
    assert_eq!(EditorSortingType::Structure as u8, 1);
    assert_eq!(EditorSortingType::Infantry as u8, 2);
    assert_eq!(EditorSortingType::Vehicle as u8, 3);
    assert_eq!(EditorSortingType::Shrubbery as u8, 4);
    assert_eq!(EditorSortingType::MiscManMade as u8, 5);
    assert_eq!(EditorSortingType::MiscNatural as u8, 6);
    assert_eq!(EditorSortingType::Debris as u8, 7);
    assert_eq!(EditorSortingType::System as u8, 8);
    assert_eq!(EditorSortingType::Audio as u8, 9);
    assert_eq!(EditorSortingType::Test as u8, 10);
    assert_eq!(EditorSortingType::ForReview as u8, 11);
    assert_eq!(EditorSortingType::Road as u8, 12);
    assert_eq!(EditorSortingType::Waypoint as u8, 13);

    assert_eq!(
        parse_editor_sorting("STRUCTURE"),
        EditorSortingType::Structure
    );
    assert_eq!(
        parse_editor_sorting("INFANTRY"),
        EditorSortingType::Infantry
    );
    assert_eq!(
        parse_editor_sorting("MISC_MAN_MADE"),
        EditorSortingType::MiscManMade
    );
    assert_eq!(
        parse_editor_sorting("WAYPOINT"),
        EditorSortingType::Waypoint
    );

    assert_eq!(
        parse_editor_sorting("Building"),
        EditorSortingType::Structure
    );
    assert_eq!(parse_editor_sorting("Unit"), EditorSortingType::Infantry);
    assert_eq!(
        parse_editor_sorting("Civilian"),
        EditorSortingType::MiscNatural
    );
}

#[test]
fn object_field_parse_rejects_unknown_fields_like_cpp() {
    let mut template = ThingTemplate::new();
    let properties = HashMap::from([("NotARealObjectField".to_string(), "1".to_string())]);

    let result = template.parse_object_fields_from_ini(&properties);

    assert!(matches!(result, Err(message) if message.contains("NotARealObjectField")));
}

#[test]
fn occlusion_delay_uses_cpp_duration_frame_conversion() {
    let mut template = ThingTemplate::new();

    // C++ INI::parseDurationUnsignedInt stores a 500 ms duration as 15
    // logic frames, rather than the literal integer 500.
    let properties = HashMap::from([("OcclusionDelay".to_string(), "500ms".to_string())]);
    template
        .parse_object_fields_from_ini(&properties)
        .expect("known C++ field should parse");
    assert_eq!(template.get_occlusion_delay(), 15);

    let properties = HashMap::from([("OcclusionDelay".to_string(), "1s".to_string())]);
    template
        .parse_object_fields_from_ini(&properties)
        .expect("duration suffix should parse");
    assert_eq!(template.get_occlusion_delay(), 30);
}

#[test]
fn shadow_parser_accepts_retail_cpp_shadow_tokens() {
    // These are the tokens in GameClient/Shadow.h's TheShadowNames table
    // and in the extracted Zero Hour object INIs.
    assert_eq!(
        parse_shadow_type("SHADOW_VOLUME").unwrap(),
        ShadowType::Volume
    );
    assert_eq!(
        parse_shadow_type("shadow_decal").unwrap(),
        ShadowType::Decal
    );
    assert_eq!(parse_shadow_type("NONE").unwrap(), ShadowType::None);
    assert_eq!(parse_shadow_type("SHADOW_DECAL").unwrap().bits(), 0x01);
    assert_eq!(parse_shadow_type("SHADOW_VOLUME").unwrap().bits(), 0x02);
    let combo = parse_shadow_type("SHADOW_ALPHA_DECAL SHADOW_DIRECTIONAL_PROJECTION").unwrap();
    assert!(combo.contains(ShadowType::AlphaDecal));
    assert!(combo.contains(ShadowType::DirectionalProjection));
}

#[test]
fn radar_priority_parser_preserves_cpp_radar_categories() {
    let mut template = ThingTemplate::new();
    for (token, expected) in [
        ("NOT_ON_RADAR", RadarPriorityType::NotOnRadar),
        ("STRUCTURE", RadarPriorityType::Structure),
        ("UNIT", RadarPriorityType::Unit),
        ("LOCAL_UNIT_ONLY", RadarPriorityType::LocalUnitOnly),
    ] {
        let properties = HashMap::from([("RadarPriority".to_string(), token.to_string())]);
        template
            .parse_object_fields_from_ini(&properties)
            .expect("retail radar priority should parse");
        assert_eq!(template.get_radar_priority(), expected);
    }
}

#[test]
fn build_parsers_accept_retail_cpp_enum_tokens() {
    assert_eq!(
        parse_build_completion("PLACED_BY_PLAYER").unwrap(),
        BuildCompletionType::PlacedByPlayer
    );
    assert_eq!(
        parse_build_completion("APPEARS_AT_RALLY_POINT").unwrap(),
        BuildCompletionType::AppearsAtRallyPoint
    );
    assert_eq!(
        parse_buildable_status("Ignore_Prerequisites").unwrap(),
        BuildableStatus::IgnorePrerequisites
    );
    assert_eq!(
        parse_buildable_status("Only_By_AI").unwrap(),
        BuildableStatus::OnlyByAi
    );
}

#[test]
fn experience_and_trainable_fields_are_parsed() {
    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        ("IsTrainable".to_string(), "Yes".to_string()),
        ("EnterGuard".to_string(), "Yes".to_string()),
        ("HijackGuard".to_string(), "No".to_string()),
        ("ExperienceValue".to_string(), "50 100 150 200".to_string()),
        (
            "ExperienceRequired".to_string(),
            "0 100 200 300".to_string(),
        ),
        ("SkillPointValue".to_string(), "1 2 3 4".to_string()),
        ("PlacementViewAngle".to_string(), "90".to_string()),
    ]);
    template
        .parse_object_fields_from_ini(&properties)
        .expect("experience fields should parse");
    assert!(template.is_trainable());
    assert!(template.is_enter_guard());
    assert!(!template.is_hijack_guard());
    assert_eq!(template.get_experience_value(0), 50);
    assert_eq!(template.get_experience_required(1), 100);
    assert!((template.get_placement_view_angle() - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn object_field_parse_accepts_cpp_fields_not_yet_wired() {
    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        ("Behavior".to_string(), "AIUpdate ModuleTag_AI".to_string()),
        (
            "UnitSpecificSounds".to_string(),
            "VoiceEnter RangerVoiceEnter".to_string(),
        ),
        (
            "UnitSpecificFX".to_string(),
            "DeathFX FX_RangerDie".to_string(),
        ),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("valid C++ object fields should be accepted");
}

#[test]
fn object_field_parse_populates_module_descriptors_from_module_fields() {
    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        (
            "Behavior".to_string(),
            "SlowDeathBehavior ModuleTag_Die".to_string(),
        ),
        (
            "Behavior#1".to_string(),
            "StealthUpdate ModuleTag_Stealth".to_string(),
        ),
        ("Body".to_string(), "ActiveBody ModuleTag_Body".to_string()),
        (
            "Draw".to_string(),
            "W3DModelDraw ModuleTag_Draw".to_string(),
        ),
        (
            "ClientUpdate".to_string(),
            "LaserUpdate ModuleTag_Client".to_string(),
        ),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("module fields should parse");

    let descriptors = template.module_descriptors();
    assert_eq!(descriptors.behavior.len(), 3);
    assert_eq!(descriptors.draw.len(), 1);
    assert_eq!(descriptors.client_update.len(), 1);

    assert_eq!(descriptors.behavior[0].name.as_str(), "SlowDeathBehavior");
    assert_eq!(descriptors.behavior[0].module_tag.as_str(), "ModuleTag_Die");
    assert!(descriptors.behavior[0].supports(ModuleInterfaceType::DIE));
    assert_eq!(
        descriptors.behavior[1].module_tag.as_str(),
        "ModuleTag_Stealth"
    );
    assert_eq!(descriptors.behavior[2].name.as_str(), "ActiveBody");
    assert!(descriptors.behavior[2].supports(ModuleInterfaceType::BODY));

    assert_eq!(descriptors.draw[0].name.as_str(), "W3DModelDraw");
    assert_eq!(descriptors.draw[0].module_tag.as_str(), "ModuleTag_Draw");
    assert!(descriptors.draw[0].supports(ModuleInterfaceType::DRAW));

    assert_eq!(descriptors.client_update[0].name.as_str(), "LaserUpdate");
    assert_eq!(
        descriptors.client_update[0].module_tag.as_str(),
        "ModuleTag_Client"
    );
    assert!(descriptors.client_update[0].supports(ModuleInterfaceType::CLIENT_UPDATE));
}

#[test]
fn object_field_parse_reads_active_body_and_production_update_bodies() {
    use crate::common::thing::module::BaseModuleData;

    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        ("Body".to_string(), "ActiveBody Tag".to_string()),
        ("Body.MaxHealth".to_string(), "4000".to_string()),
        ("Body.InitialHealth".to_string(), "4000".to_string()),
        (
            "Body.__body".to_string(),
            "MaxHealth = 4000\nInitialHealth = 4000".to_string(),
        ),
        (
            "Behavior".to_string(),
            "ProductionUpdate ModuleTag_04".to_string(),
        ),
        ("Behavior.NumDoorAnimations".to_string(), "1".to_string()),
        (
            "Behavior.__body".to_string(),
            "NumDoorAnimations = 1".to_string(),
        ),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("module bodies should parse");

    let body = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "ActiveBody")
        .expect("ActiveBody");
    assert!(body.data.downcast_ref::<BaseModuleData>().is_none());
    assert_eq!(body.data.get_ini_real("MaxHealth"), Some(4000.0));

    let production = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "ProductionUpdate")
        .expect("ProductionUpdate");
    assert_eq!(production.data.get_ini_int("NumDoorAnimations"), Some(1));
}

#[test]
fn object_field_parse_populates_audio_events_and_max_link_key() {
    NameKeyGenerator::reset();

    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        ("VoiceSelect".to_string(), "RangerVoiceSelect".to_string()),
        ("SoundMoveStart".to_string(), "RangerMoveStart".to_string()),
        (
            "MaxSimultaneousLinkKey".to_string(),
            "SharedLimitKey".to_string(),
        ),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("audio and max simultaneous link fields should parse");

    assert_eq!(
        template
            .get_voice_select()
            .map(AudioEventRts::get_event_name),
        Some("RangerVoiceSelect")
    );
    assert_eq!(
        template
            .get_sound_move_start()
            .map(AudioEventRts::get_event_name),
        Some("RangerMoveStart")
    );
    assert_eq!(
        template.get_max_simultaneous_link_key(),
        NameKeyGenerator::name_to_key("SharedLimitKey")
    );
}

#[test]
fn object_field_parse_populates_unit_specific_sound_and_fx_maps() {
    use crate::common::ini::ini_fx_list::{FXList, get_fx_list_store_mut};

    get_fx_list_store_mut().add_fx_list(FXList::new(
        crate::common::ascii_string::AsciiString::from("FX_RangerDie"),
    ));

    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        (
            "UnitSpecificSounds.TurretMoveStart".to_string(),
            "RangerTurretMoveStart".to_string(),
        ),
        (
            "UnitSpecificSounds.TurretMoveLoop".to_string(),
            "RangerTurretMoveLoop".to_string(),
        ),
        (
            "UnitSpecificFX.DeathFX".to_string(),
            "FX_RangerDie".to_string(),
        ),
        ("UnitSpecificFX.VeteranFX".to_string(), "None".to_string()),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("per-unit sound and FX maps should parse");

    assert_eq!(
        template
            .get_per_unit_sound(&AsciiString::from("TurretMoveStart"))
            .map(AudioEventRts::get_event_name),
        Some("RangerTurretMoveStart")
    );
    assert_eq!(
        template
            .get_per_unit_sound(&AsciiString::from("TurretMoveLoop"))
            .map(AudioEventRts::get_event_name),
        Some("RangerTurretMoveLoop")
    );
    assert!(
        template
            .get_per_unit_fx(&AsciiString::from("DeathFX"))
            .is_some()
    );
    assert!(
        template
            .per_unit_fx
            .get(&AsciiString::from("VeteranFX"))
            .is_some_and(Option::is_none)
    );
}

#[test]
fn object_field_parse_populates_prerequisites_from_collected_subblock() {
    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        (
            "Prerequisites.Object".to_string(),
            "AmericaBarracks AmericaWarFactory".to_string(),
        ),
        (
            "Prerequisites.Object#1".to_string(),
            "AmericaStrategyCenter".to_string(),
        ),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("collected prerequisites should parse");

    assert_eq!(template.get_prereq_count(), 2);

    let first = template.get_prereq(0).expect("first prereq");
    let first_units = first.get_unit_prereqs();
    assert_eq!(first_units.len(), 2);
    assert_eq!(first_units[0].name, "AmericaBarracks");
    assert!(!first_units[0].flags.has_or_with_prev());
    assert_eq!(first_units[1].name, "AmericaWarFactory");
    assert!(first_units[1].flags.has_or_with_prev());

    let second = template.get_prereq(1).expect("second prereq");
    let second_units = second.get_unit_prereqs();
    assert_eq!(second_units.len(), 1);
    assert_eq!(second_units[0].name, "AmericaStrategyCenter");
    assert!(!second_units[0].flags.has_or_with_prev());
}

#[test]
fn object_field_parse_populates_weapon_sets_from_collected_subblocks() {
    use crate::common::system::kind_of::KindOfMask;

    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        ("WeaponSet0.Conditions".to_string(), "HERO".to_string()),
        (
            "WeaponSet0.Weapon".to_string(),
            "PRIMARY HeroPrimary".to_string(),
        ),
        (
            "WeaponSet0.Weapon#1".to_string(),
            "SECONDARY HeroSecondary".to_string(),
        ),
        (
            "WeaponSet0.AutoChooseSources".to_string(),
            "PRIMARY FROM_PLAYER FROM_AI".to_string(),
        ),
        (
            "WeaponSet0.PreferredAgainst".to_string(),
            "SECONDARY AIRCRAFT BALLISTIC_MISSILE".to_string(),
        ),
        (
            "WeaponSet0.ShareWeaponReloadTime".to_string(),
            "Yes".to_string(),
        ),
        (
            "WeaponSet0.WeaponLockSharedAcrossSets".to_string(),
            "No".to_string(),
        ),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("weapon set properties should parse");

    assert_eq!(template.weapon_template_sets().len(), 1);
    let set = &template.weapon_template_sets()[0];
    assert!(set.types().test(WeaponSetBits::HERO));
    assert_eq!(
        set.weapon_template_name(0).map(|name| name.as_str()),
        Some("HeroPrimary")
    );
    assert_eq!(
        set.weapon_template_name(1).map(|name| name.as_str()),
        Some("HeroSecondary")
    );
    assert_eq!(set.auto_choose_mask(0), (1 << 0) | (1 << 2));
    assert_eq!(
        set.preferred_against_mask(1),
        KindOfMask::AIRCRAFT | KindOfMask::BALLISTIC_MISSILE
    );
    assert!(set.is_reload_time_shared());
    assert!(!set.is_weapon_lock_shared_across_sets());
}

#[test]
fn object_field_parse_populates_armor_sets_from_collected_subblocks() {
    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        (
            "ArmorSet0.Conditions".to_string(),
            "PLAYER_UPGRADE".to_string(),
        ),
        (
            "ArmorSet0.Armor".to_string(),
            "StructureArmorTough".to_string(),
        ),
        (
            "ArmorSet0.DamageFX".to_string(),
            "StructureDamageFXNoShake".to_string(),
        ),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("armor set properties should parse");

    assert_eq!(template.armor_template_sets().len(), 1);
    let set = &template.armor_template_sets()[0];
    assert!(set.types().test(ArmorSetBits::PLAYER_UPGRADE));
    assert_eq!(
        set.armor_template_name().map(|name| name.as_str()),
        Some("StructureArmorTough")
    );
    assert_eq!(
        set.damage_fx_name().map(|name| name.as_str()),
        Some("StructureDamageFXNoShake")
    );
}

#[test]
fn object_field_parse_treats_none_weapon_and_armor_conditions_as_empty() {
    let mut template = ThingTemplate::new();
    let properties = HashMap::from([
        ("WeaponSet0.Conditions".to_string(), "None".to_string()),
        ("WeaponSet0.Weapon".to_string(), "PRIMARY None".to_string()),
        (
            "WeaponSet0.AutoChooseSources".to_string(),
            "PRIMARY None".to_string(),
        ),
        ("ArmorSet0.Conditions".to_string(), "None".to_string()),
        ("ArmorSet0.Armor".to_string(), "None".to_string()),
        ("ArmorSet0.DamageFX".to_string(), "None".to_string()),
    ]);

    template
        .parse_object_fields_from_ini(&properties)
        .expect("None values should parse like empty C++ masks/references");

    let weapon_set = &template.weapon_template_sets()[0];
    assert!(!weapon_set.types().any());
    assert!(weapon_set.weapon_template_name(0).is_none());
    assert_eq!(weapon_set.auto_choose_mask(0), 0);

    let armor_set = &template.armor_template_sets()[0];
    assert!(!armor_set.types().any());
    assert!(armor_set.armor_template_name().is_none());
    assert!(armor_set.damage_fx_name().is_none());
}

#[test]
fn find_weapon_template_set_respects_flags() {
    let mut template = ThingTemplate::new();

    let mut base = WeaponTemplateSet::new();
    base.set_weapon_template_name(0, Some(AsciiString::from("BasePrimary")));
    template.add_weapon_template_set(base);

    let mut hero = WeaponTemplateSet::new();
    hero.types_mut().set(WeaponSetBits::HERO, true);
    hero.set_weapon_template_name(0, Some(AsciiString::from("HeroPrimary")));
    template.add_weapon_template_set(hero);

    let flags = create_weapon_set_flags();
    let base_set = template
        .find_weapon_template_set(&flags)
        .expect("expected base weapon set");
    assert_eq!(
        base_set.weapon_template_name(0).map(|name| name.as_str()),
        Some("BasePrimary"),
    );

    let mut hero_flags = create_weapon_set_flags();
    hero_flags.set(WeaponSetBits::HERO, true);
    let hero_set = template
        .find_weapon_template_set(&hero_flags)
        .expect("expected hero weapon set");
    assert_eq!(
        hero_set.weapon_template_name(0).map(|name| name.as_str()),
        Some("HeroPrimary"),
    );
}

#[test]
fn load_weapon_sets_from_definitions_populates_template() {
    use crate::common::system::kind_of::KindOfMask;

    let mut definition = WeaponSetDefinition::new();
    definition.add_condition("Hero");
    definition.set_weapon_name(0, Some(AsciiString::from("HeroPrimary")));
    definition.set_auto_choose_mask(0, Some(0x1));
    definition.set_preferred_against_mask(0, Some(KindOfMask::from_bits_retain(0x2)));
    definition.set_share_reload_time(Some(true));
    definition.set_share_weapon_lock(Some(false));

    let mut template = ThingTemplate::new();
    template
        .load_weapon_sets_from_definitions(&[definition])
        .expect("load weapon sets");

    assert_eq!(template.weapon_template_sets().len(), 1);
    let engine_set = &template.weapon_template_sets()[0];
    assert!(engine_set.types().test(WeaponSetBits::HERO));
    assert_eq!(
        engine_set.weapon_template_name(0).map(|name| name.as_str()),
        Some("HeroPrimary"),
    );
    assert_eq!(engine_set.auto_choose_mask(0), 0x1);
    assert_eq!(
        engine_set.preferred_against_mask(0),
        KindOfMask::from_bits_retain(0x2)
    );
    assert!(engine_set.is_reload_time_shared());
    assert!(!engine_set.is_weapon_lock_shared_across_sets());
}

#[test]
fn module_descriptor_helpers_reflect_module_info() {
    let mut template = ThingTemplate::new();

    let behavior_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
    template.behavior_module_info.add_module_info(
        AsciiString::from("TestBehavior"),
        AsciiString::from("TagBehavior"),
        behavior_data,
        ModuleInterfaceType::BODY.0 as i32,
        false,
        false,
    );

    let draw_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
    template.draw_module_info.add_module_info(
        AsciiString::from("TestDraw"),
        AsciiString::from("TagDraw"),
        draw_data,
        ModuleInterfaceType::DRAW.0 as i32,
        false,
        false,
    );

    let client_update_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
    template.client_update_module_info.add_module_info(
        AsciiString::from("TestClientUpdate"),
        AsciiString::from("TagClient"),
        client_update_data,
        ModuleInterfaceType::CLIENT_UPDATE.0 as i32,
        false,
        false,
    );

    let descriptor_set = template.module_descriptors();

    assert_eq!(descriptor_set.behavior.len(), 1);
    assert_eq!(descriptor_set.draw.len(), 1);
    assert_eq!(descriptor_set.client_update.len(), 1);

    assert_eq!(
        template
            .module_descriptors_for_type(ModuleType::Behavior)
            .len(),
        1
    );
    assert_eq!(
        template
            .module_descriptors_for_type(ModuleType::Behavior)
            .first()
            .map(|d| d.name.as_str()),
        Some("TestBehavior"),
    );
    assert_eq!(
        descriptor_set
            .for_type(ModuleType::Draw)
            .first()
            .map(|d| d.module_tag.as_str()),
        Some("TagDraw"),
    );
    assert_eq!(
        template
            .module_descriptors_for_type(ModuleType::ClientUpdate)
            .first()
            .map(|d| d.name.as_str()),
        Some("TestClientUpdate"),
    );

    let behavior_descriptor = &descriptor_set.behavior[0];
    assert!(behavior_descriptor.supports(ModuleInterfaceType::BODY));
    assert_eq!(behavior_descriptor.name.as_str(), "TestBehavior");

    let draw_descriptor = &descriptor_set.draw[0];
    assert!(draw_descriptor.supports(ModuleInterfaceType::DRAW));
    assert_eq!(draw_descriptor.module_tag.as_str(), "TagDraw");

    let client_descriptor = &descriptor_set.client_update[0];
    assert!(client_descriptor.supports(ModuleInterfaceType::CLIENT_UPDATE));
    assert_eq!(client_descriptor.name.as_str(), "TestClientUpdate");
}

#[test]
fn module_descriptors_register_with_global_factory() {
    clear_pending_descriptors_for_test();
    let mut guard = get_module_factory().expect("module factory mutex poisoned");
    let previous = guard.take();
    *guard = Some(ModuleFactory::new());
    drop(guard);

    let mut template = ThingTemplate::new();
    let behavior_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
    template.behavior_module_info.add_module_info(
        AsciiString::from("AutoHealBehavior"),
        AsciiString::from("TagBehavior"),
        behavior_data,
        ModuleInterfaceType::BODY.0 as i32,
        false,
        false,
    );

    let descriptors = template.module_descriptors();
    assert_eq!(descriptors.behavior.len(), 1, "descriptor not surfaced");

    {
        let guard = get_module_factory().expect("module factory mutex poisoned");
        let factory = guard
            .as_ref()
            .expect("module factory should be initialized for descriptor sync");
        let name = AsciiString::from("AutoHealBehavior");
        assert!(
            factory
                .descriptor_for(ModuleType::Behavior, &name)
                .is_some(),
            "descriptor should be recorded in global factory"
        );
    }

    let mut guard = get_module_factory().expect("module factory mutex poisoned");
    *guard = previous;
    drop(guard);
    clear_pending_descriptors_for_test();
}

#[test]
fn can_possibly_have_any_weapon_reflects_assigned_templates() {
    let mut template = ThingTemplate::new();
    assert!(!template.can_possibly_have_any_weapon());

    template.add_weapon_template_set(WeaponTemplateSet::new());
    assert!(!template.can_possibly_have_any_weapon());

    let mut armed_set = WeaponTemplateSet::new();
    armed_set.set_weapon_template_name(0, Some(AsciiString::from("ArmedPrimary")));
    template.add_weapon_template_set(armed_set);
    assert!(template.can_possibly_have_any_weapon());
}

#[test]
fn is_kind_of_handles_high_bit_masks_without_panicking() {
    use crate::common::system::kind_of::KindOfMask;
    let mut template = ThingTemplate::new();
    template.kindof = KindOfMask::DOZER.bits();

    assert!(template.is_kind_of_mask(KindOfMask::DOZER.bits()));
    assert!(!template.is_kind_of_mask(KindOfMask::COMMANDCENTER.bits()));
}

// C++ KindOf.h:96 / ThingTemplate.h m_kindof BitFlags<KINDOF_COUNT>.
#[test]
fn forceattackable_survives_template_store() {
    use crate::common::system::kind_of::KindOfMask;
    let mut template = ThingTemplate::new();
    template.set_kindof_mask(KindOfMask::FORCEATTACKABLE.bits());
    assert!(template.is_kind_of_mask(KindOfMask::FORCEATTACKABLE.bits()));
    assert_eq!(
        template.get_kindof_bits() & KindOfMask::FORCEATTACKABLE.bits(),
        KindOfMask::FORCEATTACKABLE.bits()
    );

    // Bits >= 64 used to vanish when kindof was stored as u64.
    template.set_kindof_mask(KindOfMask::HERO.bits() | KindOfMask::FORCEATTACKABLE.bits());
    assert!(template.is_kind_of_mask(KindOfMask::HERO.bits()));
    assert!(template.is_kind_of_mask(KindOfMask::FORCEATTACKABLE.bits()));
}

// C++ BitFlagsIO.h:38-107 via ThingTemplate KindOf INI field.
#[test]
fn kindof_ini_plus_hero_is_incremental() {
    use crate::common::system::kind_of::KindOfMask;
    let mut template = ThingTemplate::new();
    template
        .parse_object_fields_from_ini(&HashMap::from([(
            "KindOf".to_string(),
            "INFANTRY SELECTABLE".to_string(),
        )]))
        .expect("base KindOf should parse");
    template
        .parse_object_fields_from_ini(&HashMap::from([(
            "KindOf".to_string(),
            "+HERO".to_string(),
        )]))
        .expect("+HERO should parse incrementally");
    assert!(template.is_kind_of_mask(KindOfMask::INFANTRY.bits()));
    assert!(template.is_kind_of_mask(KindOfMask::SELECTABLE.bits()));
    assert!(template.is_kind_of_mask(KindOfMask::HERO.bits()));
}

// C++ ThingTemplate.h:374-377 isKindOf(KindOfType t) / KindOf.h:158-161 TEST_KINDOFMASK.
// ALWAYS_SELECTABLE is bit 53 (`1<<53`). Passing 53 as a mask must not match that bit.
#[test]
fn is_kind_of_treats_always_selectable_index_not_mask() {
    use crate::common::system::kind_of::KindOfMask;
    let mut template = ThingTemplate::new();
    template.set_kindof_mask(KindOfMask::ALWAYS_SELECTABLE.bits());

    // C++ `isKindOf(KINDOF_ALWAYS_SELECTABLE)` tests bit 53, not AND with 53.
    assert!(template.is_kind_of(53u32));
    assert!(!template.is_kind_of_mask(53u32));
    assert!(template.is_kind_of_mask(KindOfMask::ALWAYS_SELECTABLE.bits()));

    // 53 as a mask would only hit bits 0/2/4/5; those flags must not be implied.
    assert!(!template.is_kind_of(0u32));
    assert!(!template.is_kind_of(2u32));
    assert!(!template.is_kind_of(4u32));
    assert!(!template.is_kind_of(5u32));
}

#[test]
fn kindof_ini_unknown_name_errors() {
    let mut template = ThingTemplate::new();
    let err = template
        .parse_object_fields_from_ini(&HashMap::from([(
            "KindOf".to_string(),
            "NOT_A_REAL_KIND".to_string(),
        )]))
        .expect_err("unknown KindOf token must error");
    assert!(err.contains("NOT_A_REAL_KIND"), "{err}");
}

// C++ ThingTemplate.cpp:384-409 — real KindOf bits, not fabricated u64 wraps.
#[test]
fn gps_scrambler_masks_use_retail_kindof_bits() {
    use crate::common::system::kind_of::KindOfMask;

    let data: Arc<dyn ModuleData> = Arc::new(CapturedModuleData::new(
        "ModuleTag_Gps",
        String::new(),
        HashMap::new(),
    ));

    let mut immune_template = ThingTemplate::new();
    immune_template.set_kindof_mask(KindOfMask::OPTIMIZED_TREE.bits());
    let mut immune_info = ModuleInfo::new();
    immune_info.add_module_info(
        AsciiString::from("StealthUpdate"),
        AsciiString::from("ModuleTag_Gps"),
        Arc::clone(&data),
        1,
        false,
        true,
    );
    immune_info.set_copied_from_default(true);
    let (immune_trainable, immune_disallowed, immune_candidate) =
        immune_template.gps_scrambler_inherit_flags();
    assert!(immune_info.clear_copied_from_default_entries(
        1,
        &AsciiString::from("OtherModule"),
        immune_trainable,
        immune_disallowed,
        immune_candidate,
    ));
    assert_eq!(immune_info.get_count(), 0);

    let mut candidate_template = ThingTemplate::new();
    candidate_template.set_kindof_mask(KindOfMask::VEHICLE.bits() | KindOfMask::SCORE.bits());
    let mut candidate_info = ModuleInfo::new();
    candidate_info.add_module_info(
        AsciiString::from("StealthUpdate"),
        AsciiString::from("ModuleTag_Gps"),
        data,
        1,
        false,
        true,
    );
    candidate_info.set_copied_from_default(true);
    let (candidate_trainable, candidate_disallowed, candidate_candidate) =
        candidate_template.gps_scrambler_inherit_flags();
    assert!(!candidate_info.clear_copied_from_default_entries(
        1,
        &AsciiString::from("OtherModule"),
        candidate_trainable,
        candidate_disallowed,
        candidate_candidate,
    ));
    assert_eq!(candidate_info.get_count(), 1);
}

#[test]
fn find_armor_template_set_respects_flags() {
    let mut template = ThingTemplate::new();

    let mut base = ArmorTemplateSet::new();
    base.set_armor_template_name(Some(AsciiString::from("Base")));
    template.add_armor_template_set(base);

    let mut hero = ArmorTemplateSet::new();
    hero.types_mut().set(ArmorSetBits::HERO, true);
    hero.set_armor_template_name(Some(AsciiString::from("Hero")));
    template.add_armor_template_set(hero);

    let flags = create_armor_set_flags();
    let base_set = template
        .find_armor_template_set(&flags)
        .expect("expected base set");
    assert_eq!(base_set.armor_template_name().unwrap().as_str(), "Base");

    let mut hero_flags = create_armor_set_flags();
    hero_flags.set(ArmorSetBits::HERO, true);
    let hero_set = template
        .find_armor_template_set(&hero_flags)
        .expect("expected hero set");
    assert_eq!(hero_set.armor_template_name().unwrap().as_str(), "Hero");
}
