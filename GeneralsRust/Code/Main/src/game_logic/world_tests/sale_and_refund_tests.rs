//! Owner-side sale, refund and healing receipts.

use crate::game_logic::GameLogic;
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

#[test]
fn cancelling_production_publishes_the_owners_refund_once() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EconRef");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object EconFac\nEnd\nObject EconUnit\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Seed a local player with known cash.
    let pid = logic
        .get_players()
        .values()
        .find(|p| p.team == Team::USA)
        .map(|p| p.id)
        .expect("usa player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.resources.supplies = 1000;
    }
    {
        let mut t = ThingTemplate::new("EconFac");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("EconFac".into(), t);
    }
    {
        let mut t = ThingTemplate::new("EconUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("EconUnit".into(), t);
    }
    let fac = logic
        .create_object("EconFac", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("fac");
    // Queue a unit with cost via building_data if available.
    {
        use crate::game_logic::Resources;
        use crate::game_logic::buildings::{
            BuildingData, BuildingType, ProductionItem, ProductionKind,
        };
        let o = logic.host_object_mut(fac).expect("f");
        if o.building_data.is_none() {
            o.building_data = Some(BuildingData::new(BuildingType::Barracks));
        }
        if let Some(bd) = o.building_data.as_mut() {
            bd.production_queue.push(ProductionItem {
                runtime_entry_identity: 0,
                template_name: "EconUnit".into(),
                progress: 0.0,
                total_time: 10.0,
                construction_frames: 0,
                cost: Resources {
                    supplies: 250,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: ProductionKind::Unit,
            });
        }
    }
    assert!(logic.cancel_production(fac, "EconUnit".into()));
    let p = logic.get_player(pid).expect("p");
    // CPP ProductionUpdate.cpp:456–464 deposits immediately to the
    // controlling player before removing the production queue entry.
    assert_eq!(p.resources.supplies, 1250);

    assert_eq!(p.effective_supplies(), 1250);
    assert!(
        logic
            .host_object(fac)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    let evs = {
        logic.economy_events.complete_boundary();
        logic.economy_events.take_completed()
    };
    assert!(
        evs.iter()
            .any(|event| event.player_id == pid && event.supplies == 1250)
    );
    assert!(
        !logic.cancel_production(fac, "EconUnit".into()),
        "already cancelled queue cannot refund twice"
    );
    assert_eq!(logic.get_player(pid).unwrap().resources.supplies, 1250);
    let frame = PresentationFrame::try_build_from_logic(&mut logic, pid).unwrap();
    assert_eq!(frame.local_supplies, 1250);
}

#[test]
fn sale_start_sets_percent_and_status_immediately() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("SellPct");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("SellPad")
            .kind_of("STRUCTURE")
            .health(500.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("SellPad", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).unwrap();
        o.construction_percent = 1.0;
        o.set_status_under_construction(false);
    }
    assert!(logic.start_sell_object(oid).unwrap());
    // CPP sell start changes the canonical percent immediately to 0.999.
    assert!(
        (logic
            .host_objects()
            .lookup(&oid)
            .unwrap()
            .construction_percent
            - 0.999)
            .abs()
            < 1e-4,
        "host sell start must set 0.999 residual"
    );

    let object = logic.host_object(oid).unwrap();
    assert!(object.status.sold);
    assert!(object.status.unselectable);
    assert!(!object.status.under_construction);
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let sold = frame.objects.iter().find(|row| row.id == oid).unwrap();
    assert!(sold.sold && sold.unselectable && !sold.under_construction);
    assert!((sold.construction_percent - 0.999).abs() < 1e-4);
}

#[test]
fn selling_retains_signed_owner_percent_until_retirement() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("SellNegPct");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("SellPad")
            .kind_of("STRUCTURE MP_COUNT_FOR_VICTORY")
            .health(500.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SellPad");
        // Retail sellable structures author KINDOF_MP_COUNT_FOR_VICTORY
        // (FactionBuilding.ini; synthesized structures must carry it too —
        // buildings.rs:996-1007). Without the bit the skirmish
        // NO_BUILDINGS rule (C++ VictoryConditions.cpp:87-95 →
        // Team::hasAnyBuildings mask) defeats the sole owner on frame 0
        // and kill_player_for_victory destroys the pad mid-sell.
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        t.set_health(500.0);
        logic.templates.insert("SellPad".into(), t);
    }
    let oid = logic
        .create_object("SellPad", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("pad");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(logic.start_sell_object(oid).unwrap());

    // Advance past scaffold into negative deconstruction via full host tick
    // (frame + update_sell_list). Stop once percent is clearly negative.
    for _ in 0..200 {
        logic.update();
        if logic.host_object(oid).is_none() {
            break;
        }
        let pct = logic
            .host_object(oid)
            .map(|o| o.construction_percent)
            .unwrap_or(-1.0);
        if pct < -0.1 {
            break;
        }
    }
    let host_pct = logic
        .host_object(oid)
        .map(|o| o.construction_percent)
        .expect("still selling");
    assert!(
        host_pct < 0.0,
        "host sell percent should go negative, got {host_pct}"
    );

    let frame_number = logic.get_frame();
    for _ in 0..2 {
        let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        let sold = frame.objects.iter().find(|row| row.id == oid).unwrap();
        assert!(sold.sold && sold.unselectable);
        assert_eq!(sold.construction_percent, 0.0);
        assert_eq!(
            logic.host_object(oid).unwrap().construction_percent,
            host_pct
        );
    }
    assert_eq!(logic.get_frame(), frame_number);
    for _ in 0..240 {
        if logic.host_object(oid).is_none() {
            break;
        }
        logic.update();
    }
    assert!(logic.host_object(oid).is_none());
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert!(frame.objects.iter().all(|row| row.id != oid));
}

#[test]
fn sell_command_uses_authored_refund_value_through_world_completion() {
    use crate::command_system::{
        CommandResult, CommandSystem, CommandType, GameCommand, ModifierKeys,
    };
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use game_engine::common::global_data::with_global_data_restored;
    use std::time::SystemTime;

    with_global_data_restored(|| {
        let command_system = CommandSystem::new();
        let mut logic = GameLogic::new();
        // Make the ordinary percentage deliberately different from the
        // authored refund so this cannot pass through the fallback path.
        // GlobalData SellPercentage is admitted into the receiving world (BuildAssistant::sellObject).
        logic.admit_sell_percentage(0.25);
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 0;
        logic.add_player(player);

        let mut structure = ThingTemplate::new("RefundOverrideStructure");
        structure
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        // C++ Object INI `RefundValue = 650`: this is an exact credit, not
        // 25% of the build cost.
        structure.refund_value = 650;
        logic
            .templates
            .insert("RefundOverrideStructure".to_string(), structure);

        let structure_id = logic
            .create_object_for_player("RefundOverrideStructure", 0, glam::Vec3::ZERO)
            .expect("player-owned structure");
        {
            let object = logic.host_object_mut(structure_id).expect("structure");
            object.set_status_under_construction(false);
            object.construction_percent = 1.0;
        }

        let sell = GameCommand {
            command_type: CommandType::Sell {
                object_id: structure_id,
            },
            player_id: 0,
            command_id: 9001,
            timestamp: SystemTime::now(),
            selected_units: vec![structure_id],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            command_system.execute_command(&sell, &mut logic),
            CommandResult::Success,
            "the physical sell command must enter the normal world sell lifecycle"
        );

        for frame in 1..=240 {
            logic.set_current_frame(frame);
            logic.update_sell_list();
            logic.process_destroy_list();
            if logic.host_object(structure_id).is_none() {
                break;
            }
        }

        assert!(
            logic.host_object(structure_id).is_none(),
            "sell lifecycle must finish by destroying the structure"
        );
        assert_eq!(
            logic.get_player(0).expect("owner").effective_supplies(),
            650,
            "C++ RefundValue must override BuildCost × SellPercentage"
        );

        let mut fallback = ThingTemplate::new("DefaultRefundStructure");
        fallback
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        // Default `refund_value = 0` must retain the ordinary percentage
        // path, rather than being mistaken for a literal zero-credit sale.
        logic
            .templates
            .insert("DefaultRefundStructure".to_string(), fallback);
        let fallback_id = logic
            .create_object_for_player("DefaultRefundStructure", 0, glam::Vec3::X)
            .expect("player-owned fallback structure");
        {
            let object = logic
                .host_object_mut(fallback_id)
                .expect("fallback structure");
            object.set_status_under_construction(false);
            object.construction_percent = 1.0;
        }
        let fallback_sell = GameCommand {
            command_type: CommandType::Sell {
                object_id: fallback_id,
            },
            player_id: 0,
            command_id: 9002,
            timestamp: SystemTime::now(),
            selected_units: vec![fallback_id],
            modifier_keys: ModifierKeys::default(),
        };
        assert!(
            command_system.execute_command(&fallback_sell, &mut logic) == CommandResult::Success
        );
        for frame in 241..=480 {
            logic.set_current_frame(frame);
            logic.update_sell_list();
            logic.process_destroy_list();
            if logic.host_object(fallback_id).is_none() {
                break;
            }
        }
        assert!(logic.host_object(fallback_id).is_none());
        assert_eq!(
            logic.get_player(0).expect("owner").effective_supplies(),
            900,
            "zero RefundValue must use 25% of the 1000 BuildCost"
        );
    });
}

#[test]
fn body_heal_commits_and_records_absolute_health() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("HealAuth");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object HealU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("HealU");
        t.add_kind_of(KindOf::Infantry);
        t.set_health(100.0);
        logic.templates.insert("HealU".into(), t);
    }
    let oid = logic
        .create_object("HealU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).unwrap();
        o.health.current = 40.0;
        o.health.maximum = 100.0;
    }
    let frame = logic.get_frame();
    let (actor, health_events) = logic.host_object_and_health_events_mut(oid).unwrap();
    actor.heal(60.0, health_events, frame);
    assert!(
        (logic.host_objects().lookup(&oid).unwrap().health.current - 100.0).abs() < 1e-3,
        "body healing commits synchronously"
    );
    let evs = logic.health_events.drain_heal();
    assert!(
        evs.iter()
            .any(|e| e.target == oid && (e.health - 100.0).abs() < 1e-3),
        "heal log must carry absolute HP; got {evs:?}"
    );
}
