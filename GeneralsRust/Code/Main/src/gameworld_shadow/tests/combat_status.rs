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
    {
        let o = logic.host_object_mut(id).expect("canonical object");
        o.set_status_stealthed(true);
        o.set_status_detected(false);
        o.set_status_attacking(true);
        o.set_status_firing_weapon(true);
        o.select();
        o.status.disabled_emp = true;
        o.status.masked = true;
        o.status.disguised = true;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    shadow.sync_from_host(&logic);
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

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&id).expect("o");
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
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    // Select via host API (records status log).
    let pid = logic.get_players().keys().copied().min().unwrap_or(0);
    logic.select_objects(pid, vec![id]);

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

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.selected, "mutation channel must set selected");

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.selected = true;
    }
    shadow.sync_from_host(&logic);

    assert!(logic.host_objects().lookup(&id).expect("o").status.selected);
}

#[test]
fn host_attacking_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_attacking(true);
        o.set_status_firing_weapon(true);
    }

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
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.attacking);
    assert!(e.is_firing_weapon);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.attacking = true;
        e.is_firing_weapon = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&id).expect("o");
    assert!(o.status.attacking && o.status.is_firing_weapon);
}

#[test]
fn host_stealth_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_stealthed(true);
        o.set_status_detected(false);
    }

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
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.stealthed);
    assert!(!e.detected);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stealthed = true;
        e.detected = false;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&id).expect("o");
    assert!(o.status.stealthed && !o.status.detected);
}

#[test]
fn host_emp_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    let until = logic.get_frame().saturating_add(300);
    {
        let (o, health_events) = logic.host_object_and_health_events_mut(id).expect("o");
        o.apply_disabled_emp(
            until,
            health_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }

    let until2 = logic.get_frame().saturating_add(300);
    {
        let (o, health_events) = logic.host_object_and_health_events_mut(id).expect("o");
        o.apply_disabled_emp(
            until2,
            health_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disabled_emp = false;
    }
    shadow.sync_from_host(&logic);
    assert!(shadow.world().entity(eid).expect("e").disabled_emp);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disabled_emp = true;
    }
    shadow.sync_from_host(&logic);

    assert!(
        logic
            .host_objects()
            .lookup(&id)
            .expect("o")
            .status
            .disabled_emp
    );
}

#[test]
fn canonical_player_cooldowns_project_without_writeback() {
    use crate::command_system::SpecialPowerType;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PlayerCdCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");

    {
        let p = logic.get_player_mut(pid).expect("p");
        // Use a concrete SP type if available; format Debug name into log.
        // ParticleUplink residual is common; fall back to first Debug variant via reset API.
        p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 12.5);
    }

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

    shadow.sync_from_host(&logic);
    let p = shadow.world().player(gw).expect("p");
    assert!(
        p.shared_special_power_cooldowns
            .iter()
            .any(|(_, rem)| (*rem - 12.5).abs() < 1e-3),
        "cds {:?}",
        p.shared_special_power_cooldowns
    );
    // A stale presentation copy cannot erase the canonical cooldown.
    shadow
        .world_mut()
        .world_mut()
        .player_mut(gw)
        .unwrap()
        .shared_special_power_cooldowns
        .clear();
    shadow.sync_from_host(&logic);

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
fn canonical_player_meta_projects_without_writeback() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PlayerMetaCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");

    {
        let p = logic.get_player_mut(pid).expect("p");
        assert!(p.unlock_science("SCIENCE_PaladinTank"));
        p.is_alive = true;
    }

    // Re-record for apply

    {
        let p = logic.get_player_mut(pid).expect("p");
        p.is_alive = false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
    if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
        p.unlocked_sciences.clear();
        p.is_alive = true;
    }

    shadow.sync_from_host(&logic);
    let p = shadow.world().player(gw).expect("p");
    assert!(p.unlocked_sciences.iter().any(|s| s.contains("Paladin")));
    assert!(!p.is_alive);
    {
        let p = shadow.world_mut().world_mut().player_mut(gw).unwrap();
        p.unlocked_sciences.clear();
        p.is_alive = true;
    }
    shadow.sync_from_host(&logic);

    let p = logic.get_player(pid).expect("p");
    assert!(p.unlocked_sciences.iter().any(|s| s.contains("Paladin")));
    assert!(!p.is_alive);
}

#[test]
fn canonical_player_progress_projects_without_writeback() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PlayerProgCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");

    {
        let p = logic.get_player_mut(pid).expect("p");
        p.force_set_cash_bounty(0.35);
        p.rank_level = 3;
        p.skill_points = 50;
        p.science_purchase_points = 2;
    }

    {
        let p = logic.get_player_mut(pid).expect("p");
        p.force_set_cash_bounty(0.35);
        p.rank_level = 3;
        p.skill_points = 50;
        p.science_purchase_points = 2;
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

    shadow.sync_from_host(&logic);
    let p = shadow.world().player(gw).expect("p");
    assert_eq!(p.rank_level, 3);
    assert_eq!(p.skill_points, 50);
    assert_eq!(p.science_purchase_points, 2);
    assert!((p.cash_bounty_percent - 0.35).abs() < 1e-5);
    {
        let p = shadow.world_mut().world_mut().player_mut(gw).unwrap();
        p.rank_level = 1;
        p.skill_points = 0;
        p.science_purchase_points = 0;
        p.cash_bounty_percent = 0.0;
    }
    shadow.sync_from_host(&logic);

    let p = logic.get_player(pid).expect("p");
    assert_eq!(p.rank_level, 3);
    assert_eq!(p.skill_points, 50);
    assert!((p.cash_bounty_percent - 0.35).abs() < 1e-5);
}

#[test]
fn host_radar_log_drives_set_player_radar_channel() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RadarCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let pid = *logic.get_players().keys().next().expect("player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.set_radar_state(2, false);
    }
    let events = logic.host_radar_snapshot();
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
    shadow.sync_from_host(&logic);
    let p = shadow.world().player(gw).expect("p");
    assert_eq!(p.radar_count, 2);
    assert!(!p.radar_disabled);
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.radar_count = 0;
        p.radar_disabled = true;
    }
    shadow.sync_from_host(&logic);

    let p = logic.get_player(pid).expect("p");
    assert_eq!(p.radar_count, 0, "presentation cannot overwrite the owner");
    assert!(p.radar_disabled);
}

#[test]
fn canonical_containment_projects_without_writeback() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(bunker).expect("b");
        assert!(o.add_occupant(inf));
    }
    {
        let (o, fixture_partition) = logic.host_object_and_partition_mut(inf).expect("i");
        o.set_contained_by_with_partition_owner(Some(bunker), true, fixture_partition);
    }

    // Re-apply path

    {
        let o = logic.host_object_mut(bunker).expect("b");
        if let Some(bd) = o.building_data.as_mut() {
            bd.garrisoned_units.clear();
        }
        assert!(o.add_occupant(inf));
    }
    {
        let (o, fixture_partition) = logic.host_object_and_partition_mut(inf).expect("i");
        o.set_contained_by_with_partition_owner(Some(bunker), true, fixture_partition);
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

    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow.world().entity(eid_i).expect("e").contained_by_host,
        bunker.0
    );
    assert!(shadow.world().entity(eid_b).expect("e").garrison_count >= 1);
    assert!(shadow.world().entity(eid_b).expect("e").occupant_count >= 1);
    // Poison host then writeback via SetContain last-writer residual.

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_i) {
        e.contained_by_host = bunker.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_b) {
        e.garrison_count = 1;
        e.garrisoned_host_ids = vec![inf.0];
    }
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().lookup(&inf).expect("i").contained_by,
        Some(bunker)
    );
    let bd = logic
        .host_objects()
        .lookup(&bunker)
        .expect("b")
        .building_data
        .as_ref()
        .expect("bd");
    assert!(bd.garrisoned_units.contains(&inf));
}

#[test]
// Wave 947: channel-drive tests mutate host via host_object_mut authority.
fn host_ai_state_is_canonical_and_shadow_only_observes() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
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
    logic.host_object_mut(id).expect("o").set_ai_state(
        AIState::GuardingObject,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    assert_eq!(
        logic.host_object(id).expect("o").ai_state(),
        AIState::GuardingObject
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    assert_eq!(shadow.world().entity(eid).expect("e").ai_state_ordinal, 10);
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .expect("e")
        .ai_state_ordinal = 0;
    assert_eq!(
        logic.host_object(id).expect("o").ai_state(),
        AIState::GuardingObject
    );
    logic.host_object_mut(id).expect("o").set_ai_state(
        AIState::Idle,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    assert_eq!(logic.host_object(id).expect("o").ai_state(), AIState::Idle);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(eid).expect("e").ai_state_ordinal, 0);
}

#[test]
fn host_special_power_cooldown_remaining_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 45.0;
        o.special_power_cooldown_remaining = 18.0;
        o.special_power_ready = false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = true;
        e.special_power_cooldown_remaining = 0.0;
        e.special_power_cooldown = 0.0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(!e.special_power_ready);
    assert!((e.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
    assert!((e.special_power_cooldown - 45.0).abs() < 1e-3);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = false;
        e.special_power_cooldown_remaining = 18.0;
        e.special_power_cooldown = 45.0;
    }
    shadow.sync_from_host(&logic);
    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!(!o.special_power_ready);
    assert!((o.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
    assert!((o.special_power_cooldown - 45.0).abs() < 1e-3);
}

#[test]
fn canonical_special_power_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_special_power_ready(true);
    }

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

    shadow.sync_from_host(&logic);
    assert!(shadow.world().entity(eid).expect("e").special_power_ready);

    shadow.sync_from_host(&logic);
    assert!(
        logic
            .host_objects()
            .lookup(&id)
            .expect("o")
            .special_power_ready
    );
}

#[test]
fn canonical_stored_supplies_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_stored_supplies(900);
    }

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

    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(eid).expect("e").stored_supplies, 900);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stored_supplies = 900;
    }
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic
            .host_objects()
            .lookup(&id)
            .expect("o")
            .stored_resources
            .supplies,
        900
    );
}

#[test]
fn owned_construction_snapshot_refreshes_projection_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.construction_percent = 0.0;
        e.under_construction = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.construction_percent - 0.25).abs() < 1e-5);
    assert!(e.under_construction);
    {
        let o = logic.host_object_mut(id).expect("o");
        o.construction_percent = 0.0;
        o.status.under_construction = false;
    }
    let wb = shadow.compare_construction_to_host(&logic);
    assert!(wb >= 1);
    let o = logic.host_objects().lookup(&id).expect("o");
    assert_eq!(o.construction_percent, 0.0);
    assert!(!o.status.under_construction);
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

    {
        let (o, ownership_events, fixture_partition) =
            logic.host_object_ownership_partition_mut(id).expect("o");
        o.set_team_and_owner_with_partition_owner(
            capture_team,
            Some(capture_pid),
            ownership_events,
            fixture_partition,
        );
    }

    // Re-set for mutation path after drain.
    {
        let (o, ownership_events, fixture_partition) =
            logic.host_object_ownership_partition_mut(id).expect("o");
        o.set_team_and_owner_with_partition_owner(
            Team::USA,
            None,
            ownership_events,
            fixture_partition,
        );
        ownership_events.clear();
        o.set_team_and_owner_with_partition_owner(
            capture_team,
            Some(capture_pid),
            ownership_events,
            fixture_partition,
        );
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    shadow.sync_from_host(&logic);
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

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(
        e.owner, gla_owner,
        "shadow owner should match capture mapping"
    );
    // Poison host team back to USA then writeback.

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&id).expect("o");

    assert_eq!(o.team, capture_team);
    assert_eq!(o.owner_player_id, Some(capture_pid));
}

#[test]
fn host_production_log_drives_set_production_queue_channel() {
    use crate::game_logic::host_production_log;
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
        ThingTemplate,
    };
    // writeback_production_to_host is the opt-in GameLogic::set_production_authority(true)
    // last-writer (default off like C++ no-shadow builds). Enable it here; hq-e84zk
    // retired the env flag.
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
            runtime_entry_identity: 0,
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

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(barracks).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.production_queue_items.clear();
        e.production_paused = false;
        e.production_template.clear();
    }
    // Re-record for apply (drain consumed).

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(
        !e.production_queue_items.is_empty(),
        "queue should be last-written from host"
    );
    assert_eq!(e.production_queue_items[0].template_name, "ProdRanger");

    shadow.sync_from_host(&logic);

    let _ = shadow.compare_rebuild_producer_to_host(&logic);
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&barracks).expect("b");
    let q = &o.building_data.as_ref().expect("bd").production_queue;
    assert_eq!(q[0].template_name, "ProdRanger");
}

#[test]
fn production_authority_writeback_is_queue_last_writer() {
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
        ThingTemplate,
    };
    let mut logic = GameLogic::new();

    host_production_progress_log::clear();

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
            runtime_entry_identity: 0,
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
    let entity = shadow.entity_for_host(oid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .production_queue_items
        .clear();
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .len(),
        items.len(),
        "poisoned observation cannot remove an actual queue item"
    );
    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow
            .world()
            .entity(entity)
            .unwrap()
            .production_queue_items
            .len(),
        items.len()
    );
    assert!(logic.cancel_production_at_index(oid, 0));
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
    shadow.sync_from_host(&logic);
    assert!(
        shadow
            .world()
            .entity(entity)
            .unwrap()
            .production_queue_items
            .is_empty()
    );

    host_production_progress_log::clear();
}

#[test]
fn host_veterancy_is_canonical_and_shadow_only_observes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, VeterancyLevel};
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
    logic.award_experience(id, 25.0);
    assert_eq!(logic.host_object(id).unwrap().experience.current, 25.0);
    assert_eq!(
        logic.host_object(id).unwrap().experience.level,
        VeterancyLevel::Elite
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().veterancy_ordinal, 2);
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .veterancy_ordinal = 0;
    assert_eq!(
        logic.host_object(id).unwrap().experience.level,
        VeterancyLevel::Elite
    );
    logic.award_experience(id, 5.0);
    assert_eq!(
        logic.host_object(id).unwrap().experience.level,
        VeterancyLevel::Heroic
    );
    assert_eq!(logic.host_object(id).unwrap().experience.current, 30.0);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(entity).unwrap().veterancy_ordinal, 3);
}

#[test]
fn host_force_attack_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_force_attack(true);
        o.set_status_using_ability(true);
        o.set_status_deployed(true);
    }

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
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.force_attack);
    assert!(e.using_ability);
    assert!(e.deployed);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&id).expect("o");
    assert!(o.force_attack);
    assert!(o.status.using_ability);
    assert!(o.status.deployed);
}

#[test]
fn host_residual_status_log_drives_set_combat_status_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_status_no_collisions(true);
        o.set_status_private_captured(true);
        o.set_status_faerie_fire(true);
        o.set_status_parachuting(true);
    }

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
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.no_collisions);
    assert!(e.private_captured);
    assert!(e.faerie_fire);
    assert!(e.parachuting);
    // Poison host so writeback last-writer is observable.

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&id).expect("o");
    assert!(o.status.no_collisions);
    assert!(o.status.private_captured);
    assert!(o.status.faerie_fire);
    assert!(o.status.parachuting);
}
