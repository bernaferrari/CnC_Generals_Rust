use super::*;
#[test]
fn strict_railroad_behavior_reads_physics_and_leaf_fields_in_one_owned_block() {
    use crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModuleData;
    use game_engine::common::thing::module::ModuleData as EngineModuleData;

    // C++ RailroadGuideAIUpdate.h:28-56 appends the Physics table before
    // the leaf table. Physics friction is per second; Railroad Friction,
    // SpeedMax, and Acceleration are unconverted real values.
    let factory = catalog();
    const BODY: &str = "Mass = 3.5\nPathPrefixName = MainLine\nForwardFriction = 6\n\
        IsLocomotive = Yes\nLateralFriction = 3\nSpeedMax = 5\nZFriction = 12\n\
        Acceleration = 1.03\nAerodynamicFriction = 9\nFriction = 0.995\n\
        ShockResistance = 0.25\nShockMaxYaw = 0.1\nShockMaxPitch = 0.2\n\
        ShockMaxRoll = 0.3\nCenterOfMassOffset = -2\nAllowBouncing = Yes\n\
        AllowCollideForce = No\nKillWhenRestingOnGround = Yes\n\
        FallHeightDamageFactor = 1.25\nPitchRollYawFactor = 3\n\
        VehicleCrashesIntoBuildingWeaponTemplate = TrainBuildingHit\n\
        VehicleCrashesIntoNonBuildingWeaponTemplate = TrainUnitHit\n\
        WaitAtStationTime = 10000\nMass = 5\nEnd\nFollowingBlock\n";
    let parsed = INI::new()
        .with_inline_source(BODY, |ini| {
            let parsed = factory
                .construct(Some(ini), "RailroadBehavior", ModuleType::Behavior)
                .expect("actual registered Railroad parser accepts both tables");
            ini.read_line()?;
            assert_eq!(ini.get_line_tokens(), ["FollowingBlock"]);
            Ok(parsed)
        })
        .expect("owned Railroad body");
    let data = parsed
        .as_any()
        .downcast_ref::<RailroadBehaviorModuleData>()
        .expect("actual typed Railroad data, not a generic fallback");
    assert_eq!(data.base.mass, 5.0, "later inherited field wins");
    assert_eq!(data.path_prefix_name.as_str(), "MainLine");
    for (actual, expected) in [
        (data.base.forward_friction, 0.2),
        (data.base.lateral_friction, 0.1),
        (data.base.z_friction, 0.4),
        (data.base.aerodynamic_friction, 0.3),
    ] {
        assert!((actual - expected).abs() < f32::EPSILON);
    }
    assert_eq!(data.base.shock_resistance, 0.25);
    assert_eq!(data.base.shock_max_yaw, 0.1);
    assert_eq!(data.base.shock_max_pitch, 0.2);
    assert_eq!(data.base.shock_max_roll, 0.3);
    assert_eq!(data.base.center_of_mass_offset, -2.0);
    assert!(data.base.allow_bouncing);
    assert!(!data.base.allow_collide_force);
    assert!(data.base.kill_when_resting_on_ground);
    assert_eq!(data.base.fall_height_damage_factor, 1.25);
    assert_eq!(data.base.pitch_roll_yaw_factor, 3.0);
    assert_eq!(
        data.base
            .vehicle_crashes_into_building_weapon_template
            .as_str(),
        "TrainBuildingHit"
    );
    assert_eq!(
        data.base
            .vehicle_crashes_into_non_building_weapon_template
            .as_str(),
        "TrainUnitHit"
    );
    assert!(data.is_locomotive);
    assert_eq!(data.speed_max, 5.0);
    assert_eq!(data.acceleration, 1.03);
    assert_eq!(data.friction, 0.995);
    assert_eq!(data.wait_at_station_time, 300);

    // ModuleData's key remains one canonical value through Railroad's
    // Physics -> Behavior -> Base metadata chain; no tag is interned here.
    assert_eq!(EngineModuleData::get_module_tag_name_key(data), 0);
    let mut tagged = RailroadBehaviorModuleData::clone(data);
    EngineModuleData::set_module_tag_name_key(&mut tagged, 73);
    assert_eq!(EngineModuleData::get_module_tag_name_key(&tagged), 73);
    assert_eq!(EngineModuleData::get_module_tag_name_key(&tagged.base), 73);
    assert_eq!(
        EngineModuleData::get_module_tag_name_key(&tagged.base.base),
        73
    );
    assert_eq!(
        EngineModuleData::get_module_tag_name_key(&tagged.base.base.base),
        73
    );

    let mut factory = catalog();
    let defaults = parse(
        &mut factory,
        ModuleType::Behavior,
        "RailroadBehavior",
        "End\n",
    )
    .expect("unchanged C++ default constructor");
    let defaults = defaults
        .as_any()
        .downcast_ref::<RailroadBehaviorModuleData>()
        .unwrap();
    assert!(!defaults.is_locomotive);
    assert_eq!(defaults.running_garrison_speed_max, 1.0);
    assert_eq!(defaults.kill_speed_min, 1.0);
    assert_eq!(defaults.speed_max, 4.0);
    assert_eq!(defaults.acceleration, 1.01);
    assert_eq!(defaults.braking, 0.99);
    assert_eq!(defaults.friction, 0.97);
    assert_eq!(defaults.wait_at_station_time, 150);
    assert_eq!(defaults.base.mass, 1.0);
    assert_eq!(defaults.base.forward_friction, 0.15);
    assert_eq!(defaults.base.lateral_friction, 0.15);
    assert_eq!(defaults.base.z_friction, 0.8);
    assert_eq!(defaults.base.aerodynamic_friction, 0.0);
    assert!(!defaults.base.allow_bouncing);
    assert!(defaults.base.allow_collide_force);
    assert_eq!(defaults.base.pitch_roll_yaw_factor, 2.0);
    for (body, rejected) in [
        ("PathPrefixName = Railroad\nMass = 0\nEnd\n", "Mass = 0"),
        (
            "Mass = 2\nIsLocomotive = invalid\nEnd\n",
            "IsLocomotive = invalid",
        ),
        (
            "Mass = 2\nUnknownRailroadField = 73\nEnd\n",
            "UnknownRailroadField = 73",
        ),
    ] {
        let error = parse(&mut factory, ModuleType::Behavior, "RailroadBehavior", body)
            .expect_err("inherited parsing must remain strict");
        assert!(error.contains("RailroadBehavior"), "{error}");
        assert!(error.contains(rejected), "{error}");
        assert!(error.contains("line"), "{error}");
    }
    assert!(
        parse(
            &mut factory,
            ModuleType::Behavior,
            "RailroadBehavior",
            "Mass = 2\nPathPrefixName = Railroad\n"
        )
        .is_err(),
        "both tables require the body's End"
    );
}

#[test]
fn strict_railroad_behavior_admits_the_real_train_engine_declaration_and_rejects_unknowns() {
    use crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModuleData;

    // Licensed INIZH.big Data/INI/Object/CivilianUnit.ini:13299, copied
    // as one complete genuine RailroadBehavior body, including repeated cars.
    const BODY: &str = r#"
    PathPrefixName    = Railroad

    ;=== PARAMETERS FOR ALL CARRIAGES ===    
    RunningGarrisonSpeedMax = 1
    KillSpeedMin            = 1
    Friction                = 0.995
    BigMetalBounceSound     = BuildingFallingMetal
    SmallMetalBounceSound   = VehicleImpactHeavy
    MeatyBounceSound        = InfantryCrush
    ClicketyClackSound      = TrainClicketyClack
    WhistleSound            = TrainWhistle

    ;=== PARAMETERS THAT ARE MEANINGFUL ONLY WHEN IsLocomotive = Yes
    IsLocomotive         = Yes
    SpeedMax             = 5
    Acceleration         = 1.03
    WaitAtStationTime    = 10000     ; ten seconds
    Braking              = 0.97
    RunningSound         = TrainRunningLoop
    CrashFXTemplateName  = SpecialEffectsTrainCrashObject

    CarriageTemplateName = TrainCar03
    CarriageTemplateName = TrainCar03
    CarriageTemplateName = TrainCoal    
    CarriageTemplateName = TrainCab

    ;CarriageTemplateName = TrainCar01
    ;CarriageTemplateName = TrainCar02
    ;CarriageTemplateName = TrainCar03
    ;CarriageTemplateName = TrainCoal    
    ;CarriageTemplateName = TrainTank
    ;CarriageTemplateName = CabooseFullOfTerrorists
  End
"#;
    let mut factory = catalog();
    let parsed = parse(&mut factory, ModuleType::Behavior, "RailroadBehavior", BODY)
        .expect("genuine full retail Railroad declaration");
    let data = parsed
        .as_any()
        .downcast_ref::<RailroadBehaviorModuleData>()
        .expect("actual typed Railroad data, not a generic fallback");
    assert_eq!(data.path_prefix_name.as_str(), "Railroad");
    assert_eq!(data.running_garrison_speed_max, 1.0);
    assert_eq!(data.kill_speed_min, 1.0);
    assert_eq!(data.friction, 0.995);
    assert_eq!(
        data.big_metal_impact_default_sound.get_event_name(),
        "BuildingFallingMetal"
    );
    assert_eq!(
        data.small_metal_impact_default_sound.get_event_name(),
        "VehicleImpactHeavy"
    );
    assert_eq!(
        data.meaty_impact_default_sound.get_event_name(),
        "InfantryCrush"
    );
    assert_eq!(
        data.clickety_clack_sound.get_event_name(),
        "TrainClicketyClack"
    );
    assert_eq!(data.whistle_sound.get_event_name(), "TrainWhistle");
    assert!(data.is_locomotive);
    assert_eq!(data.speed_max, 5.0);
    assert_eq!(data.acceleration, 1.03);
    assert_eq!(data.wait_at_station_time, 300);
    assert_eq!(data.braking, 0.97);
    assert_eq!(data.running_sound.get_event_name(), "TrainRunningLoop");
    assert_eq!(
        data.crash_fx_template_name.as_str(),
        "SpecialEffectsTrainCrashObject"
    );
    assert_eq!(
        data.carriage_template_names
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["TrainCar03", "TrainCar03", "TrainCoal", "TrainCab"]
    );
    assert_eq!(data.base.mass, 1.0, "unwritten inherited defaults survive");
    assert_eq!(data.base.forward_friction, 0.15);
    let rejected = BODY.replacen("  End", "    UnknownRailroadField = 73\n  End", 1);
    let error = parse(
        &mut factory,
        ModuleType::Behavior,
        "RailroadBehavior",
        &rejected,
    )
    .expect_err("retail acceptance must remain strict");
    assert!(error.contains("RailroadBehavior"), "{error}");
    assert!(error.contains("UnknownRailroadField = 73"), "{error}");
    assert!(error.contains("line"), "{error}");
}
