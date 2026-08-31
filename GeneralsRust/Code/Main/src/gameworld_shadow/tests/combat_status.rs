//! Combat-status, player, contain, AI, special-power, and production log channels.

use super::*;

#[test]
fn set_combat_status_mutation_channel_updates_shadow_entity() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CombatStatusMut");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CbtU") {
        let mut t = ThingTemplate::new("CbtU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CbtU".into(), t);
    }
    let id = logic
        .create_object("CbtU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.queue_set_combat_status_for_host(
        crate::game_logic::host_status_log::HostStatusEvent {
            object: id,
            selected: Some(true),
            attacking: Some(true),
            moving: None,
            is_firing_weapon: Some(true),
            is_aiming_weapon: None,
            stealthed: Some(true),
            detected: Some(false),
            disabled_emp: Some(true),
            weapons_jammed: None,
            disabled_hacked: None,
            disabled_unmanned: None,
            disabled_paralyzed: None,
            disabled_subdued: None,
            masked: Some(true),
            disguised: Some(true),

            no_collisions: None,
            private_captured: None,
            disguise_transitioning_to: None,
            disguise_halfpoint_reached: None,
            faerie_fire: None,
            booby_trapped: None,
            eject_invulnerable: None,
            pilot_did_move_to_base: None,
            parachuting: None,
            parachute_open: None,
            parachute_landing_override_set: None,

            using_ability: None,
            deployed: None,
            under_construction: None,
            sold: None,
            reconstructing: None,
            unselectable: None,
            ignoring_stealth: None,
            repulsor: None,
            disabled_underpowered: None,
            disabled_freefall: None,
            is_carbomb: None,
            hijacked: None,
            force_attack: None,
        }
    ));
    let n = shadow.world_mut().apply_pending_mutations();
    assert!(n >= 1);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.stealthed);
    assert!(!e.detected);
    assert!(e.attacking);
    assert!(e.is_firing_weapon);
    assert!(e.selected);
    assert!(e.disabled_emp);
    assert!(e.masked);
    assert!(e.disguised);
    // writeback to host via combat-status last-writer residual
    {
        let o = logic.host_object_mut(id).expect("o");
        o.status.stealthed = false;
        o.status.attacking = false;
        o.status.is_firing_weapon = false;
        o.status.selected = false;
        o.status.disabled_emp = false;
        o.status.masked = false;
        o.status.disguised = false;
    }
    let wb = shadow.writeback_combat_status_to_host(&mut logic);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    assert!(wb >= 1);
    let o = logic.host_objects().get(&id).expect("o");
    assert!(o.status.stealthed);
    assert!(o.status.attacking);
    assert!(o.status.is_firing_weapon);
    assert!(o.status.selected);
    assert!(o.status.disabled_emp);
    assert!(o.status.masked);
    assert!(o.status.disguised);
}

#[test]
fn host_selection_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_status_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SelStatusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SelU") {
        let mut t = ThingTemplate::new("SelU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SelU".into(), t);
    }
    let id = logic
        .create_object("SelU", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    host_status_log::clear();
    // Select via host API (records status log).
    let pid = logic.get_players().keys().copied().min().unwrap_or(0);
    logic.select_objects(pid, vec![id]);
    let events = host_status_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.selected == Some(true)),
        "select must log selected=true"
    );
    // Re-record for session path (drain consumed).
    {
        let o = logic.host_object_mut(id).expect("o");
        o.select();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Poison shadow selected off, then apply host status events as mutations.
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.selected = false;
    }
    let status_events = host_status_log::drain();
    for ev in &status_events {
        let _ = shadow.queue_set_combat_status_for_host(*ev);
    }
    let n = shadow.apply_pending();
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.selected, "mutation channel must set selected");
    {
        let o = logic.host_object_mut(id).expect("o");
        o.status.selected = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.selected = true;
    }
    assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    assert!(logic.host_objects().get(&id).expect("o").status.selected);
}

#[test]
fn host_attacking_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_status_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkStatusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AtkU") {
        let mut t = ThingTemplate::new("AtkU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AtkU".into(), t);
    }
    let id = logic
        .create_object("AtkU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    host_status_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_attacking(true);
        o.set_status_firing_weapon(true);
    }
    let events = host_status_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.attacking == Some(true))
    );
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.is_firing_weapon == Some(true))
    );
    // Re-record for mutation apply.
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_attacking(true);
        o.set_status_firing_weapon(true);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.attacking = false;
        e.is_firing_weapon = false;
    }
    for ev in host_status_log::drain() {
        let _ = shadow.queue_set_combat_status_for_host(ev);
    }
    assert!(shadow.apply_pending() >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.attacking);
    assert!(e.is_firing_weapon);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.status.attacking = false;
        o.status.is_firing_weapon = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.attacking = true;
        e.is_firing_weapon = true;
    }
    assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    let o = logic.host_objects().get(&id).expect("o");
    assert!(o.status.attacking && o.status.is_firing_weapon);
}

#[test]
fn host_stealth_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_status_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StealthStatusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("StU") {
        let mut t = ThingTemplate::new("StU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("StU".into(), t);
    }
    let id = logic
        .create_object("StU", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    host_status_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_stealthed(true);
        o.set_status_detected(false);
    }
    let events = host_status_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.stealthed == Some(true))
    );
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.detected == Some(false))
    );
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_stealthed(true);
        o.set_status_detected(false);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stealthed = false;
        e.detected = true;
    }
    for ev in host_status_log::drain() {
        let _ = shadow.queue_set_combat_status_for_host(ev);
    }
    assert!(shadow.apply_pending() >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.stealthed);
    assert!(!e.detected);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.status.stealthed = false;
        o.status.detected = true;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stealthed = true;
        e.detected = false;
    }
    assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    let o = logic.host_objects().get(&id).expect("o");
    assert!(o.status.stealthed && !o.status.detected);
}

#[test]
fn host_emp_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_status_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EmpStatusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("EmpU") {
        let mut t = ThingTemplate::new("EmpU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("EmpU".into(), t);
    }
    let id = logic
        .create_object("EmpU", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    host_status_log::clear();
    let until = logic.get_frame().saturating_add(300);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.apply_disabled_emp(until);
    }
    let events = host_status_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.disabled_emp == Some(true)),
        "EMP apply must log disabled_emp"
    );
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.attacking == Some(false)),
        "EMP apply clears attacking via status channel"
    );
    let until2 = logic.get_frame().saturating_add(300);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.apply_disabled_emp(until2);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disabled_emp = false;
    }
    for ev in host_status_log::drain() {
        let _ = shadow.queue_set_combat_status_for_host(ev);
    }
    assert!(shadow.apply_pending() >= 1);
    assert!(shadow.world().entity(eid).expect("e").disabled_emp);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.status.disabled_emp = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disabled_emp = true;
    }
    assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    assert!(
        logic
            .host_objects()
            .get(&id)
            .expect("o")
            .status
            .disabled_emp
    );
}

#[test]
fn host_player_cooldown_log_drives_set_player_cooldowns_channel() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_player_cooldown_log;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PlayerCdCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");
    host_player_cooldown_log::clear();
    {
        let p = logic.get_player_mut(pid).expect("p");
        // Use a concrete SP type if available; format Debug name into log.
        // ParticleUplink residual is common; fall back to first Debug variant via reset API.
        p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 12.5);
    }
    let events = host_player_cooldown_log::drain();
    assert!(
        !events.is_empty()
            && events
                .iter()
                .any(|e| e.player_id == pid && !e.cooldowns.is_empty()),
        "events {:?}",
        events
    );
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 12.5);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
    if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
        p.shared_special_power_cooldowns.clear();
    }
    let n = shadow.apply_host_player_cooldown_events(&host_player_cooldown_log::drain());
    assert!(n >= 1);
    let p = shadow.world().player(gw).expect("p");
    assert!(
        p.shared_special_power_cooldowns
            .iter()
            .any(|(_, rem)| (*rem - 12.5).abs() < 1e-3),
        "cds {:?}",
        p.shared_special_power_cooldowns
    );
    // Poison host map and writeback
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.shared_special_power_cooldowns.clear();
    }
    let _wb_econ = shadow.writeback_economy_to_host(&mut logic);
    let _ = crate::game_logic::host_economy_ready_log::drain();
    assert!(_wb_econ >= 1);
    let p = logic.get_player(pid).expect("p");
    assert!(
        p.shared_special_power_cooldowns
            .values()
            .any(|rem| (*rem - 12.5).abs() < 1e-3),
        "host cds {:?}",
        p.shared_special_power_cooldowns
    );
}

#[test]
fn host_player_meta_log_drives_sciences_and_alive_channel() {
    use crate::game_logic::host_player_meta_log;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PlayerMetaCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");
    host_player_meta_log::clear();
    {
        let p = logic.get_player_mut(pid).expect("p");
        assert!(p.unlock_science("SCIENCE_PaladinTank"));
        p.is_alive = true;
        p.record_host_alive();
    }
    let events = host_player_meta_log::drain();
    assert!(
        events.iter().any(|e| matches!(
            e,
            host_player_meta_log::HostPlayerMetaEvent::Sciences { player_id, unlocked_sciences }
                if *player_id == pid && unlocked_sciences.iter().any(|s| s.contains("Paladin"))
        )),
        "sciences {:?}",
        events
    );
    assert!(events.iter().any(|e| matches!(
        e,
        host_player_meta_log::HostPlayerMetaEvent::Alive { player_id, is_alive: true }
            if *player_id == pid
    )));

    // Re-record for apply
    host_player_meta_log::clear();
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.record_host_sciences();
        p.is_alive = false;
        p.record_host_alive();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
    if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
        p.unlocked_sciences.clear();
        p.is_alive = true;
    }
    let n = shadow.apply_host_player_meta_events(&host_player_meta_log::drain());
    assert!(n >= 1);
    let p = shadow.world().player(gw).expect("p");
    assert!(p.unlocked_sciences.iter().any(|s| s.contains("Paladin")));
    assert!(!p.is_alive);
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.unlocked_sciences.clear();
        p.is_alive = true;
    }
    let _wb_econ = shadow.writeback_economy_to_host(&mut logic);
    let _ = crate::game_logic::host_economy_ready_log::drain();
    assert!(_wb_econ >= 1);
    let p = logic.get_player(pid).expect("p");
    assert!(p.unlocked_sciences.iter().any(|s| s.contains("Paladin")));
    assert!(!p.is_alive);
}

#[test]
fn host_player_progress_log_drives_set_player_progress_channel() {
    use crate::game_logic::host_player_progress_log;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PlayerProgCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");
    host_player_progress_log::clear();
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.force_set_cash_bounty(0.35);
        p.rank_level = 3;
        p.skill_points = 50;
        p.science_purchase_points = 2;
        p.record_host_progress();
    }
    let events = host_player_progress_log::drain();
    assert!(
        events.iter().any(|e| {
            e.player_id == pid && e.rank_level == 3 && (e.cash_bounty_percent - 0.35).abs() < 1e-5
        }),
        "events {:?}",
        events
    );
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.force_set_cash_bounty(0.35);
        p.rank_level = 3;
        p.skill_points = 50;
        p.science_purchase_points = 2;
        p.record_host_progress();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
    if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
        p.rank_level = 1;
        p.skill_points = 0;
        p.science_purchase_points = 0;
        p.cash_bounty_percent = 0.0;
    }
    let n = shadow.apply_host_player_progress_events(&host_player_progress_log::drain());
    assert!(n >= 1);
    let p = shadow.world().player(gw).expect("p");
    assert_eq!(p.rank_level, 3);
    assert_eq!(p.skill_points, 50);
    assert_eq!(p.science_purchase_points, 2);
    assert!((p.cash_bounty_percent - 0.35).abs() < 1e-5);
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.rank_level = 1;
        p.skill_points = 0;
        p.science_purchase_points = 0;
        p.cash_bounty_percent = 0.0;
    }
    let _wb_econ = shadow.writeback_economy_to_host(&mut logic);
    let _ = crate::game_logic::host_economy_ready_log::drain();
    assert!(_wb_econ >= 1);
    let p = logic.get_player(pid).expect("p");
    assert_eq!(p.rank_level, 3);
    assert_eq!(p.skill_points, 50);
    assert!((p.cash_bounty_percent - 0.35).abs() < 1e-5);
}

#[test]
fn host_radar_log_drives_set_player_radar_channel() {
    use crate::game_logic::host_radar_log;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RadarCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");
    host_radar_log::clear();
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.set_radar_state(2, false);
    }
    let events = host_radar_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.player_id == pid && e.radar_count == 2 && !e.radar_disabled)
    );
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.set_radar_state(2, false);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
    if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
        p.radar_count = 0;
        p.radar_disabled = true;
    }
    let n = shadow.apply_host_radar_events(&host_radar_log::drain());
    assert!(n >= 1);
    let p = shadow.world().player(gw).expect("p");
    assert_eq!(p.radar_count, 2);
    assert!(!p.radar_disabled);
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.radar_count = 0;
        p.radar_disabled = true;
    }
    let _wb_econ = shadow.writeback_economy_to_host(&mut logic);
    let _ = crate::game_logic::host_economy_ready_log::drain();
    assert!(_wb_econ >= 1);
    let p = logic.get_player(pid).expect("p");
    assert_eq!(p.radar_count, 2);
    assert!(!p.radar_disabled);
}

#[test]
fn host_contain_log_drives_set_contain_channel() {
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, Team, ThingTemplate, host_contain_log,
    };
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ContainCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["BunkC", "InfC"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            if name == "BunkC" {
                t.add_kind_of(KindOf::Structure);
            }
            logic.templates.insert(name.into(), t);
        }
    }
    let bunker = logic
        .create_object("BunkC", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    let inf = logic
        .create_object("InfC", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("i");
    {
        let o = logic.host_object_mut(bunker).expect("b");
        o.building_data = Some(BuildingData::new(BuildingType::Bunker));
        if let Some(bd) = o.building_data.as_mut() {
            bd.max_garrison = 5;
        }
    }
    host_contain_log::clear();
    {
        let o = logic.host_object_mut(bunker).expect("b");
        assert!(o.add_occupant(inf));
    }
    {
        let o = logic.host_object_mut(inf).expect("i");
        o.set_contained_by(Some(bunker));
    }
    let events = host_contain_log::drain();
    assert!(events.len() >= 2, "events {:?}", events);

    // Re-apply path
    host_contain_log::clear();
    {
        let o = logic.host_object_mut(bunker).expect("b");
        if let Some(bd) = o.building_data.as_mut() {
            bd.garrisoned_units.clear();
        }
        assert!(o.add_occupant(inf));
    }
    {
        let o = logic.host_object_mut(inf).expect("i");
        o.set_contained_by(Some(bunker));
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid_i = shadow.entity_for_host(inf).expect("map i");
    let eid_b = shadow.entity_for_host(bunker).expect("map b");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_i) {
        e.contained_by_host = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_b) {
        e.garrison_count = 0;
        e.garrisoned_host_ids.clear();
    }
    let n = shadow.apply_host_contain_events(&host_contain_log::drain());
    assert!(n >= 1);
    assert_eq!(
        shadow.world().entity(eid_i).expect("e").contained_by_host,
        bunker.0
    );
    assert!(shadow.world().entity(eid_b).expect("e").garrison_count >= 1);
    assert!(shadow.world().entity(eid_b).expect("e").occupant_count >= 1);
    // Poison host then writeback via SetContain last-writer residual.
    {
        let o = logic.host_object_mut(inf).expect("i");
        o.contained_by = None;
    }
    {
        let o = logic.host_object_mut(bunker).expect("b");
        if let Some(bd) = o.building_data.as_mut() {
            bd.garrisoned_units.clear();
        }
        o.occupants.clear();
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_i) {
        e.contained_by_host = bunker.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_b) {
        e.garrison_count = 1;
        e.garrisoned_host_ids = vec![inf.0];
    }
    assert!(shadow.writeback_contain_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_contain_ready_log::drain();
    assert_eq!(
        logic.host_objects().get(&inf).expect("i").contained_by,
        Some(bunker)
    );
    let bd = logic
        .host_objects()
        .get(&bunker)
        .expect("b")
        .building_data
        .as_ref()
        .expect("bd");
    assert!(bd.garrisoned_units.contains(&inf));
}

#[test]
// Wave 947: channel-drive tests mutate host via host_object_mut authority.
fn host_ai_state_log_drives_set_ai_state_channel() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate, host_ai_state_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiStateCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AiU") {
        let mut t = ThingTemplate::new("AiU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AiU".into(), t);
    }
    let id = logic
        .create_object("AiU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    host_ai_state_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_ai_state(AIState::GuardingObject);
    }
    let events = host_ai_state_log::drain();
    assert!(
        events.iter().any(|e| e.object == id && e.ordinal == 10),
        "expected GuardingObject ordinal 10, got {:?}",
        events
    );
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_ai_state(AIState::GuardingObject);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ai_state_ordinal = 0;
    }
    let n = shadow.apply_host_ai_state_events(&host_ai_state_log::drain());
    assert!(n >= 1);
    assert_eq!(shadow.world().entity(eid).expect("e").ai_state_ordinal, 10);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.ai_state = AIState::Idle;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ai_state_ordinal = 10; // GuardingObject
    }
    let _ = shadow.writeback_ai_state_to_host(&mut logic);
    assert_eq!(
        logic.host_objects().get(&id).expect("o").ai_state,
        AIState::GuardingObject
    );
}

#[test]
fn host_special_power_cooldown_remaining_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_special_power_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpCdCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SpU") {
        let mut t = ThingTemplate::new("SpU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.special_power_cooldown = 45.0;
        logic.templates.insert("SpU".into(), t);
    }
    let oid = logic
        .create_object("SpU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    host_special_power_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 45.0;
        o.special_power_cooldown_remaining = 18.0;
        o.special_power_ready = false;
        o.record_host_special_power();
    }
    let events = host_special_power_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && !e.ready
                && (e.cooldown_remaining - 18.0).abs() < 1e-3
                && (e.cooldown - 45.0).abs() < 1e-3
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_special_power();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = true;
        e.special_power_cooldown_remaining = 0.0;
        e.special_power_cooldown = 0.0;
    }
    let n = shadow.apply_host_special_power_events(&host_special_power_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(!e.special_power_ready);
    assert!((e.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
    assert!((e.special_power_cooldown - 45.0).abs() < 1e-3);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_ready = true;
        o.special_power_cooldown_remaining = 0.0;
        o.special_power_cooldown = 1.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = false;
        e.special_power_cooldown_remaining = 18.0;
        e.special_power_cooldown = 45.0;
    }
    assert!(shadow.writeback_special_power_to_host(&mut logic) >= 1);
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(!o.special_power_ready);
    assert!((o.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
    assert!((o.special_power_cooldown - 45.0).abs() < 1e-3);
}

#[test]
fn host_special_power_log_drives_set_special_power_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_special_power_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpReadyCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SpU") {
        let mut t = ThingTemplate::new("SpU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SpU".into(), t);
    }
    let id = logic
        .create_object("SpU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    host_special_power_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_special_power_ready(true);
    }
    let events = host_special_power_log::drain();
    assert!(events.iter().any(|e| e.object == id && e.ready));
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_special_power_ready(true);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = false;
    }
    let n = shadow.apply_host_special_power_events(&host_special_power_log::drain());
    assert!(n >= 1);
    assert!(shadow.world().entity(eid).expect("e").special_power_ready);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.special_power_ready = false;
    }
    assert!(shadow.writeback_special_power_to_host(&mut logic) >= 1);
    assert!(
        logic
            .host_objects()
            .get(&id)
            .expect("o")
            .special_power_ready
    );
}

#[test]
fn host_stored_supplies_log_drives_set_stored_supplies_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_stored_supplies_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StoreSupCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SsU") {
        let mut t = ThingTemplate::new("SsU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SsU".into(), t);
    }
    let id = logic
        .create_object("SsU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    host_stored_supplies_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_stored_supplies(900);
    }
    let events = host_stored_supplies_log::drain();
    assert!(events.iter().any(|e| e.object == id && e.supplies == 900));
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_stored_supplies(900);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stored_supplies = 0;
    }
    let n = shadow.apply_host_stored_supplies_events(&host_stored_supplies_log::drain());
    assert!(n >= 1);
    assert_eq!(shadow.world().entity(eid).expect("e").stored_supplies, 900);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.stored_resources.supplies = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stored_supplies = 900;
    }
    assert!(shadow.writeback_stored_supplies_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_stored_supplies_ready_log::drain();
    assert_eq!(
        logic
            .host_objects()
            .get(&id)
            .expect("o")
            .stored_resources
            .supplies,
        900
    );
}

#[test]
fn host_construction_progress_log_drives_set_construction_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_construction_progress_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ConstrProgCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CProg") {
        let mut t = ThingTemplate::new("CProg");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CProg".into(), t);
    }
    let id = logic
        .create_object("CProg", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let o = logic.host_object_mut(id).expect("o");
        o.construction_percent = 0.25;
        o.set_status_under_construction(true);
    }
    host_construction_progress_log::clear();
    host_construction_progress_log::record(id, 0.25, true, 0.0);
    let events = host_construction_progress_log::drain();
    assert_eq!(events.len(), 1);

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.construction_percent = 0.0;
        e.under_construction = false;
    }
    host_construction_progress_log::record(id, 0.25, true, 0.0);
    let events = host_construction_progress_log::drain();
    let n = shadow.apply_host_construction_progress_events(&events);
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.construction_percent - 0.25).abs() < 1e-5);
    assert!(e.under_construction);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.construction_percent = 0.0;
        o.status.under_construction = false;
    }
    let wb = shadow.writeback_construction_to_host(&mut logic);
    assert!(wb >= 1);
    let o = logic.host_objects().get(&id).expect("o");
    assert!((o.construction_percent - 0.25).abs() < 1e-5);
    assert!(o.status.under_construction);
}

#[test]
fn host_owner_log_drives_transfer_owner_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_owner_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OwnerXferCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("OwnU") {
        let mut t = ThingTemplate::new("OwnU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("OwnU".into(), t);
    }
    let id = logic
        .create_object("OwnU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    // C++ Object::setTeam / setControllingPlayer binds a live Player
    // (Object.cpp setTeam + setControllingPlayer). Team-only set_team clears
    // owner_player_id; capture uses set_team_and_owner with the China player.
    let capture_team = Team::China;
    // Fixture must pick a LIVE faction player. apply_skirmish_config also
    // installs the dead ReplayObserver (Team::Neutral, is_alive=false); a
    // HashMap-order find() could select it and the writeback's dead-player
    // skip would then report wb=0 nondeterministically.
    let capture_pid = logic
        .get_players()
        .values()
        .find(|p| p.is_alive && p.team != Team::USA && p.team != Team::Neutral)
        .map(|p| p.id)
        .expect("capture player");
    host_owner_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_team_and_owner(capture_team, Some(capture_pid));
    }
    let events = host_owner_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.team == capture_team),
        "expected owner log {:?}",
        events
    );
    // Re-set for mutation path after drain.
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_team_and_owner(Team::USA, None);
        host_owner_log::clear();
        o.set_team_and_owner(capture_team, Some(capture_pid));
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let gla_owner = shadow.world().entity(eid).expect("e").owner;
    assert!(
        gla_owner.is_some(),
        "captured object should map to Some owner after sync; players={:?}",
        logic.get_players().keys().collect::<Vec<_>>()
    );
    // Poison to None (neutral) then apply TransferOwner from events.
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.owner = None;
    }
    let events = host_owner_log::drain();
    assert!(!events.is_empty(), "events empty");
    let n = shadow.apply_host_owner_events(&logic, &events);
    assert!(n >= 1, "owner events {n} events={events:?}");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(
        e.owner, gla_owner,
        "shadow owner should match capture mapping"
    );
    // Poison host team back to USA then writeback.
    {
        let o = logic.host_object_mut(id).expect("o");
        o.team = Team::USA;
        o.team_color = Team::USA.get_color();
    }
    let wb = shadow.writeback_owner_to_host(&mut logic);
    let _ = crate::game_logic::host_owner_ready_log::drain();
    let o = logic.host_objects().get(&id).expect("o");
    assert!(
        wb >= 1,
        "writeback={wb} host_team={:?} shadow_owner={:?} after_host={:?}",
        Team::USA,
        e.owner,
        o.team
    );
    assert_eq!(o.team, Team::GLA);
}

#[test]
fn host_production_log_drives_set_production_queue_channel() {
    use crate::game_logic::host_production_log;
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
        ThingTemplate,
    };
    // writeback_production_to_host is the opt-in GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY
    // last-writer (default off like C++ no-shadow builds). Enable it here instead of
    // depending on another test leaking the env var, and restore afterwards.
    let prev_prod_auth = std::env::var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1");
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdQueueCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("ProdBarracks") {
        let mut t = ThingTemplate::new("ProdBarracks");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ProdBarracks".into(), t);
    }
    let barracks = logic
        .create_object("ProdBarracks", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("barracks");
    {
        let o = logic.host_object_mut(barracks).expect("b");
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            template_name: "ProdRanger".into(),
            progress: 0.0,
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
        o.building_data = Some(bd);
    }
    host_production_log::clear();
    host_production_log::record_enqueue(barracks, "ProdRanger");
    let events = host_production_log::drain();
    assert_eq!(events.len(), 1);

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(barracks).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.production_queue_items.clear();
        e.production_paused = false;
        e.production_template.clear();
    }
    // Re-record for apply (drain consumed).
    host_production_log::record_enqueue(barracks, "ProdRanger");
    let events = host_production_log::drain();
    let n = shadow.apply_host_production_events(&events, &logic);
    assert!(n >= 1, "production events applied {n}");
    let e = shadow.world().entity(eid).expect("e");
    assert!(
        !e.production_queue_items.is_empty(),
        "queue should be last-written from host"
    );
    assert_eq!(e.production_queue_items[0].template_name, "ProdRanger");
    {
        let o = logic.host_object_mut(barracks).expect("b");
        if let Some(bd) = o.building_data.as_mut() {
            bd.production_queue.clear();
        }
    }
    let wb = shadow.writeback_production_to_host(&mut logic);
    let _ = shadow.writeback_production_door_to_host(&mut logic);
    let _ = shadow.writeback_body_damage_to_host(&mut logic);
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
    assert!(wb >= 1);
    let o = logic.host_objects().get(&barracks).expect("b");
    let q = &o.building_data.as_ref().expect("bd").production_queue;
    assert_eq!(q[0].template_name, "ProdRanger");
    match prev_prod_auth {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY"),
    }
}

#[test]
fn production_authority_writeback_is_queue_last_writer() {
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
        ThingTemplate,
    };
    let prev = std::env::var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1");
    assert!(gameworld_production_authority_enabled());
    host_production_progress_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdAuthWB");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("ProdAuthBarracks") {
        let mut t = ThingTemplate::new("ProdAuthBarracks");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ProdAuthBarracks".into(), t);
    }
    let oid = logic
        .create_object(
            "ProdAuthBarracks",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("barracks");
    {
        let o = logic.host_object_mut(oid).expect("b");
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            template_name: "ProdAuthRanger".into(),
            progress: 2.0,
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
        o.building_data = Some(bd);
    }

    let items = vec![HostProductionQueueItem {
        template_name: "ProdAuthRanger".into(),
        progress: 2.0,
        total_time: 10.0,
        construction_frames: 0,
        cost_supplies: 150,
        is_upgrade: false,
        quantity_total: 1,
        quantity_produced: 0,
    }];
    host_production_progress_log::record(oid, items.clone(), 0.0, 1.0);

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let n = shadow.apply_host_production_progress_events(&host_production_progress_log::drain());
    assert!(n >= 1, "progress apply {n}");

    // Dirty host queue — authority writeback must restore shadow snapshot.
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.building_data.as_mut().unwrap().production_queue.clear();
    }
    assert!(shadow.writeback_production_to_host(&mut logic) >= 1);
    let restored = logic
        .host_object(oid)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue
        .len();
    assert_eq!(
        restored,
        items.len(),
        "writeback must restore queue under production authority"
    );

    // Authority off: writeback is a no-op.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "0");
    assert!(!gameworld_production_authority_enabled());
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.building_data.as_mut().unwrap().production_queue.clear();
    }
    assert_eq!(shadow.writeback_production_to_host(&mut logic), 0);
    assert!(
        logic
            .host_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );

    host_production_progress_log::clear();
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY"),
    }
}

#[test]
fn host_veterancy_log_drives_set_veterancy_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, VeterancyLevel, host_veterancy_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("VetStatusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("VetU") {
        let mut t = ThingTemplate::new("VetU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        // Low thresholds so gain_experience levels quickly.
        t.veterancy_xp_thresholds = [10.0, 20.0, 30.0];
        // C++ ThingTemplate ctor default m_isTrainable=FALSE (ThingTemplate.cpp:994);
        // a trainable unit is exactly what this veterancy channel models.
        t.is_trainable = true;
        logic.templates.insert("VetU".into(), t);
    }
    let id = logic
        .create_object("VetU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    host_veterancy_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.gain_experience(25.0); // Elite
    }
    let events = host_veterancy_log::drain();
    assert!(
        events.iter().any(|e| e.object == id && e.ordinal >= 2),
        "expected elite+ veterancy log, got {:?}",
        events
    );
    // Re-level for mutation path.
    {
        let o = logic.host_object_mut(id).expect("o");
        o.experience.level = VeterancyLevel::Rookie;
        o.experience.current = 0.0;
        host_veterancy_log::clear();
        o.gain_experience(25.0);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.veterancy_ordinal = 0;
    }
    for ev in host_veterancy_log::drain() {
        assert!(shadow.queue_set_veterancy_for_host(ev.object, ev.ordinal));
    }
    assert!(shadow.apply_pending() >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(
        e.veterancy_ordinal >= 2,
        "shadow ordinal {}",
        e.veterancy_ordinal
    );
    // Poison host level then writeback.
    {
        let o = logic.host_object_mut(id).expect("o");
        o.experience.level = VeterancyLevel::Rookie;
    }
    let wb = shadow.writeback_experience_to_host(&mut logic);
    assert!(wb >= 1);
    let o = logic.host_objects().get(&id).expect("o");
    assert!(matches!(
        o.experience.level,
        VeterancyLevel::Elite | VeterancyLevel::Heroic
    ));
}

#[test]
fn host_force_attack_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_status_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ForceAtkStatusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("FaU") {
        let mut t = ThingTemplate::new("FaU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("FaU".into(), t);
    }
    let id = logic
        .create_object("FaU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    host_status_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_force_attack(true);
        o.set_status_using_ability(true);
        o.set_status_deployed(true);
    }
    let events = host_status_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.force_attack == Some(true))
    );
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.using_ability == Some(true))
    );
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_force_attack(true);
        o.set_status_using_ability(true);
        o.set_status_deployed(true);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.force_attack = false;
        e.using_ability = false;
        e.deployed = false;
    }
    for ev in host_status_log::drain() {
        let _ = shadow.queue_set_combat_status_for_host(ev);
    }
    assert!(shadow.apply_pending() >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.force_attack);
    assert!(e.using_ability);
    assert!(e.deployed);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.force_attack = false;
        o.status.using_ability = false;
        o.status.deployed = false;
    }
    let wb = shadow.writeback_combat_status_to_host(&mut logic);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    assert!(wb >= 1);
    let o = logic.host_objects().get(&id).expect("o");
    assert!(o.force_attack);
    assert!(o.status.using_ability);
    assert!(o.status.deployed);
}

#[test]
fn host_residual_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_status_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ResidualStatusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("ResU") {
        let mut t = ThingTemplate::new("ResU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ResU".into(), t);
    }
    let id = logic
        .create_object("ResU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    host_status_log::clear();
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_no_collisions(true);
        o.set_status_private_captured(true);
        o.set_status_faerie_fire(true);
        o.set_status_parachuting(true);
    }
    let events = host_status_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.no_collisions == Some(true))
    );
    assert!(
        events
            .iter()
            .any(|e| e.object == id && e.private_captured == Some(true))
    );
    // Re-record for mutation apply.
    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_no_collisions(true);
        o.set_status_private_captured(true);
        o.set_status_faerie_fire(true);
        o.set_status_parachuting(true);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.no_collisions = false;
        e.private_captured = false;
        e.faerie_fire = false;
        e.parachuting = false;
    }
    for ev in host_status_log::drain() {
        let _ = shadow.queue_set_combat_status_for_host(ev);
    }
    assert!(shadow.apply_pending() >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.no_collisions);
    assert!(e.private_captured);
    assert!(e.faerie_fire);
    assert!(e.parachuting);
    // Poison host so writeback last-writer is observable.
    {
        let o = logic.host_object_mut(id).expect("o");
        o.status.no_collisions = false;
        o.status.private_captured = false;
        o.status.faerie_fire = false;
        o.status.parachuting = false;
    }
    let wb = shadow.writeback_combat_status_to_host(&mut logic);
    let _ = crate::game_logic::host_combat_status_ready_log::drain();
    assert!(wb >= 1);
    let o = logic.host_objects().get(&id).expect("o");
    assert!(o.status.no_collisions);
    assert!(o.status.private_captured);
    assert!(o.status.faerie_fire);
    assert!(o.status.parachuting);
}
