//! Movement/weapon/stealth/turret/power host-log channels.

use super::*;

#[test]
fn host_movement_log_drives_set_movement_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_movement_log};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MvCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("MvU") {
        let mut t = ThingTemplate::new("MvU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("MvU".into(), t);
    }
    let oid = logic
        .create_object("MvU", Team::USA, glam::Vec3::new(26.0, 0.0, 26.0))
        .expect("id");
    host_movement_log::clear();
    crate::game_logic::host_physics_motive_log::clear();
    crate::game_logic::host_locomotor_log::clear();
    crate::game_logic::host_bounce_land_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.movement.velocity = Vec3::new(3.0, 0.0, 4.0);
        o.movement.max_speed = 12.5;
        o.movement.path = vec![Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 0.0, 2.0)];
        o.movement.current_path_index = 1;
        o.record_host_movement();
    }
    let events = host_movement_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && (e.velocity[0] - 3.0).abs() < 1e-5
                && (e.velocity[2] - 4.0).abs() < 1e-5
                && (e.max_speed - 12.5).abs() < 1e-5
                && e.path_index == 1
                && e.path_len == 2
                && e.path_waypoints.len() == 2
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_movement();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [0.0, 0.0, 0.0];
        e.move_max_speed = 1.0;
        e.path_index = 0;
        e.path_len = 0;
        e.path_waypoints.clear();
    }
    let n = shadow.apply_host_movement_events(&host_movement_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.velocity[0] - 3.0).abs() < 1e-5);
    assert!((e.move_max_speed - 12.5).abs() < 1e-5);
    assert_eq!(e.path_index, 1);
    assert_eq!(e.path_len, 2);
    assert_eq!(e.path_waypoints.len(), 2);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.movement.velocity = Vec3::ZERO;
        o.movement.max_speed = 1.0;
        o.movement.path.clear();
        o.movement.current_path_index = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [3.0, 0.0, 4.0];
        e.move_max_speed = 12.5;
        e.path_index = 1;
        e.path_len = 2;
        e.path_waypoints = vec![[1.0, 0.0, 1.0], [2.0, 0.0, 2.0]];
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
    let o = logic.host_objects().get(&oid).expect("o");
    assert!((o.movement.velocity.x - 3.0).abs() < 1e-5);
    assert!((o.movement.max_speed - 12.5).abs() < 1e-5);
    assert_eq!(o.movement.current_path_index, 1);
    assert_eq!(o.movement.path.len(), 2);
}

#[test]
fn host_weapon_stats_log_drives_set_weapon_stats_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon, host_weapon_stats_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WsCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WsU") {
        let mut t = ThingTemplate::new("WsU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("WsU".into(), t);
    }
    let oid = logic
        .create_object("WsU", Team::GLA, glam::Vec3::new(25.0, 0.0, 25.0))
        .expect("id");
    host_weapon_stats_log::clear();
    crate::game_logic::host_body_damage_log::clear();
    crate::game_logic::host_death_type_log::clear();
    crate::game_logic::host_radar_extend_log::clear();
    crate::game_logic::host_shock_stun_log::clear();
    crate::game_logic::host_rebuild_producer_log::clear();
    crate::game_logic::host_sole_healing_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.weapon = Some(Weapon {
            damage: 33.0,
            range: 140.0,
            min_range: 4.0,
            reload_time: 0.75,
            ammo: Some(12),
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 90.0,
            ..Weapon::default()
        });
        o.secondary_weapon = Some(Weapon {
            damage: 9.0,
            range: 80.0,
            ..Weapon::default()
        });
        o.record_host_weapon_stats();
    }
    let events = host_weapon_stats_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.has_weapon
                && (e.weapon_damage - 33.0).abs() < 1e-5
                && (e.weapon_range - 140.0).abs() < 1e-5
                && e.weapon_ammo == 12
                && e.has_secondary_weapon
                && (e.secondary_weapon_damage - 9.0).abs() < 1e-5
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_weapon_stats();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.has_weapon = false;
        e.weapon_damage = 0.0;
        e.weapon_range = 0.0;
        e.weapon_min_range = 0.0;
        e.weapon_reload_time = 0.0;
        e.weapon_ammo = u32::MAX;
        e.weapon_can_target_air = false;
        e.weapon_can_target_ground = false;
        e.weapon_projectile_speed = 0.0;
        e.has_secondary_weapon = false;
        e.secondary_weapon_damage = 0.0;
        e.secondary_weapon_range = 0.0;
    }
    let n = shadow.apply_host_weapon_stats_events(&host_weapon_stats_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.has_weapon && e.has_secondary_weapon);
    assert!((e.weapon_damage - 33.0).abs() < 1e-5);
    assert!((e.weapon_range - 140.0).abs() < 1e-5);
    assert_eq!(e.weapon_ammo, 12);
    assert!((e.secondary_weapon_damage - 9.0).abs() < 1e-5);
    {
        let o = logic.host_object_mut(oid).expect("o");
        if let Some(w) = o.weapon.as_mut() {
            w.damage = 1.0;
            w.range = 1.0;
            w.min_range = 0.0;
            w.reload_time = 0.1;
            w.ammo = None;
            w.can_target_air = false;
            w.can_target_ground = true;
            w.projectile_speed = 0.0;
        }
        if let Some(w) = o.secondary_weapon.as_mut() {
            w.damage = 1.0;
            w.range = 1.0;
        }
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_damage = 33.0;
        e.weapon_range = 140.0;
        e.weapon_min_range = 4.0;
        e.weapon_reload_time = 0.75;
        e.weapon_ammo = 12;
        e.weapon_can_target_air = true;
        e.weapon_can_target_ground = true;
        e.weapon_projectile_speed = 90.0;
        e.secondary_weapon_damage = 9.0;
        e.secondary_weapon_range = 80.0;
    }
    assert!(shadow.writeback_weapon_stats_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_weapon_stats_ready_log::drain();
    let _ = shadow.writeback_fire_intent_to_host(&mut logic);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    let w = o.weapon.as_ref().expect("w");
    assert!((w.damage - 33.0).abs() < 1e-5);
    assert!((w.range - 140.0).abs() < 1e-5);
    assert_eq!(w.ammo, Some(12));
    let s = o.secondary_weapon.as_ref().expect("s");
    assert!((s.damage - 9.0).abs() < 1e-5);
    assert!((s.range - 80.0).abs() < 1e-5);
}

#[test]
fn host_vision_camo_log_drives_set_vision_camo_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_vision_camo_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("VcCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("VcU") {
        let mut t = ThingTemplate::new("VcU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("VcU".into(), t);
    }
    let oid = logic
        .create_object("VcU", Team::China, glam::Vec3::new(24.0, 0.0, 24.0))
        .expect("id");
    host_vision_camo_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.vision_spied_mask = 0b101;
        o.camo_friendly_opacity = 0.35;
        o.camo_stealth_look = 2;
        o.record_host_vision_camo();
    }
    let events = host_vision_camo_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.vision_spied_mask == 0b101
                && (e.camo_friendly_opacity - 0.35).abs() < 1e-5
                && e.camo_stealth_look == 2
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_vision_camo();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.vision_spied_mask = 0;
        e.camo_friendly_opacity = 1.0;
        e.camo_stealth_look = 0;
    }
    let n = shadow.apply_host_vision_camo_events(&host_vision_camo_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.vision_spied_mask, 0b101);
    assert!((e.camo_friendly_opacity - 0.35).abs() < 1e-5);
    assert_eq!(e.camo_stealth_look, 2);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.vision_spied_mask = 0;
        o.camo_friendly_opacity = 1.0;
        o.camo_stealth_look = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.vision_spied_mask = 0b101;
        e.camo_friendly_opacity = 0.35;
        e.camo_stealth_look = 2;
    }
    assert!(shadow.writeback_vision_camo_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_vision_camo_ready_log::drain();
    let _ = shadow.writeback_stealth_delay_to_host(&mut logic);
    let _ = crate::game_logic::host_stealth_delay_ready_log::drain();
    let _ = shadow.writeback_combat_attack_to_host(&mut logic);
    let _ = crate::game_logic::host_combat_attack_ready_log::drain();
    let _ = shadow.writeback_fire_intent_to_host(&mut logic);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.vision_spied_mask, 0b101);
    assert!((o.camo_friendly_opacity - 0.35).abs() < 1e-5);
    assert_eq!(o.camo_stealth_look, 2);
}

#[test]
fn host_disguise_log_drives_set_disguise_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_disguise_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DgCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("DgU") {
        let mut t = ThingTemplate::new("DgU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("DgU".into(), t);
    }
    let oid = logic
        .create_object("DgU", Team::GLA, glam::Vec3::new(23.0, 0.0, 23.0))
        .expect("id");
    host_disguise_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.disguise_as_template = Some("AmericaVehicleHumvee".into());
        o.disguise_as_team = Some(Team::USA);
        o.record_host_disguise();
    }
    let events = host_disguise_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid && e.template == "AmericaVehicleHumvee" && e.team_ordinal == 0
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_disguise();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disguise_as_template.clear();
        e.disguise_as_team_ordinal = 255;
    }
    let n = shadow.apply_host_disguise_events(&host_disguise_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.disguise_as_template, "AmericaVehicleHumvee");
    assert_eq!(e.disguise_as_team_ordinal, 0);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.disguise_as_template = None;
        o.disguise_as_team = None;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disguise_as_template = "AmericaVehicleHumvee".into();
        e.disguise_as_team_ordinal = 0;
    }
    assert!(shadow.writeback_disguise_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_disguise_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(
        o.disguise_as_template.as_deref(),
        Some("AmericaVehicleHumvee")
    );
    assert_eq!(o.disguise_as_team, Some(Team::USA));
}

#[test]
fn host_overlord_log_drives_set_overlord_addon_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_overlord_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OlCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("OlU") {
        let mut t = ThingTemplate::new("OlU");
        t.set_health(400.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("OlU".into(), t);
    }
    let oid = logic
        .create_object("OlU", Team::China, glam::Vec3::new(21.0, 0.0, 21.0))
        .expect("id");
    host_overlord_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.has_overlord_gattling_addon = true;
        o.has_overlord_propaganda_addon = false;
        o.overlord_bunker_capacity = Some(4);
        o.is_helix_transport = true;
        o.record_host_overlord();
    }
    let events = host_overlord_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.has_gattling
                && !e.has_propaganda
                && e.bunker_capacity == 4
                && e.is_helix_transport
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_overlord();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.has_overlord_gattling_addon = false;
        e.has_overlord_propaganda_addon = true;
        e.overlord_bunker_capacity = u16::MAX;
        e.is_helix_transport = false;
    }
    let n = shadow.apply_host_overlord_events(&host_overlord_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.has_overlord_gattling_addon && !e.has_overlord_propaganda_addon);
    assert_eq!(e.overlord_bunker_capacity, 4);
    assert!(e.is_helix_transport);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.has_overlord_gattling_addon = false;
        o.has_overlord_propaganda_addon = true;
        o.overlord_bunker_capacity = None;
        o.is_helix_transport = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.has_overlord_gattling_addon = true;
        e.has_overlord_propaganda_addon = false;
        e.overlord_bunker_capacity = 4;
        e.is_helix_transport = true;
    }
    assert!(shadow.writeback_overlord_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_overlord_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.has_overlord_gattling_addon && !o.has_overlord_propaganda_addon);
    assert_eq!(o.overlord_bunker_capacity, Some(4));
    assert!(o.is_helix_transport);
}

#[test]
fn host_stealth_flags_log_drives_set_stealth_flags_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_stealth_flags_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StfCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("StfU") {
        let mut t = ThingTemplate::new("StfU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("StfU".into(), t);
    }
    let oid = logic
        .create_object("StfU", Team::GLA, glam::Vec3::new(20.0, 0.0, 20.0))
        .expect("id");
    host_stealth_flags_log::clear();
    crate::game_logic::host_stealth_delay_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.innate_stealth = true;
        o.stealth_breaks_on_attack = true;
        o.stealth_breaks_on_move = false;
        o.is_tunnel_network = true;
        o.passengers_allowed_to_fire = true;
        o.record_host_stealth_flags();
    }
    let events = host_stealth_flags_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.innate_stealth
                && e.stealth_breaks_on_attack
                && !e.stealth_breaks_on_move
                && e.is_tunnel_network
                && e.passengers_allowed_to_fire
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_stealth_flags();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.innate_stealth = false;
        e.stealth_breaks_on_attack = false;
        e.stealth_breaks_on_move = true;
        e.is_tunnel_network = false;
        e.passengers_allowed_to_fire = false;
    }
    let n = shadow.apply_host_stealth_flags_events(&host_stealth_flags_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.innate_stealth && e.stealth_breaks_on_attack && !e.stealth_breaks_on_move);
    assert!(e.is_tunnel_network && e.passengers_allowed_to_fire);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.innate_stealth = false;
        o.stealth_breaks_on_attack = false;
        o.stealth_breaks_on_move = true;
        o.is_tunnel_network = false;
        o.passengers_allowed_to_fire = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.innate_stealth = true;
        e.stealth_breaks_on_attack = true;
        e.stealth_breaks_on_move = false;
        e.is_tunnel_network = true;
        e.passengers_allowed_to_fire = true;
    }
    assert!(shadow.writeback_stealth_flags_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_stealth_flags_ready_log::drain();
    let _ = shadow.writeback_stealth_delay_to_host(&mut logic);
    let _ = crate::game_logic::host_stealth_delay_ready_log::drain();
    let _ = shadow.writeback_combat_attack_to_host(&mut logic);
    let _ = crate::game_logic::host_combat_attack_ready_log::drain();
    let _ = shadow.writeback_fire_intent_to_host(&mut logic);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.innate_stealth && o.stealth_breaks_on_attack && !o.stealth_breaks_on_move);
    assert!(o.is_tunnel_network && o.passengers_allowed_to_fire);
}

#[test]
fn host_hive_log_drives_set_hive_slaves_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_hive_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HiveCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("HiveU") {
        let mut t = ThingTemplate::new("HiveU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("HiveU".into(), t);
    }
    let oid = logic
        .create_object("HiveU", Team::GLA, glam::Vec3::new(19.0, 0.0, 19.0))
        .expect("id");
    host_hive_log::clear();
    crate::game_logic::host_hijacker_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.hive_slave_count = 3;
        o.hive_slave_hp = 55.0;
        o.record_host_hive();
    }
    let events = host_hive_log::drain();
    assert!(
        events
            .iter()
            .any(|e| { e.object == oid && e.slave_count == 3 && (e.slave_hp - 55.0).abs() < 1e-3 }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_hive();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.hive_slave_count = 0;
        e.hive_slave_hp = 0.0;
    }
    let n = shadow.apply_host_hive_events(&host_hive_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.hive_slave_count, 3);
    assert!((e.hive_slave_hp - 55.0).abs() < 1e-3);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.hive_slave_count = 0;
        o.hive_slave_hp = 0.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.hive_slave_count = 3;
        e.hive_slave_hp = 55.0;
    }
    assert!(shadow.writeback_hive_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_hive_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.hive_slave_count, 3);
    assert!((o.hive_slave_hp - 55.0).abs() < 1e-3);
}

#[test]
fn host_contain_capacity_log_drives_set_contain_capacity_channel() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_contain_capacity_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CapCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CapU") {
        let mut t = ThingTemplate::new("CapU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CapU".into(), t);
    }
    let oid = logic
        .create_object("CapU", Team::USA, glam::Vec3::new(18.0, 0.0, 18.0))
        .expect("id");
    host_contain_capacity_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.max_transport = 5;
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.max_garrison = 8;
        o.building_data = Some(bd);
        o.record_host_contain_capacity();
    }
    let events = host_contain_capacity_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.max_transport == 5 && e.max_garrison == 8),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_contain_capacity();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.max_transport = 0;
        e.max_garrison = 0;
    }
    let n = shadow.apply_host_contain_capacity_events(&host_contain_capacity_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.max_transport, 5);
    assert_eq!(e.max_garrison, 8);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.max_transport = 0;
        if let Some(bd) = o.building_data.as_mut() {
            bd.max_garrison = 0;
        }
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.max_transport = 5;
        e.max_garrison = 8;
    }
    assert!(shadow.writeback_contain_capacity_to_host(&mut logic) >= 1);
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.max_transport, 5);
    assert_eq!(o.building_data.as_ref().map(|bd| bd.max_garrison), Some(8));
}

#[test]
fn host_overcharge_log_drives_set_overcharge_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_overcharge_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OcCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("OCU") {
        let mut t = ThingTemplate::new("OCU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("OCU".into(), t);
    }
    let oid = logic
        .create_object("OCU", Team::China, glam::Vec3::new(17.0, 0.0, 17.0))
        .expect("id");
    host_overcharge_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_overcharge_enabled(true);
    }
    let events = host_overcharge_log::drain();
    assert!(
        events.iter().any(|e| e.object == oid && e.enabled),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_overcharge();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overcharge_enabled = false;
    }
    let n = shadow.apply_host_overcharge_events(&host_overcharge_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.overcharge_enabled);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.overcharge_enabled = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overcharge_enabled = true;
    }
    assert!(shadow.writeback_overcharge_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_overcharge_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.overcharge_enabled);
}

#[test]
fn host_weapon_set_log_drives_set_weapon_set_flags_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_weapon_set_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WSetCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WSU") {
        let mut t = ThingTemplate::new("WSU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("WSU".into(), t);
    }
    let oid = logic
        .create_object("WSU", Team::USA, glam::Vec3::new(16.0, 0.0, 16.0))
        .expect("id");
    host_weapon_set_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.weapon_set_player_upgrade = true;
        o.armed_riders_upgrade_weapon_set = true;
        o.record_host_weapon_set();
    }
    let events = host_weapon_set_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.player_upgrade && e.armed_riders),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_weapon_set();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_set_player_upgrade = false;
        e.armed_riders_upgrade_weapon_set = false;
    }
    let n = shadow.apply_host_weapon_set_events(&host_weapon_set_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.weapon_set_player_upgrade && e.armed_riders_upgrade_weapon_set);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.weapon_set_player_upgrade = false;
        o.armed_riders_upgrade_weapon_set = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_set_player_upgrade = true;
        e.armed_riders_upgrade_weapon_set = true;
    }
    assert!(shadow.writeback_weapon_set_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_weapon_set_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.weapon_set_player_upgrade && o.armed_riders_upgrade_weapon_set);
}

#[test]
fn host_ai_attitude_log_drives_set_ai_attitude_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_ai_attitude_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AttCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AU") {
        let mut t = ThingTemplate::new("AU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AU".into(), t);
    }
    let oid = logic
        .create_object("AU", Team::USA, glam::Vec3::new(15.0, 0.0, 15.0))
        .expect("id");
    host_ai_attitude_log::clear();
    crate::game_logic::host_ai_mood_log::clear();
    crate::game_logic::host_ai_request_log::clear();
    crate::game_logic::host_ai_decision_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_ai_attitude_i8(2);
    }
    let events = host_ai_attitude_log::drain();
    assert!(
        events.iter().any(|e| e.object == oid && e.attitude == 2),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_ai_attitude();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ai_attitude = 0;
    }
    let n = shadow.apply_host_ai_attitude_events(&host_ai_attitude_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.ai_attitude, 2);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.ai_attitude = -2;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ai_attitude = 2;
    }
    assert!(shadow.writeback_ai_attitude_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_ai_attitude_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.ai_attitude, 2);
}

#[test]
fn host_guard_log_drives_set_guard_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_guard_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GuardCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("GU") {
        let mut t = ThingTemplate::new("GU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("GU".into(), t);
    }
    let oid = logic
        .create_object("GU", Team::USA, glam::Vec3::new(12.0, 0.0, 12.0))
        .expect("id");
    let tid = logic
        .create_object("GU", Team::USA, glam::Vec3::new(14.0, 0.0, 14.0))
        .expect("tid");
    host_guard_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_guard_position(Some(glam::Vec3::new(3.0, 0.0, 5.0)));
        o.set_guard_target(Some(tid));
    }
    let events = host_guard_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.target_host == tid.0
                && e.position
                    .map(|p| (p[0] - 3.0).abs() < 1e-3 && (p[2] - 5.0).abs() < 1e-3)
                    .unwrap_or(false)
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_guard();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.guard_position = None;
        e.guard_target_host = 0;
    }
    let n = shadow.apply_host_guard_events(&host_guard_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    let gp = e.guard_position.expect("gp");
    assert!((gp[0] - 3.0).abs() < 1e-3 && (gp[2] - 5.0).abs() < 1e-3);
    assert_eq!(e.guard_target_host, tid.0);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.guard_position = None;
        o.guard_target = None;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.guard_position = Some([3.0, 0.0, 5.0]);
        e.guard_target_host = tid.0;
    }
    assert!(shadow.writeback_guard_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_guard_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    let p = o.guard_position.expect("host gp");
    assert!((p.x - 3.0).abs() < 1e-3 && (p.z - 5.0).abs() < 1e-3);
    assert_eq!(o.guard_target, Some(tid));
}

#[test]
fn host_continuous_fire_log_drives_set_continuous_fire_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_continuous_fire_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CFireCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CFU") {
        let mut t = ThingTemplate::new("CFU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CFU".into(), t);
    }
    let oid = logic
        .create_object("CFU", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
        .expect("id");
    host_continuous_fire_log::clear();
    crate::game_logic::host_combat_attack_log::clear();
    crate::game_logic::host_fire_intent_log::clear();
    crate::game_logic::host_fire_spawn_log::clear();
    crate::game_logic::host_projectile_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.continuous_fire_level = 2;
        o.continuous_fire_consecutive = 9;
        o.continuous_fire_coast_until_frame = 44;
        o.record_host_continuous_fire();
    }
    let events = host_continuous_fire_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid && e.level == 2 && e.consecutive == 9 && e.coast_until_frame == 44
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_continuous_fire();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.continuous_fire_level = 0;
        e.continuous_fire_consecutive = 0;
        e.continuous_fire_coast_until_frame = 0;
    }
    let n = shadow.apply_host_continuous_fire_events(&host_continuous_fire_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.continuous_fire_level, 2);
    assert_eq!(e.continuous_fire_consecutive, 9);
    assert_eq!(e.continuous_fire_coast_until_frame, 44);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.continuous_fire_level = 0;
        o.continuous_fire_consecutive = 0;
        o.continuous_fire_coast_until_frame = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.continuous_fire_level = 2;
        e.continuous_fire_consecutive = 9;
        e.continuous_fire_coast_until_frame = 44;
    }
    assert!(shadow.writeback_continuous_fire_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_continuous_fire_ready_log::drain();
    let _ = shadow.writeback_combat_attack_to_host(&mut logic);
    let _ = crate::game_logic::host_combat_attack_ready_log::drain();
    let _ = shadow.writeback_fire_intent_to_host(&mut logic);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.continuous_fire_level, 2);
    assert_eq!(o.continuous_fire_consecutive, 9);
    assert_eq!(o.continuous_fire_coast_until_frame, 44);
}

#[test]
fn host_detector_log_drives_set_detector_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_detector_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DetCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("DetU") {
        let mut t = ThingTemplate::new("DetU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DetU".into(), t);
    }
    let oid = logic
        .create_object("DetU", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("id");
    host_detector_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_detector_state(true, 175.0, 12);
    }
    let events = host_detector_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.is_detector
                && (e.detection_range - 175.0).abs() < 1e-3
                && e.detection_rate_frames == 12
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_detector();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_detector = false;
        e.detection_range = 0.0;
        e.detection_rate_frames = 0;
    }
    let n = shadow.apply_host_detector_events(&host_detector_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.is_detector);
    assert!((e.detection_range - 175.0).abs() < 1e-3);
    assert_eq!(e.detection_rate_frames, 12);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.is_detector = false;
        o.detection_range = 0.0;
        o.detection_rate_frames = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_detector = true;
        e.detection_range = 175.0;
        e.detection_rate_frames = 12;
    }
    assert!(shadow.writeback_detector_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_detector_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.is_detector);
    assert!((o.detection_range - 175.0).abs() < 1e-3);
    assert_eq!(o.detection_rate_frames, 12);
}

#[test]
fn host_target_location_log_drives_set_target_location_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_target_location_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TLocCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("TLocU") {
        let mut t = ThingTemplate::new("TLocU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("TLocU".into(), t);
    }
    let oid = logic
        .create_object("TLocU", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
        .expect("id");
    host_target_location_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_target_location(Some(glam::Vec3::new(11.0, 0.0, 13.0)));
    }
    let events = host_target_location_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.location
                    .map(|p| (p[0] - 11.0).abs() < 1e-3 && (p[2] - 13.0).abs() < 1e-3)
                    .unwrap_or(false)
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_target_location();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.target_location = None;
    }
    let n = shadow.apply_host_target_location_events(&host_target_location_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    let tl = e.target_location.expect("tl");
    assert!((tl[0] - 11.0).abs() < 1e-3 && (tl[2] - 13.0).abs() < 1e-3);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.target_location = None;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.target_location = Some([11.0, 0.0, 13.0]);
    }
    assert!(shadow.writeback_target_location_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_target_location_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    let p = o.target_location.expect("host tl");
    assert!((p.x - 11.0).abs() < 1e-3 && (p.z - 13.0).abs() < 1e-3);
}

#[test]
fn host_turret_log_drives_set_turret_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_turret_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TurretCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("TurU") {
        let mut t = ThingTemplate::new("TurU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("TurU".into(), t);
    }
    let oid = logic
        .create_object("TurU", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    host_turret_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.turret_angle_deg = 33.0;
        o.turret_pitch_deg = 12.0;
        o.turret_holding = true;
        o.turret_idle_scanning = false;
        o.record_host_turret();
    }
    let events = host_turret_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && (e.angle_deg - 33.0).abs() < 1e-3
                && (e.pitch_deg - 12.0).abs() < 1e-3
                && e.holding
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_turret();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.turret_angle_deg = 0.0;
        e.turret_pitch_deg = 0.0;
        e.turret_holding = false;
    }
    let n = shadow.apply_host_turret_events(&host_turret_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.turret_angle_deg - 33.0).abs() < 1e-3);
    assert!((e.turret_pitch_deg - 12.0).abs() < 1e-3);
    assert!(e.turret_holding);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.turret_angle_deg = 0.0;
        o.turret_pitch_deg = 0.0;
        o.turret_holding = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.turret_angle_deg = 33.0;
        e.turret_pitch_deg = 12.0;
        e.turret_holding = true;
    }
    assert!(shadow.writeback_turret_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_turret_ready_log::drain();
    let _ = shadow.writeback_stealth_delay_to_host(&mut logic);
    let _ = crate::game_logic::host_stealth_delay_ready_log::drain();
    let _ = shadow.writeback_combat_attack_to_host(&mut logic);
    let _ = crate::game_logic::host_combat_attack_ready_log::drain();
    let _ = shadow.writeback_fire_intent_to_host(&mut logic);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let _ = shadow.writeback_locomotor_to_host(&mut logic);
    let _ = crate::game_logic::host_locomotor_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!((o.turret_angle_deg - 33.0).abs() < 1e-3);
    assert!((o.turret_pitch_deg - 12.0).abs() < 1e-3);
    assert!(o.turret_holding);
}

#[test]
fn host_entity_power_log_drives_set_entity_power_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_entity_power_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EPowerCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("EPU") {
        let mut t = ThingTemplate::new("EPU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("EPU".into(), t);
    }
    let oid = logic
        .create_object("EPU", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    host_entity_power_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_entity_power(50, 5);
    }
    let events = host_entity_power_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.power_provided == 50 && e.power_consumed == 5),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_entity_power();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.power_provided = 0;
        e.power_consumed = 0;
    }
    let n = shadow.apply_host_entity_power_events(&host_entity_power_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.power_provided, 50);
    assert_eq!(e.power_consumed, 5);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.power_provided = 1;
        o.power_consumed = 1;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.power_provided = 50;
        e.power_consumed = 5;
    }
    assert!(shadow.writeback_entity_power_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_entity_power_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.power_provided, 50);
    assert_eq!(o.power_consumed, 5);
}

#[test]
fn host_weapon_slot_log_drives_set_active_weapon_slot_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_weapon_slot_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WSlotCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WSU") {
        let mut t = ThingTemplate::new("WSU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("WSU".into(), t);
    }
    let oid = logic
        .create_object("WSU", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    host_weapon_slot_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_active_weapon_slot(1);
    }
    let events = host_weapon_slot_log::drain();
    assert!(
        events.iter().any(|e| e.object == oid && e.slot == 1),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_weapon_slot();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.active_weapon_slot = 0;
    }
    let n = shadow.apply_host_weapon_slot_events(&host_weapon_slot_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.active_weapon_slot, 1);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.active_weapon_slot = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.active_weapon_slot = 1;
    }
    assert!(shadow.writeback_weapon_slot_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_weapon_slot_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.active_weapon_slot, 1);
}

#[test]
fn host_weapon_bonus_log_drives_set_weapon_bonus_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_weapon_bonus_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WBonusCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WBU") {
        let mut t = ThingTemplate::new("WBU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("WBU".into(), t);
    }
    let oid = logic
        .create_object("WBU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    host_weapon_bonus_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.apply_weapon_bonus_frenzy(2, 999);
        o.weapon_bonus_horde = true;
        o.weapon_bonus_nationalism = true;
        o.battle_plan_sight_scalar_applied = 1.25;
        o.record_host_weapon_bonus();
    }
    let events = host_weapon_bonus_log::drain();
    assert!(
        events.iter().any(|e| {
            e.object == oid
                && e.frenzy
                && e.frenzy_level == 2
                && e.horde
                && e.nationalism
                && e.frenzy_until_frame == 999
                && (e.battle_plan_sight_scalar_applied - 1.25).abs() < 1e-5
        }),
        "events {:?}",
        events
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.record_host_weapon_bonus();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_bonus_frenzy = false;
        e.weapon_bonus_frenzy_level = 0;
        e.weapon_bonus_horde = false;
        e.weapon_bonus_nationalism = false;
        e.weapon_bonus_frenzy_until_frame = 0;
        e.battle_plan_sight_scalar_applied = 1.0;
    }
    let n = shadow.apply_host_weapon_bonus_events(&host_weapon_bonus_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.weapon_bonus_frenzy && e.weapon_bonus_frenzy_level == 2);
    assert!(e.weapon_bonus_horde && e.weapon_bonus_nationalism);
    assert_eq!(e.weapon_bonus_frenzy_until_frame, 999);
    assert!((e.battle_plan_sight_scalar_applied - 1.25).abs() < 1e-5);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.clear_weapon_bonus_frenzy();
        o.weapon_bonus_horde = false;
        o.weapon_bonus_nationalism = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_bonus_frenzy = true;
        e.weapon_bonus_frenzy_level = 2;
        e.weapon_bonus_horde = true;
        e.weapon_bonus_nationalism = true;
        e.weapon_bonus_frenzy_until_frame = 777;
        e.battle_plan_sight_scalar_applied = 1.5;
    }
    assert!(shadow.writeback_weapon_bonus_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_weapon_bonus_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.weapon_bonus_frenzy && o.weapon_bonus_frenzy_level == 2);
    assert!(o.weapon_bonus_horde && o.weapon_bonus_nationalism);
    assert_eq!(o.weapon_bonus_frenzy_until_frame, 777);
    assert!((o.battle_plan_sight_scalar_applied - 1.5).abs() < 1e-5);
}

#[test]
fn host_faerie_fire_log_drives_set_faerie_fire_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_faerie_fire_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FfCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerFf") {
        let mut t = ThingTemplate::new("RangerFf");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerFf".into(), t);
    }
    let oid = logic
        .create_object("RangerFf", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    host_faerie_fire_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.apply_faerie_fire(1234);
    }
    let events = host_faerie_fire_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.active && e.until_frame == 1234),
        "events {:?}",
        events
    );

    host_faerie_fire_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.apply_faerie_fire(1234);
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.faerie_fire = false;
        e.faerie_fire_until_frame = 0;
    }
    let n = shadow.apply_host_faerie_fire_events(&host_faerie_fire_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.faerie_fire);
    assert_eq!(e.faerie_fire_until_frame, 1234);

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.clear_faerie_fire();
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.faerie_fire = true;
        e.faerie_fire_until_frame = 99;
    }
    assert!(shadow.writeback_faerie_fire_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_faerie_fire_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.is_faerie_fire());
    assert_eq!(o.faerie_fire_until_frame, 99);
}

#[test]
fn host_repulsor_log_drives_set_repulsor_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_repulsor_log};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RpCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerRp") {
        let mut t = ThingTemplate::new("RangerRp");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerRp".into(), t);
    }
    let oid = logic
        .create_object("RangerRp", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    host_repulsor_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.arm_repulsor_countdown(60);
    }
    let events = host_repulsor_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.object == oid && e.active && e.until_frame == 60),
        "events {:?}",
        events
    );

    host_repulsor_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.arm_repulsor_countdown(60);
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.repulsor = false;
        e.repulsor_until_frame = 0;
    }
    let n = shadow.apply_host_repulsor_events(&host_repulsor_log::drain());
    assert!(n >= 1);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.repulsor);
    assert_eq!(e.repulsor_until_frame, 60);

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.repulsor_until_frame = 0;
        o.status.repulsor = false;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.repulsor = true;
        e.repulsor_until_frame = 12;
    }
    assert!(shadow.writeback_repulsor_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_repulsor_ready_log::drain();
    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.status.repulsor);
    assert_eq!(o.repulsor_until_frame, 12);
}
