//! Sciences, upgrades, command-set, selection-radius, and identity/visual logs.

use super::*;


#[test]
fn sync_from_host_copies_entity_xp_status_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityXpStatus");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "XpUnit", 100.0);
    let id = logic
        .create_object("XpUnit", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.experience.current = 420.0;
        obj.experience.level = crate::game_logic::VeterancyLevel::Elite;
        obj.stored_resources.supplies = 1500;
        obj.status.stealthed = true;
        obj.status.detected = true;
        obj.status.using_ability = true;
        obj.status.airborne_target = true;
        obj.status.disabled_underpowered = true;
        obj.status.disabled_unmanned = false;
        obj.status.disabled_hacked = true;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.elite_entity_count(), 1);
    assert_eq!(shadow.stealthed_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.experience_points - 420.0).abs() < 0.01);
    assert_eq!(e.veterancy_ordinal, 2);
    assert_eq!(e.stored_supplies, 1500);
    assert!(e.stealthed && e.detected && e.using_ability);
    assert!(e.airborne_target && e.disabled_underpowered && e.disabled_hacked);
    assert!(!e.disabled_unmanned);
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("experience_points")
            && src.contains("veterancy_ordinal")
            && src.contains("stealthed"),
        "sync must copy xp/status residual"
    );
}

#[test]
fn sync_from_host_copies_entity_combat_intent_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityCombatIntent");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "CbtIntentU", 100.0);
    let id = logic
        .create_object("CbtIntentU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    let guard = logic
        .create_object("CbtIntentU", Team::USA, glam::Vec3::new(8.0, 0.0, 4.0))
        .expect("guard");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.force_attack = true;
        obj.show_health_bar = false;
        obj.target_location = Some(glam::Vec3::new(10.0, 0.0, 10.0));
        obj.guard_position = Some(glam::Vec3::new(1.0, 0.0, 1.0));
        obj.guard_target = Some(guard);
        obj.ai_state = crate::game_logic::AIState::GuardingObject;
        obj.occupants = vec![guard];
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.force_attack_entity_count(), 1);
    assert!(shadow.non_idle_ai_entity_count() >= 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.force_attack);
    assert!(!e.show_health_bar);
    assert_eq!(e.guard_target_host, guard.0);
    assert_eq!(e.ai_state_ordinal, 10); // GuardingObject
    assert_eq!(e.occupant_count, 1);
    let tl = e.target_location.expect("tl");
    assert!((tl[0] - 10.0).abs() < 0.01);
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("force_attack")
            && src.contains("guard_target_host")
            && src.contains("ai_state_ordinal"),
        "sync must copy combat-intent residual"
    );
}

#[test]
fn sync_from_host_copies_entity_color_power_type() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityColorPower");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "PwrBldg", 200.0);
    let id = logic
        .create_object("PwrBldg", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.object_type = crate::game_logic::ObjectType::Building;
        obj.team_color = [0.1, 0.2, 0.3, 0.9];
        obj.power_provided = 50;
        obj.power_consumed = 5;
        obj.max_transport = 0;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.building_entity_count(), 1);
    assert_eq!(shadow.total_entity_power_provided(), 50);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.object_type_ordinal, 3);
    assert!((e.team_color[0] - 0.1).abs() < 0.001);
    assert!((e.team_color[3] - 0.9).abs() < 0.001);
    assert_eq!(e.power_provided, 50);
    assert_eq!(e.power_consumed, 5);
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("team_color")
            && src.contains("power_provided")
            && src.contains("object_type_ordinal"),
        "sync must copy color/power/type residual"
    );
}

#[test]
fn sync_from_host_copies_entity_team_status_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityTeamStatus");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "TeamStatU", 100.0);
    let id = logic
        .create_object("TeamStatU", Team::China, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.selection_radius = 12.5;
        obj.status.moving = true;
        obj.status.attacking = true;
        obj.status.under_construction = false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.entity_count_for_team_ordinal(1), 1, "China ordinal");
    assert_eq!(shadow.moving_entity_count(), 1);
    assert_eq!(shadow.attacking_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.team_ordinal, 1);
    assert!((e.selection_radius - 12.5).abs() < 0.01);
    assert!(e.moving && e.attacking);
    assert!(!e.under_construction);
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("team_ordinal")
            && src.contains("selection_radius")
            && src.contains("status.moving"),
        "sync must copy team/status residual"
    );
}

#[test]
fn sync_from_host_copies_entity_selection_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntitySelectResidual");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "SelResU", 100.0);
    let id = logic
        .create_object("SelResU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.selected = true;
        obj.max_health = 150.0;
        obj.construction_percent = 0.4;
        obj.status.destroyed = false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.selected_entity_count(), 1);
    assert_eq!(shadow.under_construction_entity_count(), 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.selected);
    assert!((e.max_health - 150.0).abs() < 0.01);
    assert!((e.construction_percent - 0.4).abs() < 0.01);
    assert!(!e.destroyed);
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("e.selected = obj.selected") && src.contains("e.construction_percent"),
        "sync must copy entity selection/construction residual"
    );
}

#[test]
fn sync_players_copies_alive_and_cash_bounty() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AliveBountyShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    // C++ Player.cpp:443 `m_isPlayerDead = m_observer; // observers are dead`:
    // apply_skirmish_config installs the FactionObserver ReplayObserver
    // (Team::Neutral, is_alive=false), so it must not count as alive.
    assert!(
        logic
            .get_players()
            .values()
            .any(|p| p.name == "ReplayObserver" && !p.is_alive),
        "fixture must include the dead ReplayObserver side"
    );
    let n_alive = n_players - 1;
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.cash_bounty_percent = 0.2;
        p.color_rgb = (12, 34, 56);
        p.is_alive = true;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.alive_player_count(), n_alive);
    assert!((shadow.max_cash_bounty_percent() - 0.2).abs() < 0.001);
    let tinted = shadow
        .world
        .world()
        .active_players()
        .any(|(_, p)| p.color_rgb == (12, 34, 56));
    assert!(tinted, "color_rgb residual must copy");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.is_alive = false;
    }
    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow.alive_player_count(),
        n_alive.saturating_sub(1),
        "killing pid must drop exactly one alive player"
    );
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("is_alive")
            && src.contains("cash_bounty_percent")
            && src.contains("color_rgb"),
        "sync must refresh alive/bounty/color residual"
    );
}

#[test]
fn sync_players_copies_radar_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RadarShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.radar_count = 2;
        p.radar_disabled = false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(
        shadow.radar_residual_present(),
        "shadow must copy host radar_count"
    );
    assert!(
        shadow.any_player_has_radar(),
        "hasRadar residual: count>0 && !disabled"
    );
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.radar_disabled = true;
    }
    shadow.sync_from_host(&logic);
    assert!(
        shadow.radar_residual_present(),
        "disabled flag must still be residual-present"
    );
    assert!(
        !shadow.any_player_has_radar(),
        "disabled radar must fail hasRadar residual"
    );
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("radar_count") && src.contains("radar_disabled"),
        "sync_players must refresh radar residual"
    );
}

#[test]
fn sync_players_copies_rank_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RankShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.rank_level = 4;
        p.skill_points = 512;
        p.science_purchase_points = 3;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw = shadow.host_player_to_gw.get(&pid).copied().expect("mapped");
    let pd = shadow.world().player(gw).expect("pd");
    assert_eq!(pd.rank_level, 4);
    assert_eq!(pd.skill_points, 512);
    assert_eq!(pd.science_purchase_points, 3);
    // Last-writer writeback
    {
        let p = shadow.world_mut().player_mut(gw).expect("pdmut");
        p.rank_level = 5;
        p.skill_points = 600;
        p.science_purchase_points = 4;
    }
    let wb = shadow.writeback_economy_to_host(&mut logic);
    let _ = crate::game_logic::host_economy_ready_log::drain();
    assert!(wb >= 1);
    let host = logic.get_player(pid).expect("host");
    assert_eq!(host.rank_level, 5);
    assert_eq!(host.skill_points, 600);
    assert_eq!(host.science_purchase_points, 4);
}

#[test]
fn sync_players_copies_shared_special_power_cooldowns() {
    use crate::command_system::SpecialPowerType;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SwCdShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.shared_special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 55.0);
        p.shared_special_power_cooldowns
            .insert(SpecialPowerType::ScudStorm, 10.0);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw = shadow.host_player_to_gw.get(&pid).copied().expect("map");
    let pd = shadow.world().player(gw).expect("pd");
    assert!(
        pd.shared_special_power_cooldowns
            .iter()
            .any(|(k, v)| k == "ParticleCannon" && (*v - 55.0).abs() < 1e-5),
        "must copy ParticleCannon cooldown"
    );
    // last-writer writeback
    {
        let p = shadow.world_mut().player_mut(gw).expect("m");
        if let Some((_, v)) = p
            .shared_special_power_cooldowns
            .iter_mut()
            .find(|(k, _)| k == "ParticleCannon")
        {
            *v = 3.0;
        }
    }
    let _ = shadow.writeback_shared_special_power_cooldowns_to_host(&mut logic);
    let host = logic.get_player(pid).expect("h");
    assert!(
        (host
            .shared_special_power_cooldowns
            .get(&SpecialPowerType::ParticleCannon)
            .copied()
            .unwrap_or(-1.0)
            - 3.0)
            .abs()
            < 1e-5
    );
}

#[test]
fn sync_players_copies_power_produced_consumed() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PowerBarShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.power_produced = 120;
        p.power_consumed = 45;
        p.power_available = 75;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(
        shadow.power_bar_residual_present(),
        "shadow must copy host power_produced/consumed"
    );
    let pd = shadow
        .world
        .world()
        .active_players()
        .map(|(_, p)| p)
        .find(|p| p.power_produced == 120 && p.power_consumed == 45)
        .expect("mapped power bar player");
    assert_eq!(pd.power_available, 75);
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("power_produced") && src.contains("power_consumed"),
        "sync_players must refresh power bar residual"
    );
}

#[test]
fn sync_players_copies_unlocked_sciences() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ScienceShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.unlocked_sciences.insert("SCIENCE_PaladinTank".into());
        p.unlocked_sciences.insert("SCIENCE_Pathfinder".into());
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(
        shadow.unlocked_science_count() >= 2,
        "shadow must copy host unlocked sciences"
    );
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    assert!(
        src.contains("host_player_science_and_upgrades") && src.contains("unlocked_sciences"),
        "sync_players must refresh unlocked_sciences residual"
    );
}

#[test]
fn host_upgrade_complete_applies_to_shadow_player() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("UpgradeShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = logic.get_players().keys().copied().min().expect("player");
    // Record a completed upgrade on the host registry.
    let frame = logic.get_frame();
    logic.host_upgrades_mut().record_complete(
        "Upgrade_AmericaRangerFlashBangGrenade",
        pid,
        frame,
        1,
    );
    let events = logic.host_upgrades().completed_this_frame_snapshot();
    assert!(!events.is_empty(), "host must expose completed_this_frame");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let n = shadow.apply_host_upgrade_events(&events);
    assert!(n >= 1, "upgrade events applied {n}");
    assert!(
        shadow.completed_upgrade_count() >= 1,
        "shadow player must retain completed upgrade"
    );
    // Source honesty: session must drain upgrade snapshot.
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let idx = src
        .find("fn shadow_session_after_host_tick")
        .expect("session");
    let window = &src[idx..idx + 6000];
    assert!(
        window.contains("completed_this_frame_snapshot")
            && window.contains("apply_host_upgrade_events"),
        "session must apply host upgrade completes"
    );
}

#[test]
fn host_command_set_log_drives_set_command_set_channel() {
    use crate::game_logic::{host_command_set_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CsCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CsU") {
        let mut t = ThingTemplate::new("CsU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("CsU".into(), t);
    }
    let oid = logic
        .create_object("CsU", Team::GLA, glam::Vec3::new(22.0, 0.0, 22.0))
        .expect("id");
    host_command_set_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_command_set_override(Some("Command_DemoSuicide".into()));
    }
    let events = host_command_set_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.command_set == "Command_DemoSuicide"),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_command_set();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.command_set_override.clear();
    }
    let n = shadow.apply_host_command_set_events(&host_command_set_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.command_set_override, "Command_DemoSuicide");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.command_set_override = None;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.command_set_override = "Command_DemoSuicide".into();
    }
    assert!(shadow.writeback_command_set_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_command_set_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(
        o.command_set_override.as_deref(),
        Some("Command_DemoSuicide")
    );
}

#[test]
fn host_selection_radius_log_drives_set_selection_radius_channel() {
    use crate::game_logic::{host_selection_radius_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SrCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SrU") {
        let mut t = ThingTemplate::new("SrU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SrU".into(), t);
    }
    let oid = logic
        .create_object("SrU", Team::USA, glam::Vec3::new(27.0, 0.0, 27.0))
        .expect("id");
    host_selection_radius_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_selection_radius(14.5);
    }
    let events = host_selection_radius_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && (e.selection_radius - 14.5).abs() < 1e-5),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_selection_radius();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.selection_radius = 1.0;
    }
    let n = shadow.apply_host_selection_radius_events(&host_selection_radius_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.selection_radius - 14.5).abs() < 1e-5);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.selection_radius = 1.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.selection_radius = 14.5;
    }
    assert!(shadow.writeback_selection_radius_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_selection_radius_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!((o.selection_radius - 14.5).abs() < 1e-5);
}

#[test]
fn host_ground_height_log_drives_set_ground_height_channel() {
    use crate::game_logic::{host_ground_height_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GhCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("GhU") {
        let mut t = ThingTemplate::new("GhU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("GhU".into(), t);
    }
    let oid = logic
        .create_object("GhU", Team::USA, glam::Vec3::new(34.0, 0.0, 34.0))
        .expect("id");
    host_ground_height_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_ground_height_residual(12.5, true);
    }
    let events = host_ground_height_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid && (e.ground_height - 12.5).abs() < 1e-5 && e.from_terrain
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_ground_height();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ground_height = 0.0;
        e.ground_height_from_terrain = false;
    }
    let n = shadow.apply_host_ground_height_events(&host_ground_height_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.ground_height - 12.5).abs() < 1e-5);
    assert!(e.ground_height_from_terrain);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.ground_height = 0.0;
        o.ground_height_from_terrain = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ground_height = 12.5;
        e.ground_height_from_terrain = true;
    }
    assert!(shadow.writeback_ground_height_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_ground_height_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!((o.ground_height - 12.5).abs() < 1e-5);
    assert!(o.ground_height_from_terrain);
}

#[test]
fn host_model_mesh_log_drives_set_model_mesh_channel() {
    use crate::game_logic::{host_model_mesh_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MmCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerMesh") {
        let mut t = ThingTemplate::new("RangerMesh");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.model_name = Some("airanger_s".into());
        logic.templates.insert("RangerMesh".into(), t);
    }
    let oid = logic
        .create_object("RangerMesh", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    host_model_mesh_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_model_mesh_residual("avtank", 1.25);
    }
    let events = host_model_mesh_log::drain();
    assert!(
        events.iter().any(|e| e.object == oid
            && e.model_key == "avtank"
            && (e.mesh_scale - 1.25).abs() < 1e-5),
        "events {:?}",
        events
    );

    // Re-apply path
    host_model_mesh_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_model_mesh_residual("avtank", 1.25);
    }
    let n = shadow.apply_host_model_mesh_events(&host_model_mesh_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.model_key, "avtank");
    assert!((e.mesh_scale - 1.25).abs() < 1e-5);
}

#[test]
fn host_fow_log_drives_set_fow_channel() {
    use crate::game_logic::{host_fow_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FowCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerFow") {
        let mut t = ThingTemplate::new("RangerFow");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerFow".into(), t);
    }
    let oid = logic
        .create_object("RangerFow", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    host_fow_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_fow_residual(0.35, 1.0, 0.5);
    }
    let events = host_fow_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && (e.visibility_alpha - 0.35).abs() < 1e-5
                && (e.is_explored - 1.0).abs() < 1e-5
                && (e.visibility_falloff - 0.5).abs() < 1e-5
        }),
        "events {:?}",
        events
    );

    host_fow_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_fow_residual(0.35, 1.0, 0.5);
    }
    let n = shadow.apply_host_fow_events(&host_fow_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.fow_visibility_alpha - 0.35).abs() < 1e-5);
    assert!((e.fow_is_explored - 1.0).abs() < 1e-5);
    assert!((e.fow_visibility_falloff - 0.5).abs() < 1e-5);
}

#[test]
fn host_kind_of_log_drives_set_kind_of_bits_channel() {
    use crate::game_logic::{host_kind_of_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("KoCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerKo") {
        let mut t = ThingTemplate::new("RangerKo");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("RangerKo".into(), t);
    }
    let oid = logic
        .create_object("RangerKo", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    let bits = {
        let o = logic.host_objects().get(&oid).expect("o");
        o.presentation_kind_of_bits()
    };
    assert!(bits != 0, "bits {bits}");

    host_kind_of_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_kind_of_bits_residual(bits | (1u32 << 10)); // set Hero bit residual
    }
    let events = host_kind_of_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.kind_of_bits == (bits | (1u32 << 10))),
        "events {:?}",
        events
    );

    host_kind_of_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_kind_of_bits_residual(bits | (1u32 << 10));
    }
    let n = shadow.apply_host_kind_of_events(&host_kind_of_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.kind_of_bits, bits | (1u32 << 10));
}

#[test]
fn host_identity_log_drives_set_identity_channel() {
    use crate::game_logic::{host_identity_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("IdCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("IdU") {
        let mut t = ThingTemplate::new("IdU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("IdU".into(), t);
    }
    let oid = logic
        .create_object("IdU", Team::USA, glam::Vec3::new(33.0, 0.0, 33.0))
        .expect("id");
    host_identity_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.name = "ScriptRanger".into();
        o.team_color = [0.1, 0.2, 0.3, 1.0];
        o.record_host_identity();
    }
    let events = host_identity_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.name == "ScriptRanger"
                && (e.team_color[0] - 0.1).abs() < 1e-5
                && (e.team_color[2] - 0.3).abs() < 1e-5
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_identity();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.display_name.clear();
        e.team_color = [1.0, 1.0, 1.0, 1.0];
    }
    let n = shadow.apply_host_identity_events(&host_identity_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.display_name, "ScriptRanger");
    assert!((e.team_color[0] - 0.1).abs() < 1e-5);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.name.clear();
        o.team_color = [0.0, 0.0, 0.0, 1.0];
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.display_name = "ScriptRanger".into();
        e.team_color = [0.1, 0.2, 0.3, 1.0];
    }
    assert!(shadow.writeback_identity_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_identity_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.name, "ScriptRanger");
    assert!((o.team_color[0] - 0.1).abs() < 1e-5);
}

#[test]
fn host_building_type_log_drives_set_building_type_channel() {
    use crate::game_logic::{
        host_building_type_log, BuildingData, BuildingType, KindOf, Team, ThingTemplate,
    };
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BtCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("BtU") {
        let mut t = ThingTemplate::new("BtU");
        t.set_health(400.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("BtU".into(), t);
    }
    let oid = logic
        .create_object("BtU", Team::USA, glam::Vec3::new(32.0, 0.0, 32.0))
        .expect("id");
    host_building_type_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.building_data = Some(BuildingData::new(BuildingType::Barracks));
        o.record_host_building_type();
    }
    let events = host_building_type_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.is_building && e.building_type_ordinal == 1),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_building_type();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_building = false;
        e.building_type_ordinal = 255;
    }
    let n = shadow.apply_host_building_type_events(&host_building_type_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.is_building);
    assert_eq!(e.building_type_ordinal, 1);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.building_data = Some(BuildingData::new(BuildingType::PowerPlant));
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_building = true;
        e.building_type_ordinal = 1;
    }
    assert!(shadow.writeback_building_type_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_building_type_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(
        o.building_data.as_ref().map(|b| b.building_type),
        Some(BuildingType::Barracks)
    );
}

#[test]
fn host_crush_vision_log_drives_set_crush_vision_channel() {
    use crate::game_logic::{host_crush_vision_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CvCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CvU") {
        let mut t = ThingTemplate::new("CvU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("CvU".into(), t);
    }
    let oid = logic
        .create_object("CvU", Team::USA, glam::Vec3::new(30.0, 0.0, 30.0))
        .expect("id");
    host_crush_vision_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.crusher_level = 2;
        o.crushable_level = 1;
        o.vision_range = 175.0;
        o.shroud_clearing_range = 200.0;
        o.record_host_crush_vision();
    }
    let events = host_crush_vision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.crusher_level == 2
                && e.crushable_level == 1
                && (e.vision_range - 175.0).abs() < 1e-5
                && (e.shroud_clearing_range - 200.0).abs() < 1e-5
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_crush_vision();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.crusher_level = 0;
        e.crushable_level = 0;
        e.vision_range = 0.0;
        e.shroud_clearing_range = 0.0;
    }
    let n = shadow.apply_host_crush_vision_events(&host_crush_vision_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.crusher_level, 2);
    assert_eq!(e.crushable_level, 1);
    assert!((e.vision_range - 175.0).abs() < 1e-5);
    assert!((e.shroud_clearing_range - 200.0).abs() < 1e-5);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.crusher_level = 0;
        o.crushable_level = 0;
        o.vision_range = 0.0;
        o.shroud_clearing_range = 0.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.crusher_level = 2;
        e.crushable_level = 1;
        e.vision_range = 175.0;
        e.shroud_clearing_range = 200.0;
    }
    assert!(shadow.writeback_crush_vision_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_crush_vision_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.crusher_level, 2);
    assert_eq!(o.crushable_level, 1);
    assert!((o.vision_range - 175.0).abs() < 1e-5);
    assert!((o.shroud_clearing_range - 200.0).abs() < 1e-5);
}

#[test]
fn host_demo_mine_cheer_log_drives_set_demo_mine_cheer_channel() {
    use crate::game_logic::{
        host_demo_mine_cheer_log, host_mines::HostMineData, host_mines::HostMineKind, KindOf, Team,
        ThingTemplate,
    };
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DmcCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("DmcU") {
        let mut t = ThingTemplate::new("DmcU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("DmcU".into(), t);
    }
    let oid = logic
        .create_object("DmcU", Team::GLA, glam::Vec3::new(29.0, 0.0, 29.0))
        .expect("id");
    host_demo_mine_cheer_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.demo_suicided_detonating = true;
        o.cheer_timer = 2.5;
        o.mine_data = Some(HostMineData::new(HostMineKind::LandMine));
        o.record_host_demo_mine_cheer();
    }
    let events = host_demo_mine_cheer_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.demo_suicided_detonating
                && e.has_mine_data
                && (e.cheer_timer - 2.5).abs() < 1e-5
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_demo_mine_cheer();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.demo_suicided_detonating = false;
        e.has_mine_data = false;
        e.cheer_timer = 0.0;
    }
    let n = shadow.apply_host_demo_mine_cheer_events(&host_demo_mine_cheer_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.demo_suicided_detonating && e.has_mine_data);
    assert!((e.cheer_timer - 2.5).abs() < 1e-5);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.demo_suicided_detonating = false;
        o.cheer_timer = 0.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.demo_suicided_detonating = true;
        e.has_mine_data = true;
        e.cheer_timer = 2.5;
    }
    assert!(shadow.writeback_demo_mine_cheer_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_demo_mine_cheer_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.demo_suicided_detonating);
    assert!((o.cheer_timer - 2.5).abs() < 1e-5);
    assert!(o.mine_data.is_some());
}

#[test]
fn host_model_condition_log_drives_set_model_condition_channel() {
    use crate::game_logic::{host_model_condition_log, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("McCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("McU") {
        let mut t = ThingTemplate::new("McU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("McU".into(), t);
    }
    let oid = logic
        .create_object("McU", Team::China, glam::Vec3::new(28.0, 0.0, 28.0))
        .expect("id");
    host_model_condition_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.model_condition_bits = 0b1011;
        o.record_host_model_condition();
    }
    let events = host_model_condition_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.model_condition_bits == 0b1011),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_model_condition();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.model_condition_bits = 0;
    }
    let n = shadow.apply_host_model_condition_events(&host_model_condition_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.model_condition_bits, 0b1011);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.model_condition_bits = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.model_condition_bits = 0b1011;
    }
    assert!(shadow.writeback_model_condition_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_model_condition_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.model_condition_bits, 0b1011);
}
