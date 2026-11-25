use super::*;

const UPGRADE: &str = "Upgrade_OwnedCostModifier";

#[test]
fn authored_portable_capture_runs_child_callbacks_on_new_controllers_default_team() {
    use crate::presentation_frame::PresentationFrame;
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

    fn world(contain_class: &str, hull_cost_first: bool) -> (GameLogic, u32) {
        let mut world = GameLogic::new();
        for (id, color) in [(1, (255, 0, 0)), (2, (0, 0, 255))] {
            let mut player = Player::new(id, Team::USA, "PortableOwner", true);
            player.color_rgb = color;
            player.color_night_rgb = color;
            world.add_player(player);
        }
        const UPGRADE: &str = "Upgrade_CapturePortableCost";
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &mut world, UPGRADE, "OBJECT", 0, 1,
        );
        let hull_admission = if contain_class == "HelixContain" {
            "INFANTRY"
        } else {
            "PORTABLE_STRUCTURE"
        };
        let hull_team = {
            let mut factory = world.team_factory.lock().unwrap();
            let (_, actions) = factory
                .init_team_owned_with_effects(
                    "CapturedHullTeam".into(),
                    "RustPlayer2".into(),
                    false,
                    Some(&game_engine::common::dict::Dict::new()),
                    Some(2),
                )
                .unwrap();
            assert!(actions.is_empty());
            let (team, actions) = factory
                .find_team_with_owned_effects("CapturedHullTeam")
                .unwrap();
            assert!(actions.is_empty());
            let id = team.read().unwrap().get_id();
            id
        };
        let hull_cost = format!(
            "Behavior = CostModifierUpgrade HullCost\n TriggeredBy = {UPGRADE}\n EffectKindOf = VEHICLE\n Percentage = -10%\nEnd"
        );
        let (before_payload, after_payload) = if hull_cost_first {
            (hull_cost.as_str(), "")
        } else {
            ("", hull_cost.as_str())
        };
        world
            .admit_object_ini_sources([(
                "portable-capture.ini".to_owned(),
                format!(
                    r#"
Object CaptureHull
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody HullBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = GrantUpgradeCreate GrantHullCost
  UpgradeToGrant = {UPGRADE}
  ExemptStatus = UNDER_CONSTRUCTION
 End
 {before_payload}
 Behavior = {contain_class} PayloadContain
  Slots = 1
  AllowInsideKindOf = {hull_admission}
  PayloadTemplateName = CapturePayload CapturePassenger
 End
 {after_payload}
End
Object CapturePayload
 KindOf = VEHICLE PORTABLE_STRUCTURE SELECTABLE
 TransportSlotCount = 1
 Body = ActiveBody PayloadBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = GrantUpgradeCreate GrantCost
  UpgradeToGrant = {UPGRADE}
  ExemptStatus = UNDER_CONSTRUCTION
 End
 Behavior = CostModifierUpgrade PayloadCost
  TriggeredBy = {UPGRADE}
  EffectKindOf = VEHICLE
  Percentage = -25%
 End
 Behavior = TransportContain PassengerContain
  Slots = 1
  AllowInsideKindOf = INFANTRY
 End
End
Object CapturePassenger
 KindOf = INFANTRY
 TransportSlotCount = 1
 Body = ActiveBody PassengerBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#
                ),
            )])
            .unwrap();
        (world, hull_team)
    }

    fn spawn(world: &mut GameLogic) -> (ObjectId, ObjectId) {
        let hull = world
            .create_object_for_player("CaptureHull", 1, glam::Vec3::ZERO)
            .unwrap();
        let payload = world
            .overlord_helix_portable_occupant_id(hull)
            .expect("payload installed by the authored hull factory");
        assert_eq!(
            world
                .host_object(payload)
                .unwrap()
                .cost_modifier_upgrade_executed,
            [true]
        );
        (hull, payload)
    }

    fn quote(world: &GameLogic) -> [u32; 2] {
        [1, 2].map(|player| world.modified_build_cost_supplies(player, "CaptureHull", 800))
    }

    fn cost_order(world: &GameLogic, owner: u32) -> Vec<f32> {
        world.players[&owner]
            .kind_of_production_cost_changes
            .iter()
            .map(|entry| entry.percent)
            .collect()
    }

    for (contain_class, hull_cost_first) in [("HelixContain", false), ("OverlordContain", true)] {
        let (mut receiver, hull_team) = world(contain_class, hull_cost_first);
        let (mut peer, peer_hull_team) = world(contain_class, hull_cost_first);
        assert_eq!(peer_hull_team, hull_team);
        let (hull, payload) = spawn(&mut receiver);
        assert_eq!(spawn(&mut peer), (hull, payload));
        let passenger_carrier = if contain_class == "HelixContain" {
            hull
        } else {
            payload
        };
        let [passenger] = receiver
            .host_object(passenger_carrier)
            .unwrap()
            .contained_unit_ids()
        else {
            panic!("ordinary passenger in the authored physical carrier");
        };
        let passenger = *passenger;
        let original_team = receiver.host_default_team_id(1).unwrap();
        let capture_default = receiver.host_default_team_id(2).unwrap();
        assert_ne!(hull_team, capture_default);
        let before = quote(&receiver);
        assert_eq!(before, [539, 800]);
        assert!(receiver.set_host_object_team(hull, hull_team).unwrap());

        let expected_cost_order = if hull_cost_first {
            [-0.10, -0.25]
        } else {
            [-0.25, -0.10]
        };
        assert_eq!(
            cost_order(&receiver, 2),
            expected_cost_order,
            "{contain_class}: nested transfer runs at the containment module's slot"
        );
        assert_eq!(quote(&receiver), [before[1], before[0]], "{contain_class}");
        assert_eq!(quote(&peer), before, "same ObjectIds in the peer");
        assert!(cost_order(&peer, 2).is_empty());
        let child = receiver.host_object(payload).unwrap();
        assert_eq!(child.team_instance_id, Some(capture_default));
        assert_eq!(child.contained_by, Some(hull));
        assert_eq!(
            receiver.host_object(passenger).unwrap().contained_by,
            Some(passenger_carrier),
            "the outer onCapture never eagerly ejects either roster"
        );
        assert_eq!(
            receiver.host_object(passenger).unwrap().owner_player_id,
            Some(1),
            "an ordinary rider is not captured with the portable payload"
        );
        assert_eq!(
            receiver
                .host_object(passenger_carrier)
                .unwrap()
                .pending_evacuate_on_stop,
            contain_class == "OverlordContain",
            "only the Overlord payload's own TransportContain orders its passenger to exit"
        );
        assert_eq!(
            receiver.host_object(hull).unwrap().team_instance_id,
            Some(hull_team)
        );
        {
            let factory = receiver.team_factory.lock().unwrap();
            for (team, member, expected) in [
                (original_team, payload, false),
                (capture_default, payload, true),
                (hull_team, hull, true),
                (hull_team, payload, false),
            ] {
                let team = factory.find_team_by_id(team).unwrap();
                assert_eq!(team.read().unwrap().has_member(member.0), expected);
            }
        }
        let frame = PresentationFrame::build_from_logic(&mut receiver, 2);
        let drawable = frame
            .objects
            .iter()
            .find(|object| object.id == payload)
            .unwrap();
        assert_eq!(drawable.owner_player_id, Some(2));
        assert_eq!(drawable.team_color, [0.0, 0.0, 1.0, 1.0]);

        let builder = SnapshotBuilder::new();
        let saved = builder.create_world_snapshot(&receiver).unwrap();
        let bytes = bincode_legacy::serialize(&saved).unwrap();
        let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, restored_hull_team) = world(contain_class, hull_cost_first);
        assert_eq!(restored_hull_team, hull_team);
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();
        assert_eq!(quote(&restored), [before[1], before[0]]);
        assert_eq!(cost_order(&restored, 2), expected_cost_order);
        assert_eq!(
            restored.host_object(payload).unwrap().team_instance_id,
            Some(capture_default)
        );
    }
}

const STOCK_REFINERY: &str = "TechOilRefinery";
const STOCK_CRUSADER: &str = "AmericaTankCrusader";
const STOCK_PRICE: u32 = 900;
const STOCK_DISCOUNTED_PRICE: u32 = 810;

fn stock_definitions() -> crate::parallel_harness::RetailWorldDefinitions {
    use crate::parallel_harness::{RetailWorldDefinitions, explicit_retail_install};

    let install = std::env::var_os("GENERALS_RETAIL_CONTENT_ROOT")
        .map(std::path::PathBuf::from)
        .or_else(crate::map_frame_scenario::find_lone_eagle_retail_install)
        .expect("a real mounted retail install is required");
    let install = explicit_retail_install(&install).unwrap();
    RetailWorldDefinitions::load_from_install(Some(&install))
        .unwrap_or_else(|error| panic!("{}: {error}", install.display()))
}

fn stock_world(definitions: &crate::parallel_harness::RetailWorldDefinitions) -> GameLogic {
    let mut world = GameLogic::new();
    definitions.admit(&mut world).unwrap();
    let upgrade = world
        .upgrade_template("Upgrade_CostReduction")
        .expect("stock Upgrade.ini is admitted into the receiving UpgradeCenter");
    assert_eq!(
        upgrade.get_upgrade_type(),
        gamelogic::upgrade::UpgradeType::Object
    );
    assert_ne!(upgrade.get_mask().bits(), 0);

    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    let mut side = game_engine::common::dict::Dict::new();
    side.set_ascii_string(game_engine::common::well_known_keys::key_player_name(), "");
    neutral.apply_map_side_dict(&side, false, &mut Default::default());
    world.add_player(neutral);
    world.add_player(Player::new(1, Team::USA, "Capturer", true));
    world.add_player(Player::new(2, Team::USA, "Recapturer", true));
    world.frame = 40;

    assert!(world.ensure_host_spawn_template(STOCK_CRUSADER));
    let crusader = &world.get_templates()[STOCK_CRUSADER];
    assert!(crusader.is_kind_of(crate::game_logic::KindOf::Vehicle));
    assert_eq!(crusader.build_cost.supplies, STOCK_PRICE);
    world
}

fn stock_refinery(world: &mut GameLogic) -> ObjectId {
    use crate::game_logic::script_loader::PlacedObject;
    use game_engine::common::{dict::Dict, name_key_generator::NameKeyGenerator};

    const STOCK_UPGRADE: &str = "Upgrade_CostReduction";
    let mut properties = Dict::new();
    properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "team");
    let placement = PlacedObject {
        template: STOCK_REFINERY.to_owned(),
        flags: 0,
        name: None,
        position: gamelogic::common::Coord3D::new(180.0, 220.0, 0.0),
        rotation: None,
        team_name: None,
        player_id: None,
        upgrade: None,
        unsellable: None,
        enabled: None,
        powered: None,
        indestructible: None,
        object_weather: None,
        properties,
    };
    let mut effects = Default::default();
    let birth = world
        .create_placed_map_object_with_effects_checked(
            &placement,
            glam::Vec3::new(180.0, 0.0, 220.0),
            &mut effects,
        )
        .unwrap()
        .expect("stock refinery on the reserved Neutral default Team");
    let id = birth.id;
    let refinery = world.host_object(id).unwrap();
    let [grant] = refinery.get_template().grant_upgrade_creates.as_slice() else {
        panic!("stock refinery must have one authored GrantUpgradeCreate");
    };
    assert_eq!(grant.upgrade_name, STOCK_UPGRADE);
    assert!(!grant.exempt_under_construction);
    assert!(!refinery.has_object_upgrade_complete(STOCK_UPGRADE));
    assert_eq!(stock_quotes(world), [STOCK_PRICE; 3]);

    // GameLogic.cpp:1870–1889 applies map properties and then completes
    // the placement. With no ExemptStatus, GrantUpgradeCreate.cpp:59–88
    // skips onCreate and grants from onBuildComplete.
    world
        .apply_update_obj_values_from_map_properties_with_effects(
            id,
            &placement.properties,
            &mut effects,
        )
        .unwrap();
    birth.finish(world, &mut effects).unwrap();

    let refinery = world.host_object(id).unwrap();
    assert!(refinery.has_object_upgrade_complete(STOCK_UPGRADE));
    assert_eq!(refinery.cost_modifier_upgrade_executed.as_slice(), &[true]);
    assert_eq!(
        stock_quotes(world),
        [STOCK_DISCOUNTED_PRICE, STOCK_PRICE, STOCK_PRICE]
    );
    id
}

fn stock_quotes(world: &GameLogic) -> [u32; 3] {
    [0, 1, 2].map(|player| world.modified_build_cost_supplies(player, STOCK_CRUSADER, STOCK_PRICE))
}

fn capture_stock_refinery(world: &mut GameLogic, refinery: ObjectId, owner: u32) {
    let team = world.host_default_team_id(owner).unwrap();
    assert!(world.set_host_object_team(refinery, team).unwrap());
    let refinery = world.host_object(refinery).unwrap();
    assert_eq!(refinery.owner_player_id, Some(owner));
    assert_eq!(refinery.team_instance_id, Some(team));
}

#[test]
fn mounted_stock_refinery_grants_captures_restores_and_deletes_its_cost_modifier() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mounted_stock_refinery_grants_captures_restores_and_deletes_its_cost_modifier",
        || {
            let definitions = stock_definitions();
            let mut world = stock_world(&definitions);
            let mut peer = stock_world(&definitions);
            assert_eq!(stock_quotes(&world), [STOCK_PRICE; 3]);

            let refinery = stock_refinery(&mut world);
            let peer_refinery = stock_refinery(&mut peer);
            assert_eq!(refinery, peer_refinery);
            assert_eq!(
                world.host_object(refinery).unwrap().owner_player_id,
                Some(0)
            );
            capture_stock_refinery(&mut world, refinery, 1);
            capture_stock_refinery(&mut peer, peer_refinery, 2);
            assert_eq!(
                stock_quotes(&world),
                [STOCK_PRICE, STOCK_DISCOUNTED_PRICE, STOCK_PRICE]
            );
            assert_eq!(
                stock_quotes(&peer),
                [STOCK_PRICE, STOCK_PRICE, STOCK_DISCOUNTED_PRICE]
            );

            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&world).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut restored = stock_world(&definitions);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(stock_quotes(&restored), stock_quotes(&world));

            capture_stock_refinery(&mut restored, refinery, 2);
            assert_eq!(stock_quotes(&restored), stock_quotes(&peer));
            restored.destroy_object(refinery).unwrap();
            assert_eq!(stock_quotes(&restored), [STOCK_PRICE; 3]);
            restored.process_destroy_list();
            assert!(restored.host_object(refinery).is_none());
            assert_eq!(
                stock_quotes(&world),
                [STOCK_PRICE, STOCK_DISCOUNTED_PRICE, STOCK_PRICE]
            );
            assert_eq!(
                stock_quotes(&peer),
                [STOCK_PRICE, STOCK_PRICE, STOCK_DISCOUNTED_PRICE]
            );
        },
    );
}

#[test]
fn mounted_stock_refinery_rubble_immediately_returns_its_bonus_to_neutral() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_usa_pilot::HostDeathType;

    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mounted_stock_refinery_rubble_immediately_returns_its_bonus_to_neutral",
        || {
            let definitions = stock_definitions();
            let mut world = stock_world(&definitions);
            let mut peer = stock_world(&definitions);
            let refinery = stock_refinery(&mut world);
            let peer_refinery = stock_refinery(&mut peer);
            assert_eq!(refinery, peer_refinery);
            capture_stock_refinery(&mut world, refinery, 1);
            capture_stock_refinery(&mut peer, peer_refinery, 1);
            assert_eq!(
                stock_quotes(&world),
                [STOCK_PRICE, STOCK_DISCOUNTED_PRICE, STOCK_PRICE]
            );
            assert!(
                world
                    .host_object(refinery)
                    .unwrap()
                    .has_captured_model_condition()
            );

            world
                .apply_owned_kill(refinery, DamageType::Unresistable, HostDeathType::Normal)
                .unwrap()
                .expect("lethal damage through the receiving ActiveBody");
            // TechBuildingBehavior.cpp:98–108 executes in onDie. The
            // authored KeepObjectDie never schedules physical retirement.
            let neutral_team = world.host_default_team_id(0).unwrap();
            let rubble = world.host_object(refinery).expect("retained stock rubble");
            assert!(rubble.status.on_die_started && rubble.status.effectively_dead);
            assert_eq!(
                stock_quotes(&world),
                [STOCK_DISCOUNTED_PRICE, STOCK_PRICE, STOCK_PRICE]
            );
            assert_eq!(rubble.owner_player_id, Some(0));
            assert_eq!(rubble.team_instance_id, Some(neutral_team));
            assert!(!rubble.has_captured_model_condition());
            world.process_destroy_list();
            assert!(world.host_object(refinery).is_some());

            assert!(peer.host_object(peer_refinery).unwrap().is_alive());
            assert_eq!(
                stock_quotes(&peer),
                [STOCK_PRICE, STOCK_DISCOUNTED_PRICE, STOCK_PRICE]
            );
        },
    );
}

fn world(percent: &str, create_source: bool) -> (GameLogic, Option<ObjectId>) {
    let mut world = GameLogic::new();
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "CostOwnerA",
        true,
    ));
    world.add_player(crate::game_logic::Player::new(
        1,
        Team::China,
        "CostOwnerB",
        true,
    ));
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(&format!("Upgrade {UPGRADE}\nType = OBJECT\nEnd\n"), |ini| {
        ini.read_line()?;
        world.upgrade_definitions.parse_upgrade_definition(ini)
    })
    .expect("admit cost upgrade definition");
    world
        .admit_object_ini_sources([(
            "cost_modifier_owner.ini".to_owned(),
            format!(
                r#"
Object CostModifierSource
 KindOf = STRUCTURE SELECTABLE
 Body = ActiveBody SourceBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = CostModifierUpgrade AuthoredCost
  TriggeredBy = {UPGRADE}
  EffectKindOf = VEHICLE AIRCRAFT
  Percentage = {percent}
 End
End
Object CostModifierVehicle
 KindOf = VEHICLE SELECTABLE
 BuildCost = 1000
 Body = ActiveBody VehicleBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object CostModifierDualKind
 KindOf = VEHICLE AIRCRAFT SELECTABLE
 BuildCost = 1000
 Body = ActiveBody DualBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#
            ),
        )])
        .expect("admit typed CostModifierUpgrade content");
    let source = create_source.then(|| {
        world
            .create_object_for_player("CostModifierSource", 0, glam::Vec3::ZERO)
            .unwrap()
    });
    (world, source)
}

#[test]
fn authored_cost_modifier_is_world_scoped_across_capture_delete_and_snapshot() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
    let (mut a, source_a) = world("-10%", true);
    let (mut b, source_b) = world("-25%", true);
    let source_a = source_a.unwrap();
    let source_b = source_b.unwrap();
    assert_eq!(
        source_a, source_b,
        "fixture must collide on ObjectId across worlds"
    );
    a.apply_upgrade_to_object(source_a, UPGRADE);
    b.apply_upgrade_to_object(source_b, UPGRADE);
    assert_eq!(
        a.modified_build_cost_supplies(0, "CostModifierVehicle", 1000),
        1000
    );
    assert_eq!(
        a.modified_build_cost_supplies(0, "CostModifierDualKind", 1000),
        900
    );
    assert_eq!(
        b.modified_build_cost_supplies(0, "CostModifierDualKind", 1000),
        750
    );

    let new_team = a.players[&1]
        .default_team_id
        .expect("player's admitted default team");
    assert!(a.set_host_object_team(source_a, new_team).unwrap());
    assert_eq!(
        a.modified_build_cost_supplies(0, "CostModifierDualKind", 1000),
        1000
    );
    assert_eq!(
        a.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        900
    );
    assert_eq!(
        b.modified_build_cost_supplies(0, "CostModifierDualKind", 1000),
        750
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&a).expect("capture world");
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _) = world("-10%", false);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("restore world");
    assert_eq!(
        restored.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        900
    );
    restored.destroy_object(source_a).unwrap();
    assert_eq!(
        restored.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        1000
    );

    b.destroy_object(source_b).unwrap();
    assert_eq!(
        b.modified_build_cost_supplies(0, "CostModifierDualKind", 1000),
        1000
    );
}
