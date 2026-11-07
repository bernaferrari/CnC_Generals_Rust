use super::*;

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
    let retail = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini"),
    )
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
    let retail = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/CivilianBuilding.ini"),
    )
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
    let faction_buildings = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini"),
    )
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
    use crate::game_logic::host_hero_abilities::LeftoverSaPhase;

    // SpecialAbilityUpdate.cpp:391-400 faces before entering unpack;
    // :614-640 counts unpack frames only after that phase begins.
    const SOURCE: &str = r#"
Object BurtonChargeUnpackProbe
  KindOf = INFANTRY SELECTABLE CAN_ATTACK
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface ModuleTag_AI
    AutoAcquireEnemiesWhenIdle = No
  End
  Locomotor = SET_NORMAL ColonelBurtonGroundLocomotor
  Behavior = SpecialAbilityUpdate ModuleTag_Timed
    SpecialPowerTemplate = SpecialAbilityColonelBurtonTimedCharges
    UnpackTime = 200
    PackUnpackVariationFactor = 0
    FleeRangeAfterCompletion = 100
    FlipOwnerAfterUnpacking = Yes
  End
End
Object ChargePlantTargetProbe
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 500
    InitialHealth = 500
  End
End
"#;
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(SOURCE, "burton_charge_live_probe.ini")
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
    assert_eq!(burton_tpl.authored_ai_update_interface(), Some(true));
    assert_eq!(
        burton_tpl.locomotor_name.as_deref(),
        Some("ColonelBurtonGroundLocomotor")
    );
    // The effect creates a real TimedC4Charge through the receiving
    // factory. Admit its retail definition alongside the two probes.
    admit_fixture_content(
        &mut logic,
        retail_receiving_fixture(&[("burton_charge_live_probe.ini", SOURCE)]),
        &[
            "BurtonChargeUnpackProbe",
            "ChargePlantTargetProbe",
            crate::game_logic::host_mines::BURTON_TIMED_CHARGE_OBJECT,
        ],
    );

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
    logic.queue_pending_special_ability(
        burton_id,
        PendingSpecialAbility::PlantTimedDemoCharge { target_id },
    );
    let frame = logic.frame;
    {
        let (burton, attack_events) = logic
            .host_object_and_attack_events_mut(burton_id)
            .expect("burton mut");
        // The probe must turn through PI before its 200ms unpack begins.
        // Keep a bounded turn capability, rather than snapping its yaw.
        assert_eq!(
            burton.cur_locomotor_name.as_deref(),
            Some("ColonelBurtonGroundLocomotor")
        );
        assert!(burton.movement.max_speed > 0.0);
        burton.movement.turn_rate = std::f32::consts::PI;
        burton.set_ai_state(
            AIState::SpecialAbility,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        // set_target admits an ordinary attack and replaces the ability
        // state. The ability order owns its state and target separately.
        burton.set_order_target(
            Some(target_id),
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        assert_eq!(burton.ai_state(), AIState::SpecialAbility);
    }

    const FRAME_DT: f32 = 1.0 / 30.0;
    logic.frame += 1;
    logic
        .update_ai(&[burton_id, target_id], FRAME_DT)
        .expect("receiving AI operation succeeds");
    assert_eq!(
        logic
            .hero_abilities()
            .leftover_channel(burton_id)
            .map(|channel| channel.phase),
        Some(LeftoverSaPhase::Facing)
    );
    assert_eq!(
        logic.mine_residual_places(),
        0,
        "UnpackTime 200ms must delay plant"
    );

    // Observe the real facing boundary, so turn time cannot consume the
    // unpack allowance. A PI/sec turn needs about 30 logic frames.
    for _ in 0..60 {
        logic.frame += 1;
        logic
            .update_ai(&[burton_id, target_id], FRAME_DT)
            .expect("receiving AI operation succeeds");
        assert_eq!(logic.mine_residual_places(), 0, "no plant while facing");
        if logic
            .hero_abilities()
            .leftover_channel(burton_id)
            .is_some_and(|channel| channel.phase == LeftoverSaPhase::Unpacking)
        {
            break;
        }
    }
    let unpack = logic
        .hero_abilities()
        .leftover_channel(burton_id)
        .expect("facing must reach the authored unpack phase");
    assert_eq!(unpack.phase, LeftoverSaPhase::Unpacking);
    assert!((unpack.remaining_seconds - 0.2).abs() < 0.000_1);

    // Five frames are still short of 200ms. The sixth ends unpack and
    // permits the effect; C++ finishAbility then orders the flee path.
    for _ in 0..5 {
        logic.frame += 1;
        logic
            .update_ai(&[burton_id, target_id], FRAME_DT)
            .expect("receiving AI operation succeeds");
        assert_eq!(
            logic.mine_residual_places(),
            0,
            "charge must not plant before the full 200ms unpack"
        );
        assert_eq!(
            logic
                .hero_abilities()
                .leftover_channel(burton_id)
                .map(|channel| channel.phase),
            Some(LeftoverSaPhase::Unpacking)
        );
    }
    logic.frame += 1;
    logic
        .update_ai(&[burton_id, target_id], FRAME_DT)
        .expect("receiving AI operation succeeds");
    assert!(
        logic.mine_residual_places() >= 1,
        "charge plants after UnpackTime"
    );
    let fleeing = logic.host_object(burton_id).expect("flee order");
    assert_eq!(fleeing.ai_state(), AIState::Moving);
    assert!(fleeing.requested_destination.is_some());
    assert_eq!(fleeing.ignored_obstacle_id, Some(target_id));
    for _ in 0..120 {
        let pos = logic
            .host_object(burton_id)
            .expect("burton fleeing")
            .get_position();
        if pos.distance(Vec3::new(2.0, 0.0, 0.0)) > 1.0 {
            break;
        }
        logic.frame += 1;
        // finishAbility orders movement; AI state updates do not
        // integrate its locomotor. Main's movement phase runs before
        // the support-state phase on the next frame (step.rs:555,1371).
        logic.update_movement_for_test(&[burton_id, target_id], FRAME_DT);
        logic
            .update_ai(&[burton_id, target_id], FRAME_DT)
            .expect("receiving AI operation succeeds");
    }
    let pos = logic
        .host_object(burton_id)
        .expect("burton after plant")
        .get_position();
    assert!(
        pos.distance(Vec3::new(2.0, 0.0, 0.0)) > 1.0,
        "finishAbility flee after plant, pos={pos:?}"
    );
}
