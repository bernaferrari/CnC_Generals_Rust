use super::*;

#[test]
fn authored_produced_at_helipad_kindof_survives_object_ini_parsing() {
    // hq-mzd8i: C++ KINDOF_PRODUCED_AT_HELIPAD is an authored token, not a
    // basename heuristic. Tagged custom names keep the bit; aircraft without
    // the token do not.
    use std::sync::Arc;
    let common = Arc::new(game_engine::common::common_services::CommonServices::new());
    game_engine::common::common_services::with_common_services(&common, || {
        let source = r#"
Object CustomTaggedHelo
  KindOf = VEHICLE AIRCRAFT PRODUCED_AT_HELIPAD SELECTABLE
  Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
End
Object CustomAirOnlyJet
  KindOf = VEHICLE AIRCRAFT SELECTABLE
  Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
End
"#;
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(source, "produced_at_helipad_probe.ini")
            .expect("parse probe");
        let tagged = GameLogic::build_template_from_object_definition(
            "CustomTaggedHelo",
            parser.get_definition("CustomTaggedHelo").expect("tagged"),
            None,
        );
        let air_only = GameLogic::build_template_from_object_definition(
            "CustomAirOnlyJet",
            parser.get_definition("CustomAirOnlyJet").expect("jet"),
            None,
        );
        assert!(
            tagged.is_kind_of(KindOf::ProducedAtHelipad),
            "authored PRODUCED_AT_HELIPAD must land on the template"
        );
        assert!(
            !air_only.is_kind_of(KindOf::ProducedAtHelipad),
            "aircraft without the token must not inherit helipad via name"
        );
        assert!(crate::game_logic::GameLogic::template_is_produced_at_helipad(&tagged));
        assert!(!crate::game_logic::GameLogic::template_is_produced_at_helipad(&air_only));
    });
}

fn retail_service_and_dozer_kindof_tags_survive_object_ini_parsing() {
    use std::path::Path;

    // Parse the actual retail definitions rather than a name-shaped
    // fixture.  C++ ActionManager distinguishes these exact KindOf tags:
    // AmericaBarracks is a HEAL_PAD, AmericaWarFactory a REPAIR_PAD,
    // AmericaAirfield an FS_AIRFIELD, and the construction unit a DOZER.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("Main crate must remain three levels below repository root");
    let faction_buildings = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini"),
    )
    .expect("retail FactionBuilding.ini");
    let america_vehicles = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/AmericaVehicle.ini"),
    )
    .expect("retail AmericaVehicle.ini");

    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
        .expect("parse retail faction structures");
    parser
        .parse_ini_content(&america_vehicles, "AmericaVehicle.ini")
        .expect("parse retail USA vehicles");

    let parsed = |name| {
        GameLogic::build_template_from_object_definition(
            name,
            parser
                .get_definition(name)
                .unwrap_or_else(|| panic!("missing retail Object {name}")),
            None,
        )
    };
    assert!(parsed("AmericaAirfield").is_kind_of(KindOf::FSAirfield));
    assert!(parsed("AmericaBarracks").is_kind_of(KindOf::HealPad));
    assert!(parsed("AmericaWarFactory").is_kind_of(KindOf::RepairPad));
    assert!(parsed("AmericaVehicleDozer").is_kind_of(KindOf::Dozer));
}

#[test]
fn retail_hack_internet_metadata_drives_field_and_contained_income() {
    use glam::Vec3;

    // This is deliberately a retail parser-to-runtime proof, rather than
    // a `TestHacker` basename fixture.  ChinaInfantryHacker carries the
    // exact HackInternetAIUpdate and MONEY_HACKER KindOf; ChinaInternetCenter
    // carries InternetHackContain's eight transport slots and one-kind
    // admission mask.
    let content = retail_receiving_fixture(&[]);

    let hacker_template = GameLogic::build_template_from_object_definition(
        "ChinaInfantryHacker",
        content
            .projections()
            .get("ChinaInfantryHacker")
            .expect("retail China hacker definition"),
        None,
    );
    let hacker_data = hacker_template
        .hack_internet_ai_update
        .expect("retail HackInternetAIUpdate metadata");
    assert!(hacker_template.is_kind_of(KindOf::MoneyHacker));
    assert_eq!(hacker_template.transport_slot_count, Some(1));
    assert_eq!(hacker_data.unpack_time_frames, 219);
    assert_eq!(hacker_data.pack_time_frames, 154);
    assert_eq!(hacker_data.cash_update_delay_frames, 60);
    assert_eq!(hacker_data.cash_update_delay_fast_frames, 54);
    assert_eq!(hacker_data.regular_cash_amount, 5);
    assert_eq!(hacker_data.veteran_cash_amount, 6);
    assert_eq!(hacker_data.elite_cash_amount, 8);
    assert_eq!(hacker_data.heroic_cash_amount, 10);
    assert!((hacker_data.xp_per_cash_update - 1.0).abs() < f32::EPSILON);

    let center_template = GameLogic::build_template_from_object_definition(
        "ChinaInternetCenter",
        content
            .projections()
            .get("ChinaInternetCenter")
            .expect("retail China Internet Center definition"),
        None,
    );
    assert_eq!(
        center_template.contain_module.kind,
        ContainModuleKind::InternetHack
    );
    assert_eq!(center_template.contain_module.slots, Some(8));
    assert_eq!(
        center_template.contain_module.admission,
        ContainAdmission::MoneyHackerOnly
    );

    let mut logic = GameLogic::new();
    let mut china = Player::new(1, Team::China, "China", true);
    china.resources.supplies = 0;
    logic.add_player(china);
    admit_fixture_content(
        &mut logic,
        content,
        &["ChinaInfantryHacker", "ChinaInternetCenter"],
    );

    let field_hacker = logic
        .create_object_for_player("ChinaInfantryHacker", 1, Vec3::new(16.0, 0.0, 0.0))
        .expect("field hacker");
    let contained_hacker = logic
        .create_object_for_player("ChinaInfantryHacker", 1, Vec3::new(1.0, 0.0, 0.0))
        .expect("contained hacker");
    let center = logic
        .create_object_for_player("ChinaInternetCenter", 1, Vec3::ZERO)
        .expect("Internet Center");

    // This checks the frozen input authority and the arrival-side
    // containment bookkeeping together: a generic infantry unit cannot
    // stand in for the parsed MONEY_HACKER contract.
    assert!(logic.can_unit_enter_normal_target(contained_hacker, center));
    assert!(
        logic
            .host_object_mut(center)
            .expect("Internet Center object")
            .add_occupant(contained_hacker)
    );
    logic.set_fixture_object_contained_by(contained_hacker, Some(center));

    logic.frame = 0;
    assert!(
        logic
            .start_hacker_internet_hack(field_hacker)
            .expect("checked hacker Idle entry")
    );
    logic
        .update_hacker_income()
        .expect("receiving AI operation succeeds");
    assert!(logic.hacker_income().is_hacking(contained_hacker));
    assert_eq!(logic.get_player(1).unwrap().resources.supplies, 0);

    // UNPACKING (219) then CashUpdateDelay, then decrement-fire.
    // Contained uses fast delay 54 → 219+54+1=274; field 219+60+1=280.
    logic.frame = 273;
    logic
        .update_hacker_income()
        .expect("receiving AI operation succeeds");
    assert_eq!(logic.get_player(1).unwrap().resources.supplies, 0);
    logic.frame = 274;
    logic
        .update_hacker_income()
        .expect("receiving AI operation succeeds");
    assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
    assert_eq!(logic.get_player(1).unwrap().statistics.money_earned(), 5);
    logic.frame = 279;
    logic
        .update_hacker_income()
        .expect("receiving AI operation succeeds");
    assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
    logic.frame = 280;
    logic
        .update_hacker_income()
        .expect("receiving AI operation succeeds");
    assert_eq!(logic.get_player(1).unwrap().resources.supplies, 10);
    assert_eq!(logic.get_player(1).unwrap().statistics.money_earned(), 10);
}

#[test]
fn retail_hacker_disable_pair_uses_exact_power_identity_not_microwave_alias() {
    use std::collections::HashMap;
    use std::path::Path;

    // The live boot path owns this global store.  Focused unit tests do
    // not run the shell bootstrap, so seed only the two actual retail
    // records if they are absent.  Both deliberately share Common's C++
    // enum; the parser below must still admit HDB alone by canonical
    // SpecialPowerTemplate identity.
    {
        use game_engine::common::rts::special_power::get_special_power_store_mut;

        let mut powers = get_special_power_store_mut();
        for (name, reload) in [
            ("SpecialAbilityHackerDisableBuilding", "500"),
            ("SpecialAbilityMicrowaveDisableBuilding", "4000"),
        ] {
            if powers.find_template(name).is_none() {
                let mut fields = HashMap::new();
                fields.insert(
                    "Enum".to_string(),
                    "SPECIAL_HACKER_DISABLE_BUILDING".to_string(),
                );
                fields.insert("ReloadTime".to_string(), reload.to_string());
                fields.insert("PublicTimer".to_string(), "No".to_string());
                powers
                    .parse_special_power_definition(name, &fields)
                    .unwrap_or_else(|error| {
                        panic!("seed required retail special power {name}: {error}")
                    });
            }
        }
    }

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("Main crate must remain three levels below repository root");
    let special_power = std::fs::read_to_string(
        repo_root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/SpecialPower.ini"),
    )
    .expect("retail SpecialPower.ini");
    assert!(special_power.contains("SpecialPower SpecialAbilityHackerDisableBuilding"));
    assert!(special_power.contains("SpecialPower SpecialAbilityMicrowaveDisableBuilding"));
    assert!(special_power.contains("ReloadTime        = 500"));
    assert!(special_power.contains("ReloadTime        = 4000"));

    let china_infantry = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/ChinaInfantry.ini"),
    )
    .expect("retail ChinaInfantry.ini");
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&china_infantry, "ChinaInfantry.ini")
        .expect("parse retail China hacker");
    // Microwave shares the C++ HDB enum and must expose the same
    // disable-building channel, keyed by its own SpecialPowerTemplate.
    parser
        .parse_ini_content(
            r#"
Object MicrowaveAliasProbe
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = SpecialAbility ModuleTag_Microwave
    SpecialPowerTemplate = SpecialAbilityMicrowaveDisableBuilding
    UpdateModuleStartsAttack = Yes
  End
  Behavior = SpecialAbilityUpdate ModuleTag_MicrowaveUpdate
    SpecialPowerTemplate = SpecialAbilityMicrowaveDisableBuilding
    StartAbilityRange = 150.0
    UnpackTime = 1
    PreparationTime = 1
    PersistentPrepTime = 1
    EffectDuration = 1
    PackTime = 1
  End
End
"#,
            "microwave_alias_probe.ini",
        )
        .expect("parse microwave alias probe");

    let hacker = GameLogic::build_template_from_object_definition(
        "ChinaInfantryHacker",
        parser
            .get_definition("ChinaInfantryHacker")
            .expect("retail China hacker"),
        None,
    );
    let hdb = hacker
        .hacker_disable_building
        .expect("retail HDB pair must survive parser");
    assert_eq!(
        hdb.special_power_template,
        "SpecialAbilityHackerDisableBuilding"
    );
    assert_eq!(hdb.reload_time_frames, 15);
    assert_eq!(hdb.start_ability_range, 150.0);
    assert_eq!(hdb.unpack_time_ms, 7_300);
    assert_eq!(hdb.preparation_time_ms, 3_000);
    assert_eq!(hdb.persistent_prep_time_ms, 333);
    assert_eq!(hdb.effect_duration_ms, 2_000);
    assert_eq!(hdb.pack_time_ms, 5_133);
    assert!(!hdb.persistence_requires_recharge);

    let microwave = GameLogic::build_template_from_object_definition(
        "MicrowaveAliasProbe",
        parser
            .get_definition("MicrowaveAliasProbe")
            .expect("microwave alias probe"),
        None,
    );
    let microwave_hdb = microwave
        .hacker_disable_building
        .expect("Microwave SPECIAL_HACKER_DISABLE_BUILDING pair must expose the disable channel");
    assert_eq!(
        microwave_hdb.special_power_template,
        "SpecialAbilityMicrowaveDisableBuilding"
    );
    assert_eq!(
        microwave_hdb.command_power(),
        crate::command_system::SpecialPowerType::MicrowaveDisableBuilding
    );
    assert!(!microwave_hdb.is_hacker_command());
    assert_eq!(microwave_hdb.reload_time_frames, 120);
    assert_eq!(microwave_hdb.start_ability_range, 150.0);
    assert_eq!(microwave_hdb.unpack_time_ms, 1);
    assert_eq!(microwave_hdb.preparation_time_ms, 1);
    assert_eq!(microwave_hdb.persistent_prep_time_ms, 1);
    assert_eq!(microwave_hdb.effect_duration_ms, 1);
    assert_eq!(microwave_hdb.pack_time_ms, 1);
}

#[test]
fn retail_superweapon_modules_drive_arm_limit_energy_and_presentation_not_names() {
    use glam::Vec3;
    // The complete receiving content below captures actual retail
    // SpecialPower source before typed Object modules are admitted.

    const SPOOF_SOURCE: &str = r#"
Object AmericaParticleCannonUplinkNamedButNoModule
  KindOf = STRUCTURE SELECTABLE POWERED FS_SUPERWEAPON
End
"#;
    let standalone = crate::assets::ReceivingObjectDefinitions::from_boot_sources(&[(
        "superweapon_name_spoof.ini".into(),
        SPOOF_SOURCE.into(),
    )])
    .expect("standalone authored-absence projection");
    let content = retail_receiving_fixture(&[("superweapon_name_spoof.ini", SPOOF_SOURCE)]);

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    admit_fixture_content(
        &mut logic,
        content,
        &[
            "AmericaParticleCannonUplink",
            "GLAScudStorm",
            "ChinaNuclearMissileLauncher",
            "AmericaParticleCannonUplinkNamedButNoModule",
        ],
    );
    logic.load_own_command_ini_for_test(&retail_command_ini());

    // Inspect the exact template produced by actual receiving admission,
    // including its immutable SpecialPower catalog and typed module roster.
    let parsed = |name: &str| {
        logic
            .templates
            .get(name)
            .cloned()
            .unwrap_or_else(|| panic!("missing admitted retail Object {name}"))
    };
    let particle = parsed("AmericaParticleCannonUplink");
    let scud = parsed("GLAScudStorm");
    let nuke = parsed("ChinaNuclearMissileLauncher");
    let spoof = parsed("AmericaParticleCannonUplinkNamedButNoModule");

    fn first_power_name(template: &ThingTemplate) -> Option<&str> {
        template
            .special_power_modules
            .first()
            .map(|module| module.special_power_template.as_str())
    }
    assert_eq!(
        first_power_name(&particle),
        Some("SuperweaponParticleUplinkCannon")
    );
    assert_eq!(first_power_name(&scud), Some("SuperweaponScudStorm"));
    assert_eq!(first_power_name(&nuke), Some("SuperweaponNeutronMissile"));
    assert!(particle.special_power_modules[0].module_tag.is_some());
    assert_eq!(particle.energy_production, Some(-10));
    assert_eq!(scud.energy_production, Some(0));
    assert_eq!(nuke.energy_production, Some(-10));
    assert_eq!(
        particle.max_simultaneous_link_key.as_deref(),
        Some("Superweapon")
    );
    assert!(particle.max_simultaneous_determined_by_superweapon_restriction);
    assert!(spoof.special_power_modules.is_empty());
    // Without Default/Object.ini, this exact source authors no power field;
    // a superweapon-shaped name must not invent one. The actual receiving
    // namespace also includes DefaultThingTemplate EnergyProduction = 0
    // (ThingFactory.cpp114-120 copies that template before the Object body).
    let standalone_spoof = GameLogic::build_template_from_object_definition(
        "AmericaParticleCannonUplinkNamedButNoModule",
        standalone
            .get("AmericaParticleCannonUplinkNamedButNoModule")
            .unwrap(),
        None,
    );
    assert_eq!(standalone_spoof.energy_production, None);
    assert_eq!(spoof.energy_production, Some(0));
    assert!(!spoof.has_superweapon_restriction_link_key());

    logic.skirmish_rules.limit_superweapons = true;

    // The retail POWERED cannon consumes ten Energy. Supply real owned
    // generation rather than relying on a presentation-only zero fallback.
    let plant_id = logic
        .create_object_for_player("AmericaPowerPlant", 1, Vec3::new(-80.0, 0.0, 0.0))
        .expect("actual receiving retail power producer");
    assert!(
        logic
            .host_object(plant_id)
            .unwrap()
            .get_template()
            .energy_production
            .unwrap()
            > 0
    );
    logic
        .create_object_for_player("AmericaPowerPlant", 1, Vec3::new(-120.0, 0.0, 0.0))
        .expect("second actual five-Energy retail producer");

    let particle_id = logic
        .create_object_for_player("AmericaParticleCannonUplink", 1, Vec3::ZERO)
        .expect("retail particle structure");
    let spoof_id = logic
        .create_object_for_player(
            "AmericaParticleCannonUplinkNamedButNoModule",
            1,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("name spoof structure");
    assert_eq!(
        logic
            .host_object(spoof_id)
            .unwrap()
            .get_template()
            .energy_production,
        Some(0),
        "the live paired spoof retains its original inherited zero"
    );
    use crate::command_system::SpecialPowerType as P;
    assert!(logic.host_object(particle_id).expect("particle object")
        .special_power_countdown_seconds(&P::ParticleCannon) > 0.0);
    assert!(
        logic
            .host_object(spoof_id)
            .expect("spoof object")
            .special_power_cooldowns
            .is_empty()
    );
    assert!(!logic.can_start_superweapon_building_for_player(1, "AmericaParticleCannonUplink"));
    assert!(logic.can_start_superweapon_building_for_player(
        1,
        "AmericaParticleCannonUplinkNamedButNoModule"
    ));

    let particle_object = logic.host_object_mut(particle_id).expect("particle object");
    particle_object.set_special_power_ready_seconds(&P::ParticleCannon, 0.0);
    assert!(logic.is_special_power_ready_for(particle_id, &P::ParticleCannon));
    assert!(!logic.is_special_power_ready_for(spoof_id, &P::ParticleCannon));
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, 1);
    let particle_presentation = frame
        .objects
        .iter()
        .find(|object| object.id == particle_id)
        .expect("particle presentation");
    assert_eq!(
        particle_presentation
            .special_power_ready_template_name
            .as_deref(),
        Some("SuperweaponParticleUplinkCannon")
    );
    assert!(
        frame
            .objects
            .iter()
            .find(|object| object.id == spoof_id)
            .expect("spoof presentation")
            .special_power_ready_template_name
            .is_none()
    );

    logic.select_objects(1, vec![particle_id]);
    let particle_frame =
        crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, 1);
    assert!(particle_frame.unit_command_buttons().iter().any(|button| {
        let n = button.command_name.to_ascii_lowercase();
        n.contains("particle") && button.enabled
    }));
    logic.select_objects(1, vec![spoof_id]);
    let spoof_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, 1);
    assert!(!spoof_frame.unit_command_buttons().iter().any(|button| {
        button
            .command_name
            .to_ascii_lowercase()
            .contains("particle")
    }));
}

#[test]
fn parsed_parking_place_behavior_keeps_authored_shape_without_name_fallback() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object ArbitraryParkingIdentity
  Type = Structure
  Model = ArbitraryParkingModel
  KindOf = STRUCTURE SELECTABLE FS_AIRFIELD
  Behavior = ParkingPlaceBehavior ModuleTag_Parking
    NumRows = 3
    NumCols = 2
    ApproachHeight = 72.5
    LandingDeckHeightOffset = 4.25
    HasRunways = Yes
    ParkInHangars = Yes
    HealAmountPerSecond = 7.5
  End
End
Object AirfieldNamedButNoParkingBehavior
  Type = Structure
  Model = NoParkingModel
  KindOf = STRUCTURE SELECTABLE FS_AIRFIELD
End
"#,
            "parking_place_metadata_probe.ini",
        )
        .expect("parse parking place metadata probe");

    let parsed = GameLogic::build_template_from_object_definition(
        "ArbitraryParkingIdentity",
        parser
            .get_definition("ArbitraryParkingIdentity")
            .expect("parking definition"),
        None,
    );
    let metadata = parsed.parking_place.expect("authored parking metadata");
    assert_eq!(metadata.num_rows, 3);
    assert_eq!(metadata.num_cols, 2);
    assert_eq!(metadata.capacity(), Some(6));
    assert_eq!(metadata.runway_count(), Some(2));
    assert!((metadata.approach_height - 72.5).abs() < f32::EPSILON);
    assert!((metadata.landing_deck_height_offset - 4.25).abs() < f32::EPSILON);
    assert!(metadata.has_runways);
    assert!(metadata.park_in_hangars);
    assert!((metadata.heal_amount_per_second - 7.5).abs() < f32::EPSILON);

    let no_behavior = GameLogic::build_template_from_object_definition(
        "AirfieldNamedButNoParkingBehavior",
        parser
            .get_definition("AirfieldNamedButNoParkingBehavior")
            .expect("no-behavior definition"),
        None,
    );
    assert!(no_behavior.is_kind_of(KindOf::FSAirfield));
    assert!(
        no_behavior.parking_place.is_none(),
        "FSAirfield/name alone must not fabricate ParkingPlaceBehavior"
    );
}

#[test]
fn parsed_rebuild_hole_expose_die_keeps_authored_hole_name() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object GLAScudStorm
  KindOf = STRUCTURE
  Behavior = RebuildHoleExposeDie ModuleTag_Hole
    HoleName = GLAScudStormRebuildHole
    HoleMaxHealth = 500.0
    TransferAttackers = Yes
  End
End
Object AmericaCommandCenter
  KindOf = STRUCTURE COMMANDCENTER
End
"#,
            "rebuild_hole_test.ini",
        )
        .expect("parse rebuild hole expose probe");
    let scud = GameLogic::build_template_from_object_definition(
        "GLAScudStorm",
        parser
            .get_definition("GLAScudStorm")
            .expect("scud definition"),
        None,
    );
    let expose = scud
        .rebuild_hole_expose
        .expect("authored RebuildHoleExposeDie");
    assert_eq!(expose.hole_name, "GLAScudStormRebuildHole");
    assert!((expose.hole_max_health - 500.0).abs() < f32::EPSILON);
    assert!(expose.transfer_attackers);

    let usa = GameLogic::build_template_from_object_definition(
        "AmericaCommandCenter",
        parser
            .get_definition("AmericaCommandCenter")
            .expect("usa cc definition"),
        None,
    );
    assert!(
        usa.rebuild_hole_expose.is_none(),
        "USA CC name/command KindOf must not fabricate a hole module"
    );
}

#[test]
fn parsed_max_simultaneous_of_type_reads_numeric_and_restriction() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object AmericaWarFactory
  KindOf = STRUCTURE
  MaxSimultaneousOfType = 1
End
Object AmericaInfantryColonelBurton
  KindOf = INFANTRY
  MaxSimultaneousOfType = 1
End
Object AmericaParticleCannonUplink
  KindOf = STRUCTURE
  MaxSimultaneousOfType = DeterminedBySuperweaponRestriction
  MaxSimultaneousLinkKey = Superweapon
End
Object AmericaInfantryRanger
  KindOf = INFANTRY
End
"#,
            "max_simultaneous_probe.ini",
        )
        .expect("parse MaxSimultaneous probe");
    let factory = GameLogic::build_template_from_object_definition(
        "AmericaWarFactory",
        parser.get_definition("AmericaWarFactory").expect("factory"),
        None,
    );
    assert_eq!(factory.max_simultaneous_of_type, 1);
    assert!(!factory.max_simultaneous_determined_by_superweapon_restriction);

    let burton = GameLogic::build_template_from_object_definition(
        "AmericaInfantryColonelBurton",
        parser
            .get_definition("AmericaInfantryColonelBurton")
            .expect("burton"),
        None,
    );
    assert_eq!(burton.max_simultaneous_of_type, 1);

    let particle = GameLogic::build_template_from_object_definition(
        "AmericaParticleCannonUplink",
        parser
            .get_definition("AmericaParticleCannonUplink")
            .expect("puc"),
        None,
    );
    assert_eq!(particle.max_simultaneous_of_type, 0);
    assert!(particle.max_simultaneous_determined_by_superweapon_restriction);
    assert_eq!(
        particle.max_simultaneous_link_key.as_deref(),
        Some("Superweapon")
    );

    let ranger = GameLogic::build_template_from_object_definition(
        "AmericaInfantryRanger",
        parser
            .get_definition("AmericaInfantryRanger")
            .expect("ranger"),
        None,
    );
    assert_eq!(ranger.max_simultaneous_of_type, 0);
    assert!(!ranger.max_simultaneous_determined_by_superweapon_restriction);
}

#[test]
fn leftover_prerequisites_parse_blocks_black_market_without_palace() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object GLAPalace
  KindOf = STRUCTURE
End
Object GLABlackMarket
  KindOf = STRUCTURE
  Prerequisites
    Object = GLAPalace
  End
End
Object AmericaSupplyDropZone
  KindOf = STRUCTURE
  Prerequisites
    Object = AmericaStrategyCenter
  End
End
"#,
            "leftover_prereq_probe.ini",
        )
        .expect("parse leftover prereq probe");
    let market = GameLogic::build_template_from_object_definition(
        "GLABlackMarket",
        parser
            .get_definition("GLABlackMarket")
            .expect("market definition"),
        None,
    );
    assert_eq!(market.production_prerequisites.len(), 1);
    let units = market.production_prerequisites[0].get_unit_prereqs();
    assert_eq!(units.len(), 1);
    if !units[0].name.is_empty() {
        assert_eq!(units[0].name, "GLAPalace");
        assert!(!units[0].flags.has_or_with_prev());
    }

    let drop = GameLogic::build_template_from_object_definition(
        "AmericaSupplyDropZone",
        parser
            .get_definition("AmericaSupplyDropZone")
            .expect("drop definition"),
        None,
    );
    let drop_units = drop.production_prerequisites[0].get_unit_prereqs();
    assert_eq!(drop_units.len(), 1);
    if !drop_units[0].name.is_empty() {
        assert_eq!(drop_units[0].name, "AmericaStrategyCenter");
    }

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::GLA, "GLA", true));
    logic
        .templates
        .insert("GLAPalace".into(), ThingTemplate::new("GLAPalace"));
    logic.templates.insert("GLABlackMarket".into(), market);
    assert!(
        !logic.player_satisfies_build_prerequisites(0, "GLABlackMarket"),
        "Black Market must not fail-open without Palace"
    );
    // Player::canBuild gates the build command (BuildAssistant::canMakeUnit,
    // BuildAssistant.cpp:1266); buildObjectNow and ThingFactory::newObject
    // never consult prerequisites, so the scaffold adapter is not the gate.
    let palace = logic
        .create_object("GLAPalace", Team::GLA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("palace");
    assert!(
        logic
            .host_object(palace)
            .is_some_and(|o| o.is_constructed())
    );
    assert!(logic.player_satisfies_build_prerequisites(0, "GLABlackMarket"));
}

#[test]
fn parsed_flight_deck_behavior_keeps_authored_shape_without_name_fallback() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object ArbitraryCarrierIdentity
  Type = Vehicle
  Model = ArbitraryCarrierModel
  KindOf = VEHICLE SELECTABLE AIRCRAFT_CARRIER
  Behavior = FlightDeckBehavior ModuleTag_Deck
    NumRunways = 2
    NumSpacesPerRunway = 4
    ApproachHeight = 50
    LandingDeckHeightOffset = 22
    HealAmountPerSecond = 10
    ParkingCleanupPeriod = 1000
    HumanFollowPeriod = 500
    PayloadTemplate = AircraftCarrierRaptor
    ReplacementDelay = 10000
    DockAnimationDelay = 2000
    LaunchWaveDelay = 1500
    LaunchRampDelay = 1000
    LowerRampDelay = 2000
    CatapultFireDelay = 500
    Runway1CatapultSystem = AircraftCarrierCatapultSteam
  End
End
Object CarrierNamedButNoDeckBehavior
  Type = Vehicle
  Model = NoDeckModel
  KindOf = VEHICLE SELECTABLE AIRCRAFT_CARRIER
End
"#,
            "flight_deck_metadata_probe.ini",
        )
        .expect("parse flight deck metadata probe");

    let parsed = GameLogic::build_template_from_object_definition(
        "ArbitraryCarrierIdentity",
        parser
            .get_definition("ArbitraryCarrierIdentity")
            .expect("carrier definition"),
        None,
    );
    let metadata = parsed.flight_deck.expect("authored flight deck metadata");
    assert_eq!(metadata.num_rows, 4);
    assert_eq!(metadata.num_cols, 2);
    assert_eq!(metadata.capacity(), Some(8));
    assert_eq!(metadata.payload_template, "AircraftCarrierRaptor");
    assert!((metadata.approach_height - 50.0).abs() < f32::EPSILON);
    assert!((metadata.landing_deck_height_offset - 22.0).abs() < f32::EPSILON);
    assert_eq!(metadata.cleanup_frames, 30);
    assert_eq!(metadata.human_follow_frames, 15);
    assert_eq!(metadata.replacement_frames, 300);
    assert_eq!(metadata.dock_animation_frames, 60);
    assert_eq!(metadata.launch_wave_frames, 45);
    assert_eq!(metadata.launch_ramp_frames, 30);
    assert_eq!(metadata.lower_ramp_frames, 60);
    assert_eq!(metadata.catapult_fire_frames, 15);
    assert_eq!(
        metadata.catapult_system[0].as_deref(),
        Some("AircraftCarrierCatapultSteam")
    );

    let no_behavior = GameLogic::build_template_from_object_definition(
        "CarrierNamedButNoDeckBehavior",
        parser
            .get_definition("CarrierNamedButNoDeckBehavior")
            .expect("no-behavior definition"),
        None,
    );
    assert!(
        no_behavior.flight_deck.is_none(),
        "carrier KindOf/name alone must not fabricate FlightDeckBehavior"
    );
}

#[test]
fn retail_deploy_style_modules_preserve_authored_timings_and_flags() {
    // These are the source-authored DeployStyle blocks for the retail
    // Sentry Drone and Nuke Launcher.  Sentry's nested Turret is
    // intentional: its following fields prove module attribution remains
    // intact through a nested `End`.
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object AmericaVehicleSentryDrone
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  Behavior = DeployStyleAIUpdate ModuleTag_04
    Turret
      TurretTurnRate = 180
    End
    PackTime = 1000
    UnpackTime = 1000
    TurretsFunctionOnlyWhenDeployed = Yes
    TurretsMustCenterBeforePacking = Yes
  End
End
Object ChinaVehicleNukeLauncher
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  Behavior = DeployStyleAIUpdate ModuleTag_04
    Turret
      TurretTurnRate = 80
    End
    PackTime = 3333
    UnpackTime = 3333
    ResetTurretBeforePacking = No
    TurretsFunctionOnlyWhenDeployed = Yes
    TurretsMustCenterBeforePacking = Yes
    ManualDeployAnimations = Yes
  End
End
Object VehicleWithoutDeployStyle
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
End
"#,
            "retail_deploy_style_probe.ini",
        )
        .expect("parse retail DeployStyle probes");

    let sentry = GameLogic::build_template_from_object_definition(
        "AmericaVehicleSentryDrone",
        parser
            .get_definition("AmericaVehicleSentryDrone")
            .expect("sentry definition"),
        None,
    );
    let sentry_data = sentry
        .deploy_style_metadata
        .as_ref()
        .expect("sentry authored DeployStyle");
    assert_eq!(sentry_data.pack_time_frames, 30);
    assert_eq!(sentry_data.unpack_time_frames, 30);
    assert!(!sentry_data.reset_turret_before_packing);
    assert!(sentry_data.turrets_function_only_when_deployed);
    assert!(sentry_data.turrets_must_center_before_packing);
    assert!(!sentry_data.manual_deploy_animations);

    let nuke = GameLogic::build_template_from_object_definition(
        "ChinaVehicleNukeLauncher",
        parser
            .get_definition("ChinaVehicleNukeLauncher")
            .expect("nuke definition"),
        None,
    );
    let nuke_data = nuke
        .deploy_style_metadata
        .as_ref()
        .expect("nuke authored DeployStyle");
    assert_eq!(nuke_data.pack_time_frames, 100);
    assert_eq!(nuke_data.unpack_time_frames, 100);
    assert!(!nuke_data.reset_turret_before_packing);
    assert!(nuke_data.turrets_function_only_when_deployed);
    assert!(nuke_data.turrets_must_center_before_packing);
    assert!(nuke_data.manual_deploy_animations);

    let ordinary = GameLogic::build_template_from_object_definition(
        "VehicleWithoutDeployStyle",
        parser
            .get_definition("VehicleWithoutDeployStyle")
            .expect("ordinary vehicle definition"),
        None,
    );
    assert!(ordinary.is_kind_of(KindOf::Vehicle));
    assert!(
        ordinary.deploy_style_metadata.is_none(),
        "VEHICLE KindOf/name must not synthesize DeployStyle authority"
    );
}
