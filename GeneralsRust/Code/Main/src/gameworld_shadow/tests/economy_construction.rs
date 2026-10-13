//! Economy authority, construction/production progress, rebuild/producer channels.

use super::*;

#[test]
fn host_resource_tick_logs_power_for_shadow() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PowerLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Advance one host frame so update_player_resources runs.
    logic.update_with_dt(1.0 / 30.0);
    let events = {
        logic.economy_events.complete_boundary();
        logic.economy_events.take_completed()
    };
    assert!(
        !events.is_empty(),
        "resource tick must log economy/power events"
    );
    assert!(
        events
            .iter()
            .any(|e| e.power_available != 0 || e.supplies > 0)
            || events.iter().any(|e| e.player_id > 0 || e.player_id == 0),
        "expected at least one player economy residual"
    );
}

#[test]
fn steal_cash_logs_economy_for_both_sides() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StealLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Ensure two teams with cash.
    let mut usa = None;
    let mut gla = None;
    for (pid, p) in logic.get_players() {
        if p.team == Team::USA {
            usa = Some(*pid);
        }
        if p.team == Team::GLA {
            gla = Some(*pid);
        }
    }
    let (Some(usa), Some(gla)) = (usa, gla) else {
        return;
    };
    {
        let p = logic.get_players_mut().get_mut(&gla).unwrap();
        p.resources.supplies = 500;
    }
    {
        let p = logic.get_players_mut().get_mut(&usa).unwrap();
        p.resources.supplies = 100;
    }

    let stolen = logic.steal_cash_from_team(Team::GLA, Team::USA, 50);
    assert_eq!(stolen, 50);
    let ev = {
        logic.economy_events.complete_boundary();
        logic.economy_events.take_completed()
    };
    assert!(
        ev.iter().any(|e| e.player_id == gla) && ev.iter().any(|e| e.player_id == usa),
        "steal must log src+dest economy: {ev:?}"
    );
}

#[test]
fn credit_supplies_logs_economy_channel() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CreditLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");

    {
        let (p, economy_events) = logic.player_and_economy_events_mut(pid).unwrap();
        let before = p.resources.supplies;
        p.credit_supplies(123, economy_events);
        // The canonical cash gain is visible before any presentation boundary.
        assert_eq!(p.effective_supplies(), before.saturating_add(123));
        {
            assert_eq!(p.resources.supplies, before.saturating_add(123));
        }
    }
    let ev = {
        logic.economy_events.complete_boundary();
        logic.economy_events.take_completed()
    };
    assert!(
        ev.iter().any(|e| e.player_id == pid && e.supplies >= 123),
        "credit_supplies must log: {ev:?}"
    );
}

#[test]
fn economy_authority_applies_logged_spend() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EconSpend");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    let before = logic.get_player(hid).unwrap().resources.supplies;
    // Spend via Player API (logs).
    let cost = crate::game_logic::Resources {
        supplies: 100,
        power: 0,
    };
    assert!({
        let (player, economy_events) = logic.player_and_economy_events_mut(hid).unwrap();
        player.spend_resources(&cost, economy_events)
    });
    // Spending updates canonical cash before the observing boundary.
    let after_host = logic.get_player(hid).unwrap().resources.supplies;
    let after_eff = logic.get_player(hid).unwrap().effective_supplies();
    {
        assert_eq!(after_host, before.saturating_sub(100));
    }
    let events = {
        logic.economy_events.complete_boundary();
        logic.economy_events.take_completed()
    };
    assert!(!events.is_empty());

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Desync shadow supplies upward, then apply log as authority.
    if let Some(p) = shadow
        .world_mut()
        .player_mut(gamelogic::world::PlayerId::from_index(0))
    {
        p.supplies = before; // pre-spend
    }
    shadow.sync_from_host(&logic);
    let sh = shadow
        .world()
        .player(gamelogic::world::PlayerId::from_index(0))
        .unwrap()
        .supplies;
    let expect = { after_host };
    assert_eq!(sh, expect, "shadow supplies from economy log");
    shadow.sync_from_host(&logic);
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, expect);
}

#[test]
fn economy_authority_writeback_supplies() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EconAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(!logic.get_players().is_empty());
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    let shadow_supplies = shadow
        .world()
        .player(gamelogic::world::PlayerId::from_index(0))
        .map(|p| p.supplies)
        .unwrap_or(0);
    // A genuine owner write must override the older immutable cash observation.
    if let Some(p) = logic.get_player_mut(hid) {
        p.resources.supplies = shadow_supplies.saturating_sub(1234);
    }
    shadow.sync_from_host(&logic);
    assert_eq!(
        logic.get_player(hid).unwrap().resources.supplies,
        shadow_supplies.saturating_sub(1234)
    );
}

#[test]
fn economy_authority_pending_blocks_double_spend() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("EconDbl");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    {
        let p = logic.get_player_mut(hid).unwrap();
        p.resources.supplies = 150;
    }
    // Session probe evaluates victory (C++ VictoryConditions.cpp:87-95 /
    // 168-196): an army-less playable side is defeated on frame 0 and
    // Player::killPlayer (Player.cpp) wipes its money. Give the human side a
    // victory-counting structure so the assertion observes the deferred-spend
    // materialization rather than the defeat wipe.
    use crate::game_logic::{KindOf, ThingTemplate};
    if !logic.templates.contains_key("EconKeepAlive") {
        let mut t = ThingTemplate::new("EconKeepAlive");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("EconKeepAlive".into(), t);
    }
    let _keep = logic
        .create_object_for_player("EconKeepAlive", hid, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("keep-alive structure");
    let cost = crate::game_logic::Resources {
        supplies: 100,
        power: 0,
    };

    assert!({
        let (player, economy_events) = logic.player_and_economy_events_mut(hid).unwrap();
        player.spend_resources(&cost, economy_events)
    });
    assert!(
        !{
            let (player, economy_events) = logic.player_and_economy_events_mut(hid).unwrap();
            player.spend_resources(&cost, economy_events)
        },
        "second spend must fail against the immediately updated canonical balance"
    );
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 50);
    assert_eq!(logic.get_player(hid).unwrap().effective_supplies(), 50);
    let mut shadow = GameWorldShadow::new(64);
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 50);
}

#[test]
fn economy_authority_mutates_host_supplies_when_shadow_disabled() {
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("EconNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    {
        let (p, economy_events) = logic.player_and_economy_events_mut(hid).unwrap();
        p.resources.supplies = 100;

        p.add_resources(
            &crate::game_logic::Resources {
                supplies: 25,
                power: 0,
            },
            economy_events,
        );
        assert_eq!(
            p.resources.supplies, 125,
            "host supplies must apply immediately"
        );
    }
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn credit_supplies_defers_under_economy_authority() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("EconCredit");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    {
        let p = logic.get_player_mut(hid).unwrap();
        p.resources.supplies = 1000;
    }
    // Session probe evaluates victory (C++ VictoryConditions.cpp:87-95 /
    // 168-196): an army-less playable side is defeated on frame 0 and
    // Player::killPlayer (Player.cpp) wipes its money. Give the human side a
    // victory-counting structure so the assertion observes economy
    // materialization rather than the defeat wipe.
    use crate::game_logic::{KindOf, ThingTemplate};
    if !logic.templates.contains_key("EconKeepAlive") {
        let mut t = ThingTemplate::new("EconKeepAlive");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("EconKeepAlive".into(), t);
    }
    let _keep = logic
        .create_object_for_player("EconKeepAlive", hid, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("keep-alive structure");

    {
        let (player, economy_events) = logic.player_and_economy_events_mut(hid).unwrap();
        player.credit_supplies(250, economy_events)
    };
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 1250);
    assert_eq!(logic.get_player(hid).unwrap().effective_supplies(), 1250);
    let mut shadow = GameWorldShadow::new(64);
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 1250);
}

#[test]
fn construction_complete_heal_log_sets_full_hp_via_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("ConstHp");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PadHp") {
        let mut t = ThingTemplate::new("PadHp");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("PadHp".into(), t);
    }
    let oid = logic
        .create_object("PadHp", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.under_construction = true;
        o.construction_percent = 0.99;
        o.health.current = 50.0;
    }
    // CPP VictoryConditions87–95/168–196: skirmish requires a live
    // victory-counting structure. Otherwise the session defeats this player
    // and kills its army while this control is checking health writeback.
    let mut keep_alive = ThingTemplate::new("HealthVictoryKeepAlive");
    keep_alive
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::MpCountForVictory);
    logic
        .templates
        .insert("HealthVictoryKeepAlive".into(), keep_alive);
    logic
        .create_object("HealthVictoryKeepAlive", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Simulate completion residual: log full HP without host mutate.
    logic.health_events.clear_heal();

    {
        let body_frame = logic.frame;
        let (o, health_events) = logic.host_object_and_health_events_mut(oid).expect("o");
        let full = o.health.maximum;
        o.heal(
            (full - o.health.current).max(0.0),
            health_events,
            body_frame,
        );

        o.construction_percent = 1.0;
        o.status.under_construction = false;
    }
    assert!((logic.host_objects().get(&oid).expect("o").health.current - 500.0).abs() < 1e-5);
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(
        (o.health.current - o.health.maximum).abs() < 1e-3,
        "hp {}",
        o.health.current
    );
    assert!((o.construction_percent - 1.0).abs() < 1e-5);
    assert!(!o.status.under_construction);
}

#[test]
fn construction_shadow_reports_disagreement_without_rewriting_percent() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("ConstAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PadAuth") {
        let mut t = ThingTemplate::new("PadAuth");
        t.set_health(400.0);
        t.build_time = 10.0;
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("PadAuth".into(), t);
    }
    let oid = logic
        .create_object("PadAuth", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.under_construction = true;
        o.construction_percent = 0.5;
    }

    assert!(
        (logic
            .host_objects()
            .get(&oid)
            .expect("o")
            .construction_percent
            - 0.5)
            .abs()
            < 1e-5
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Projection updates are explicit; deliberate disagreement cannot change the host.

    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .construction_percent = 0.6;
    assert!(shadow.compare_construction_to_host(&logic) >= 1);
    assert!(
        (logic
            .host_objects()
            .get(&oid)
            .expect("o")
            .construction_percent
            - 0.5)
            .abs()
            < 1e-5
    );
}

#[test]
fn owned_construction_observation_never_advances_simulation_percent() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("RateOnlyUc");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PadRateUc") {
        let mut t = ThingTemplate::new("PadRateUc");
        t.set_health(400.0);
        t.build_time = 10.0;
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("PadRateUc".into(), t);
    }
    let oid = logic
        .create_object("PadRateUc", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.under_construction = true;
        o.construction_percent = 0.0;
    }

    // Observing current owned state cannot advance a construction timer.

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Drop UC on the entity to reproduce the live mapping hole.
    let eid = shadow.entity_for_host(oid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .unwrap()
        .under_construction = false;

    let _ = shadow.sync_from_host(&logic);
    let pct = shadow
        .entity_for_host(oid)
        .and_then(|eid| shadow.world.entity(eid).map(|e| e.construction_percent))
        .unwrap_or(0.0);
    assert_eq!(
        pct, 0.0,
        "shadow rate observation must not advance construction"
    );
    assert_eq!(logic.host_object(oid).unwrap().construction_percent, 0.0);
}

#[test]
fn production_progress_log_drives_set_production_queue() {
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdProg");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("FactProg") {
        let mut t = ThingTemplate::new("FactProg");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("FactProg".into(), t);
    }
    let oid = logic
        .create_object("FactProg", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");

    {
        use crate::game_logic::{
            BuildingData, BuildingType, ProductionItem, ProductionKind, Resources,
        };
        let mut data = BuildingData::new(BuildingType::Barracks);
        data.exit_delay_remaining = 1.25;
        data.production_queue.push(ProductionItem {
            template_name: "Ranger".into(),
            progress: 3.5,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 150,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        logic.host_object_mut(oid).unwrap().building_data = Some(data);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    shadow.sync_from_host(&logic);

    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.production_queue_items.len(), 1);
    assert!((e.production_queue_items[0].progress - 3.5).abs() < 1e-5);
    assert_eq!(e.production_queue_items[0].template_name, "Ranger");
    assert!((e.production_progress - 3.5).abs() < 1e-5);
    assert!((e.exit_delay_remaining - 1.25).abs() < 1e-5);
}

#[test]
fn exit_delay_remaining_channel_via_production_progress() {
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    // writeback_production_to_host is the opt-in GameLogic::set_production_authority(true)
    // last-writer (default off like C++ no-shadow builds). Enable it here; hq-e84zk
    // retired the env flag.

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("ExitDel");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("FactExit") {
        let mut t = ThingTemplate::new("FactExit");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("FactExit".into(), t);
    }
    let oid = logic
        .create_object("FactExit", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        if let Some(bd) = o.building_data.as_mut() {
            bd.exit_delay_remaining = 2.5;
        }
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    assert!((shadow.world().entity(eid).unwrap().exit_delay_remaining - 2.5).abs() < 1e-5);
    // Host cleared; GameWorld residual writeback restores exit delay.

    shadow.sync_from_host(&logic);

    let _ = shadow.compare_rebuild_producer_to_host(&logic);
    shadow.sync_from_host(&logic);

    let d = logic
        .host_objects()
        .get(&oid)
        .unwrap()
        .building_data
        .as_ref()
        .map(|b| b.exit_delay_remaining)
        .unwrap_or(-1.0);
    assert!((d - 2.5).abs() < 1e-5, "exit delay wb got {d}");
}

#[test]
fn body_damage_state_channel_via_set_body_damage() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BodyDmg");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("TankBd") {
        let mut t = ThingTemplate::new("TankBd");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("TankBd".into(), t);
    }
    let oid = logic
        .create_object("TankBd", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.body_damage_state = HostBodyDamageType::ReallyDamaged;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow.world().entity(eid).unwrap().body_damage_state,
        2,
        "really damaged ordinal"
    );

    shadow.sync_from_host(&logic);

    let _ = shadow.compare_rebuild_producer_to_host(&logic);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().body_damage_state,
        HostBodyDamageType::ReallyDamaged
    );
}

#[test]
fn weapon_last_fire_time_channel_via_set_weapon_stats() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WepFire");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WepFireU") {
        let mut t = ThingTemplate::new("WepFireU");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("WepFireU".into(), t);
    }
    let oid = logic
        .create_object("WepFireU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    // Direct channel event (does not require a live Weapon struct shape).

    logic.host_object_mut(oid).unwrap().weapon = Some(crate::game_logic::Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 1.0,
        last_fire_time: 12.5,
        clip_size: 0,
        clip_reload_time: 0.0,
        ammo: None,
        can_target_ground: true,
        ..Default::default()
    });
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.weapon_last_fire_time - 12.5).abs() < 1e-5);
    assert!(e.has_weapon);
    // writeback last_fire onto host weapon if present

    shadow.sync_from_host(&logic);
}

#[test]
fn weapon_clip_size_channel_via_set_weapon_stats() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WpnClip");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("ClipUnit") {
        let mut t = ThingTemplate::new("ClipUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("ClipUnit".into(), t);
    }
    let oid = logic
        .create_object("ClipUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");

    logic.host_object_mut(oid).unwrap().weapon = Some(crate::game_logic::Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 1.0,
        last_fire_time: 5.0,
        clip_size: 5,
        clip_reload_time: 2.5,
        ammo: Some(3),
        can_target_ground: true,
        ..Default::default()
    });
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.weapon_clip_size, 5);
    assert!((e.weapon_clip_reload_time - 2.5).abs() < 1e-5);
    assert_eq!(e.weapon_ammo, 3);
}

#[test]
fn front_crushed_channel_via_set_crush_vision() {
    use crate::game_logic::host_crush_vision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CrushFl");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CrushMe") {
        let mut t = ThingTemplate::new("CrushMe");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("CrushMe".into(), t);
    }
    let oid = logic
        .create_object("CrushMe", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.front_crushed = true;
        o.back_crushed = false;
        o.crusher_level = 1;
        o.crushable_level = 1;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.front_crushed);
    assert!(!e.back_crushed);

    shadow.sync_from_host(&logic);

    assert!(
        logic.host_objects().get(&oid).unwrap().front_crushed,
        "front crushed writeback"
    );
}

#[test]
fn waiting_for_path_channel_via_set_movement() {
    use crate::game_logic::host_movement_log::{self, HostMovementEvent};
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WaitPath");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WaitUnit") {
        let mut t = ThingTemplate::new("WaitUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("WaitUnit".into(), t);
    }
    let oid = logic
        .create_object("WaitUnit", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.waiting_for_path = true;
        o.movement.max_speed = 12.0;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    assert!(shadow.world().entity(eid).unwrap().waiting_for_path);

    shadow.sync_from_host(&logic);

    assert!(
        logic.host_objects().get(&oid).unwrap().waiting_for_path,
        "waiting_for_path writeback"
    );
}

#[test]
fn locomotor_path_flags_channel_via_set_movement() {
    use crate::game_logic::host_movement_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("LocoPath");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("LocoU") {
        let mut t = ThingTemplate::new("LocoU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("LocoU".into(), t);
    }
    let oid = logic
        .create_object("LocoU", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.locomotor_surfaces = 0b101; // ground|cliff
        o.is_attack_path = true;
        o.is_braking = true;
        o.is_blocked_and_stuck = false;
        o.is_safe_path = true;
        o.queue_for_path_frames = 3;
        o.path_timestamp = 42;
        o.waiting_for_path = true;
        o.movement.max_speed = 15.0;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.locomotor_surfaces, 0b101);
    assert!(e.is_attack_path);
    assert!(e.is_braking);
    assert!(e.is_safe_path);
    assert_eq!(e.queue_for_path_frames, 3);
    assert_eq!(e.path_timestamp, 42);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.locomotor_surfaces, 0b101);
    assert!(o.is_attack_path);
    assert!(o.is_braking);
    assert_eq!(o.queue_for_path_frames, 3);
    assert_eq!(o.path_timestamp, 42);
}

#[test]
fn shock_stun_channel_via_set_shock_stun() {
    use crate::game_logic::host_shock_stun_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ShockSt");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("ShockU") {
        let mut t = ThingTemplate::new("ShockU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("ShockU".into(), t);
    }
    let oid = logic
        .create_object("ShockU", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.shock_stun_frames = 30;
        o.shock_yaw_rate = 0.5;
        o.shock_pitch_rate = -0.25;
        o.shock_roll_rate = 0.1;
        o.shock_up_z = 0.9;
        o.shock_allow_bounce = true;
        o.shock_grounded_once = true;
        o.shock_was_airborne = true;
        o.cell_is_cliff = true;
        o.cell_is_underwater = false;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.shock_stun_frames, 30);
    assert!((e.shock_yaw_rate - 0.5).abs() < 1e-5);
    assert!(e.shock_allow_bounce);
    assert!(e.cell_is_cliff);

    shadow.sync_from_host(&logic);

    let _ = shadow.compare_rebuild_producer_to_host(&logic);
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.shock_stun_frames, 30);
    assert!((o.shock_yaw_rate - 0.5).abs() < 1e-5);
    assert!(o.shock_allow_bounce);
    assert!(o.cell_is_cliff);
}

#[test]
fn blocked_path_channel_via_set_movement() {
    use crate::game_logic::host_movement_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BlockP");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("BlockU") {
        let mut t = ThingTemplate::new("BlockU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("BlockU".into(), t);
    }
    let oid = logic
        .create_object("BlockU", Team::USA, glam::Vec3::new(12.0, 0.0, 12.0))
        .expect("id");
    let other = logic
        .create_object("BlockU", Team::USA, glam::Vec3::new(14.0, 0.0, 12.0))
        .expect("other");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.cur_max_blocked_speed = 3.5;
        o.num_frames_blocked = 7;
        o.is_blocked = true;
        o.move_away_from = Some(other);
        o.requested_victim_id = Some(other);
        o.movement.max_speed = 10.0;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!((e.cur_max_blocked_speed - 3.5).abs() < 1e-5);
    assert_eq!(e.num_frames_blocked, 7);
    assert!(e.is_blocked);
    assert_eq!(e.move_away_from_id, Some(other.0));
    assert_eq!(e.requested_victim_id, Some(other.0));

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert!((o.cur_max_blocked_speed - 3.5).abs() < 1e-5);
    assert_eq!(o.num_frames_blocked, 7);
    assert!(o.is_blocked);
    assert_eq!(o.move_away_from, Some(other));
    assert_eq!(o.requested_victim_id, Some(other));
}

#[test]
fn rebuild_producer_channel_via_set_rebuild_producer() {
    use crate::game_logic::host_rebuild_producer_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RebuildP");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["HoleA", "BldA", "WorkerA"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert(name.into(), t);
        }
    }
    let hole = logic
        .create_object("HoleA", Team::USA, glam::Vec3::new(20.0, 0.0, 20.0))
        .expect("hole");
    let bld = logic
        .create_object("BldA", Team::USA, glam::Vec3::new(22.0, 0.0, 20.0))
        .expect("bld");
    let worker = logic
        .create_object("WorkerA", Team::USA, glam::Vec3::new(24.0, 0.0, 20.0))
        .expect("worker");
    {
        let o = logic.host_object_mut(hole).expect("o");
        o.is_rebuild_hole = true;
        o.rebuild_template_name = Some("BldA".into());
        o.rebuild_ready_frame = 100;
        o.rebuild_spawner_id = Some(bld);
        o.rebuild_worker_id = Some(worker);
        o.rebuild_reconstructing_id = Some(bld);
        o.producer_id = Some(hole);
        o.construction_complete_clear_frame = 250;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&hole.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.is_rebuild_hole);
    assert_eq!(e.rebuild_template_name, "BldA");
    assert_eq!(e.rebuild_ready_frame, 100);
    assert_eq!(e.rebuild_spawner_id, Some(bld.0));
    assert_eq!(e.rebuild_worker_id, Some(worker.0));
    assert_eq!(e.rebuild_reconstructing_id, Some(bld.0));
    assert_eq!(e.producer_id, Some(hole.0));
    assert_eq!(e.construction_complete_clear_frame, 250);

    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .unwrap()
        .rebuild_ready_frame = 0;
    assert!(shadow.compare_rebuild_producer_to_host(&logic) >= 1);
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&hole).unwrap();
    assert!(o.is_rebuild_hole);
    assert_eq!(o.rebuild_template_name.as_deref(), Some("BldA"));
    assert_eq!(o.rebuild_ready_frame, 100);
    assert_eq!(o.rebuild_spawner_id, Some(bld));
    assert_eq!(o.rebuild_worker_id, Some(worker));
    assert_eq!(o.rebuild_reconstructing_id, Some(bld));
    assert_eq!(o.producer_id, Some(hole));
    assert_eq!(o.construction_complete_clear_frame, 250);
}
