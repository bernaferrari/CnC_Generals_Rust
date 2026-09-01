//! Behavioral tests for Object INI template construction.
use super::*;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        let america_vehicles =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/AmericaVehicle.ini",
            ))
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
        use std::path::Path;

        // This is deliberately a retail parser-to-runtime proof, rather than
        // a `TestHacker` basename fixture.  ChinaInfantryHacker carries the
        // exact HackInternetAIUpdate and MONEY_HACKER KindOf; ChinaInternetCenter
        // carries InternetHackContain's eight transport slots and one-kind
        // admission mask.
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let china_infantry =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/ChinaInfantry.ini",
            ))
            .expect("retail ChinaInfantry.ini");
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&china_infantry, "ChinaInfantry.ini")
            .expect("parse retail China infantry");
        parser
            .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
            .expect("parse retail faction buildings");

        let hacker_template = GameLogic::build_template_from_object_definition(
            "ChinaInfantryHacker",
            parser
                .get_definition("ChinaInfantryHacker")
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
            parser
                .get_definition("ChinaInternetCenter")
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
        logic
            .templates
            .insert("ChinaInfantryHacker".to_string(), hacker_template);
        logic
            .templates
            .insert("ChinaInternetCenter".to_string(), center_template);

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
        logic
            .host_object_mut(contained_hacker)
            .expect("contained hacker object")
            .set_contained_by(Some(center));

        logic.frame = 0;
        assert!(logic.start_hacker_internet_hack(field_hacker));
        logic.update_hacker_income();
        assert!(logic.hacker_income().is_hacking(contained_hacker));
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 0);

        // UNPACKING (219) then CashUpdateDelay, then decrement-fire.
        // Contained uses fast delay 54 → 219+54+1=274; field 219+60+1=280.
        logic.frame = 273;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 0);
        logic.frame = 274;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
        assert_eq!(logic.get_player(1).unwrap().statistics.money_earned, 5);
        logic.frame = 279;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
        logic.frame = 280;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 10);
        assert_eq!(logic.get_player(1).unwrap().statistics.money_earned, 10);
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
            repo_root.join("windows_game/extracted_big_files_v2/INI/SpecialPower.ini"),
        )
        .expect("retail SpecialPower.ini");
        assert!(special_power.contains("SpecialPower SpecialAbilityHackerDisableBuilding"));
        assert!(special_power.contains("SpecialPower SpecialAbilityMicrowaveDisableBuilding"));
        assert!(special_power.contains("ReloadTime        = 500"));
        assert!(special_power.contains("ReloadTime        = 4000"));

        let china_infantry =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/ChinaInfantry.ini",
            ))
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
        let microwave_hdb = microwave.hacker_disable_building.expect(
            "Microwave SPECIAL_HACKER_DISABLE_BUILDING pair must expose the disable channel",
        );
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
        use std::collections::HashMap;
        use std::path::Path;

        // The fixture uses the actual retail FactionBuilding declarations and
        // only seeds the corresponding loaded SpecialPower records when the
        // wider shell bootstrap has not already done so.
        {
            use game_engine::common::rts::special_power::get_special_power_store_mut;

            let mut powers = get_special_power_store_mut();
            for (name, enum_name, reload) in [
                (
                    "SuperweaponParticleUplinkCannon",
                    "SPECIAL_PARTICLE_UPLINK_CANNON",
                    "240000",
                ),
                ("SuperweaponScudStorm", "SPECIAL_SCUD_STORM", "300000"),
                (
                    "SuperweaponNeutronMissile",
                    "SPECIAL_NEUTRON_MISSILE",
                    "360000",
                ),
            ] {
                if powers.find_template(name).is_none() {
                    let mut fields = HashMap::new();
                    fields.insert("Enum".to_string(), enum_name.to_string());
                    fields.insert("ReloadTime".to_string(), reload.to_string());
                    fields.insert("PublicTimer".to_string(), "Yes".to_string());
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
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
            .expect("parse retail faction structures");
        parser
            .parse_ini_content(
                r#"
Object AmericaParticleCannonUplinkNamedButNoModule
  Type = Structure
  KindOf = STRUCTURE SELECTABLE POWERED FS_SUPERWEAPON
End
"#,
                "superweapon_name_spoof.ini",
            )
            .expect("parse name spoof");

        let parsed = |name| {
            GameLogic::build_template_from_object_definition(
                name,
                parser
                    .get_definition(name)
                    .unwrap_or_else(|| panic!("missing retail Object {name}")),
                None,
            )
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
        assert_eq!(spoof.energy_production, None);
        assert!(!spoof.has_superweapon_restriction_link_key());

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.templates.insert(particle.name.clone(), particle);
        logic.templates.insert(scud.name.clone(), scud);
        logic.templates.insert(nuke.name.clone(), nuke);
        logic.templates.insert(spoof.name.clone(), spoof);
        logic.skirmish_rules.limit_superweapons = true;

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
        use crate::command_system::SpecialPowerType as P;
        assert!(
            logic
                .host_object(particle_id)
                .expect("particle object")
                .special_power_cooldowns
                .contains_key(&P::ParticleCannon)
        );
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
        particle_object
            .special_power_cooldowns
            .remove(&P::ParticleCannon);
        particle_object.special_power_cooldown_remaining = 0.0;
        particle_object.refresh_special_power_aggregate_cooldown();
        assert!(logic.is_special_power_ready_for(particle_id, &P::ParticleCannon));
        assert!(!logic.is_special_power_ready_for(spoof_id, &P::ParticleCannon));
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 1);
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
            crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 1);
        assert!(particle_frame.unit_command_buttons().iter().any(|button| {
            let n = button.command_name.to_ascii_lowercase();
            n.contains("particle") && button.enabled
        }));
        logic.select_objects(1, vec![spoof_id]);
        let spoof_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 1);
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
        assert!(
            logic
                .create_object_under_construction("GLABlackMarket", Team::GLA, glam::Vec3::ZERO)
                .is_none(),
            "under-construction Black Market requires Palace"
        );
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

    #[test]
    fn production_exit_modules_preserve_queue_and_default_authority() {
        // Retail-shaped values from FactionBuilding.ini: ChinaBarracks has a
        // QueueProductionExitUpdate (300 ms = 9 logic frames); AmericaWarFactory
        // uses DefaultProductionExitUpdate and therefore has no artificial
        // delay/burst state.  Neither result is inferred from the Object name.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object RetailChinaProducer
  Type = Structure
  KindOf = STRUCTURE
  Behavior = QueueProductionExitUpdate ModuleTag_Exit
    UnitCreatePoint = X:0.0 Y:-25.0 Z:0.0
    NaturalRallyPoint = X:36.0 Y:-25.0 Z:0.0
    ExitDelay = 300
    AllowAirborneCreation = Yes
    InitialBurst = 2
  End
End
Object RetailAmericaProducer
  Type = Structure
  KindOf = STRUCTURE
  Behavior = DefaultProductionExitUpdate ModuleTag_Exit
    UnitCreatePoint = X:-10.0 Y:-30.0 Z:0.0
    NaturalRallyPoint = X:53.0 Y:-30.0 Z:0.0
    UseSpawnRallyPoint = Yes
  End
End
Object RetailSupplyCenterProducer
  Type = Structure
  KindOf = STRUCTURE SUPPLY_CENTER
  Behavior = SupplyCenterProductionExitUpdate ModuleTag_Exit
    UnitCreatePoint = X:0.0 Y:0.0 Z:0.0
    NaturalRallyPoint = X:24.0 Y:0.0 Z:0.0
  End
End
Object BarracksNamedWithoutExitBehavior
  Type = Structure
  KindOf = STRUCTURE
End
"#,
                "production_exit_metadata_probe.ini",
            )
            .expect("parse production exit probes");

        let china = GameLogic::build_template_from_object_definition(
            "RetailChinaProducer",
            parser
                .get_definition("RetailChinaProducer")
                .expect("Queue definition"),
            None,
        );
        let queue = china
            .production_exit_metadata
            .expect("authored QueueProductionExitUpdate");
        assert_eq!(queue.style, ProductionExitStyle::Queue);
        assert_eq!(queue.unit_create_point, [0.0, -25.0, 0.0]);
        assert_eq!(queue.natural_rally_point, [36.0, -25.0, 0.0]);
        assert_eq!(queue.exit_delay_frames, 9);
        assert!(queue.allow_airborne_creation);
        assert_eq!(queue.initial_burst, 2);

        let america = GameLogic::build_template_from_object_definition(
            "RetailAmericaProducer",
            parser
                .get_definition("RetailAmericaProducer")
                .expect("Default definition"),
            None,
        );
        let default_exit = america
            .production_exit_metadata
            .expect("authored DefaultProductionExitUpdate");
        assert_eq!(default_exit.style, ProductionExitStyle::Default);
        assert_eq!(default_exit.unit_create_point, [-10.0, -30.0, 0.0]);
        assert_eq!(default_exit.natural_rally_point, [53.0, -30.0, 0.0]);
        assert_eq!(default_exit.exit_delay_frames, 0);
        assert!(default_exit.use_spawn_rally_point);

        let supply_center = GameLogic::build_template_from_object_definition(
            "RetailSupplyCenterProducer",
            parser
                .get_definition("RetailSupplyCenterProducer")
                .expect("SupplyCenter definition"),
            None,
        );
        let supply_exit = supply_center
            .production_exit_metadata
            .expect("authored SupplyCenterProductionExitUpdate");
        assert_eq!(supply_exit.style, ProductionExitStyle::SupplyCenter);
        assert!(supply_exit.is_supply_center());
        assert_eq!(supply_exit.natural_rally_point, [24.0, 0.0, 0.0]);

        let absent = GameLogic::build_template_from_object_definition(
            "BarracksNamedWithoutExitBehavior",
            parser
                .get_definition("BarracksNamedWithoutExitBehavior")
                .expect("no-exit definition"),
            None,
        );
        assert!(
            absent.production_exit_metadata.is_none(),
            "a producer-shaped basename must not synthesize an exit interface"
        );

        // The runtime seeds retail definitions over a small starter-template
        // catalogue.  Enrichment must preserve this exact Queue interface
        // when the producer already exists; it is not only a newly-created
        // template concern.
        let mut seeded = GameLogic::new();
        seeded.templates.insert(
            "RetailChinaProducer".to_string(),
            crate::game_logic::ThingTemplate::new("RetailChinaProducer"),
        );
        assert_eq!(
            seeded.seed_asset_definition_templates_from_snapshot(vec![(
                "RetailChinaProducer".to_string(),
                parser
                    .get_definition("RetailChinaProducer")
                    .expect("Queue definition for existing template")
                    .clone(),
            )]),
            0,
        );
        assert_eq!(
            seeded
                .templates
                .get("RetailChinaProducer")
                .and_then(|template| template.production_exit_metadata),
            Some(queue),
            "existing starter producers must receive parsed exit metadata"
        );

        // Exercise the actual retail definitions too, including their trailing
        // INI comments and module tags.  These values are the live China Red
        // Guard Queue producer and USA factory Default producer, not a
        // hand-copied template-name fallback.
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let retail =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        let mut retail_parser = crate::assets::IniParser::new();
        retail_parser
            .parse_ini_content(&retail, "FactionBuilding.ini")
            .expect("parse retail production exits");
        let retail_china = GameLogic::build_template_from_object_definition(
            "ChinaBarracks",
            retail_parser
                .get_definition("ChinaBarracks")
                .expect("retail ChinaBarracks"),
            None,
        );
        let retail_queue = retail_china
            .production_exit_metadata
            .expect("retail China QueueProductionExitUpdate");
        assert_eq!(retail_queue.style, ProductionExitStyle::Queue);
        assert_eq!(retail_queue.exit_delay_frames, 9);
        assert_eq!(retail_queue.initial_burst, 0);
        assert_eq!(retail_queue.unit_create_point, [0.0, -25.0, 0.0]);
        assert_eq!(retail_queue.natural_rally_point, [36.0, -25.0, 0.0]);

        let retail_factory = GameLogic::build_template_from_object_definition(
            "AmericaWarFactory",
            retail_parser
                .get_definition("AmericaWarFactory")
                .expect("retail AmericaWarFactory"),
            None,
        );
        let retail_default = retail_factory
            .production_exit_metadata
            .expect("retail DefaultProductionExitUpdate");
        assert_eq!(retail_default.style, ProductionExitStyle::Default);
        assert_eq!(retail_default.unit_create_point, [-10.0, -30.0, 0.0]);
        assert_eq!(retail_default.natural_rally_point, [53.0, -30.0, 0.0]);
    }

    #[test]
    fn retail_refund_value_keeps_zero_fallback_and_exact_override() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object RetailFallbackRefund
  Type = Structure
  Model = RetailFallbackRefundModel
  KindOf = STRUCTURE SELECTABLE
  BuildCost = 1000
  RefundValue = 0
End
Object GLASupplyStash
  Type = Structure
  Model = UBSupply
  KindOf = STRUCTURE SELECTABLE
  BuildCost = 1500
  RefundValue = 650
End
"#,
                "retail_refund_value_probe.ini",
            )
            .expect("parse refund value probe");

        let fallback = GameLogic::build_template_from_object_definition(
            "RetailFallbackRefund",
            parser
                .get_definition("RetailFallbackRefund")
                .expect("fallback definition"),
            None,
        );
        assert_eq!(
            fallback.refund_value, 0,
            "zero retains SellPercentage fallback"
        );

        // Retail GLA Supply Stash uses the exact override rather than the
        // 1500 BuildCost × SellPercentage route.
        let stash = GameLogic::build_template_from_object_definition(
            "GLASupplyStash",
            parser
                .get_definition("GLASupplyStash")
                .expect("GLA Supply Stash definition"),
            None,
        );
        assert_eq!(stash.build_cost.supplies, 1_500);
        assert_eq!(stash.refund_value, 650);
    }

    #[test]
    fn parsed_dock_modules_define_dock_family_without_template_name_matching() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryWarehouseIdentity
  Type = Structure
  Model = ArbitraryWarehouseModel
  KindOf = STRUCTURE SELECTABLE SUPPLY_SOURCE
  Behavior = SupplyWarehouseDockUpdate ModuleTag_06
    StartingBoxes = 400
  End
End
Object ArbitraryFerryIdentity
  Type = Structure
  Model = ArbitraryFerryModel
  KindOf = SELECTABLE TRANSPORT
  Behavior = RailedTransportContain ModuleTag_03
    Slots = 10
  End
  Behavior = RailedTransportDockUpdate ModuleTag_06
    NumberApproachPositions = 9
  End
End
"#,
                "dock_metadata_probe.ini",
            )
            .expect("parse dock metadata probe");

        let warehouse = GameLogic::build_template_from_object_definition(
            "ArbitraryWarehouseIdentity",
            parser
                .get_definition("ArbitraryWarehouseIdentity")
                .expect("warehouse definition"),
            None,
        );
        assert_eq!(warehouse.dock_kind, DockKind::SupplyWarehouse);
        assert_eq!(warehouse.dock_starting_boxes, Some(400));

        let ferry = GameLogic::build_template_from_object_definition(
            "ArbitraryFerryIdentity",
            parser
                .get_definition("ArbitraryFerryIdentity")
                .expect("ferry definition"),
            None,
        );
        assert_eq!(ferry.dock_kind, DockKind::RailedTransport);
        assert_eq!(ferry.railed_transport_slots, Some(10));
        assert!(
            !ferry.is_kind_of(KindOf::Vehicle),
            "the test proves RailedTransportContain is not inferred from VEHICLE"
        );
    }

    #[test]
    fn authored_supply_source_kindof_drives_gather_bridge_not_template_name() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryRetailSupplyIdentity
  Type = Structure
  Model = SupplySourceModel
  KindOf = STRUCTURE SELECTABLE SUPPLY_SOURCE CANNOT_BUILD_NEAR_SUPPLIES
End
Object SupplyNamedButNotAuthored
  Type = Structure
  Model = SupplyLookingModel
  KindOf = STRUCTURE SELECTABLE
End
"#,
                "supply_source_kindof_probe.ini",
            )
            .expect("parse supply source metadata probe");

        let source = GameLogic::build_template_from_object_definition(
            "ArbitraryRetailSupplyIdentity",
            parser
                .get_definition("ArbitraryRetailSupplyIdentity")
                .expect("authored supply source definition"),
            None,
        );
        assert!(source.is_kind_of(KindOf::SupplySource));
        assert!(source.is_kind_of(KindOf::CannotBuildNearSupplies));
        assert!(source.is_kind_of(KindOf::Resource));
        assert!(source.is_kind_of(KindOf::Harvestable));

        let lookalike = GameLogic::build_template_from_object_definition(
            "SupplyNamedButNotAuthored",
            parser
                .get_definition("SupplyNamedButNotAuthored")
                .expect("supply-looking definition"),
            None,
        );
        assert!(!lookalike.is_kind_of(KindOf::SupplySource));
        assert!(!lookalike.is_kind_of(KindOf::CannotBuildNearSupplies));
        assert!(!lookalike.is_kind_of(KindOf::Resource));
        assert!(!lookalike.is_kind_of(KindOf::Harvestable));

        // Actual retail map supply objects declare the same capability.  The
        // parser must retain it rather than relying on their familiar names.
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let retail = std::fs::read_to_string(repo_root.join(
            "windows_game/extracted_big_files_v2/INI/Object/CivilianBuilding.ini",
        ))
        .expect("retail CivilianBuilding.ini");
        let mut retail_parser = crate::assets::IniParser::new();
        retail_parser
            .parse_ini_content(&retail, "CivilianBuilding.ini")
            .expect("parse retail CivilianBuilding.ini");
        for template_name in ["SupplyDock", "SupplyPile", "SupplyPileSmall"] {
            let template = GameLogic::build_template_from_object_definition(
                template_name,
                retail_parser
                    .get_definition(template_name)
                    .expect("retail supply source definition"),
                None,
            );
            assert!(
                template.is_kind_of(KindOf::SupplySource),
                "{template_name} must retain retail KINDOF_SUPPLY_SOURCE"
            );
        }
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        retail_parser
            .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
            .expect("parse retail FactionBuilding.ini");
        for template_name in ["AmericaSupplyCenter", "GLASupplyStash", "ChinaSupplyCenter"] {
            let template = GameLogic::build_template_from_object_definition(
                template_name,
                retail_parser
                    .get_definition(template_name)
                    .expect("retail supply-center definition"),
                None,
            );
            assert!(
                template.is_kind_of(KindOf::CannotBuildNearSupplies),
                "{template_name} must retain retail KINDOF_CANNOT_BUILD_NEAR_SUPPLIES"
            );
        }

        // Normal offline boot already has a hand-authored starter template
        // before the retail Object catalogue enriches it.  Enrichment must
        // carry this exact build rule too; otherwise headless games would
        // regress to a name-based exception even though parsed data is live.
        let mut seeded = GameLogic::new();
        seeded.templates.insert(
            "AmericaSupplyCenter".to_string(),
            ThingTemplate::new("AmericaSupplyCenter"),
        );
        assert_eq!(
            seeded.seed_asset_definition_templates_from_snapshot(vec![(
                "AmericaSupplyCenter".to_string(),
                retail_parser
                    .get_definition("AmericaSupplyCenter")
                    .expect("retail America supply center definition")
                    .clone(),
            )]),
            0,
            "the existing starter template is enriched rather than replaced"
        );
        assert!(
            seeded
                .templates
                .get("AmericaSupplyCenter")
                .is_some_and(|template| template.is_kind_of(KindOf::CannotBuildNearSupplies))
        );
    }

    #[test]
    fn parsed_capture_modules_keep_power_target_and_garrison_semantics() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryCaptureSourceIdentity
  Type = Infantry
  Model = ArbitraryCaptureSourceModel
  KindOf = INFANTRY SELECTABLE
  Behavior = SpecialAbility ModuleTag_Capture
    SpecialPowerTemplate = SpecialAbilityRebelCaptureBuilding
    StartsPaused = Yes
  End
  Behavior = SpecialAbilityUpdate ModuleTag_CaptureUpdate
    SpecialPowerTemplate = SpecialAbilityRebelCaptureBuilding
    StartAbilityRange = 5.0
    UnpackTime = 3000
    PreparationTime = 20000
    PackTime = 2000
  End
  Behavior = UnpauseSpecialPowerUpgrade ModuleTag_CaptureUpgrade
    SpecialPowerTemplate = SpecialAbilityRebelCaptureBuilding
    TriggeredBy = Upgrade_InfantryCaptureBuilding
  End
End
Object ArbitraryCaptureTargetIdentity
  Type = Structure
  Model = ArbitraryCaptureTargetModel
  KindOf = STRUCTURE SELECTABLE CAPTURABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
  End
End
Object ArbitraryImmuneTargetIdentity
  Type = Structure
  Model = ArbitraryImmuneTargetModel
  KindOf = STRUCTURE SELECTABLE IMMUNE_TO_CAPTURE
End
"#,
                "capture_metadata_probe.ini",
            )
            .expect("parse capture metadata probe");

        let source = GameLogic::build_template_from_object_definition(
            "ArbitraryCaptureSourceIdentity",
            parser
                .get_definition("ArbitraryCaptureSourceIdentity")
                .expect("capture source definition"),
            None,
        );
        assert_eq!(source.capture_power, CapturePowerKind::Rebel);
        assert!(source.capture_starts_paused);
        assert_eq!(
            source.capture_upgrade_trigger.as_deref(),
            Some("Upgrade_InfantryCaptureBuilding")
        );
        assert_eq!(source.capture_start_ability_range, Some(5.0));
        assert_eq!(source.capture_unpack_time_ms, Some(3_000));
        assert_eq!(source.capture_preparation_time_ms, Some(20_000));
        assert_eq!(source.capture_pack_time_ms, Some(2_000));

        let target = GameLogic::build_template_from_object_definition(
            "ArbitraryCaptureTargetIdentity",
            parser
                .get_definition("ArbitraryCaptureTargetIdentity")
                .expect("capture target definition"),
            None,
        );
        assert!(target.capturable);
        assert!(!target.immune_to_capture);
        assert_eq!(target.garrison_contain_max, Some(5));

        let immune = GameLogic::build_template_from_object_definition(
            "ArbitraryImmuneTargetIdentity",
            parser
                .get_definition("ArbitraryImmuneTargetIdentity")
                .expect("immune target definition"),
            None,
        );
        assert!(!immune.capturable);
        assert!(immune.immune_to_capture);
    }

    #[test]
    fn parsed_charge_plant_keeps_unpack_variation_and_flee() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryBurtonChargeIdentity
  Type = Infantry
  Model = ArbitraryBurtonChargeModel
  KindOf = INFANTRY SELECTABLE
  Behavior = SpecialAbilityUpdate ModuleTag_Timed
    SpecialPowerTemplate = SpecialAbilityColonelBurtonTimedCharges
    UnpackTime = 5500
    PackTime = 0
    PackUnpackVariationFactor = 0.2
    FleeRangeAfterCompletion = 100
    FlipOwnerAfterUnpacking = Yes
  End
  Behavior = SpecialAbilityUpdate ModuleTag_Remote
    SpecialPowerTemplate = SpecialAbilityColonelBurtonRemoteCharges
    UnpackTime = 5500
    PackUnpackVariationFactor = 0.2
    FleeRangeAfterCompletion = 100
    FlipOwnerAfterUnpacking = Yes
  End
End
"#,
                "charge_plant_metadata_probe.ini",
            )
            .expect("parse charge plant metadata probe");

        let source = GameLogic::build_template_from_object_definition(
            "ArbitraryBurtonChargeIdentity",
            parser
                .get_definition("ArbitraryBurtonChargeIdentity")
                .expect("charge source definition"),
            None,
        );
        let timed = source
            .charge_plant_ability_for_timed()
            .expect("timed C4 update");
        assert_eq!(timed.unpack_time_ms, 5_500);
        assert_eq!(timed.pack_unpack_variation_factor, 0.2);
        assert_eq!(timed.flee_range_after_completion, 100.0);
        assert!(timed.flip_object_after_unpacking);
        let remote = source
            .charge_plant_ability_for_remote()
            .expect("remote C4 update");
        assert_eq!(remote.unpack_time_ms, 5_500);
        assert_eq!(remote.pack_unpack_variation_factor, 0.2);
        assert_eq!(remote.flee_range_after_completion, 100.0);
    }

    #[test]
    fn burton_charge_unpacks_then_plants_then_flees() {
        // SpecialAbilityUpdate.cpp:397-441 unpack, trigger, finishAbility flee.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object BurtonChargeUnpackProbe
  Type = Infantry
  KindOf = INFANTRY SELECTABLE CAN_ATTACK
  Behavior = SpecialAbilityUpdate ModuleTag_Timed
    SpecialPowerTemplate = SpecialAbilityColonelBurtonTimedCharges
    UnpackTime = 200
    PackUnpackVariationFactor = 0
    FleeRangeAfterCompletion = 100
    FlipOwnerAfterUnpacking = Yes
  End
End
Object ChargePlantTargetProbe
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
End
"#,
                "burton_charge_live_probe.ini",
            )
            .expect("parse live charge probe");

        let mut logic = GameLogic::new();
        let burton_tpl = GameLogic::build_template_from_object_definition(
            "BurtonChargeUnpackProbe",
            parser
                .get_definition("BurtonChargeUnpackProbe")
                .expect("burton probe"),
            None,
        );
        assert_eq!(
            burton_tpl
                .charge_plant_ability_for_timed()
                .map(|m| m.unpack_time_ms),
            Some(200)
        );
        logic
            .templates
            .insert("BurtonChargeUnpackProbe".into(), burton_tpl);
        let mut target_tpl = GameLogic::build_template_from_object_definition(
            "ChargePlantTargetProbe",
            parser
                .get_definition("ChargePlantTargetProbe")
                .expect("target probe"),
            None,
        );
        target_tpl.set_health(500.0);
        logic
            .templates
            .insert("ChargePlantTargetProbe".into(), target_tpl);

        let burton_id = logic
            .create_object(
                "BurtonChargeUnpackProbe",
                Team::USA,
                Vec3::new(2.0, 0.0, 0.0),
            )
            .expect("burton");
        let target_id = logic
            .create_object(
                "ChargePlantTargetProbe",
                Team::GLA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("target");
        {
            let burton = logic.host_object_mut(burton_id).expect("burton mut");
            burton.set_ai_state(AIState::SpecialAbility);
            burton.set_target(Some(target_id));
        }
        logic.queue_pending_special_ability(
            burton_id,
            PendingSpecialAbility::PlantTimedDemoCharge { target_id },
        );

        logic.update_ai(&[burton_id, target_id], 1.0 / 60.0);
        assert_eq!(
            logic.mine_residual_places(),
            0,
            "UnpackTime 200ms must delay plant"
        );

        for _ in 0..20 {
            logic.update_ai(&[burton_id, target_id], 1.0 / 60.0);
        }
        assert!(
            logic.mine_residual_places() >= 1,
            "charge plants after UnpackTime"
        );
        let pos = logic
            .host_object(burton_id)
            .expect("burton after plant")
            .get_position();
        assert!(
            pos.distance(Vec3::new(2.0, 0.0, 0.0)) > 1.0,
            "finishAbility flee after plant, pos={pos:?}"
        );
    }

    #[test]
    fn temporary_weapon_behavior_metadata_is_source_ordered_and_not_live_state() {
        use crate::game_logic::host_temporary_weapon_behavior::{
            FireWeaponWhenDamagedWeaponRole, TemporaryWeaponSlot,
        };

        let source = r#"
Object TemporaryWeaponContractProbe
  Behavior = FireWeaponWhenDamagedBehavior ModuleTag_Damage
    StartsActive = Yes
    DamageTypes = NONE +FLAME +POISON -FLAME
    DamageAmount = 2.5
    ReactionWeaponDamaged = SharedTempWeapon
    ContinuousWeaponDamaged = SharedTempWeapon
    TriggeredBy = Upgrade_A Upgrade_B
    ConflictsWith = Upgrade_C
    RemovesUpgrades = Upgrade_D
    FXListUpgrade = UpgradeFX
    RequiresAllTriggers = Yes
  End
  Behavior = FireWeaponWhenDeadBehavior ModuleTag_Death
    StartsActive = No
    DeathWeapon = DeathTempWeapon
    DeathTypes = NONE +EXPLODED
    VeterancyLevels = NONE +VETERAN
    ExemptStatus = +UNDER_CONSTRUCTION
    RequiredStatus = DEPLOYED
  End
End
"#;
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(source, "temporary_weapon_contract_probe.ini")
            .expect("parse source behavior modules");
        let template = GameLogic::build_template_from_object_definition(
            "TemporaryWeaponContractProbe",
            parser
                .get_definition("TemporaryWeaponContractProbe")
                .expect("source definition"),
            None,
        );

        assert_eq!(template.fire_weapon_when_damaged_behaviors.len(), 1);
        assert_eq!(template.fire_weapon_when_dead_behaviors.len(), 1);
        let damaged = &template.fire_weapon_when_damaged_behaviors[0];
        assert_eq!(damaged.module_source_index, 0);
        assert_eq!(damaged.module_tag.as_deref(), Some("ModuleTag_Damage"));
        assert!(damaged.starts_active);
        assert_eq!(damaged.damage_types.0, 1u64 << 9, "POISON only");
        assert_eq!(damaged.damage_amount, 2.5);
        assert_eq!(damaged.upgrade_mux.triggered_by, ["Upgrade_A", "Upgrade_B"]);
        let specs = damaged.runtime_specs();
        assert_eq!(specs.len(), 2);
        assert_ne!(specs[0].key, specs[1].key);
        assert_eq!(
            specs[0].key.role,
            FireWeaponWhenDamagedWeaponRole::ReactionDamaged
        );
        assert_eq!(
            specs[1].key.role,
            FireWeaponWhenDamagedWeaponRole::ContinuousDamaged
        );
        assert!(
            specs
                .iter()
                .all(|spec| spec.weapon_slot == TemporaryWeaponSlot::Primary)
        );

        let dead = &template.fire_weapon_when_dead_behaviors[0];
        assert_eq!(dead.module_source_index, 1);
        assert_eq!(dead.module_tag.as_deref(), Some("ModuleTag_Death"));
        assert!(!dead.starts_active);
        assert_eq!(dead.death_weapon.as_deref(), Some("DeathTempWeapon"));
        assert_eq!(dead.death_types.0, 1u32 << 4);
        assert_eq!(dead.veterancy_levels.0, 1u8 << 1);
        assert!(dead.die_mux_allows(4, 1, 1u64 << 44));
        assert_eq!(
            dead.ephemeral_weapon_spec()
                .expect("C++ creates a fresh death weapon")
                .weapon_slot,
            TemporaryWeaponSlot::Primary
        );

        // The template retains source contracts only.  There is deliberately
        // no object-owned damaged-behavior runtime here until a versioned
        // ObjectSnapshot tail can preserve all eight C++ Weapon Xfers.
        assert!(
            template
                .fire_weapon_when_damaged_behaviors
                .iter()
                .all(|metadata| !metadata.runtime_specs().is_empty())
        );
    }

    #[test]
    fn exact_retail_catalogue_seed_preserves_data_and_never_overwrites_curated_templates() {
        let mut logic = GameLogic::new();
        let mut curated = ThingTemplate::new("AmericaTankCrusader");
        curated.set_health(777.0).set_model("CuratedExactModel");
        logic
            .templates
            .insert("AmericaTankCrusader".to_string(), curated);

        let mut retail_unit = ObjectDefinition::new("AmericaTankCrusader".to_string());
        retail_unit.object_type = "Vehicle".to_string();
        retail_unit.hit_points = Some(480.0);
        retail_unit.model_name = Some("AVCrusader".to_string());
        retail_unit.scale = 0.9;
        retail_unit.scale_was_specified = true;
        retail_unit.attributes.insert(
            "KindOf".to_string(),
            "VEHICLE SELECTABLE CAN_ATTACK".to_string(),
        );
        retail_unit
            .attributes
            .insert("BuildCost".to_string(), "900".to_string());

        let mut retail_new = ObjectDefinition::new("AmericaTankPaladin".to_string());
        retail_new.object_type = "Vehicle".to_string();
        retail_new.hit_points = Some(600.0);
        retail_new.model_name = Some("AVPaladin".to_string());
        retail_new.scale = 0.66;
        retail_new.scale_was_specified = true;
        retail_new.primary_weapon = Some("PaladinTankGun".to_string());
        retail_new.secondary_weapon = Some("PaladinPointDefenseLaser".to_string());
        retail_new.attributes.insert(
            "KindOf".to_string(),
            "VEHICLE SELECTABLE CAN_ATTACK".to_string(),
        );
        retail_new
            .attributes
            .insert("BuildCost".to_string(), "1100".to_string());

        let mut sound_anchor = ObjectDefinition::new("AmbientOnlyRetailAnchor".to_string());
        sound_anchor
            .attributes
            .insert("SoundAmbient".to_string(), "AmbientWind".to_string());

        let inserted = logic.seed_asset_definition_templates_from_snapshot(vec![
            ("AmericaTankPaladin".to_string(), retail_new),
            ("AmericaTankCrusader".to_string(), retail_unit),
            ("AmbientOnlyRetailAnchor".to_string(), sound_anchor),
        ]);

        assert_eq!(inserted, 2);
        let curated_after = logic
            .templates
            .get("AmericaTankCrusader")
            .expect("curated template retained");
        assert_eq!(curated_after.max_health, 777.0);
        assert_eq!(
            curated_after.model_name.as_deref(),
            Some("CuratedExactModel")
        );
        assert!((curated_after.asset_scale - 0.9).abs() < f32::EPSILON);

        let added = logic
            .templates
            .get("AmericaTankPaladin")
            .expect("retail definition seeded");
        assert_eq!(added.max_health, 600.0);
        assert_eq!(added.build_cost.supplies, 1100);
        assert_eq!(added.model_name.as_deref(), Some("AVPaladin"));
        assert!((added.asset_scale - 0.66).abs() < f32::EPSILON);
        assert_eq!(added.primary_weapon_name.as_deref(), Some("PaladinTankGun"));
        assert_eq!(
            added.secondary_weapon_name.as_deref(),
            Some("PaladinPointDefenseLaser")
        );
        assert!(added.is_kind_of(KindOf::Vehicle));
        let ambient = logic
            .templates
            .get("AmbientOnlyRetailAnchor")
            .expect("SoundAmbient-only map object is now a live template");
        assert_eq!(ambient.sound_ambient.as_deref(), Some("AmbientWind"));
        assert!(ambient.model_name.is_none());
    }

    #[test]
    fn pristine_models_do_not_match_condition_or_faction_variants() {
        let pairs = [
            ("ABPWRPLANT", "ABPWRPLANT_d06"),
            ("ABBarracks", "ABBarracks_FA"),
            ("ABPatriot", "ABPatriotSW"),
            ("NBConYard", "NBConYard_FA"),
            ("UBCmdHQ", "UBArFrcCmd"),
            ("PTDogwod01", "PTDogwod01_S"),
        ];

        for (pristine, wrong_variant) in pairs {
            assert_eq!(
                GameLogic::find_exact_available_model_name(
                    pristine,
                    vec![format!("{wrong_variant}.W3D")].into_iter(),
                ),
                None,
                "{pristine} must not select distinct ConditionState/faction asset {wrong_variant}"
            );
            assert_eq!(
                GameLogic::find_exact_available_model_name(
                    pristine,
                    vec![format!("Art/W3D/{pristine}.W3D")].into_iter(),
                ),
                Some(pristine.to_string()),
                "{pristine} must retain exact retail basename lookup"
            );
        }
    }

    #[test]
    fn hand_authored_unit_templates_keep_their_verified_retail_w3d_identity() {
        let mut logic = GameLogic::new();
        logic.setup_templates();

        // Each value is the DefaultConditionState `Model` for the named
        // retail Object INI identity and an exact W3DZH.big basename.  Keep
        // this table explicit: a visual fallback must be unavailable rather
        // than silently turning one game unit into another.
        let expected_models = [
            ("USA_Ranger", "airngr_skn"),
            ("USA_MissileDefender", "nithnt_skn"),
            ("USA_CrusaderTank", "avleopard"),
            ("USA_PaladinTank", "avpaladin"),
            ("USA_Raptor", "avraptor"),
            ("GLA_Soldier", "uirgrd_skn"),
            ("GLA_RPGTrooper", "uitunf_skn"),
            ("GLA_Technical", "uvtechtrck"),
            ("GLA_ScorpionTank", "uvlitetank"),
            ("GLA_MarauderTank", "uvmarauder"),
            ("GLAInfantryTerrorist", "uitrst_skn"),
            ("GLAVehicleCombatBike", "uvcombike"),
            ("China_RedGuard", "nicnsc_skn"),
            ("China_TankHunter", "nimsst_skn"),
            ("China_BattlemasterTank", "nvbtmstr"),
            ("China_OverlordTank", "nvovrlrd"),
            ("China_InfernoCannon", "nvinferno"),
            ("China_MiG", "nvmig"),
            ("China_Helix", "nvhelix"),
        ];

        for (template_name, expected_model) in expected_models {
            let template = logic
                .templates
                .get(template_name)
                .unwrap_or_else(|| panic!("missing curated template {template_name}"));
            assert_eq!(
                template.model_name.as_deref(),
                Some(expected_model),
                "{template_name} must keep its own retail visual identity"
            );
        }

        // C++ WeaponSet classifies airborne targets through their gameplay
        // KindOf, not their Drawable model.  These curated starters must
        // therefore retain the authored VEHICLE bit as well as AIRCRAFT.
        for template_name in ["USA_Raptor", "China_MiG", "China_Helix"] {
            let template = logic
                .templates
                .get(template_name)
                .unwrap_or_else(|| panic!("missing curated aircraft {template_name}"));
            assert!(template.is_kind_of(KindOf::Vehicle));
            assert!(template.is_kind_of(KindOf::Aircraft));
        }
    }

    #[test]
    fn create_object_uses_ini_create_policy_not_unit_name_hardcodes() {
        // C++ Object.cpp:160-497 / GameLogic::friend_createObject: Object ctor
        // builds from ThingTemplate INI and each module's onObjectCreated.
        // Strategy Center AutoChooseSources=PRIMARY NONE is FactionBuilding.ini:6970;
        // Quad Cannon AntiGround/AntiAirborne is Weapon.ini:2637-2660;
        // Toxin Tractor FireOCLAfterWeaponCooldownUpdate is GLAVehicle.ini:3697.
        // create_object must honor those template/module bits. Name residuals
        // remain only when Object INI was never loaded (unit tests).
        use glam::Vec3;

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object AmericaStrategyCenter
  Type = Building
  KindOf = PRELOAD STRUCTURE SELECTABLE IMMOBILE FS_STRATEGY_CENTER
  WeaponSet
    Conditions = None
    Weapon = PRIMARY StrategyCenterGun
    AutoChooseSources = PRIMARY NONE
  End
End
Object GLAVehicleQuadCannon
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  WeaponSet
    Conditions = None
    Weapon = PRIMARY QuadCannonGun
    Weapon = SECONDARY QuadCannonGunAir
  End
End
Object GLAVehicleToxinTruck
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  WeaponSet
    Conditions = None
    Weapon = PRIMARY ToxinTruckGun
    Weapon = SECONDARY ToxinTruckSprayer
    AutoChooseSources = SECONDARY NONE
  End
  Behavior = FireOCLAfterWeaponCooldownUpdate ModuleTag_13
    WeaponSlot = SECONDARY
    OCL = OCL_PoisonFieldMedium
    MinShotsToCreateOCL = 4
  End
End
Object ChemSpillSprayer
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  Behavior = FireOCLAfterWeaponCooldownUpdate ModuleTag_01
    WeaponSlot = SECONDARY
    OCL = OCL_PoisonFieldMedium
  End
End
Object OrdinaryAttackableBunker
  Type = Building
  KindOf = STRUCTURE SELECTABLE CAN_ATTACK
End
"#,
                "create_policy_probe.ini",
            )
            .expect("parse create-policy probes");

        let parsed = |name: &str| {
            GameLogic::build_template_from_object_definition(
                name,
                parser
                    .get_definition(name)
                    .unwrap_or_else(|| panic!("missing probe Object {name}")),
                None,
            )
        };

        let strategy = parsed("AmericaStrategyCenter");
        assert!(
            strategy.primary_auto_choose_none,
            "authored AutoChooseSources=PRIMARY NONE must land on the template"
        );
        assert!(!strategy.has_fire_ocl_after_weapon_cooldown);
        assert_eq!(
            strategy.primary_weapon_name.as_deref(),
            Some("StrategyCenterGun")
        );

        let toxin = parsed("GLAVehicleToxinTruck");
        assert!(
            toxin.has_fire_ocl_after_weapon_cooldown,
            "FireOCLAfterWeaponCooldownUpdate must install from the Behavior, not the unit name"
        );
        assert!(!toxin.primary_auto_choose_none);

        let spill = parsed("ChemSpillSprayer");
        assert!(
            spill.has_fire_ocl_after_weapon_cooldown,
            "a non-toxin-tractor name with the module must still carry FireOCL create policy"
        );

        let bunker = parsed("OrdinaryAttackableBunker");
        assert!(
            !bunker.primary_auto_choose_none,
            "CAN_ATTACK alone must not invent AutoChooseSources=PRIMARY NONE"
        );

        crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();

        let mut logic = GameLogic::new();
        for name in [
            "AmericaStrategyCenter",
            "GLAVehicleQuadCannon",
            "GLAVehicleToxinTruck",
            "ChemSpillSprayer",
            "OrdinaryAttackableBunker",
        ] {
            logic.templates.insert(name.to_string(), parsed(name));
        }

        let center_id = logic
            .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("strategy center");
        {
            let center = logic.host_object(center_id).expect("center");
            if let Some(weapon) = center.weapon.as_ref() {
                assert!(
                    (weapon.damage - 25.0).abs() > 0.01 || (weapon.range - 100.0).abs() > 0.01,
                    "AutoChooseSources=PRIMARY NONE must not invent Weapon::default"
                );
            }
        }

        let bunker_id = logic
            .create_object(
                "OrdinaryAttackableBunker",
                Team::USA,
                Vec3::new(10.0, 0.0, 0.0),
            )
            .expect("bunker");
        {
            let bunker = logic.host_object(bunker_id).expect("bunker");
            let weapon = bunker
                .weapon
                .as_ref()
                .expect("kind-based default remains when Object INI has no AutoChoose NONE");
            assert!((weapon.damage - 25.0).abs() < 0.01);
        }

        let spill_id = logic
            .create_object("ChemSpillSprayer", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
            .expect("spill sprayer");
        {
            let spill = logic.host_object(spill_id).expect("spill");
            assert!(
                spill.fire_ocl_after_cooldown.is_some(),
                "FireOCL module data must install without a toxin-tractor template name"
            );
            assert!(
                spill.secondary_weapon.is_none(),
                "non-toxin FireOCL must not invent ToxinTruckSprayer"
            );
        }

        let toxin_id = logic
            .create_object("GLAVehicleToxinTruck", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
            .expect("toxin");
        assert!(
            logic
                .host_object(toxin_id)
                .expect("toxin")
                .fire_ocl_after_cooldown
                .is_some()
        );

        let quad_id = logic
            .create_object("GLAVehicleQuadCannon", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
            .expect("quad");
        {
            let quad = logic.host_object(quad_id).expect("quad");
            if let Some(primary) = quad.weapon.as_ref() {
                assert!(primary.can_target_ground);
                assert!(!primary.can_target_air);
            }
            if let Some(secondary) = quad.secondary_weapon.as_ref() {
                assert!(secondary.can_target_air);
                assert!(!secondary.can_target_ground);
            }
        }

        // Missing-INI fallback: name residual still binds when Object INI
        // never set has_fire_ocl_after_weapon_cooldown.
        let mut fallback = ThingTemplate::new("TestToxinTruck");
        fallback
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .set_health(240.0);
        assert!(!fallback.has_fire_ocl_after_weapon_cooldown);
        logic
            .templates
            .insert("TestToxinTruck".to_string(), fallback);
        let fallback_id = logic
            .create_object("TestToxinTruck", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
            .expect("missing-INI toxin");
        {
            let fallback = logic.host_object(fallback_id).expect("fallback toxin");
            assert!(
                fallback.fire_ocl_after_cooldown.is_some(),
                "missing-INI toxin name still installs FireOCL residual"
            );
            assert!(
                fallback.secondary_weapon.is_some(),
                "missing-INI toxin name still binds spray residual"
            );
        }
    }

    #[test]
    fn create_object_uses_already_loaded_host_templates_not_thing_factory() {
        // C++ ThingFactory.cpp findTemplate is an in-memory hash after
        // GameEngine::init loaded Object INI once. Rust
        // TheThingFactory::find_template lazy-inits every Object INI (14s+
        // on Lone Eagle). Host create_object must bind already-loaded
        // self.templates only.
        let create_src = include_str!("../create_destroy_die.rs");
        assert!(
            create_src.contains("fn ensure_host_spawn_template"),
            "create_object must resolve host catalog via ensure_host_spawn_template"
        );
        assert!(
            !create_src.contains("TheThingFactory::find_template("),
            "create_object must not call TheThingFactory::find_template"
        );
        assert!(
            !create_src.contains("init_thing_factory("),
            "create_object must not lazy-init ThingFactory"
        );

        let mut logic = GameLogic::new();
        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let exact = logic
            .create_object("USA_Ranger", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("exact host template");
        assert_eq!(
            logic.host_object(exact).expect("exact").template_name,
            "USA_Ranger"
        );

        let aliased = logic
            .create_object("usa_ranger", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("case-insensitive already-loaded host template");
        assert_eq!(
            logic.host_object(aliased).expect("alias").template_name,
            "USA_Ranger"
        );

        let cached_miss = "CachedMissPropThatMustNotResynthesize";
        logic
            .unresolved_spawn_templates
            .insert(cached_miss.to_string());
        let before = logic.templates.len();
        assert!(
            logic
                .create_object(cached_miss, Team::Neutral, glam::Vec3::ZERO)
                .is_none()
        );
        assert_eq!(
            logic.templates.len(),
            before,
            "cached unresolved names must not re-enter template synthesis"
        );

        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        assert_eq!(
            crate::game_logic::weapon_bootstrap::ensure_host_weapon_store(),
            0,
            "weapon store must not reload after first host seed"
        );
        let _ = crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store();
        assert_eq!(
            crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store(),
            0,
            "locomotor store must not reload after first host seed"
        );
    }

    #[test]
    fn heal_cave_tunnel_contain_kinds_map_and_heal_contain_auto_exits() {
        use glam::Vec3;

        // C++ HealContain.cpp:68-157 heals then auto-exits.
        // C++ TunnelTracker.cpp:225-268 healObjects sliver / snap-to-max.
        // Pre-fix: spawn_templates mapped Heal/Cave/Tunnel to ContainModuleKind::None.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeHealContainBarracks
  Type = Structure
  KindOf = STRUCTURE SELECTABLE HEAL_PAD
  Body = ActiveBody ModuleTag_01
    MaxHealth = 1000.0
  End
  Behavior = HealContain ModuleTag_Heal
    ContainMax = 10
    TimeForFullHeal = 2000
    AllowInsideKindOf = INFANTRY
    AllowAlliesInside = Yes
    AllowEnemiesInside = No
    AllowNeutralInside = No
  End
End
Object ProbeCaveContain
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = CaveContain ModuleTag_Cave
    ContainMax = 10
    CaveIndex = 4
  End
End
Object ProbeTunnelContain
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = TunnelContain ModuleTag_Tunnel
    TimeForFullHeal = 5000
  End
End
Object ProbeHealInfantry
  Type = Infantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 80.0
  End
End

"#,
                "heal_cave_tunnel_contain_probe.ini",
            )
            .expect("parse contain kind probe");

        let barracks = GameLogic::build_template_from_object_definition(
            "ProbeHealContainBarracks",
            parser
                .get_definition("ProbeHealContainBarracks")
                .expect("heal barracks"),
            None,
        );
        assert_eq!(barracks.contain_module.kind, ContainModuleKind::Heal);
        assert_eq!(barracks.contain_module.slots, Some(10));
        assert_eq!(barracks.contain_module.frames_for_full_heal, Some(60));
        assert_eq!(
            barracks.contain_module.admission,
            ContainAdmission::InfantryOnly
        );
        assert!(barracks.contain_module.allow_allies_inside);
        assert!(!barracks.contain_module.allow_enemies_inside);
        assert!(!barracks.contain_module.allow_neutral_inside);
        assert!(barracks.garrison_contain_max.is_none());

        let cave = GameLogic::build_template_from_object_definition(
            "ProbeCaveContain",
            parser.get_definition("ProbeCaveContain").expect("cave"),
            None,
        );
        assert_eq!(cave.contain_module.kind, ContainModuleKind::Cave);
        assert_eq!(cave.contain_module.slots, Some(10));
        assert_eq!(cave.contain_module.cave_index, 4);

        let tunnel = GameLogic::build_template_from_object_definition(
            "ProbeTunnelContain",
            parser.get_definition("ProbeTunnelContain").expect("tunnel"),
            None,
        );
        assert_eq!(tunnel.contain_module.kind, ContainModuleKind::Tunnel);
        assert_eq!(tunnel.contain_module.frames_for_full_heal, Some(150));

        let infantry = GameLogic::build_template_from_object_definition(
            "ProbeHealInfantry",
            parser
                .get_definition("ProbeHealInfantry")
                .expect("infantry"),
            None,
        );

        let mut logic = GameLogic::new();
        let usa = Player::new(1, Team::USA, "USA", true);
        logic.add_player(usa);
        logic
            .templates
            .insert("ProbeHealContainBarracks".to_string(), barracks);
        logic
            .templates
            .insert("ProbeHealInfantry".to_string(), infantry);
        logic
            .templates
            .insert("ProbeTunnelContain".to_string(), tunnel);

        let pad = logic
            .create_object_for_player("ProbeHealContainBarracks", 1, Vec3::ZERO)
            .expect("heal barracks");
        let unit = logic
            .create_object_for_player("ProbeHealInfantry", 1, Vec3::ZERO)
            .expect("infantry");
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 20.0;
            obj.health.maximum = 80.0;
        }

        assert!(
            logic.can_unit_enter_normal_target(unit, pad),
            "damaged infantry must enter HealContain"
        );
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 80.0;
        }
        assert!(
            !logic.can_unit_enter_normal_target(unit, pad),
            "C++ ActionManager.cpp:636-644 rejects full-health HealContain enter"
        );
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 20.0;
            obj.set_ai_state(AIState::Entering);
            obj.target = Some(pad);
        }

        logic.frame = 10;
        logic.update_support_states(&[unit], 1.0 / 30.0);
        assert_eq!(
            logic.host_object(unit).and_then(|o| o.contained_by),
            Some(pad)
        );
        let after_enter = logic
            .host_object(unit)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        // First contained update applies max/60 sliver (HealContain.cpp:148).
        assert!(
            (after_enter - (20.0 + 80.0 / 60.0)).abs() < 0.05 || after_enter > 20.0,
            "HealContain must sliver-heal, got {after_enter}"
        );

        for _ in 0..60 {
            logic.frame = logic.frame.saturating_add(1);
            logic.update_support_states(&[unit], 1.0 / 30.0);
        }
        let after = logic.host_object(unit).expect("unit after heal");
        assert!(
            after.health.current >= after.health.maximum - 0.01,
            "HealContain must finish at max health"
        );
        assert!(
            after.contained_by.is_none(),
            "HealContain.cpp:97-101 auto-exits when done healing"
        );
        assert!(logic.tunnel_network.honesty_heal_contain_auto_exit_ok());

        let tunnel_id = logic
            .create_object_for_player("ProbeTunnelContain", 1, Vec3::new(40.0, 0.0, 0.0))
            .expect("tunnel");
        let rider = logic
            .create_object_for_player("ProbeHealInfantry", 1, Vec3::new(40.0, 0.0, 0.0))
            .expect("tunnel rider");
        if let Some(obj) = logic.host_object_mut(rider) {
            obj.health.current = 30.0;
            obj.health.maximum = 80.0;
        }
        assert!(logic.tunnel_network.record_enter(
            crate::game_logic::host_tunnel_network::tunnel_system_key(Some(1), Team::USA),
            rider,
            tunnel_id,
        ));
        logic
            .tunnel_network
            .stamp_contained_by_frame(rider, logic.frame);
        let before_tunnel = logic
            .host_object(rider)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        logic.frame = logic.frame.saturating_add(1);
        logic.update_support_states(&[rider], 1.0 / 30.0);
        let after_tunnel = logic
            .host_object(rider)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        assert!(
            after_tunnel > before_tunnel,
            "TunnelTracker::healObjects must sliver-heal, {before_tunnel} -> {after_tunnel}"
        );
        assert!(logic.tunnel_network.honesty_heal_objects_ok());
    }

    #[test]
    fn chinook_forbid_huge_vehicle_does_not_fail_close_enter() {
        use crate::game_logic::{ContainAdmission, ContainModuleKind};
        use game_engine::common::system::kind_of::KindOfMask;
        use glam::Vec3;

        // C++ OpenContain.cpp:856-866 + TransportContain.cpp:136-193:
        // AllowInsideKindOf = INFANTRY VEHICLE,
        // ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE.
        // Infantry and Humvees board; Overlord (HUGE_VEHICLE) does not.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeCombatChinook
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE TRANSPORT
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 300.0
  End
  Behavior = TransportContain ModuleTag_01
    Slots = 8
    AllowInsideKindOf = INFANTRY VEHICLE
    ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE
  End
End
Object ProbeChinookRanger
  Type = Infantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 100.0
  End
End
Object ProbeChinookHumvee
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 200.0
  End
End
Object ProbeChinookOverlord
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE HUGE_VEHICLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 1100.0
  End
End
"#,
                "chinook_huge_vehicle_enter.ini",
            )
            .expect("parse chinook huge-vehicle probe");

        let chinook = GameLogic::build_template_from_object_definition(
            "ProbeCombatChinook",
            parser
                .get_definition("ProbeCombatChinook")
                .expect("chinook"),
            None,
        );
        assert_eq!(chinook.contain_module.kind, ContainModuleKind::Transport);
        assert_eq!(
            chinook.contain_module.admission,
            ContainAdmission::InfantryOrVehicle
        );
        assert_ne!(
            chinook.contain_module.forbid_inside_kind_of & KindOfMask::HUGE_VEHICLE.bits(),
            0,
            "ForbidInsideKindOf HUGE_VEHICLE must survive parse"
        );

        let ranger = GameLogic::build_template_from_object_definition(
            "ProbeChinookRanger",
            parser.get_definition("ProbeChinookRanger").expect("ranger"),
            None,
        );
        let humvee = GameLogic::build_template_from_object_definition(
            "ProbeChinookHumvee",
            parser.get_definition("ProbeChinookHumvee").expect("humvee"),
            None,
        );
        let overlord = GameLogic::build_template_from_object_definition(
            "ProbeChinookOverlord",
            parser
                .get_definition("ProbeChinookOverlord")
                .expect("overlord"),
            None,
        );
        assert!(
            overlord.is_kind_of(crate::game_logic::KindOf::HugeVehicle),
            "Overlord KindOf HUGE_VEHICLE must be retained"
        );

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic
            .templates
            .insert("ProbeCombatChinook".to_string(), chinook);
        logic
            .templates
            .insert("ProbeChinookRanger".to_string(), ranger);
        logic
            .templates
            .insert("ProbeChinookHumvee".to_string(), humvee);
        logic
            .templates
            .insert("ProbeChinookOverlord".to_string(), overlord);

        let bird = logic
            .create_object_for_player("ProbeCombatChinook", 1, Vec3::ZERO)
            .expect("chinook");
        let infantry = logic
            .create_object_for_player("ProbeChinookRanger", 1, Vec3::ZERO)
            .expect("ranger");
        let vehicle = logic
            .create_object_for_player("ProbeChinookHumvee", 1, Vec3::ZERO)
            .expect("humvee");
        let huge = logic
            .create_object_for_player("ProbeChinookOverlord", 1, Vec3::ZERO)
            .expect("overlord");

        assert!(
            logic.can_unit_enter_normal_target(infantry, bird),
            "Ranger must board Chinook"
        );
        assert!(
            logic.can_unit_enter_normal_target(vehicle, bird),
            "Humvee must board Chinook"
        );
        assert!(
            !logic.can_unit_enter_normal_target(huge, bird),
            "Overlord HUGE_VEHICLE must be forbidden"
        );
    }

    #[test]
    fn unauthored_garrison_transport_default_infantry_only_not_any_mobile() {
        use crate::game_logic::{ContainAdmission, ContainModuleKind};
        use game_engine::common::system::kind_of::KindOfMask;
        use glam::Vec3;

        // C++ GarrisonContain.cpp / TransportContain.cpp / MobNexusContain.cpp
        // ctors and leftover Defaults: allow_inside = KINDOF_INFANTRY.
        // Un-authored AllowInsideKindOf must not become AnyMobile.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeDefaultGarrison
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
  End
End
Object ProbeDefaultTransport
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE TRANSPORT
  TransportSlotCount = 1
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
  End
End
Object ProbeDefaultCave
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = CaveContain ModuleTag_Cave
    ContainMax = 10
    CaveIndex = 0
  End
End
Object ProbeDefaultRanger
  Type = Infantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 100.0
  End
End
Object ProbeDefaultHumvee
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 200.0
  End
End
"#,
                "unauthored_allow_inside_kindof_probe.ini",
            )
            .expect("parse unauthored allow-inside probe");

        let garrison = GameLogic::build_template_from_object_definition(
            "ProbeDefaultGarrison",
            parser
                .get_definition("ProbeDefaultGarrison")
                .expect("garrison"),
            None,
        );
        assert_eq!(garrison.contain_module.kind, ContainModuleKind::Garrison);
        assert_eq!(
            garrison.contain_module.admission,
            ContainAdmission::InfantryOnly
        );
        assert_eq!(
            garrison.contain_module.allow_inside_kind_of,
            KindOfMask::INFANTRY.bits()
        );

        let transport = GameLogic::build_template_from_object_definition(
            "ProbeDefaultTransport",
            parser
                .get_definition("ProbeDefaultTransport")
                .expect("transport"),
            None,
        );
        assert_eq!(transport.contain_module.kind, ContainModuleKind::Transport);
        assert_eq!(
            transport.contain_module.admission,
            ContainAdmission::InfantryOnly
        );
        assert_eq!(
            transport.contain_module.allow_inside_kind_of,
            KindOfMask::INFANTRY.bits()
        );

        let cave = GameLogic::build_template_from_object_definition(
            "ProbeDefaultCave",
            parser.get_definition("ProbeDefaultCave").expect("cave"),
            None,
        );
        assert_eq!(cave.contain_module.kind, ContainModuleKind::Cave);
        assert_eq!(cave.contain_module.admission, ContainAdmission::AnyMobile);
        assert_eq!(cave.contain_module.allow_inside_kind_of, 0);

        let ranger = GameLogic::build_template_from_object_definition(
            "ProbeDefaultRanger",
            parser.get_definition("ProbeDefaultRanger").expect("ranger"),
            None,
        );
        let humvee = GameLogic::build_template_from_object_definition(
            "ProbeDefaultHumvee",
            parser.get_definition("ProbeDefaultHumvee").expect("humvee"),
            None,
        );

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic
            .templates
            .insert("ProbeDefaultGarrison".to_string(), garrison);
        logic
            .templates
            .insert("ProbeDefaultTransport".to_string(), transport);
        logic
            .templates
            .insert("ProbeDefaultRanger".to_string(), ranger);
        logic
            .templates
            .insert("ProbeDefaultHumvee".to_string(), humvee);

        let bunker = logic
            .create_object_for_player("ProbeDefaultGarrison", 1, Vec3::ZERO)
            .expect("garrison");
        let truck = logic
            .create_object_for_player("ProbeDefaultTransport", 1, Vec3::ZERO)
            .expect("transport");
        let infantry = logic
            .create_object_for_player("ProbeDefaultRanger", 1, Vec3::ZERO)
            .expect("ranger");
        let vehicle = logic
            .create_object_for_player("ProbeDefaultHumvee", 1, Vec3::ZERO)
            .expect("humvee");

        assert!(
            logic.can_unit_enter_normal_target(infantry, bunker),
            "Infantry must enter un-authored GarrisonContain"
        );
        assert!(
            !logic.can_unit_enter_normal_target(vehicle, bunker),
            "Vehicle must not enter un-authored GarrisonContain"
        );
        assert!(
            logic.can_unit_enter_normal_target(infantry, truck),
            "Infantry must enter un-authored TransportContain"
        );
        assert!(
            !logic.can_unit_enter_normal_target(vehicle, truck),
            "Vehicle must not enter un-authored TransportContain"
        );
    }

    #[test]
    fn garrison_initial_roster_parses_template_and_count() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeGarrisonRosterBunker
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    InitialRoster = GLAInfantryRebel 3
  End
End
"#,
                "garrison_initial_roster_probe.ini",
            )
            .expect("parse garrison roster probe");

        let bunker = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonRosterBunker",
            parser
                .get_definition("ProbeGarrisonRosterBunker")
                .expect("roster bunker"),
            None,
        );
        assert_eq!(bunker.contain_module.kind, ContainModuleKind::Garrison);
        assert_eq!(
            bunker.contain_module.initial_roster_template,
            "GLAInfantryRebel"
        );
        assert_eq!(bunker.contain_module.initial_roster_count, 3);
    }

    #[test]
    fn contain_module_parses_enter_and_exit_sounds() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeGarrisonEnterSound
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    EnterSound = GarrisonEnter
    ExitSound = GarrisonExit
  End
End
Object ProbeTransportEnterSound
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
    EnterSound = GarrisonEnter
    ExitSound = GarrisonExit
  End
End
Object ProbeHumveeSilentContain
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
  End
End
"#,
                "contain_enter_exit_sound_probe.ini",
            )
            .expect("parse contain sound probe");

        let bunker = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonEnterSound",
            parser
                .get_definition("ProbeGarrisonEnterSound")
                .expect("garrison sound"),
            None,
        );
        assert_eq!(bunker.contain_module.enter_sound, "GarrisonEnter");
        assert_eq!(bunker.contain_module.exit_sound, "GarrisonExit");

        let transport = GameLogic::build_template_from_object_definition(
            "ProbeTransportEnterSound",
            parser
                .get_definition("ProbeTransportEnterSound")
                .expect("transport sound"),
            None,
        );
        assert_eq!(transport.contain_module.kind, ContainModuleKind::Transport);
        assert_eq!(transport.contain_module.enter_sound, "GarrisonEnter");
        assert_eq!(transport.contain_module.exit_sound, "GarrisonExit");

        let humvee = GameLogic::build_template_from_object_definition(
            "ProbeHumveeSilentContain",
            parser
                .get_definition("ProbeHumveeSilentContain")
                .expect("silent transport"),
            None,
        );
        assert_eq!(humvee.contain_module.kind, ContainModuleKind::Transport);
        assert!(
            humvee.contain_module.enter_sound.is_empty()
                && humvee.contain_module.exit_sound.is_empty(),
            "Humvee comments EnterSound out — C++ stays silent"
        );
    }

    #[test]
    fn garrison_heal_objects_parses_and_heals_occupants() {
        use glam::Vec3;

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeGarrisonHealBunker
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    HealObjects = Yes
    TimeForFullHeal = 2000
  End
End
Object ProbeGarrisonHealInfantry
  Type = Infantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 80.0
  End
End
"#,
                "garrison_heal_objects_probe.ini",
            )
            .expect("parse garrison heal probe");

        let bunker = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonHealBunker",
            parser
                .get_definition("ProbeGarrisonHealBunker")
                .expect("heal bunker"),
            None,
        );
        assert!(bunker.contain_module.heal_objects);
        assert_eq!(bunker.contain_module.frames_for_full_heal, Some(60));

        let infantry = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonHealInfantry",
            parser
                .get_definition("ProbeGarrisonHealInfantry")
                .expect("heal infantry"),
            None,
        );

        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("ProbeGarrisonHealBunker".to_string(), bunker);
        logic
            .templates
            .insert("ProbeGarrisonHealInfantry".to_string(), infantry);
        let pad = logic
            .create_object("ProbeGarrisonHealBunker", Team::USA, Vec3::ZERO)
            .expect("heal bunker");
        let unit = logic
            .create_object("ProbeGarrisonHealInfantry", Team::USA, Vec3::ZERO)
            .expect("infantry");
        assert!(
            logic
                .host_object_mut(pad)
                .expect("bunker")
                .add_occupant(unit)
        );
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 20.0;
            obj.health.maximum = 80.0;
            obj.set_contained_by(Some(pad));
        }
        logic
            .tunnel_network
            .stamp_contained_by_frame(unit, logic.frame);
        logic.frame = logic.frame.saturating_add(1);
        logic.update_support_states(&[unit], 1.0 / 30.0);
        let after = logic
            .host_object(unit)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        let sliver = 80.0 / 60.0;
        assert!(
            (after - (20.0 + sliver)).abs() < 0.05,
            "HealObjects must apply one sliver, not TimeForFullHeal+HealObjects double, got {after}"
        );

        assert!(
            logic
                .host_object(unit)
                .is_some_and(|o| o.contained_by == Some(pad)),
            "garrison heal must not auto-exit like HealContain"
        );
    }

    #[test]
    fn crate_vision_uses_shroud_clearing_range_and_unlooks_on_move() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        let mut tpl = ThingTemplate::new("VisionProbe");
        tpl.sight_range = 100.0;
        tpl.shroud_clearing_range = 240.0;
        logic.templates.insert("VisionProbe".into(), tpl);
        let id = logic
            .create_object_for_player("VisionProbe", 1, Vec3::new(10.0, 0.0, 20.0))
            .expect("spawn");

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        logic.update_main_crate_vision();
        let first = *logic.vision_last_looks.get(&id).expect("looker");
        assert!((first.3 - 240.0).abs() < 0.01, "look radius {}", first.3);
        logic.update_main_crate_vision();
        assert_eq!(logic.vision_last_looks.len(), 1);

        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_position(Vec3::new(80.0, 0.0, 90.0));
        }
        logic.update_main_crate_vision();
        let moved = *logic.vision_last_looks.get(&id).expect("moved looker");
        assert!((moved.0 - 80.0).abs() < 0.01);
        assert!((moved.1 - 90.0).abs() < 0.01);
        assert!((moved.3 - 240.0).abs() < 0.01);

        if let Some(obj) = logic.host_object_mut(id) {
            obj.health.current = 0.0;
        }
        logic.update_main_crate_vision();
        assert!(
            !logic.vision_last_looks.contains_key(&id),
            "death must unlook"
        );
    }

    /// C++ PartitionManager.cpp:1582-1688 — object FOW is footprint COI mix.
    #[test]
    fn object_fow_uses_coi_mix_not_vision_range_circle() {
        use gamelogic::common::types::ObjectShroudStatus;
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.add_player(Player::new(2, Team::China, "China", false));

        let mut looker = ThingTemplate::new("Looker");
        looker.sight_range = 100.0;
        looker.shroud_clearing_range = 80.0;
        logic.templates.insert("Looker".into(), looker);

        let mut scout = ThingTemplate::new("EnemyScout");
        scout.add_kind_of(KindOf::Infantry).set_health(80.0);
        logic.templates.insert("EnemyScout".into(), scout);

        let mut bunker = ThingTemplate::new("EnemyBunker");
        bunker
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Immobile)
            .set_health(400.0);
        bunker.geometry_info = crate::game_logic::HostGeometryInfo {
            geom_type: crate::game_logic::HostGeometryType::Box,
            is_small: false,
            height: 20.0,
            major_radius: 20.0,
            minor_radius: 20.0,
            authored: true,
        };
        logic.templates.insert("EnemyBunker".into(), bunker);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let _looker_id = logic
            .create_object_for_player("Looker", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("looker");
        let near_id = logic
            .create_object_for_player("EnemyScout", 2, Vec3::new(10.0, 0.0, 0.0))
            .expect("near");
        let far_id = logic
            .create_object_for_player("EnemyScout", 2, Vec3::new(300.0, 0.0, 0.0))
            .expect("far");
        let bunker_id = logic
            .create_object_for_player("EnemyBunker", 2, Vec3::new(10.0, 0.0, 10.0))
            .expect("bunker");

        logic.update_main_crate_vision();
        {
            let shroud = get_shroud_manager();
            let mgr = shroud.lock().expect("shroud");
            assert_eq!(
                mgr.get_host_object_shroud_status(1, near_id.0),
                Some(ObjectShroudStatus::Clear),
                "enemy on revealed cells is CLEAR"
            );
            assert_eq!(
                mgr.get_host_object_shroud_status(1, far_id.0),
                Some(ObjectShroudStatus::Shrouded),
                "enemy on unrevealed cells is SHROUDED, not a VisionRange ghost"
            );
            assert_eq!(
                mgr.get_host_object_shroud_status(1, bunker_id.0),
                Some(ObjectShroudStatus::Clear)
            );
            assert!(mgr.host_object_ever_seen(1, bunker_id.0));
        }

        if let Some(obj) = logic.host_object_mut(_looker_id) {
            obj.set_position(Vec3::new(300.0, 0.0, 300.0));
        }
        logic.update_main_crate_vision();
        {
            let shroud = get_shroud_manager();
            let mgr = shroud.lock().expect("shroud");
            assert_eq!(
                mgr.get_host_object_shroud_status(1, near_id.0),
                Some(ObjectShroudStatus::Shrouded),
                "mobile enemy in fog must not linger as a fog ghost"
            );
            assert_eq!(
                mgr.get_host_object_shroud_status(1, bunker_id.0),
                Some(ObjectShroudStatus::Fogged),
                "seen immobile enemy building stays FOGGED ghost"
            );
        }
    }

    /// hq-rxwoc: object FOW mix reads leftover DiscreteCircle looker cells,
    /// not PARTITION_MANAGER's square `±ceil(r/40)` + world `r²` reject.
    #[test]
    fn object_fow_mix_reads_leftover_discrete_circle_not_square() {
        use gamelogic::common::types::ObjectShroudStatus;
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.add_player(Player::new(2, Team::China, "China", false));

        let mut looker = ThingTemplate::new("CircleLooker");
        looker.sight_range = 100.0;
        looker.shroud_clearing_range = 240.0;
        logic.templates.insert("CircleLooker".into(), looker);

        let mut scout = ThingTemplate::new("RimScout");
        scout.add_kind_of(KindOf::Infantry).set_health(80.0);
        scout.geometry_info = crate::game_logic::HostGeometryInfo {
            geom_type: crate::game_logic::HostGeometryType::Sphere,
            is_small: true,
            height: 2.0,
            major_radius: 1.0,
            minor_radius: 1.0,
            authored: true,
        };
        logic.templates.insert("RimScout".into(), scout);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let _looker_id = logic
            .create_object_for_player("CircleLooker", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("looker");
        // Cell (5, 3): DiscreteCircle radius 6 includes the y=3 span x=±5.
        // Square world r² from (0,0) with r=240 excludes cell-center (220,140).
        let rim_id = logic
            .create_object_for_player("RimScout", 2, Vec3::new(210.0, 0.0, 130.0))
            .expect("rim");

        logic.update_main_crate_vision();
        {
            let shroud = get_shroud_manager();
            let mgr = shroud.lock().expect("shroud");
            assert_eq!(
                mgr.get_host_object_shroud_status(1, rim_id.0),
                Some(ObjectShroudStatus::Clear),
                "rim cell on leftover DiscreteCircle must be CLEAR, not square-disk SHROUDED"
            );
        }
    }

    #[test]
    fn looker_mask_uses_player_relationship_and_unlook_persist_150() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        assert_eq!(super::host_unlook_persist_frames(), 150);

        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "USA", true);
        usa.alliance_team = 7;
        let mut china = Player::new(1, Team::China, "China", false);
        china.alliance_team = 7;
        logic.add_player(usa);
        logic.add_player(china);

        let mut tpl = ThingTemplate::new("AllyLooker");
        tpl.sight_range = 50.0;
        tpl.shroud_clearing_range = 80.0;
        logic.templates.insert("AllyLooker".into(), tpl);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let id = logic
            .create_object_for_player("AllyLooker", 0, Vec3::new(10.0, 0.0, 10.0))
            .expect("spawn");
        logic.update_main_crate_vision();
        let look = *logic.vision_last_looks.get(&id).expect("looker");
        assert_ne!(look.4 & (1u32 << 0), 0, "owner bit");
        assert_ne!(
            look.4 & (1u32 << 1),
            0,
            "script/skirmish ally must share look"
        );
    }

    #[test]
    fn transport_passengers_stop_looking() {
        use crate::game_logic::{ContainModuleKind, ContainModuleMetadata};
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));

        let mut chinook = ThingTemplate::new("ChinookLook");
        chinook.shroud_clearing_range = 200.0;
        chinook.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Transport,
            slots: Some(8),
            ..ContainModuleMetadata::default()
        };
        logic.templates.insert("ChinookLook".into(), chinook);

        let mut ranger = ThingTemplate::new("RangerLook");
        ranger.shroud_clearing_range = 150.0;
        logic.templates.insert("RangerLook".into(), ranger);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let bird = logic
            .create_object_for_player("ChinookLook", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("chinook");
        let rider = logic
            .create_object_for_player("RangerLook", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        if let Some(obj) = logic.host_object_mut(rider) {
            obj.set_contained_by(Some(bird));
        }
        logic.update_main_crate_vision();
        assert!(
            logic.vision_last_looks.contains_key(&bird),
            "container still looks"
        );
        assert!(
            !logic.vision_last_looks.contains_key(&rider),
            "transport passenger must not look"
        );
    }

    #[test]
    fn shroud_reveal_to_all_range_looks_for_enemies() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        logic.add_player(Player::new(1, Team::China, "China", false));

        let mut tpl = ThingTemplate::new("StratCenter");
        tpl.shroud_clearing_range = 200.0;
        tpl.shroud_reveal_to_all_range = 50.0;
        logic.templates.insert("StratCenter".into(), tpl);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let id = logic
            .create_object_for_player("StratCenter", 0, Vec3::new(20.0, 0.0, 20.0))
            .expect("center");
        logic.update_main_crate_vision();
        let reveal = *logic.vision_last_reveal_all.get(&id).expect("reveal-all");
        assert!((reveal.3 - 50.0).abs() < 0.01);
        assert_ne!(reveal.4 & (1u32 << 1), 0, "enemy bit in reveal-all mask");
        assert_eq!(reveal.4 & (1u32 << 0), 0, "owner is not enemies/neutral");

        if let Some(obj) = logic.host_object_mut(id) {
            obj.status.stealthed = true;
            obj.status.detected = false;
            obj.status.disguised = false;
        }
        logic.update_main_crate_vision();
        assert!(
            !logic.vision_last_reveal_all.contains_key(&id),
            "stealthed-not-detected skips reveal-to-all"
        );
    }

    /// C++ Object.cpp:4961-4962 — vision-spied units look for the spy.
    #[test]
    fn crate_vision_spied_mask_makes_moving_looker() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        logic.add_player(Player::new(1, Team::China, "China", false));

        let mut tpl = ThingTemplate::new("SpiedScout");
        tpl.sight_range = 50.0;
        tpl.shroud_clearing_range = 80.0;
        logic.templates.insert("SpiedScout".into(), tpl);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let id = logic
            .create_object_for_player("SpiedScout", 1, Vec3::new(10.0, 0.0, 20.0))
            .expect("spawn");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_vision_spied_by_player(0, true);
        }
        logic.update_main_crate_vision();
        let look = *logic.vision_last_looks.get(&id).expect("looker");
        assert_ne!(look.4 & (1u32 << 1), 0, "owner still looks for self");
        assert_ne!(
            look.4 & (1u32 << 0),
            0,
            "vision_spied_mask must OR the spy into looking_mask"
        );
        assert!((look.3 - 80.0).abs() < 0.01);

        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_position(Vec3::new(90.0, 0.0, 110.0));
        }
        logic.update_main_crate_vision();
        let moved = *logic.vision_last_looks.get(&id).expect("moved looker");
        assert!((moved.0 - 90.0).abs() < 0.01);
        assert!((moved.1 - 110.0).abs() < 0.01);
        assert_ne!(moved.4 & (1u32 << 0), 0, "spy look follows the unit");
    }

    /// C++ Object.cpp:5128-5140 — UC look is bounding-circle, not stored range.
    #[test]
    fn crate_vision_under_construction_uses_bounding_circle() {
        use crate::game_logic::{HostGeometryInfo, HostGeometryType};
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));

        let mut tpl = ThingTemplate::new("WarFactoryPad");
        tpl.sight_range = 100.0;
        tpl.shroud_clearing_range = 240.0;
        tpl.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Box,
            is_small: false,
            height: 20.0,
            major_radius: 10.0,
            minor_radius: 10.0,
            authored: true,
        };
        logic.templates.insert("WarFactoryPad".into(), tpl);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let id = logic
            .create_object_for_player("WarFactoryPad", 1, Vec3::new(40.0, 0.0, 40.0))
            .expect("spawn");
        let expected_circle = (10.0_f32 * 10.0 + 10.0 * 10.0).sqrt();
        if let Some(obj) = logic.host_object_mut(id) {
            obj.status.under_construction = true;
            obj.shroud_clearing_range = 240.0;
        }
        logic.update_main_crate_vision();
        let uc = *logic.vision_last_looks.get(&id).expect("uc looker");
        assert!(
            (uc.3 - expected_circle).abs() < 0.01,
            "UC look must be bounding circle {}, got {}",
            expected_circle,
            uc.3
        );

        if let Some(obj) = logic.host_object_mut(id) {
            obj.status.under_construction = false;
        }
        logic.update_main_crate_vision();
        let done = *logic.vision_last_looks.get(&id).expect("complete looker");
        assert!(
            (done.3 - 240.0).abs() < 0.01,
            "completed look uses stored ShroudClearingRange, got {}",
            done.3
        );
    }

    /// C++ Object.cpp:5045-5080 — live look applies Object::shroud cover.
    #[test]
    fn crate_vision_applies_object_shroud_cover_to_non_allies() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        logic.add_player(Player::new(1, Team::China, "China", false));
        let mut ally = Player::new(2, Team::GLA, "GLA", false);
        ally.alliance_team = 0;
        if let Some(usa) = logic.players.get_mut(&0) {
            usa.alliance_team = 0;
        }
        logic.add_player(ally);

        let mut tpl = ThingTemplate::new("CoverVan");
        tpl.sight_range = 50.0;
        tpl.shroud_clearing_range = 80.0;
        logic.templates.insert("CoverVan".into(), tpl);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let id = logic
            .create_object_for_player("CoverVan", 0, Vec3::new(30.0, 0.0, 40.0))
            .expect("spawn");
        logic.update_main_crate_vision();
        assert!(
            !logic.vision_last_shroud.contains_key(&id),
            "default ShroudRange 0 must not cover"
        );

        assert!(logic.apply_active_shroud_upgrade(id, 175.0));
        let cover = *logic
            .vision_last_shroud
            .get(&id)
            .expect("upgrade must restamp cover immediately");
        assert!((cover.0 - 30.0).abs() < 0.01);
        assert!((cover.1 - 40.0).abs() < 0.01);
        assert!((cover.3 - 175.0).abs() < 0.01);
        assert_eq!(cover.4 & (1u32 << 0), 0, "owner is Allies, not covered");
        assert_ne!(cover.4 & (1u32 << 1), 0, "enemy must be in shrouding mask");
        assert_eq!(
            cover.4 & (1u32 << 2),
            0,
            "script/skirmish ally must see through"
        );

        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_position(Vec3::new(90.0, 0.0, 110.0));
        }
        logic.update_main_crate_vision();
        let moved = *logic.vision_last_shroud.get(&id).expect("moved cover");
        assert!((moved.0 - 90.0).abs() < 0.01);
        assert!((moved.1 - 110.0).abs() < 0.01);
        assert!((moved.3 - 175.0).abs() < 0.01);
        assert_ne!(moved.4 & (1u32 << 1), 0);

        if let Some(obj) = logic.host_object_mut(id) {
            obj.status.under_construction = true;
        }
        logic.update_main_crate_vision();
        assert!(
            !logic.vision_last_shroud.contains_key(&id),
            "UNDER_CONSTRUCTION must unshroud"
        );

        if let Some(obj) = logic.host_object_mut(id) {
            obj.status.under_construction = false;
        }
        logic.update_main_crate_vision();
        assert!(logic.vision_last_shroud.contains_key(&id));

        if let Some(obj) = logic.host_object_mut(id) {
            obj.health.current = 0.0;
        }
        logic.update_main_crate_vision();
        assert!(
            !logic.vision_last_shroud.contains_key(&id),
            "death must unshroud"
        );
    }

    #[test]
    fn ini_crusher_crushable_levels_stamp_live_objects() {
        // C++ ThingTemplate.cpp:226-227 parseUnsignedByte CrusherLevel/CrushableLevel.
        // C++ Object.cpp:1156-1164 getCrusherLevel/getCrushableLevel from template.
        // C++ Object.cpp:1146 crush when crusherLevel > crushableLevel.
        // C++ ToppleUpdate.cpp:352 / W3DTreeBuffer.cpp:1179 crusher_level > 1.
        let mut car_def = ObjectDefinition::new("CivilianCar01".to_string());
        car_def
            .attributes
            .insert("CrushableLevel".to_string(), "1".to_string());
        car_def
            .attributes
            .insert("KindOf".to_string(), "VEHICLE".to_string());

        let mut tank_def = ObjectDefinition::new("AmericaTankCrusader".to_string());
        tank_def
            .attributes
            .insert("CrusherLevel".to_string(), "2".to_string());
        tank_def
            .attributes
            .insert("CrushableLevel".to_string(), "2".to_string());
        tank_def
            .attributes
            .insert("KindOf".to_string(), "VEHICLE".to_string());

        let mut overlord_def = ObjectDefinition::new("ChinaTankOverlord".to_string());
        overlord_def
            .attributes
            .insert("CrusherLevel".to_string(), "3".to_string());
        overlord_def
            .attributes
            .insert("CrushableLevel".to_string(), "3".to_string());
        overlord_def
            .attributes
            .insert("KindOf".to_string(), "VEHICLE".to_string());

        let mut prop_def = ObjectDefinition::new("TreeOak".to_string());
        prop_def
            .attributes
            .insert("CrushableLevel".to_string(), "1".to_string());

        let car_t =
            GameLogic::build_template_from_object_definition("CivilianCar01", &car_def, None);
        let tank_t = GameLogic::build_template_from_object_definition(
            "AmericaTankCrusader",
            &tank_def,
            None,
        );
        let ov_t = GameLogic::build_template_from_object_definition(
            "ChinaTankOverlord",
            &overlord_def,
            None,
        );
        let prop_t = GameLogic::build_template_from_object_definition("TreeOak", &prop_def, None);

        assert_eq!(car_t.crusher_level, 0);
        assert_eq!(car_t.crushable_level, 1);
        assert_eq!(tank_t.crusher_level, 2);
        assert_eq!(tank_t.crushable_level, 2);
        assert_eq!(ov_t.crusher_level, 3);
        assert_eq!(ov_t.crushable_level, 3);
        assert_eq!(prop_t.crushable_level, 1);

        let car = Object::new(car_t, ObjectId(1), Team::Neutral);
        let tank = Object::new(tank_t, ObjectId(2), Team::USA);
        let ov = Object::new(ov_t, ObjectId(3), Team::China);
        let prop = Object::new(prop_t, ObjectId(4), Team::Neutral);

        assert_eq!(car.crushable_level, 1);
        assert_eq!(car.crusher_level, 0);
        assert_eq!(tank.crusher_level, 2);
        assert_eq!(ov.crusher_level, 3);
        assert_eq!(prop.crushable_level, 1);
        assert!(ov.crusher_level > tank.crusher_level);

        use crate::game_logic::host_partition_collision_physics_residual::can_crush_only_residual;
        assert!(
            can_crush_only_residual(tank.crusher_level, car.crushable_level, false, false),
            "tank CrusherLevel 2 must crush car CrushableLevel 1"
        );
        assert!(
            can_crush_only_residual(tank.crusher_level, prop.crushable_level, false, false),
            "tank must crush props"
        );
        assert!(
            can_crush_only_residual(ov.crusher_level, tank.crushable_level, false, false),
            "Overlord 3 must crush tank 2"
        );
        assert!(!can_crush_only_residual(
            tank.crusher_level,
            ov.crushable_level,
            false,
            false
        ));
        assert!(!crate::game_logic::host_topple::crusher_can_topple(1));
        assert!(crate::game_logic::host_topple::crusher_can_topple(
            tank.crusher_level
        ));
        assert!(crate::game_logic::host_topple::crusher_can_topple(
            ov.crusher_level
        ));

        // KindOf Vehicle must not invent CrusherLevel=1 (pre-fix physics_motion.rs:113).
        let mut bare = ThingTemplate::new("BareCar");
        bare.add_kind_of(KindOf::Vehicle);
        let bare_obj = Object::new(bare, ObjectId(5), Team::Neutral);
        assert_eq!(bare_obj.crusher_level, 0);
        assert_eq!(bare_obj.crushable_level, 255);
    }

    #[test]
    fn ini_geometry_stamps_live_template_not_kind_radii() {
        // C++ ThingTemplate.cpp:201-205 / Geometry.cpp:26-58 parse Geometry*.
        // Retail AmericaTankBattleMaster: BOX 13 / 9 / 10, IsSmall Yes.
        let mut def = ObjectDefinition::new("AmericaTankBattleMaster".to_string());
        def.attributes.insert(
            "KindOf".to_string(),
            "VEHICLE SELECTABLE CAN_ATTACK".to_string(),
        );
        def.attributes
            .insert("Geometry".to_string(), "BOX".to_string());
        def.attributes
            .insert("GeometryMajorRadius".to_string(), "13.0".to_string());
        def.attributes
            .insert("GeometryMinorRadius".to_string(), "9.0".to_string());
        def.attributes
            .insert("GeometryHeight".to_string(), "10.0".to_string());
        def.attributes
            .insert("GeometryIsSmall".to_string(), "Yes".to_string());
        def.attributes
            .insert("StructureRubbleHeight".to_string(), "8".to_string());

        let template =
            GameLogic::build_template_from_object_definition("AmericaTankBattleMaster", &def, None);
        assert!(template.geometry_info.authored);
        assert_eq!(
            template.geometry_info.geom_type,
            crate::game_logic::HostGeometryType::Box
        );
        assert!((template.geometry_info.major_radius - 13.0).abs() < 1e-4);
        assert!((template.geometry_info.minor_radius - 9.0).abs() < 1e-4);
        assert!((template.geometry_info.height - 10.0).abs() < 1e-4);
        assert!(template.geometry_info.is_small);
        assert_eq!(template.structure_rubble_height, 8);

        let obj = Object::new(template, ObjectId(1), Team::USA);
        let expected_circle = 13.0f32.hypot(9.0);
        assert!(
            (obj.selection_radius - expected_circle).abs() < 1e-4,
            "selection must be bounding circle {}, not Vehicle 15; got {}",
            expected_circle,
            obj.selection_radius
        );
        assert!((obj.thing.geometry.radius - expected_circle).abs() < 1e-4);
        assert!((obj.thing.geometry.bounds_max.x - 13.0).abs() < 1e-4);
        assert!((obj.thing.geometry.bounds_max.z - 9.0).abs() < 1e-4);
        assert!((obj.thing.geometry.bounds_max.y - 10.0).abs() < 1e-4);

        // Unknown Geometry token fails closed (keeps default SPHERE).
        let mut bad = ObjectDefinition::new("BogusGeom".to_string());
        bad.attributes
            .insert("Geometry".to_string(), "PYRAMID".to_string());
        let bare = GameLogic::build_template_from_object_definition("BogusGeom", &bad, None);
        assert!(!bare.geometry_info.authored);
        assert_eq!(
            bare.geometry_info.geom_type,
            crate::game_logic::HostGeometryType::Sphere
        );
    }

    #[test]
    fn ini_shroud_reveal_to_all_range_and_kindofs() {
        let mut def = ObjectDefinition::new("AmericaStrategyCenter".to_string());
        def.attributes
            .insert("ShroudRevealToAllRange".to_string(), "50.0".to_string());
        def.attributes.insert(
            "KindOf".to_string(),
            "STRUCTURE REVEAL_TO_ALL ALWAYS_VISIBLE".to_string(),
        );
        let template =
            GameLogic::build_template_from_object_definition("AmericaStrategyCenter", &def, None);
        assert!((template.shroud_reveal_to_all_range - 50.0).abs() < 1e-4);
        assert!(template.reveal_to_all);
        assert!(template.always_visible);
    }

    #[test]
    fn omitted_experience_value_defaults_to_zero_not_invented_50_100() {
        let tree = GameLogic::build_template_from_object_definition(
            "Tree",
            &ObjectDefinition::new("Tree".to_string()),
            None,
        );
        assert_eq!(tree.experience_value, 0.0);
        assert_eq!(tree.experience_values, [0.0; 4]);

        let mut structure = ObjectDefinition::new("CivilianBuilding".to_string());
        structure
            .attributes
            .insert("KindOf".to_string(), "STRUCTURE".to_string());
        let structure =
            GameLogic::build_template_from_object_definition("CivilianBuilding", &structure, None);
        assert_eq!(structure.experience_value, 0.0);
        assert_eq!(structure.experience_values, [0.0; 4]);

        let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
        authored
            .attributes
            .insert("ExperienceValue".to_string(), "20 20 40 60".to_string());
        let authored = GameLogic::build_template_from_object_definition(
            "AmericaInfantryRanger",
            &authored,
            None,
        );
        assert_eq!(authored.experience_value, 20.0);
        assert_eq!(authored.experience_values, [20.0, 20.0, 40.0, 60.0]);
    }

    #[test]
    fn omitted_vision_range_defaults_to_zero_not_150() {
        let tree = GameLogic::build_template_from_object_definition(
            "Tree",
            &ObjectDefinition::new("Tree".to_string()),
            None,
        );
        assert_eq!(tree.sight_range, 0.0);

        let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
        authored
            .attributes
            .insert("VisionRange".to_string(), "150.0".to_string());
        let authored = GameLogic::build_template_from_object_definition(
            "AmericaInfantryRanger",
            &authored,
            None,
        );
        assert!((authored.sight_range - 150.0).abs() < 1e-4);

        let mut zero = ObjectDefinition::new("Prop".to_string());
        zero.attributes
            .insert("VisionRange".to_string(), "0".to_string());
        let zero = GameLogic::build_template_from_object_definition("Prop", &zero, None);
        assert_eq!(zero.sight_range, 0.0);
    }

    #[test]
    fn omitted_threat_value_defaults_to_zero_not_build_cost() {
        let mut costly = ObjectDefinition::new("AmericaWarFactory".to_string());
        costly
            .attributes
            .insert("BuildCost".to_string(), "2000".to_string());
        let costly =
            GameLogic::build_template_from_object_definition("AmericaWarFactory", &costly, None);
        assert_eq!(costly.threat_value, 0);
        assert_eq!(costly.build_cost.supplies, 2000);

        let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
        authored
            .attributes
            .insert("ThreatValue".to_string(), "1".to_string());
        authored
            .attributes
            .insert("BuildCost".to_string(), "225".to_string());
        let authored = GameLogic::build_template_from_object_definition(
            "AmericaInfantryRanger",
            &authored,
            None,
        );
        assert_eq!(authored.threat_value, 1);
        assert_eq!(authored.build_cost.supplies, 225);

        let mut zero = ObjectDefinition::new("Prop".to_string());
        zero.attributes
            .insert("ThreatValue".to_string(), "0".to_string());
        let zero = GameLogic::build_template_from_object_definition("Prop", &zero, None);
        assert_eq!(zero.threat_value, 0);
    }
}
