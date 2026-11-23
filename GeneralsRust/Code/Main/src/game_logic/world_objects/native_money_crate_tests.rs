use super::super::*;
use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;
use crate::game_logic::presentation_definitions::PresentationDefinitions;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

const ALTERNATE: &str = "Upgrade_MoneyAlternate";
const CRATE_POINT: Vec3 = Vec3::new(200.0, 0.0, 200.0);
const AWAY: Vec3 = Vec3::new(155.0, 0.0, 200.0);

fn admit_presentation(world: &mut GameLogic) {
    let definitions = PresentationDefinitions::from_ordered_sources(
        &[],
        &[(
            "money-crate-misc.ini",
            "MiscAudio\nCrateMoney = ReceivingCrateDing\nEnd\n".into(),
        )],
        &[(
            "money-crate-animations.ini",
            "Animation MoneyPickUp\nNumberImages = 1\nImage = CashImage\nAnimationMode = LOOP\nEnd\n"
                .into(),
        )],
        &|name| name == "CashImage",
    )
    .unwrap();
    world.admit_presentation_definitions(definitions).unwrap();
    world.set_draw_icon_ui(true);
}

fn fixture() -> GameLogic {
    const PICKER: &str = r#"
Object MoneyPicker
  KindOf = VEHICLE SELECTABLE
  Body = ActiveBody MoneyPickerBody
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface MoneyPickerAI
  End
  Geometry = CYLINDER
  GeometryMajorRadius = 4
  GeometryMinorRadius = 4
  GeometryHeight = 4
  GeometryIsSmall = Yes
End
"#;
    let mut source = PICKER.to_owned();
    for (object, base, boosts) in [
        ("NoBoostCrate", 5, vec![]),
        (
            "MissingBoostCrate",
            10,
            vec![("Upgrade_MissingCrate", 17), (ALTERNATE, 3)],
        ),
        (
            "AlternateBoostCrate",
            60,
            vec![(ALTERNATE, 42), (UPGRADE_AMERICA_SUPPLY_LINES, 99)],
        ),
        (
            "StockRowsCrate",
            100,
            vec![(UPGRADE_AMERICA_SUPPLY_LINES, 25)],
        ),
    ] {
        source.push_str(&format!(
            "Object {object}\nKindOf = CRATE\n             Body = ActiveBody MoneyCrateBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\n             Behavior = MoneyCrateCollide MoneyData\nMoneyProvided = {base}\n             ExecuteAnimation = MoneyPickUp\nExecuteAnimationTime = 4.0\nExecuteAnimationZRise = 15.0\n"
        ));
        for (upgrade, amount) in boosts {
            source.push_str(&format!(
                "UpgradedBoost = UpgradeType:{upgrade} Boost:{amount}\n"
            ));
        }
        source.push_str(
            "End\nGeometry = BOX\nGeometryMajorRadius = 4\nGeometryMinorRadius = 4\nGeometryHeight = 4\nGeometryIsSmall = Yes\nEnd\n",
        );
    }
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("native-money-crates.ini".into(), source)])
        .unwrap();
    world.with_own_services_mut(|world| {
        for name in [ALTERNATE, UPGRADE_AMERICA_SUPPLY_LINES] {
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source(&format!("Upgrade {name}\nType = PLAYER\nEnd\n"), |ini| {
                ini.read_line()?;
                world.upgrade_definitions.parse_upgrade_definition(ini)
            })
            .unwrap();
        }
    });
    world.add_player(Player::new(1, Team::USA, "MoneyReceiver", true));
    admit_presentation(&mut world);
    world.frame = 40;
    world
}

fn move_picker(
    world: &mut GameLogic,
    picker: ObjectId,
    point: Vec3,
    effects: &mut ScriptPresentationEffects<'_>,
) {
    assert!(world.set_owned_object_position_with_effects(picker, point, world.frame, effects));
    world.update_owned_partition_with_effects(effects).unwrap();
}

fn pickup(world: &mut GameLogic, picker: ObjectId, name: &str, amount: u32) {
    let mut effects = ScriptPresentationEffects::default();
    move_picker(world, picker, AWAY, &mut effects);
    let crate_id = world
        .create_object_with_script_effects_checked(
            name,
            Team::Neutral,
            CRATE_POINT,
            None,
            &mut effects,
        )
        .unwrap()
        .expect("admitted authored money crate");
    let funds = world.get_player(1).unwrap().resources.supplies;
    let earned = world.get_player(1).unwrap().statistics.money_earned();
    let random = world.logic_random.seed_words();
    world
        .update_owned_partition_with_effects(&mut effects)
        .unwrap();
    assert!(!world.host_object(crate_id).unwrap().status.destroyed);
    assert_eq!(world.get_player(1).unwrap().resources.supplies, funds);
    world.queued_audio_events.clear();

    assert!(world.set_owned_object_position_with_effects(
        picker,
        CRATE_POINT,
        world.frame,
        &mut effects,
    ));
    let picker_object = world.host_object(picker).unwrap();
    let crate_object = world.host_object(crate_id).unwrap();
    assert!(picker_object.has_ai_update_interface());
    assert!(
        host_geom_collides(picker_object, crate_object).is_some(),
        "{name}: the shaped actors must overlap at contact",
    );
    world
        .update_owned_partition_with_effects(&mut effects)
        .unwrap();
    assert!(
        world
            .partition_manager
            .neighbor_object_ids_of(picker.0)
            .contains(&crate_id.0),
        "{name}: both actors must share a partition cell until physical retirement",
    );
    assert_eq!(
        world.get_player(1).unwrap().resources.supplies,
        funds.wrapping_add(amount),
        "{name}: MoneyCrateCollide deposits on real contact",
    );
    assert_eq!(
        world.get_player(1).unwrap().statistics.money_earned(),
        earned + amount
    );
    assert_eq!(world.logic_random.seed_words(), random);
    assert!(world.queued_audio_events.iter().any(|event| {
        event.event_type == "ReceivingCrateDing" && event.object_id == Some(picker)
    }));
    // InGameUI inserts new world animations at the front.
    let animation = world.session_view.world_animations.births().next().unwrap();
    assert_eq!(animation.template, "MoneyPickUp");
    assert_eq!(animation.display_time_seconds, 4.);
    assert_eq!(animation.z_rise_per_second, 15.);
    assert!(animation.fades);
    assert_eq!(animation.position, CRATE_POINT);
    assert_eq!(animation.crate_id, crate_id);
    assert!(world.host_money_crates.money_floating_texts.is_empty());

    assert!(world.host_object(crate_id).unwrap().status.destroyed);
    world.process_destroy_list();
    assert!(world.host_object(crate_id).is_none());
    world
        .update_owned_partition_with_effects(&mut effects)
        .unwrap();
    assert_eq!(
        world.get_player(1).unwrap().resources.supplies,
        funds.wrapping_add(amount)
    );
}

fn restore(source: &GameLogic, target: &mut GameLogic) {
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    builder.restore_from_snapshot(&decoded, target).unwrap();
}

#[test]
fn authored_money_crate_uses_first_completed_row_from_the_receiving_world() {
    let mut world = fixture();
    let mut peer = fixture();
    let picker = world
        .create_object_for_player("MoneyPicker", 1, AWAY)
        .unwrap();
    let peer_picker = peer
        .create_object_for_player("MoneyPicker", 1, AWAY)
        .unwrap();
    assert_eq!(picker, peer_picker);

    pickup(&mut world, picker, "NoBoostCrate", 5);
    world.restore_completed_player_upgrade(1, UPGRADE_AMERICA_SUPPLY_LINES);
    // The second row wins while the first upgrade is incomplete.
    pickup(&mut world, picker, "AlternateBoostCrate", 159);
    peer.restore_completed_player_upgrade(1, ALTERNATE);
    pickup(&mut peer, peer_picker, "StockRowsCrate", 100);
    pickup(&mut peer, peer_picker, "AlternateBoostCrate", 102);

    world.restore_completed_player_upgrade(1, ALTERNATE);
    let mut receiving = fixture();
    restore(&world, &mut receiving);
    // With both completed the first authored row takes precedence. An
    // unknown name never counts as an admitted completed upgrade.
    pickup(&mut receiving, picker, "AlternateBoostCrate", 102);
    receiving.restore_completed_player_upgrade(1, "Upgrade_MissingCrate");
    pickup(&mut receiving, picker, "MissingBoostCrate", 13);
    pickup(&mut receiving, picker, "StockRowsCrate", 125);
    pickup(&mut peer, peer_picker, "StockRowsCrate", 100);
}

#[test]
fn authored_money_cargo_does_not_pay_before_partition_contact() {
    use crate::game_logic::host_deliver_payload::HostDeliverPayloadKind;

    let mut world = fixture();
    world.restore_completed_player_upgrade(1, UPGRADE_AMERICA_SUPPLY_LINES);
    let funds = world.get_player(1).unwrap().resources.supplies;
    let kind = HostDeliverPayloadKind::SupplyDropZoneCrate;
    let mission = world.host_deliver_payloads.queue_for_owner(
        kind,
        ObjectId(1234),
        Team::USA,
        Some(1),
        CRATE_POINT,
        world.frame,
        "StockRowsCrate",
    );
    for _ in 0..300 {
        world.update_deliver_payloads();
        if !world
            .host_deliver_payloads
            .completed_of_kind(kind)
            .is_empty()
        {
            break;
        }
        world.frame += 1;
    }
    let completed = world.host_deliver_payloads.completed_of_kind(kind);
    assert_eq!(completed.len(), 1);
    assert_eq!(
        world
            .host_deliver_payloads
            .get(mission)
            .unwrap()
            .cash_credited,
        0
    );
    assert!(!completed[0].spawned_payload_ids.is_empty());
    assert_eq!(world.get_player(1).unwrap().resources.supplies, funds);
}

#[test]
fn mounted_stock_money_crate_deposits_authored_supply_lines_at_partition_contact() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mounted_stock_money_crate_deposits_authored_supply_lines_at_partition_contact",
        || {
            let install = std::env::var_os("GENERALS_RETAIL_CONTENT_ROOT")
                .map(std::path::PathBuf::from)
                .or_else(crate::map_frame_scenario::find_lone_eagle_retail_install)
                .expect("a real mounted retail install is required");
            let startup =
                crate::parallel_harness::RetailWorldDefinitions::load_from_install(Some(&install))
                    .unwrap_or_else(|error| panic!("{}: {error}", install.display()));
            let mut world = GameLogic::new();
            startup.admit(&mut world).unwrap();
            admit_presentation(&mut world);
            world.add_player(Player::new(1, Team::USA, "RetailMoneyReceiver", true));
            world.frame = 40;

            let picker = world
                .create_object_for_player("AmericaTankCrusader", 1, AWAY)
                .unwrap();
            pickup(&mut world, picker, "100DollarCrate", 100);
            world.restore_completed_player_upgrade(1, UPGRADE_AMERICA_SUPPLY_LINES);
            pickup(&mut world, picker, "SupplyDropZoneCrate", 275);
        },
    );
}
