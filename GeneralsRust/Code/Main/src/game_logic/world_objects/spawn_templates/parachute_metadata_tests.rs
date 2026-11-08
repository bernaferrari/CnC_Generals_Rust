use super::*;
#[test]
fn actual_parachute_and_hijacker_definitions_keep_module_identity_and_values() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object ArbitraryRider
 Type = Infantry
 Behavior = HijackerUpdate ModuleTag_Hijacker
  ParachuteName = ArbitraryAuthoredChute
 End
End
Object ArbitraryAuthoredChute
 Type = Vehicle
 Behavior = ParachuteContain ModuleTag_Contain
  AllowInsideKindOf = INFANTRY VEHICLE
  PitchRateMax = 60
  RollRateMax = 30
  ParachuteOpenDist = 45
  FreeFallDamagePercent = 25%
  ParachuteOpenSound = CustomOpen
 End
End
"#,
            "parachute-owner.ini",
        )
        .unwrap();
    let rider = GameLogic::build_template_from_object_definition(
        "ArbitraryRider",
        parser.get_definition("ArbitraryRider").unwrap(),
        None,
    );
    assert_eq!(
        rider.hijacker_parachute_name.as_deref(),
        Some("ArbitraryAuthoredChute")
    );
    let chute = GameLogic::build_template_from_object_definition(
        "ArbitraryAuthoredChute",
        parser.get_definition("ArbitraryAuthoredChute").unwrap(),
        None,
    );
    assert_eq!(
        chute.contain_module.kind,
        crate::game_logic::ContainModuleKind::Parachute
    );
    assert_eq!(chute.contain_module.slots, None);
    let data = chute.contain_module.parachute.as_ref().unwrap();
    assert!((data.pitch_rate_max - 60f32.to_radians() / 30.).abs() < 1e-6);
    assert_eq!(data.open_distance, 45.);
    assert_eq!(data.free_fall_damage_percent, 0.25);
    assert_eq!(data.open_sound, "CustomOpen");
    assert_eq!(data.low_altitude_damping, 0.2);
    let plain = ThingTemplate::new("AmericaParachute");
    assert!(plain.contain_module.parachute.is_none());
    assert!(plain.hijacker_parachute_name.is_none());
}

#[test]
fn parachute_zero_rate_does_not_consume_a_logic_draw() {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "parachute-rng.ini".into(),
            "Object ChuteRng\n\
             KindOf = VEHICLE\n\
             Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
             Behavior = ParachuteContain ChuteTag\nPitchRateMax = 0\nRollRateMax = 30\nEnd\n\
             End\n".into(),
        )]).unwrap(),
    ).unwrap();
    world.set_logic_random_seed(0x1234);
    let mut expected = world.logic_random.snapshot();
    // ParachuteContain.cpp:100-101 and RandomValue.cpp:263-269: the
    // zero-width pitch interval consumes nothing; roll consumes one draw.
    let roll = expected.next_real(-std::f32::consts::PI / 180.0, std::f32::consts::PI / 180.0);
    let id = world.create_object("ChuteRng", crate::game_logic::Team::GLA, glam::Vec3::ZERO).unwrap();
    let object = world.host_object(id).unwrap();
    assert_eq!(object.status.parachute_pitch_rate, 0.0);
    assert!((object.status.parachute_roll_rate - roll).abs() < 1e-6);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
}
