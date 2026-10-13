//! Movement/weapon/stealth/turret/power host-log channels.

use super::*;

#[test]
fn canonical_movement_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.movement.velocity = Vec3::new(3.0, 0.0, 4.0);
        o.movement.max_speed = 12.5;
        o.movement.path = vec![Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 0.0, 2.0)];
        o.movement.current_path_index = 1;
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

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.velocity[0] - 3.0).abs() < 1e-5);
    assert!((e.move_max_speed - 12.5).abs() < 1e-5);
    assert_eq!(e.path_index, 1);
    assert_eq!(e.path_len, 2);
    assert_eq!(e.path_waypoints.len(), 2);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [3.0, 0.0, 4.0];
        e.move_max_speed = 12.5;
        e.path_index = 1;
        e.path_len = 2;
        e.path_waypoints = vec![[1.0, 0.0, 1.0], [2.0, 0.0, 2.0]];
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!((o.movement.velocity.x - 3.0).abs() < 1e-5);
    assert!((o.movement.max_speed - 12.5).abs() < 1e-5);
    assert_eq!(o.movement.current_path_index, 1);
    assert_eq!(o.movement.path.len(), 2);
}

#[test]
fn canonical_weapon_stats_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
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

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.has_weapon && e.has_secondary_weapon);
    assert!((e.weapon_damage - 33.0).abs() < 1e-5);
    assert!((e.weapon_range - 140.0).abs() < 1e-5);
    assert_eq!(e.weapon_ammo, 12);
    assert!((e.secondary_weapon_damage - 9.0).abs() < 1e-5);

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
    shadow.sync_from_host(&logic);

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
fn canonical_vision_camo_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.vision_spied_mask = 0b101;
        o.camo_friendly_opacity = 0.35;
        o.camo_stealth_look = 2;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.vision_spied_mask = 0;
        e.camo_friendly_opacity = 1.0;
        e.camo_stealth_look = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.vision_spied_mask, 0b101);
    assert!((e.camo_friendly_opacity - 0.35).abs() < 1e-5);
    assert_eq!(e.camo_stealth_look, 2);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.vision_spied_mask = 0b101;
        e.camo_friendly_opacity = 0.35;
        e.camo_stealth_look = 2;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.vision_spied_mask, 0b101);
    assert!((o.camo_friendly_opacity - 0.35).abs() < 1e-5);
    assert_eq!(o.camo_stealth_look, 2);
}

#[test]
fn canonical_disguise_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.disguise_as_template = Some("AmericaVehicleHumvee".into());
        o.disguise_as_team = Some(Team::USA);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disguise_as_template.clear();
        e.disguise_as_team_ordinal = 255;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.disguise_as_template, "AmericaVehicleHumvee");
    assert_eq!(e.disguise_as_team_ordinal, 0);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disguise_as_template = "AmericaVehicleHumvee".into();
        e.disguise_as_team_ordinal = 0;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(
        o.disguise_as_template.as_deref(),
        Some("AmericaVehicleHumvee")
    );
    assert_eq!(o.disguise_as_team, Some(Team::USA));
}

#[test]
fn canonical_overlord_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.has_overlord_gattling_addon = true;
        o.has_overlord_propaganda_addon = false;
        o.overlord_bunker_capacity = Some(4);
        o.is_helix_transport = true;
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

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.has_overlord_gattling_addon && !e.has_overlord_propaganda_addon);
    assert_eq!(e.overlord_bunker_capacity, 4);
    assert!(e.is_helix_transport);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.has_overlord_gattling_addon = true;
        e.has_overlord_propaganda_addon = false;
        e.overlord_bunker_capacity = 4;
        e.is_helix_transport = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.has_overlord_gattling_addon && !o.has_overlord_propaganda_addon);
    assert_eq!(o.overlord_bunker_capacity, Some(4));
    assert!(o.is_helix_transport);
}

#[test]
fn canonical_stealth_flags_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.innate_stealth = true;
        o.stealth_breaks_on_attack = true;
        o.stealth_breaks_on_move = false;
        o.is_tunnel_network = true;
        o.passengers_allowed_to_fire = true;
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

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.innate_stealth && e.stealth_breaks_on_attack && !e.stealth_breaks_on_move);
    assert!(e.is_tunnel_network && e.passengers_allowed_to_fire);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.innate_stealth = true;
        e.stealth_breaks_on_attack = true;
        e.stealth_breaks_on_move = false;
        e.is_tunnel_network = true;
        e.passengers_allowed_to_fire = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.innate_stealth && o.stealth_breaks_on_attack && !o.stealth_breaks_on_move);
    assert!(o.is_tunnel_network && o.passengers_allowed_to_fire);
}

#[test]
fn canonical_hive_state_is_projected_without_observer_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.hive_slave_count = 3;
        o.hive_slave_hp = 55.0;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.hive_slave_count = 0;
        e.hive_slave_hp = 0.0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.hive_slave_count, 3);
    assert!((e.hive_slave_hp - 55.0).abs() < 1e-3);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.hive_slave_count = 3;
        e.hive_slave_hp = 55.0;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.hive_slave_count, 3);
    assert!((o.hive_slave_hp - 55.0).abs() < 1e-3);
}

#[test]
fn canonical_contain_capacity_projects_without_writeback() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.max_transport = 5;
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.max_garrison = 8;
        o.building_data = Some(bd);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.max_transport = 0;
        e.max_garrison = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.max_transport, 5);
    assert_eq!(e.max_garrison, 8);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.max_transport = 5;
        e.max_garrison = 8;
    }
    shadow.sync_from_host(&logic);
    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.max_transport, 5);
    assert_eq!(o.building_data.as_ref().map(|bd| bd.max_garrison), Some(8));
}

#[test]
fn canonical_overcharge_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_overcharge_enabled(true);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overcharge_enabled = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.overcharge_enabled);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overcharge_enabled = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.overcharge_enabled);
}

#[test]
fn canonical_weapon_set_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.weapon_set_player_upgrade = true;
        o.armed_riders_upgrade_weapon_set = true;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_set_player_upgrade = false;
        e.armed_riders_upgrade_weapon_set = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.weapon_set_player_upgrade && e.armed_riders_upgrade_weapon_set);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_set_player_upgrade = true;
        e.armed_riders_upgrade_weapon_set = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.weapon_set_player_upgrade && o.armed_riders_upgrade_weapon_set);
}

#[test]
fn host_ai_attitude_is_canonical_and_shadow_only_observes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    fn world() -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("AttitudeOwner");
        template.set_health(100.0);
        template.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AttitudeOwner".into(), template);
        let id = logic
            .create_object("AttitudeOwner", Team::USA, glam::Vec3::new(15.0, 0.0, 15.0))
            .unwrap();
        (logic, id)
    }
    let (mut first, id) = world();
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(2);
    let (mut second, peer_id) = world();
    assert_eq!(id, peer_id);
    assert_eq!(
        first.host_objects()[&id].ai_attitude,
        2,
        "construction must not publish another owner"
    );
    second
        .host_object_mut(peer_id)
        .unwrap()
        .set_ai_attitude_i8(-2);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&first);
    let eid = shadow.host_to_entity[&id.0];
    assert_eq!(shadow.world().entity(eid).unwrap().ai_attitude, 2);
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .unwrap()
        .ai_attitude = -1;
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(1);
    assert_eq!(first.host_objects()[&id].ai_attitude, 1);
    assert_eq!(second.host_objects()[&peer_id].ai_attitude, -2);
    shadow.sync_from_host(&first);
    assert_eq!(shadow.world().entity(eid).unwrap().ai_attitude, 1);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = first.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.host_objects()[&id].ai_attitude, 1);
    restored.host_object_mut(id).unwrap().set_ai_attitude_i8(-2);
    assert_eq!(first.host_objects()[&id].ai_attitude, 1);
    second.reset();
    drop(second);
    assert_eq!(first.host_objects()[&id].ai_attitude, 1);
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(127);
    assert_eq!(
        first.host_objects()[&id].ai_attitude,
        2,
        "preserve original attitude bounds"
    );
    first
        .host_object_mut(id)
        .unwrap()
        .set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Passive);
    assert_eq!(first.host_objects()[&id].ai_attitude, -1);
}

#[test]
fn host_guard_is_canonical_and_shadow_only_observes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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
    let position = glam::Vec3::new(3.0, 0.0, 5.0);
    logic
        .host_object_mut(oid)
        .unwrap()
        .set_guard_position(Some(position));
    logic
        .host_object_mut(oid)
        .unwrap()
        .set_guard_target(Some(tid));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    assert_eq!(
        shadow.world().entity(entity).unwrap().guard_position,
        Some([3.0, 0.0, 5.0])
    );
    assert_eq!(
        shadow.world().entity(entity).unwrap().guard_target_host,
        tid.0
    );
    let copy = shadow.world_mut().world_mut().entity_mut(entity).unwrap();
    copy.guard_position = None;
    copy.guard_target_host = 0;
    assert_eq!(
        logic.host_object(oid).unwrap().guard_position,
        Some(position)
    );
    assert_eq!(logic.host_object(oid).unwrap().guard_target, Some(tid));
    logic.host_object_mut(oid).unwrap().set_guard_position(None);
    logic.host_object_mut(oid).unwrap().set_guard_target(None);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(entity).unwrap().guard_position, None);
    assert_eq!(shadow.world().entity(entity).unwrap().guard_target_host, 0);
    assert_eq!(logic.host_object(oid).unwrap().guard_target, None);
}

#[test]
fn canonical_continuous_fire_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.continuous_fire_level = 2;
        o.consecutive_shots_at_target = 9;
        o.continuous_fire_coast_until_frame = 44;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.continuous_fire_level = 0;
        e.continuous_fire_consecutive = 0;
        e.continuous_fire_coast_until_frame = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.continuous_fire_level, 2);
    assert_eq!(e.continuous_fire_consecutive, 9);
    assert_eq!(e.continuous_fire_coast_until_frame, 44);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.continuous_fire_level = 2;
        e.continuous_fire_consecutive = 9;
        e.continuous_fire_coast_until_frame = 44;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.continuous_fire_level, 2);
    assert_eq!(o.consecutive_shots_at_target, 9);
    assert_eq!(o.continuous_fire_coast_until_frame, 44);
}

#[test]
fn canonical_detector_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_detector_state(true, 175.0, 12);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_detector = false;
        e.detection_range = 0.0;
        e.detection_rate_frames = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.is_detector);
    assert!((e.detection_range - 175.0).abs() < 1e-3);
    assert_eq!(e.detection_rate_frames, 12);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_detector = true;
        e.detection_range = 175.0;
        e.detection_rate_frames = 12;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.is_detector);
    assert!((o.detection_range - 175.0).abs() < 1e-3);
    assert_eq!(o.detection_rate_frames, 12);
}

#[test]
fn host_target_location_log_drives_set_target_location_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_target_location(Some(glam::Vec3::new(11.0, 0.0, 13.0)));
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.target_location = None;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    let tl = e.target_location.expect("tl");
    assert!((tl[0] - 11.0).abs() < 1e-3 && (tl[2] - 13.0).abs() < 1e-3);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.target_location = Some([11.0, 0.0, 13.0]);
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    let p = o.target_location.expect("host tl");
    assert!((p.x - 11.0).abs() < 1e-3 && (p.z - 13.0).abs() < 1e-3);
}

#[test]
fn canonical_turret_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.turret_angle_deg = 33.0;
        o.turret_pitch_deg = 12.0;
        o.turret_holding = true;
        o.turret_idle_scanning = false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.turret_angle_deg = 0.0;
        e.turret_pitch_deg = 0.0;
        e.turret_holding = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.turret_angle_deg - 33.0).abs() < 1e-3);
    assert!((e.turret_pitch_deg - 12.0).abs() < 1e-3);
    assert!(e.turret_holding);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.turret_angle_deg = 33.0;
        e.turret_pitch_deg = 12.0;
        e.turret_holding = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!((o.turret_angle_deg - 33.0).abs() < 1e-3);
    assert!((o.turret_pitch_deg - 12.0).abs() < 1e-3);
    assert!(o.turret_holding);
}

#[test]
fn canonical_entity_power_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_entity_power(50, 5);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.power_provided = 0;
        e.power_consumed = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.power_provided, 50);
    assert_eq!(e.power_consumed, 5);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.power_provided = 50;
        e.power_consumed = 5;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.power_provided, 50);
    assert_eq!(o.power_consumed, 5);
}

#[test]
fn canonical_weapon_slot_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_active_weapon_slot(1);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.active_weapon_slot = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.active_weapon_slot, 1);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.active_weapon_slot = 1;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert_eq!(o.active_weapon_slot, 1);
}

#[test]
fn canonical_weapon_bonus_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.apply_weapon_bonus_frenzy(2, 999);
        o.weapon_bonus_horde = true;
        o.weapon_bonus_nationalism = true;
        o.battle_plan_sight_scalar_applied = 1.25;
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

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.weapon_bonus_frenzy && e.weapon_bonus_frenzy_level == 2);
    assert!(e.weapon_bonus_horde && e.weapon_bonus_nationalism);
    assert_eq!(e.weapon_bonus_frenzy_until_frame, 999);
    assert!((e.battle_plan_sight_scalar_applied - 1.25).abs() < 1e-5);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.weapon_bonus_frenzy = true;
        e.weapon_bonus_frenzy_level = 2;
        e.weapon_bonus_horde = true;
        e.weapon_bonus_nationalism = true;
        e.weapon_bonus_frenzy_until_frame = 777;
        e.battle_plan_sight_scalar_applied = 1.5;
    }
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .weapon_bonus_frenzy_until_frame,
        999,
        "presentation mutation cannot write authoritative bonuses"
    );
    {
        let o = logic.host_object_mut(oid).unwrap();
        o.apply_weapon_bonus_frenzy(2, 777);
        o.battle_plan_sight_scalar_applied = 1.5;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.weapon_bonus_frenzy && o.weapon_bonus_frenzy_level == 2);
    assert!(o.weapon_bonus_horde && o.weapon_bonus_nationalism);
    assert_eq!(o.weapon_bonus_frenzy_until_frame, 777);
    assert!((o.battle_plan_sight_scalar_applied - 1.5).abs() < 1e-5);
}

#[test]
fn canonical_faerie_fire_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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
    logic.host_object_mut(oid).unwrap().apply_faerie_fire(1234);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.faerie_fire = false;
        e.faerie_fire_until_frame = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.faerie_fire);
    assert_eq!(e.faerie_fire_until_frame, 1234);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.faerie_fire = true;
        e.faerie_fire_until_frame = 99;
    }
    assert_eq!(
        logic.host_object(oid).unwrap().faerie_fire_until_frame,
        1234
    );
    logic.host_object_mut(oid).unwrap().apply_faerie_fire(99);
    assert_eq!(
        logic.host_object(oid).unwrap().faerie_fire_until_frame,
        1234,
        "a shorter application preserves the current deadline"
    );
    logic.host_object_mut(oid).unwrap().clear_faerie_fire();
    logic.host_object_mut(oid).unwrap().apply_faerie_fire(99);
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.is_faerie_fire());
    assert_eq!(o.faerie_fire_until_frame, 99);
}

#[test]
fn canonical_repulsor_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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
    {
        let object = logic.host_object_mut(oid).unwrap();
        object.set_status_repulsor(true);
        object.arm_repulsor_countdown(60);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.repulsor = false;
        e.repulsor_until_frame = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.repulsor);
    assert_eq!(e.repulsor_until_frame, 60);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.repulsor = true;
        e.repulsor_until_frame = 12;
    }
    assert_eq!(logic.host_object(oid).unwrap().repulsor_until_frame, 60);
    logic
        .host_object_mut(oid)
        .unwrap()
        .arm_repulsor_countdown(12);
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).expect("o");
    assert!(o.status.repulsor);
    assert_eq!(o.repulsor_until_frame, 12);
}
