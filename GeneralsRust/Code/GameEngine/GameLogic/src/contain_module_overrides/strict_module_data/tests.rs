use super::*;

// C++ Module.h:96-99,138-144: empty tables still parse and malformed module
// fields propagate out of friend_newModuleData. No module instance is made.
const MODULES: &[(ModuleType, &str)] = &[
    (ModuleType::Draw, "W3DModelDraw"),
    (ModuleType::Draw, "W3DDefaultDraw"),
    (ModuleType::Draw, "W3DDependencyModelDraw"),
    (ModuleType::Draw, "W3DTankDraw"),
    (ModuleType::Draw, "W3DOverlordTankDraw"),
    (ModuleType::Draw, "W3DOverlordAircraftDraw"),
    (ModuleType::Draw, "W3DOverlordTruckDraw"),
    (ModuleType::Draw, "W3DPoliceCarDraw"),
    (ModuleType::Draw, "W3DProjectileStreamDraw"),
    (ModuleType::Draw, "W3DRopeDraw"),
    (ModuleType::Draw, "W3DScienceModelDraw"),
    (ModuleType::Draw, "W3DSupplyDraw"),
    (ModuleType::Draw, "W3DTankTruckDraw"),
    (ModuleType::Draw, "W3DTracerDraw"),
    (ModuleType::Draw, "W3DTreeDraw"),
    (ModuleType::Draw, "W3DPropDraw"),
    (ModuleType::Draw, "W3DTruckDraw"),
    (ModuleType::Draw, "W3DLaserDraw"),
    (ModuleType::Draw, "W3DDebrisDraw"),
    (ModuleType::ClientUpdate, "LaserUpdate"),
    (ModuleType::ClientUpdate, "BeaconClientUpdate"),
    (ModuleType::ClientUpdate, "SwayClientUpdate"),
    (
        ModuleType::ClientUpdate,
        "AnimatedParticleSysBoneClientUpdate",
    ),
    (ModuleType::Behavior, "InactiveBody"),
    (ModuleType::Behavior, "ActiveBody"),
    (ModuleType::Behavior, "StructureBody"),
    (ModuleType::Behavior, "HighlanderBody"),
    (ModuleType::Behavior, "ImmortalBody"),
    (ModuleType::Behavior, "HiveStructureBody"),
    (ModuleType::Behavior, "UndeadBody"),
];

struct DataParsers {
    callbacks: std::collections::HashMap<(usize, &'static str), FallibleModuleDataProc>,
}

impl DataParserRegistrar for DataParsers {
    fn register_data_parser(
        &mut self,
        name: &'static str,
        kind: ModuleType,
        parser: FallibleModuleDataProc,
    ) {
        assert!(
            self.callbacks
                .insert((kind as usize, name), parser)
                .is_none(),
            "duplicate parser {name}"
        );
    }
}

impl DataParsers {
    fn construct(
        &self,
        ini: Option<&mut INI>,
        name: &str,
        kind: ModuleType,
    ) -> Result<Arc<dyn ModuleData>, String> {
        let parser = self
            .callbacks
            .iter()
            .find_map(|(&(registered_kind, registered_name), parser)| {
                (registered_kind == kind as usize && registered_name == name).then_some(*parser)
            })
            .ok_or_else(|| format!("No actual typed parser for {name}"))?;
        parser(ini).map(Arc::from)
    }
}

fn catalog() -> DataParsers {
    let mut parsers = DataParsers {
        callbacks: std::collections::HashMap::new(),
    };
    register_native_data_parsers(&mut parsers);
    parsers
}

fn parse(
    factory: &mut DataParsers,
    kind: ModuleType,
    name: &str,
    body: &str,
) -> Result<Arc<dyn ModuleData>, String> {
    INI::new()
        .with_inline_source(body, |ini| Ok(factory.construct(Some(ini), name, kind)))
        .expect("owned inline source")
}

#[test]
fn every_native_draw_client_update_and_body_has_an_error_preserving_parser() {
    let mut factory = catalog();
    assert_eq!(MODULES.len(), 30);
    for &(kind, name) in MODULES {
        let data = parse(&mut factory, kind, name, "End\n")
            .unwrap_or_else(|error| panic!("{name} empty/default data: {error}"));
        assert_eq!(
            data.get_module_tag_name_key(),
            game_engine::common::name_key_generator::NAMEKEY_INVALID,
            "{name} validation data remains unkeyed until runtime admission"
        );
    }
}

#[test]
fn every_native_data_parser_propagates_unknown_fields_including_empty_tables() {
    let mut factory = catalog();
    for &(kind, name) in MODULES {
        let error = parse(
            &mut factory,
            kind,
            name,
            "NotAnAuthoredModuleField = 4\nEnd\n",
        )
        .expect_err(name);
        assert!(error.contains(name), "class diagnostic: {error}");
        assert!(error.contains("line"), "source diagnostic: {error}");
        assert!(
            error.contains("NotAnAuthoredModuleField = 4"),
            "actual rejected field and value: {error}"
        );
    }
}

#[test]
fn every_native_data_parser_rejects_an_unterminated_body() {
    let mut factory = catalog();
    for &(kind, name) in MODULES {
        assert!(parse(&mut factory, kind, name, "\n").is_err(), "{name}");
    }
}

#[test]
fn strict_data_keeps_real_typed_values_and_independent_receiving_catalogs() {
    let mut a = catalog();
    let mut b = catalog();
    let draw = parse(
        &mut a,
        ModuleType::Draw,
        "W3DLaserDraw",
        "NumBeams = 7\nEnd\n",
    )
    .expect("typed laser draw");
    assert_eq!(
        draw.as_any()
            .downcast_ref::<W3DLaserDrawModuleData>()
            .unwrap()
            .num_beams,
        7
    );
    let peer = parse(
        &mut b,
        ModuleType::Draw,
        "W3DLaserDraw",
        "NumBeams = 2\nEnd\n",
    )
    .expect("peer typed laser draw");
    assert_eq!(
        peer.as_any()
            .downcast_ref::<W3DLaserDrawModuleData>()
            .unwrap()
            .num_beams,
        2
    );
    assert_eq!(
        draw.as_any()
            .downcast_ref::<W3DLaserDrawModuleData>()
            .unwrap()
            .num_beams,
        7
    );

    let body = parse(
        &mut a,
        ModuleType::Behavior,
        "ActiveBody",
        "MaxHealth = 123.5\nEnd\n",
    )
    .expect("typed body");
    assert_eq!(
        body.as_any()
            .downcast_ref::<ActiveBodyModuleData>()
            .unwrap()
            .max_health,
        123.5
    );
    let client = parse(
        &mut a,
        ModuleType::ClientUpdate,
        "BeaconClientUpdate",
        "RadarPulseFrequency = 2000ms\nEnd\n",
    )
    .expect("typed client update");
    assert_eq!(
        client
            .as_any()
            .downcast_ref::<BeaconClientUpdateModuleData>()
            .unwrap()
            .frames_between_radar_pulses,
        60
    );
}

#[test]
fn strict_numeric_errors_do_not_change_compatibility_constructor_behavior() {
    let mut factory = catalog();
    let mut compatibility_factory = ModuleFactory::new();
    super::super::register_native_module_overrides(&mut compatibility_factory).unwrap();
    for (kind, name, body) in [
        (
            ModuleType::Draw,
            "W3DLaserDraw",
            "NumBeams = invalid\nEnd\n",
        ),
        (
            ModuleType::Behavior,
            "ActiveBody",
            "MaxHealth = invalid\nEnd\n",
        ),
        (
            ModuleType::ClientUpdate,
            "BeaconClientUpdate",
            "RadarPulseFrequency = invalid\nEnd\n",
        ),
    ] {
        assert!(parse(&mut factory, kind, name, body).is_err(), "{name}");
        INI::new()
            .with_inline_source(body, |ini| {
                assert!(
                    compatibility_factory
                        .new_module_data_from_ini(Some(ini), name, kind, "Compatibility")
                        .is_ok(),
                    "{name}"
                );
                Ok(())
            })
            .expect("owned compatibility source");
    }
}

fn behavior_classes() -> impl Iterator<Item = &'static str> {
    [
        death::NAMES,
        projectile_updates::NAMES,
        updates::NAMES,
        production::NAMES,
        ai_upgrades_powers::NAMES,
        behaviors::NAMES,
        collide::NAMES,
        creation::NAMES,
        contain::NAMES,
    ]
    .into_iter()
    .flatten()
    .copied()
}

#[test]
fn every_registered_behavior_class_has_real_typed_fallible_data() {
    let mut factory = catalog();
    let mut names = std::collections::HashSet::new();
    for name in behavior_classes() {
        assert!(names.insert(name), "duplicate strict registration {name}");
        let data = factory
            .construct(None, name, ModuleType::Behavior)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            !data
                .as_any()
                .is::<game_engine::common::thing::module::CapturedModuleData>(),
            "{name} must retain actual typed data"
        );
    }
    assert_eq!(names.len(), 187);
    for name in [
        "DestroyDie",
        "PhysicsBehavior",
        "AIUpdateInterface",
        "OpenContain",
        "ProductionUpdate",
        "GrantUpgradeCreate",
    ] {
        assert!(names.contains(name), "critical native family {name}");
    }
}

#[test]
fn native_behavior_data_propagates_real_parser_errors() {
    let mut factory = catalog();
    for (name, body) in [
        ("DestroyDie", "NotADieField = 1\nEnd\n"),
        ("PhysicsBehavior", "Mass = invalid\nEnd\n"),
        (
            "AIUpdateInterface",
            "AutoAcquireEnemiesWhenIdle = invalid\nEnd\n",
        ),
        ("OpenContain", "ContainMax = invalid\nEnd\n"),
        ("ProductionUpdate", "NumDoorAnimations = invalid\nEnd\n"),
        ("GrantUpgradeCreate", "NotACreateField = 1\nEnd\n"),
    ] {
        let error = parse(&mut factory, ModuleType::Behavior, name, body).expect_err(name);
        assert!(error.contains(name), "{name}: {error}");
        assert!(error.contains("line"), "{name}: {error}");
    }
}

#[test]
fn native_behavior_data_keeps_physics_ai_contain_and_production_values() {
    let mut factory = catalog();
    let physics = parse(
        &mut factory,
        ModuleType::Behavior,
        "PhysicsBehavior",
        "Mass = 12.5\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        physics
            .as_any()
            .downcast_ref::<PhysicsBehaviorModuleData>()
            .unwrap()
            .mass,
        12.5
    );
    let ai = parse(
        &mut factory,
        ModuleType::Behavior,
        "AIUpdateInterface",
        "AutoAcquireEnemiesWhenIdle = YES\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        ai.as_any()
            .downcast_ref::<AIUpdateModuleData>()
            .unwrap()
            .auto_acquire_enemies_when_idle(),
        crate::object::update::ai_update_interface::AUTO_ACQUIRE_IDLE
    );
    let contain = parse(
        &mut factory,
        ModuleType::Behavior,
        "OpenContain",
        "ContainMax = 7\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        contain
            .as_any()
            .downcast_ref::<super::super::contain::ContainModuleDataAdapter<OpenContainModuleData>>(
            )
            .unwrap()
            .contain_data()
            .contain_max,
        7
    );
    let production = parse(
        &mut factory,
        ModuleType::Behavior,
        "ProductionUpdate",
        "NumDoorAnimations = 3\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        production
            .as_any()
            .downcast_ref::<ProductionUpdateModuleData>()
            .unwrap()
            .num_door_animations,
        3
    );
}

#[path = "inherited_fields_tests.rs"]
mod inherited_fields;

#[test]
fn data_only_default_callbacks_do_not_allocate_ambient_module_tag_keys() {
    // Only default/empty data is covered here. Authored model bone/name keys
    // still require the separate explicit identity migration (hq-bl6s7).
    let before = NameKeyGenerator::name_to_key("StrictNativeDataTagSentinelBefore");
    let mut parsers = catalog();
    for &(kind, name) in MODULES {
        let data = parse(&mut parsers, kind, name, "End\n").unwrap();
        assert_eq!(data.get_module_tag_name_key(), 0, "{name}");
    }
    for name in behavior_classes() {
        let data = parsers.construct(None, name, ModuleType::Behavior).unwrap();
        assert_eq!(data.get_module_tag_name_key(), 0, "{name}");
    }
    let after = NameKeyGenerator::name_to_key("StrictNativeDataTagSentinelAfter");
    assert_eq!(after, before + 1, "default data allocated an ambient key");
}

#[test]
fn native_catalog_records_actual_class_masks_before_authored_validation() {
    use game_engine::common::thing::module::ModuleInterfaceType;
    let mut factory = ModuleFactory::new();
    super::super::register_native_module_overrides(&mut factory).unwrap();
    for &(kind, name) in MODULES {
        let expected = match kind {
            ModuleType::Draw => ModuleInterfaceType::DRAW,
            ModuleType::ClientUpdate => ModuleInterfaceType::CLIENT_UPDATE,
            ModuleType::Behavior => ModuleInterfaceType::BODY,
            _ => unreachable!(),
        };
        assert_eq!(
            factory.find_module_interface_mask(name, kind),
            expected,
            "{name}"
        );
    }
    for name in behavior_classes() {
        assert_ne!(
            factory.find_module_interface_mask(name, ModuleType::Behavior),
            ModuleInterfaceType::NONE,
            "actual registered class {name} cannot acquire its mask from an authored field"
        );
    }
    for kind in [ModuleType::Draw, ModuleType::ClientUpdate] {
        let actual: Vec<_> = factory
            .descriptors_in_registration_order(kind)
            .iter()
            .map(|descriptor| descriptor.name.to_string())
            .collect();
        let expected: Vec<_> = MODULES
            .iter()
            .filter_map(|&(module_kind, name)| (module_kind == kind).then_some(name.to_owned()))
            .collect();
        assert_eq!(actual, expected, "actual class registration order");
    }
}

#[test]
fn strict_jet_slow_death_admits_the_real_airforce_spectre_declaration_and_rejects_unknowns() {
    // Licensed INIZH.big Data/INI/Object/AirforceGeneral.ini:521, the first
    // Jet declaration in actual ordered receiving admission. C++ Jet's base
    // parser admits DestructionDelay before these leaf fields.
    const BODY: &str = r#"    DestructionDelay                = 99999999  ; destruction will happen when we
    RollRate                        = 0.0
    RollRateDelta                   = 100%      ;each frame, rollrate = rollrate * rollrateDelta
    PitchRate                       = 0.0
    FallHowFast                     = 110.0%    ;Bigger is faster (can be over 100%,it's a fraction of gravity)
    FXInitialDeath                  = FX_SpectreGunshipExplosionLight
    OCLInitialDeath                 = None
    DelaySecondaryFromInitialDeath  = 500       ; in milliseconds
    FXSecondary                     = FX_spectreDeathExplosion
    OCLSecondary                    = None
    FXHitGround                     = FX_spectreGunshipDeathExplosion
    OCLHitGround                    = OCL_A10DeathHitGround
    DelayFinalBlowUpFromHitGround   = 200       ; in milliseconds
  ; FXFinalBlowUp                   = FX_spectreGunshipDeathExplosion
    OCLFinalBlowUp                  = OCL_SpectreDeathFinalBlowUp
  End
"#;
    let mut factory = catalog();
    let data = parse(
        &mut factory,
        ModuleType::Behavior,
        "JetSlowDeathBehavior",
        BODY,
    )
    .expect("genuine full retail JetSlowDeath declaration");
    let jet = data.as_any().downcast_ref::<
        crate::object::behavior::jet_slow_death_behavior::JetSlowDeathBehaviorModuleData
    >().expect("actual typed Jet data, not a generic fallback");
    assert_eq!(jet.base.destruction_delay, 3_000_000);
    assert_eq!(jet.base.probability_modifier, 10);
    assert_eq!(jet.roll_rate, 0.0);
    assert_eq!(jet.roll_rate_delta, 1.0);
    assert_eq!(jet.pitch_rate, 0.0);
    assert!((jet.fall_how_fast - 1.1).abs() < f32::EPSILON);
    assert_eq!(jet.delay_secondary_from_initial_death, 15);
    assert_eq!(jet.delay_final_blow_up_from_hit_ground, 6);
    assert_eq!(
        jet.fx_initial_death.as_ref().map(|fx| fx.name()),
        Some("FX_SpectreGunshipExplosionLight")
    );
    assert_eq!(
        jet.fx_secondary.as_ref().map(|fx| fx.name()),
        Some("FX_spectreDeathExplosion")
    );
    assert_eq!(
        jet.fx_hit_ground.as_ref().map(|fx| fx.name()),
        Some("FX_spectreGunshipDeathExplosion")
    );
    assert!(
        jet.fx_final_blow_up.is_none(),
        "the retail field is commented out"
    );
    assert!(jet.ocl_initial_death.is_none(), "authored None stays None");
    let rejected = BODY.replacen("  End", "    UnknownJetField = 73\n  End", 1);
    let error = parse(
        &mut factory,
        ModuleType::Behavior,
        "JetSlowDeathBehavior",
        &rejected,
    )
    .expect_err("retail acceptance must remain strict");
    assert!(error.contains("JetSlowDeathBehavior"), "{error}");
    assert!(error.contains("UnknownJetField = 73"), "{error}");
    assert!(error.contains("line"), "{error}");
}

#[test]
fn strict_countermeasures_admits_the_real_airforce_spectre_upgrade_and_rejects_unknowns() {
    // Licensed INIZH.big Data/INI/Object/AirforceGeneral.ini:617. The C++
    // CountermeasuresBehavior.h:54-75 table includes UpgradeMuxData at once.
    const BODY: &str = r#"    TriggeredBy           = Upgrade_AmericaCountermeasures
    FlareTemplateName     = CountermeasureFlare
    FlareBoneBaseName     = Flare ; Name of the base flare bone (Flare01, Flare02, Flare03)
    VolleySize            = 4     ; Number of flares launched per volley (requires bones)
    VolleyArcAngle        = 90.0  ; Max angle of flare relative to forward direction (with VolleySize of 1, flare will always goes straight back).
    VolleyVelocityFactor  = 2.0   ; Shoots out flares at a stronger velocity with a higher value.
    DelayBetweenVolleys   = 1000  ; Time between flare volleys
    NumberOfVolleys       = 5     ; Number of times the volleys will fire before reloading
    ReloadTime            = 0     ; After all volleys launched, then reloading must occur. If 0, then reloading occurs at airstrip only.
    EvasionRate           = 30%   ; With active flares, the specified percentage will be diverted.
    ReactionLaunchLatency = 0     ; Reaction between getting shot at and the firing of the first volley of countermeasures.
    MissileDecoyDelay     = 200   ; A reported missile that has been determined to hit a decoy will wait this long before acquiring countermeasures.
  End
"#;
    let mut factory = catalog();
    let data = parse(
        &mut factory,
        ModuleType::Behavior,
        "CountermeasuresBehavior",
        BODY,
    )
    .expect("genuine full retail Countermeasures declaration");
    let countermeasures = data.as_any().downcast_ref::<
        crate::object::behavior::countermeasures_behavior::CountermeasuresBehaviorModuleData
    >().expect("actual typed Countermeasures data");
    assert_eq!(
        countermeasures
            .upgrade_mux_data
            .activation_upgrade_names
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["Upgrade_AmericaCountermeasures"]
    );
    assert!(!countermeasures.upgrade_mux_data.requires_all_triggers);
    assert_eq!(
        countermeasures.flare_template_name.as_str(),
        "CountermeasureFlare"
    );
    assert_eq!(countermeasures.flare_bone_base_name.as_str(), "Flare");
    assert_eq!(countermeasures.volley_size, 4);
    assert_eq!(countermeasures.volley_arc_angle, 90_f32.to_radians());
    assert_eq!(countermeasures.volley_velocity_factor, 2.0);
    assert_eq!(countermeasures.frames_between_volleys, 30);
    assert_eq!(countermeasures.number_of_volleys, 5);
    assert_eq!(countermeasures.reload_frames, 0);
    assert!((countermeasures.evasion_rate - 0.3).abs() < f32::EPSILON);
    assert_eq!(countermeasures.countermeasure_reaction_frames, 0);
    assert_eq!(countermeasures.missile_decoy_frames, 6);
    let rejected = BODY.replacen("  End", "    UnknownCountermeasureField = 79\n  End", 1);
    let error = parse(
        &mut factory,
        ModuleType::Behavior,
        "CountermeasuresBehavior",
        &rejected,
    )
    .expect_err("retail admission remains strict after mux inheritance");
    assert!(error.contains("CountermeasuresBehavior"), "{error}");
    assert!(error.contains("UnknownCountermeasureField = 79"), "{error}");
}

#[test]
fn strict_special_power_subclasses_share_the_original_base_table_and_owned_values() {
    use crate::object::special_powers::*;
    // SpecialPowerModule.cpp:53-68 is inherited by each source subclass.
    const BODY: &str = "StartsPaused = Yes\n\
        InitiateSound = SourceInitiateSound\n\
        UpdateModuleStartsAttack = Yes\n\
        ScriptedSpecialPowerOnly = Yes\n\
        End\n";
    let mut factory = catalog();
    for name in [
        "BaikonurLaunchPower",
        "CashBountyPower",
        "CashHackSpecialPower",
        "CleanupAreaPower",
        "FireWeaponPower",
        "OCLSpecialPower",
        "SpecialAbility",
        "SpyVisionSpecialPower",
        "DefectorSpecialPower",
    ] {
        let data = parse(&mut factory, ModuleType::Behavior, name, BODY).expect(name);
        let base = match name {
            "BaikonurLaunchPower" => {
                &data
                    .as_any()
                    .downcast_ref::<BaikonurLaunchPowerModuleData>()
                    .expect("BaikonurLaunchPower typed data")
                    .base
            }
            "CashBountyPower" => {
                &data
                    .as_any()
                    .downcast_ref::<CashBountyPowerModuleData>()
                    .expect("CashBountyPower typed data")
                    .base
            }
            "CashHackSpecialPower" => {
                &data
                    .as_any()
                    .downcast_ref::<CashHackSpecialPowerModuleData>()
                    .expect("CashHackSpecialPower typed data")
                    .base
            }
            "CleanupAreaPower" => {
                &data
                    .as_any()
                    .downcast_ref::<CleanupAreaPowerModuleData>()
                    .expect("CleanupAreaPower typed data")
                    .base
            }
            "FireWeaponPower" => {
                &data
                    .as_any()
                    .downcast_ref::<FireWeaponPowerModuleData>()
                    .expect("FireWeaponPower typed data")
                    .base
            }
            "OCLSpecialPower" => {
                &data
                    .as_any()
                    .downcast_ref::<OclSpecialPowerModuleData>()
                    .expect("OCLSpecialPower typed data")
                    .base
            }
            "SpecialAbility" => {
                &data
                    .as_any()
                    .downcast_ref::<SpecialAbilityModuleData>()
                    .expect("SpecialAbility typed data")
                    .base
            }
            "SpyVisionSpecialPower" => {
                &data
                    .as_any()
                    .downcast_ref::<SpyVisionSpecialPowerModuleData>()
                    .expect("SpyVisionSpecialPower typed data")
                    .base
            }
            "DefectorSpecialPower" => {
                &data
                    .as_any()
                    .downcast_ref::<DefectorSpecialPowerModuleData>()
                    .expect("DefectorSpecialPower typed data")
                    .base
            }
            _ => unreachable!(),
        };
        assert!(base.starts_paused, "{name}");
        assert!(base.update_module_starts_attack, "{name}");
        assert!(base.scripted_special_power_only, "{name}");
        assert_eq!(
            base.initiate_sound.get_event_name(),
            "SourceInitiateSound",
            "{name}"
        );
        let error = parse(
            &mut factory,
            ModuleType::Behavior,
            name,
            "StartsPaused = nonsense\nEnd\n",
        )
        .expect_err("strict inherited bool");
        assert!(error.contains("StartsPaused = nonsense"), "{name}: {error}");
        let error = parse(
            &mut factory,
            ModuleType::Behavior,
            name,
            "UnknownSpecialPowerField = 91\nEnd\n",
        )
        .expect_err("unknown still rejected");
        assert!(
            error.contains("UnknownSpecialPowerField = 91"),
            "{name}: {error}"
        );
    }
    // This data class is not registered in the supported native catalog.
    // Fix its same source inheritance without enabling a production module.
    let mut demoralize = DemoralizeSpecialPowerModuleData::default();
    INI::new()
        .with_inline_source(BODY, |ini| demoralize.parse_from_ini(ini))
        .expect("existing Demoralize data API inherits the same table");
    assert!(demoralize.base.starts_paused);
    assert!(demoralize.base.update_module_starts_attack);
    assert!(demoralize.base.scripted_special_power_only);
    assert_eq!(
        demoralize.base.initiate_sound.get_event_name(),
        "SourceInitiateSound"
    );
    assert!(
        factory
            .construct(None, "DemoralizeSpecialPower", ModuleType::Behavior)
            .is_err()
    );
}

#[test]
fn strict_special_powers_admit_the_real_airforce_ocl_and_cleanup_declarations() {
    use crate::object::special_powers::{CleanupAreaPowerModuleData, OclSpecialPowerModuleData};
    // Licensed INIZH.big AirforceGeneral.ini:1481 and :5730, respectively.
    const OCL_BODY: &str = r#"    SpecialPowerTemplate = SuperweaponDaisyCutter  ;@@KRIS@@
    OCL                  = SUPERWEAPON_DaisyCutter
    CreateLocation       = USE_OWNER_OBJECT
    ScriptedSpecialPowerOnly = Yes
  End
"#;
    const CLEANUP_BODY: &str = r#"    SpecialPowerTemplate          = SpecialAbilityAmbulanceCleanupArea
    MaxMoveDistanceFromLocation   = 300.0 ;allows the unit to move around while cleaning up
    InitiateSound = AmbulanceVoiceDetox
  End
"#;
    let mut factory = catalog();
    let data = parse(
        &mut factory,
        ModuleType::Behavior,
        "OCLSpecialPower",
        OCL_BODY,
    )
    .expect("actual retail OCL leaf plus inherited script-only field");
    let ocl = data
        .as_any()
        .downcast_ref::<OclSpecialPowerModuleData>()
        .unwrap();
    assert!(ocl.base.scripted_special_power_only);
    assert!(!ocl.base.starts_paused);
    assert_eq!(
        ocl.base
            .special_power_template
            .as_ref()
            .map(|value| value.get_name()),
        Some("SuperweaponDaisyCutter")
    );
    assert_eq!(ocl.default_ocl.as_str(), "SUPERWEAPON_DaisyCutter");
    assert_eq!(
        ocl.create_loc,
        crate::object::special_powers::ocl_special_power::OCLCreateLocType::UseOwnerObject
    );
    let data = parse(
        &mut factory,
        ModuleType::Behavior,
        "CleanupAreaPower",
        CLEANUP_BODY,
    )
    .expect("actual retail Cleanup leaf plus inherited initiate sound");
    let cleanup = data
        .as_any()
        .downcast_ref::<CleanupAreaPowerModuleData>()
        .unwrap();
    assert_eq!(cleanup.cleanup_move_range, 300.0);
    assert_eq!(
        cleanup.base.initiate_sound.get_event_name(),
        "AmbulanceVoiceDetox"
    );
    assert_eq!(
        cleanup
            .base
            .special_power_template
            .as_ref()
            .map(|value| value.get_name()),
        Some("SpecialAbilityAmbulanceCleanupArea")
    );
    for (name, body) in [
        ("OCLSpecialPower", OCL_BODY),
        ("CleanupAreaPower", CLEANUP_BODY),
    ] {
        let body = body.replacen("  End", "    UnknownRetailPowerField = 97\n  End", 1);
        let error = parse(&mut factory, ModuleType::Behavior, name, &body)
            .expect_err("real retail admission must remain strict");
        assert!(error.contains(name), "{error}");
        assert!(error.contains("UnknownRetailPowerField = 97"), "{error}");
    }
}

#[test]
fn strict_deliver_payload_admits_genuine_labeled_drop_coordinates_and_rejects_bad_components() {
    use crate::common::Coord3D;
    use crate::object::update::ai_update::deliver_payload_ai_update::DeliverPayloadAIUpdateModuleData;
    let mut factory = catalog();
    // C++ getSepsColon includes both ':' and '=' as separators.
    for coordinates in [
        "X:0 Y:0 Z:-10",
        "x=0 y=0 z=-10",
        "X:=0 Y:=0 Z:=-10",
        "X : 0 Y : 0 Z : -10",
    ] {
        let data = parse(
            &mut factory,
            ModuleType::Behavior,
            "DeliverPayloadAIUpdate",
            &format!("DropOffset = {coordinates}\nDropVariance = X:10 Y:20 Z:30\nEnd\n"),
        )
        .expect("actual registered DeliverPayloadAIUpdate labeled retail coordinates");
        let data = data
            .as_any()
            .downcast_ref::<DeliverPayloadAIUpdateModuleData>()
            .expect("actual typed payload AI data");
        assert_eq!(data.drop_offset, Coord3D::new(0.0, 0.0, -10.0));
        assert_eq!(data.drop_variance, Coord3D::new(10.0, 20.0, 30.0));
    }
    for body in [
        "DropOffset = X:0 Y:bad Z:-10\nEnd\n",
        "DropVariance = X:1 Y:2\nEnd\n",
        "DropOffset = Y:1 X:2 Z:3\nEnd\n",
        "DropOffset = X:1 Q:2 Z:3\nEnd\n",
    ] {
        assert!(
            parse(
                &mut factory,
                ModuleType::Behavior,
                "DeliverPayloadAIUpdate",
                body
            )
            .is_err(),
            "malformed labeled component must reject: {body}"
        );
    }
}

#[path = "ai_parser_tests.rs"]
mod ai_parser_tests;

#[path = "railroad_parser_tests.rs"]
mod railroad_parser_tests;

#[path = "inherited_behavior_parser_tests.rs"]
mod inherited_behavior_parser_tests;

#[path = "coordinate_decal_parser_tests.rs"]
mod coordinate_decal_parser_tests;

#[path = "draw_condition_parser_tests.rs"]
mod draw_condition_parser_tests;

#[path = "veterancy_science_owner_tests.rs"]
mod veterancy_science_owner_tests;
