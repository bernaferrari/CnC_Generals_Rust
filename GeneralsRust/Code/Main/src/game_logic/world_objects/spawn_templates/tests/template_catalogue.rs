use super::*;

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
fn exact_retail_catalogue_seed_preserves_curated_health_and_admits_authored_art() {
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
    assert_eq!(curated_after.model_name.as_deref(), Some("AVCrusader"));
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

    const SOURCE: &str = r#"
Object AmericaStrategyCenter
  KindOf = PRELOAD STRUCTURE SELECTABLE IMMOBILE FS_STRATEGY_CENTER
  WeaponSet
    Conditions = None
    Weapon = PRIMARY StrategyCenterGun
    AutoChooseSources = PRIMARY NONE
  End
End
Object GLAVehicleQuadCannon
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  WeaponSet
    Conditions = None
    Weapon = PRIMARY QuadCannonGun
    Weapon = SECONDARY QuadCannonGunAir
  End
End
Object GLAVehicleToxinTruck
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
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  Behavior = FireOCLAfterWeaponCooldownUpdate ModuleTag_01
    WeaponSlot = SECONDARY
    OCL = OCL_PoisonFieldMedium
  End
End
Object OrdinaryAttackableBunker
  KindOf = STRUCTURE SELECTABLE CAN_ATTACK
End
"#;
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(SOURCE, "create_policy_probe.ini")
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
    admit_fixture_source(
        &mut logic,
        "create_policy_probe.ini",
        SOURCE,
        &[
            "AmericaStrategyCenter",
            "GLAVehicleQuadCannon",
            "GLAVehicleToxinTruck",
            "ChemSpillSprayer",
            "OrdinaryAttackableBunker",
        ],
    );

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

    // Missing-INI fallback is an independent programmatic fixture. The
    // authored world above keeps its receiving owner; an uncaptured
    // template cannot be added to that owner's immutable namespace.
    let mut logic = GameLogic::new();
    // Name residual still binds when Object INI never supplied the module.
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
