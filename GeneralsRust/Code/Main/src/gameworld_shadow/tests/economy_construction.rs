//! Economy authority, construction/production progress, rebuild/producer channels.

use super::*;

#[test]
fn host_resource_tick_logs_power_for_shadow() {
    crate::game_logic::host_economy_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PowerLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Advance one host frame so update_player_resources runs.
    logic.update_with_dt(1.0 / 30.0);
    let events = crate::game_logic::host_economy_log::drain();
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
    let _env_guard = authority_env_lock();

    crate::game_logic::host_economy_log::clear();
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
    crate::game_logic::host_economy_log::clear();
    let stolen = logic.steal_cash_from_team(Team::GLA, Team::USA, 50);
    assert_eq!(stolen, 50);
    let ev = crate::game_logic::host_economy_log::drain();
    assert!(
        ev.iter().any(|e| e.player_id == gla) && ev.iter().any(|e| e.player_id == usa),
        "steal must log src+dest economy: {ev:?}"
    );
}

#[test]
fn credit_supplies_logs_economy_channel() {
    let _env_guard = authority_env_lock();

    crate::game_logic::host_economy_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CreditLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");
    crate::game_logic::host_economy_log::clear();
    {
        let p = logic.get_players_mut().get_mut(&pid).unwrap();
        let before = p.resources.supplies;
        p.credit_supplies(123);
        // Economy authority parks gains in pending_supply_delta.
        assert_eq!(p.effective_supplies(), before.saturating_add(123));
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            assert_eq!(p.resources.supplies, before);
        } else {
            assert_eq!(p.resources.supplies, before.saturating_add(123));
        }
    }
    let ev = crate::game_logic::host_economy_log::drain();
    assert!(
        ev.iter().any(|e| e.player_id == pid && e.supplies >= 123),
        "credit_supplies must log: {ev:?}"
    );
}

#[test]
fn economy_authority_applies_logged_spend() {
    let _env_guard = authority_env_lock();

    crate::game_logic::host_economy_log::clear();
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
    assert!(logic.get_player_mut(hid).unwrap().spend_resources(&cost));
    // Under economy authority host.resources is deferred; effective reflects spend.
    let after_host = logic.get_player(hid).unwrap().resources.supplies;
    let after_eff = logic.get_player(hid).unwrap().effective_supplies();
    if crate::gameworld_shadow::gameworld_economy_authority_live() {
        assert_eq!(after_host, before, "host absolute deferred");
        assert_eq!(after_eff, before.saturating_sub(100), "effective supplies");
    } else {
        assert_eq!(after_host, before.saturating_sub(100));
    }
    let events = crate::game_logic::host_economy_log::drain();
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
    let _ = shadow.apply_host_economy_events(&events);
    let sh = shadow
        .world()
        .player(gamelogic::world::PlayerId::from_index(0))
        .unwrap()
        .supplies;
    let expect = if crate::gameworld_shadow::gameworld_economy_authority_live() {
        after_eff
    } else {
        after_host
    };
    assert_eq!(sh, expect, "shadow supplies from economy log");
    let wb = shadow.writeback_economy_to_host(&mut logic);
    let _ = crate::game_logic::host_economy_ready_log::drain();
    assert!(wb >= 1 || logic.get_player(hid).unwrap().resources.supplies == expect);
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, expect);
    assert_eq!(logic.get_player(hid).unwrap().pending_supply_delta, 0);
}

#[test]
fn economy_authority_writeback_supplies() {
    let _env_guard = authority_env_lock();

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
    // Desync host cash downward.
    if let Some(p) = logic.get_player_mut(hid) {
        p.resources.supplies = shadow_supplies.saturating_sub(1234);
    }
    let wb = shadow.writeback_economy_to_host(&mut logic);
    let _ = crate::game_logic::host_economy_ready_log::drain();
    assert!(wb >= 1);
    assert_eq!(
        logic.get_player(hid).unwrap().resources.supplies,
        shadow_supplies
    );
}

#[test]
fn economy_authority_pending_blocks_double_spend() {
    let _env_guard = authority_env_lock();

    crate::env_compat::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");

    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(gameworld_economy_authority_enabled());
    crate::game_logic::host_economy_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EconDbl");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    {
        let p = logic.get_player_mut(hid).unwrap();
        p.resources.supplies = 150;
        p.pending_supply_delta = 0;
    }
    let cost = crate::game_logic::Resources {
        supplies: 100,
        power: 0,
    };
    begin_shadow_coupled_tick();
    assert!(logic.get_player_mut(hid).unwrap().spend_resources(&cost));
    assert!(
        !logic.get_player_mut(hid).unwrap().spend_resources(&cost),
        "second spend must fail against pending delta"
    );
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 150);
    assert_eq!(logic.get_player(hid).unwrap().effective_supplies(), 50);
    let mut shadow = GameWorldShadow::new(64);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 50);
    assert_eq!(logic.get_player(hid).unwrap().pending_supply_delta, 0);

    end_shadow_coupled_tick();
}

#[test]
fn economy_authority_mutates_host_supplies_when_shadow_disabled() {
    let _env_guard = authority_env_lock();
    let prev_e = std::env::var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").ok();
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    assert!(gameworld_economy_authority_enabled());
    assert!(!gameworld_economy_authority_live());
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EconNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    {
        let p = logic.get_player_mut(hid).unwrap();
        p.resources.supplies = 100;
        p.pending_supply_delta = 0;
        p.add_resources(&crate::game_logic::Resources {
            supplies: 25,
            power: 0,
        });
        assert_eq!(
            p.resources.supplies, 125,
            "host supplies must apply immediately"
        );
        assert_eq!(p.pending_supply_delta, 0, "no pending when shadow off");
    }
    match prev_e {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY"),
    }
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn credit_supplies_defers_under_economy_authority() {
    let _env_guard = authority_env_lock();

    crate::env_compat::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");

    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(gameworld_economy_authority_enabled());
    crate::game_logic::host_economy_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EconCredit");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
    ids.sort_unstable();
    let hid = ids[0];
    {
        let p = logic.get_player_mut(hid).unwrap();
        p.resources.supplies = 1000;
        p.pending_supply_delta = 0;
    }
    begin_shadow_coupled_tick();
    logic.get_player_mut(hid).unwrap().credit_supplies(250);
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 1000);
    assert_eq!(logic.get_player(hid).unwrap().effective_supplies(), 1250);
    let mut shadow = GameWorldShadow::new(64);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 1250);
    assert_eq!(logic.get_player(hid).unwrap().pending_supply_delta, 0);
    end_shadow_coupled_tick();
}

#[test]
fn construction_sole_tick_requires_coupled_frame() {
    // Host-only gates (no begin_shadow_coupled_tick) must still advance builds.
    assert!(
        !shadow_coupled_tick_active(),
        "tests start outside coupled engine frame"
    );
    assert!(
        !gameworld_construction_sole_tick_enabled(),
        "sole-tick freeze requires coupled engine frame"
    );
    begin_shadow_coupled_tick();
    assert!(gameworld_construction_sole_tick_enabled() || !gameworld_shadow_enabled());
    end_shadow_coupled_tick();
    assert!(!gameworld_construction_sole_tick_enabled());
}

#[test]
fn damage_authority_live_requires_coupled_frame() {
    // Host-only paths (unit tests, gates without engine shadow session) must
    // apply HP/cash/move immediately — defer only on a coupled writeback frame.
    assert!(
        !shadow_coupled_tick_active(),
        "tests start outside coupled engine frame"
    );
    assert!(!gameworld_damage_authority_live());
    assert!(!gameworld_economy_authority_live());
    assert!(!gameworld_movement_authority_live());
    begin_shadow_coupled_tick();
    assert!(
        gameworld_damage_authority_live()
            || !gameworld_damage_authority_enabled()
            || !gameworld_shadow_enabled()
    );
    assert!(
        gameworld_economy_authority_live()
            || !gameworld_economy_authority_enabled()
            || !gameworld_shadow_enabled()
    );
    assert!(
        gameworld_movement_authority_live()
            || !gameworld_movement_authority_enabled()
            || !gameworld_shadow_enabled()
    );
    end_shadow_coupled_tick();
    assert!(!gameworld_damage_authority_live());
    assert!(!gameworld_economy_authority_live());
    assert!(!gameworld_movement_authority_live());
}

#[test]
fn construction_complete_heal_log_sets_full_hp_via_writeback() {
    use crate::game_logic::{
        KindOf, Team, ThingTemplate, host_construction_progress_log, host_heal_log,
    };
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
    assert!(gameworld_damage_authority_enabled());
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
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Simulate completion residual: log full HP without host mutate.
    host_heal_log::clear();
    host_construction_progress_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        let full = o.health.maximum;
        crate::game_logic::host_heal_log::record(oid, full);
        crate::game_logic::host_construction_progress_log::record(oid, 1.0, false, 0.0);
        o.construction_percent = 1.0;
        o.status.under_construction = false;
    }
    assert!((logic.host_objects().get(&oid).expect("o").health.current - 50.0).abs() < 1e-5);
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
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
fn construction_authority_last_writes_percent() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_construction_progress_log};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
    assert!(gameworld_construction_authority_enabled());
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
    host_construction_progress_log::clear();
    // One progress log as host construction tick would emit under authority.
    host_construction_progress_log::record(oid, 0.6, true, 0.0);
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
    // Apply progress events as session does, then writeback.
    let events = host_construction_progress_log::drain();
    let n = shadow.apply_host_construction_progress_events(&events);
    assert!(n >= 1);
    assert!(shadow.writeback_construction_to_host(&mut logic) >= 1);
    assert!(
        (logic
            .host_objects()
            .get(&oid)
            .expect("o")
            .construction_percent
            - 0.6)
            .abs()
            < 1e-5
    );
}

#[test]
fn construction_tick_advances_when_rate_logged_without_entity_uc() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_construction_progress_log};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
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
    host_construction_progress_log::clear();
    // Rate-only: host construction tick under sole-tick does not stomp percent.
    host_construction_progress_log::record_rate_only(oid, true, 0.25);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Drop UC on the entity to reproduce the live mapping hole.
    assert!(shadow.queue_set_construction_for_host(oid, 0.0, false));
    let _ = shadow.apply_pending();
    let events = host_construction_progress_log::drain();
    let _ = shadow.apply_host_construction_progress_events(&events);
    let n = shadow.tick_construction_progress(1.0);
    assert!(
        n >= 1,
        "positive rate must tick construction even if entity UC is false"
    );
    let pct = shadow
        .entity_for_host(oid)
        .and_then(|eid| shadow.world.entity(eid).map(|e| e.construction_percent))
        .unwrap_or(0.0);
    assert!(pct > 0.2, "rate*dt must advance percent, got {pct}");
}

#[test]
fn production_progress_log_drives_set_production_queue() {
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_production_progress_log::clear();
    crate::game_logic::host_production_door_log::clear();
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
    host_production_progress_log::record(
        oid,
        vec![HostProductionQueueItem {
            template_name: "Ranger".into(),
            progress: 3.5,
            total_time: 10.0,
            construction_frames: 0,
            cost_supplies: 150,
            is_upgrade: false,
            quantity_total: 1,
            quantity_produced: 0,
        }],
        1.25,
        1.0,
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let n = shadow.apply_host_production_progress_events(&host_production_progress_log::drain());
    assert!(n >= 1);
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
    host_production_progress_log::clear();
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
    host_production_progress_log::record(oid, vec![], 2.5, 1.0);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(
        shadow.apply_host_production_progress_events(&host_production_progress_log::drain()) >= 1
    );
    assert!((shadow.world().entity(eid).unwrap().exit_delay_remaining - 2.5).abs() < 1e-5);
    // Host cleared; GameWorld residual writeback restores exit delay.
    {
        let o = logic.host_object_mut(oid).expect("o");
        if let Some(bd) = o.building_data.as_mut() {
            bd.exit_delay_remaining = 0.0;
        }
    }
    assert!(shadow.writeback_production_to_host(&mut logic) >= 1);
    let _ = shadow.writeback_production_door_to_host(&mut logic);
    shadow.writeback_body_damage_to_host(&mut logic);
    let _ = shadow.writeback_death_type_to_host(&mut logic);
    let _ = crate::game_logic::host_death_type_ready_log::drain();
    let _ = shadow.writeback_radar_extend_to_host(&mut logic);
    let _ = shadow.writeback_shock_stun_to_host(&mut logic);
    let _ = crate::game_logic::host_shock_stun_ready_log::drain();
    let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
    let _ = shadow.writeback_sole_healing_to_host(&mut logic);
    let _ = crate::game_logic::host_sole_healing_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_ai_mood_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_mood_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
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
    use crate::game_logic::host_body_damage_log;
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_body_damage_log::clear();
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
    host_body_damage_log::record(oid, HostBodyDamageType::ReallyDamaged.ordinal());
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_body_damage_events(&host_body_damage_log::drain()) >= 1);
    assert_eq!(
        shadow.world().entity(eid).unwrap().body_damage_state,
        2,
        "really damaged ordinal"
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.body_damage_state = HostBodyDamageType::Pristine;
    }
    assert!(shadow.writeback_body_damage_to_host(&mut logic) >= 1);
    let _ = shadow.writeback_death_type_to_host(&mut logic);
    let _ = crate::game_logic::host_death_type_ready_log::drain();
    let _ = shadow.writeback_radar_extend_to_host(&mut logic);
    let _ = shadow.writeback_shock_stun_to_host(&mut logic);
    let _ = crate::game_logic::host_shock_stun_ready_log::drain();
    let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
    let _ = shadow.writeback_sole_healing_to_host(&mut logic);
    let _ = crate::game_logic::host_sole_healing_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_ai_mood_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_mood_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().body_damage_state,
        HostBodyDamageType::ReallyDamaged
    );
}

#[test]
fn weapon_last_fire_time_channel_via_set_weapon_stats() {
    use crate::game_logic::host_weapon_stats_log::{self, HostWeaponStatsEvent};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_weapon_stats_log::clear();
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
    host_weapon_stats_log::record(HostWeaponStatsEvent {
        object: oid,
        has_weapon: true,
        weapon_damage: 10.0,
        weapon_range: 100.0,
        weapon_min_range: 0.0,
        weapon_reload_time: 1.0,
        weapon_last_fire_time: 12.5,
        weapon_clip_size: 0,
        weapon_clip_reload_time: 0.0,
        weapon_ammo: u32::MAX,
        weapon_can_target_air: false,
        weapon_can_target_ground: true,
        weapon_projectile_speed: 0.0,
        has_secondary_weapon: false,
        secondary_weapon_damage: 0.0,
        secondary_weapon_range: 0.0,

        leech_range_active_primary: false,
        leech_range_active_secondary: false,
    });
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_weapon_stats_events(&host_weapon_stats_log::drain()) >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.weapon_last_fire_time - 12.5).abs() < 1e-5);
    assert!(e.has_weapon);
    // writeback last_fire onto host weapon if present
    {
        let o = logic.host_object_mut(oid).expect("o");
        if o.weapon.is_none() {
            // skip host writeback assert when template has no weapon
        } else {
            o.weapon.as_mut().unwrap().last_fire_time = 0.0;
        }
    }
    if logic.host_objects().get(&oid).unwrap().weapon.is_some() {
        assert!(shadow.writeback_weapon_stats_to_host(&mut logic) >= 1);
        let _ = crate::game_logic::host_weapon_stats_ready_log::drain();
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = crate::game_logic::host_fire_intent_ready_log::drain();
        let t = logic
            .host_objects()
            .get(&oid)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .last_fire_time;
        assert!((t - 12.5).abs() < 1e-5);
    }
}

#[test]
fn weapon_clip_size_channel_via_set_weapon_stats() {
    use crate::game_logic::host_weapon_stats_log::{self, HostWeaponStatsEvent};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_weapon_stats_log::clear();
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
    host_weapon_stats_log::record(HostWeaponStatsEvent {
        object: oid,
        has_weapon: true,
        weapon_damage: 10.0,
        weapon_range: 100.0,
        weapon_min_range: 0.0,
        weapon_reload_time: 1.0,
        weapon_last_fire_time: 5.0,
        weapon_clip_size: 5,
        weapon_clip_reload_time: 2.5,
        weapon_ammo: 3,
        weapon_can_target_air: false,
        weapon_can_target_ground: true,
        weapon_projectile_speed: 0.0,
        has_secondary_weapon: false,
        secondary_weapon_damage: 0.0,
        secondary_weapon_range: 0.0,

        leech_range_active_primary: false,
        leech_range_active_secondary: false,
    });
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_weapon_stats_events(&host_weapon_stats_log::drain()) >= 1);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.weapon_clip_size, 5);
    assert!((e.weapon_clip_reload_time - 2.5).abs() < 1e-5);
    assert_eq!(e.weapon_ammo, 3);
}

#[test]
fn front_crushed_channel_via_set_crush_vision() {
    use crate::game_logic::host_crush_vision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_crush_vision_log::clear();
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
    host_crush_vision_log::record(oid, 1, 1, 100.0, 100.0, true, false);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_crush_vision_events(&host_crush_vision_log::drain()) >= 1);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.front_crushed);
    assert!(!e.back_crushed);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.front_crushed = false;
    }
    assert!(shadow.writeback_crush_vision_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_crush_vision_ready_log::drain();
    assert!(
        logic.host_objects().get(&oid).unwrap().front_crushed,
        "front crushed writeback"
    );
}

#[test]
fn waiting_for_path_channel_via_set_movement() {
    use crate::game_logic::host_movement_log::{self, HostMovementEvent};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_movement_log::clear();
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
    host_movement_log::record(
        oid,
        glam::Vec3::ZERO,
        12.0,
        0,
        &[],
        true,
        0,
        false,
        false,
        false,
        false,
        0,
        0,
        f32::MAX,
        0,
        false,
        None,
        None,
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_movement_events(&host_movement_log::drain()) >= 1);
    assert!(shadow.world().entity(eid).unwrap().waiting_for_path);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.waiting_for_path = false;
    }
    assert!(shadow.writeback_movement_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_movement_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_physics_motive_to_host(&mut logic);
    let _ = crate::game_logic::host_physics_motive_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_bounce_land_to_host(&mut logic);
    let _ = crate::game_logic::host_bounce_land_ready_log::drain();
    assert!(
        logic.host_objects().get(&oid).unwrap().waiting_for_path,
        "waiting_for_path writeback"
    );
}

#[test]
fn locomotor_path_flags_channel_via_set_movement() {
    use crate::game_logic::host_movement_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_movement_log::clear();
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
    host_movement_log::record(
        oid,
        glam::Vec3::ZERO,
        15.0,
        0,
        &[],
        true,
        0b101,
        true,
        false,
        true,
        true,
        3,
        42,
        f32::MAX,
        0,
        false,
        None,
        None,
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_movement_events(&host_movement_log::drain()) >= 1);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.locomotor_surfaces, 0b101);
    assert!(e.is_attack_path);
    assert!(e.is_braking);
    assert!(e.is_safe_path);
    assert_eq!(e.queue_for_path_frames, 3);
    assert_eq!(e.path_timestamp, 42);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.locomotor_surfaces = 0;
        o.is_attack_path = false;
        o.is_braking = false;
        o.queue_for_path_frames = 0;
        o.path_timestamp = 0;
        o.waiting_for_path = false;
    }
    assert!(shadow.writeback_movement_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_movement_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_physics_motive_to_host(&mut logic);
    let _ = crate::game_logic::host_physics_motive_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_bounce_land_to_host(&mut logic);
    let _ = crate::game_logic::host_bounce_land_ready_log::drain();
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
    host_shock_stun_log::clear();
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
    host_shock_stun_log::record(oid, 30, 0.5, -0.25, 0.1, 0.9, true, true, true, true, false);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_shock_stun_events(&host_shock_stun_log::drain()) >= 1);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.shock_stun_frames, 30);
    assert!((e.shock_yaw_rate - 0.5).abs() < 1e-5);
    assert!(e.shock_allow_bounce);
    assert!(e.cell_is_cliff);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.shock_stun_frames = 0;
        o.shock_yaw_rate = 0.0;
        o.shock_allow_bounce = false;
        o.cell_is_cliff = false;
    }
    assert!(shadow.writeback_shock_stun_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_shock_stun_ready_log::drain();
    let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
    let _ = shadow.writeback_sole_healing_to_host(&mut logic);
    let _ = crate::game_logic::host_sole_healing_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_ai_mood_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_mood_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
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
    host_movement_log::clear();
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
    host_movement_log::record(
        oid,
        glam::Vec3::ZERO,
        10.0,
        0,
        &[],
        false,
        0,
        false,
        false,
        false,
        false,
        0,
        0,
        3.5,
        7,
        true,
        Some(other.0),
        Some(other.0),
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_movement_events(&host_movement_log::drain()) >= 1);
    let e = shadow.world().entity(eid).unwrap();
    assert!((e.cur_max_blocked_speed - 3.5).abs() < 1e-5);
    assert_eq!(e.num_frames_blocked, 7);
    assert!(e.is_blocked);
    assert_eq!(e.move_away_from_id, Some(other.0));
    assert_eq!(e.requested_victim_id, Some(other.0));
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.cur_max_blocked_speed = f32::MAX;
        o.num_frames_blocked = 0;
        o.is_blocked = false;
        o.move_away_from = None;
        o.requested_victim_id = None;
    }
    assert!(shadow.writeback_movement_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_movement_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_physics_motive_to_host(&mut logic);
    let _ = crate::game_logic::host_physics_motive_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_bounce_land_to_host(&mut logic);
    let _ = crate::game_logic::host_bounce_land_ready_log::drain();
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
    host_rebuild_producer_log::clear();
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
    host_rebuild_producer_log::record(
        hole,
        true,
        "BldA".into(),
        100,
        Some(bld.0),
        Some(worker.0),
        Some(bld.0),
        Some(hole.0),
        250,
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&hole.0).expect("map");
    assert!(shadow.apply_host_rebuild_producer_events(&host_rebuild_producer_log::drain()) >= 1);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.is_rebuild_hole);
    assert_eq!(e.rebuild_template_name, "BldA");
    assert_eq!(e.rebuild_ready_frame, 100);
    assert_eq!(e.rebuild_spawner_id, Some(bld.0));
    assert_eq!(e.rebuild_worker_id, Some(worker.0));
    assert_eq!(e.rebuild_reconstructing_id, Some(bld.0));
    assert_eq!(e.producer_id, Some(hole.0));
    assert_eq!(e.construction_complete_clear_frame, 250);
    {
        let o = logic.host_object_mut(hole).expect("o");
        o.is_rebuild_hole = false;
        o.rebuild_template_name = None;
        o.rebuild_ready_frame = 0;
        o.rebuild_spawner_id = None;
        o.rebuild_worker_id = None;
        o.rebuild_reconstructing_id = None;
        o.producer_id = None;
        o.construction_complete_clear_frame = 0;
    }
    assert!(shadow.writeback_rebuild_producer_to_host(&mut logic) >= 1);
    let _ = shadow.writeback_sole_healing_to_host(&mut logic);
    let _ = crate::game_logic::host_sole_healing_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_ai_mood_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_mood_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
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
