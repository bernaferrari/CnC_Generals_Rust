//! Sciences, upgrades, command-set, selection-radius, and identity/visual logs.

use super::*;

#[test]
fn sync_from_host_copies_entity_xp_status_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityXpStatus");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object XpUnit\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "XpUnit", 100.0);
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
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.experience_points - 420.0).abs() < 0.01);
    assert_eq!(e.veterancy_ordinal, 2);
    assert_eq!(e.stored_supplies, 1500);
    assert!(e.stealthed && e.detected && e.using_ability);
    assert!(e.airborne_target && e.disabled_underpowered && e.disabled_hacked);
    assert!(!e.disabled_unmanned);
}

#[test]
fn sync_from_host_copies_entity_color_power_type() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityColorPower");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object PwrBldg\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "PwrBldg", 200.0);
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
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.object_type_ordinal, 3);
    assert!((e.team_color[0] - 0.1).abs() < 0.001);
    assert!((e.team_color[3] - 0.9).abs() < 0.001);
    assert_eq!(e.power_provided, 50);
    assert_eq!(e.power_consumed, 5);
}

#[test]
fn sync_from_host_copies_entity_team_status_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityTeamStatus");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object TeamStatU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "TeamStatU", 100.0);
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
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.team_ordinal, 1);
    assert!((e.selection_radius - 12.5).abs() < 0.01);
    assert!(e.moving && e.attacking);
    assert!(!e.under_construction);
}

#[test]
fn sync_from_host_copies_entity_selection_residual() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntitySelectResidual");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SelResU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "SelResU", 100.0);
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
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.selected);
    assert!((e.max_health - 150.0).abs() < 0.01);
    assert!((e.construction_percent - 0.4).abs() < 0.01);
    assert!(!e.destroyed);
}

#[test]
fn canonical_command_set_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CsCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object CsU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("CsU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("CsU".into(), t);
    }
    let oid = logic
        .create_object("CsU", Team::GLA, glam::Vec3::new(22.0, 0.0, 22.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_command_set_override(Some("Command_DemoSuicide".into()));
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.command_set_override.clear();
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.command_set_override, "Command_DemoSuicide");

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.command_set_override = "Command_DemoSuicide".into();
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(
        o.command_set_override.as_deref(),
        Some("Command_DemoSuicide")
    );
}

#[test]
fn canonical_selection_radius_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SrCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SrU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SrU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SrU".into(), t);
    }
    let oid = logic
        .create_object("SrU", Team::USA, glam::Vec3::new(27.0, 0.0, 27.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_selection_radius(14.5);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.selection_radius = 1.0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.selection_radius - 14.5).abs() < 1e-5);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.selection_radius = 14.5;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!((o.selection_radius - 14.5).abs() < 1e-5);
}

#[test]
fn canonical_ground_height_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GhCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object GhU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("GhU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("GhU".into(), t);
    }
    let oid = logic
        .create_object("GhU", Team::USA, glam::Vec3::new(34.0, 0.0, 34.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_ground_height_residual(12.5, true);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ground_height = 0.0;
        e.ground_height_from_terrain = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.ground_height - 12.5).abs() < 1e-5);
    assert!(e.ground_height_from_terrain);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.ground_height = 12.5;
        e.ground_height_from_terrain = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!((o.ground_height - 12.5).abs() < 1e-5);
    assert!(o.ground_height_from_terrain);
}

#[test]
fn canonical_effective_model_mesh_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MmCh");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("RangerMesh")
                .kind_of("INFANTRY SELECTABLE")
                .health(100.0)
                .walker()
                .field("Scale = 1.25")
                .model("avtank"),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("RangerMesh", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    // Re-apply path

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.model_key, "avtank");
    assert!((e.mesh_scale - 1.25).abs() < 1e-5);
}

#[test]
fn canonical_fow_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FowCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object RangerFow\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("RangerFow");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerFow".into(), t);
    }
    let oid = logic
        .create_object("RangerFow", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    let local = logic.local_player_id().expect("local player");
    let other = logic
        .get_players()
        .values()
        .find(|player| player.id != local && player.is_alive)
        .map(|player| (player.id, player.team))
        .expect("other live owner");
    {
        let (object, ownership_events, fixture_partition) =
            logic.host_object_ownership_partition_mut(oid).unwrap();
        object.set_team_and_owner_with_partition_owner(
            other.1,
            Some(other.0),
            ownership_events,
            fixture_partition,
        );
    };
    {
        let shroud = logic.world_services.shroud();
        let mut shroud = shroud.lock().unwrap();
        shroud.mark_host_object_explored(local, oid.0);
        shroud.mark_host_vision_ready();
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(
        e.fow_visibility_alpha,
        crate::fow_rendering::ObjectVisibility::FOGGED.visibility_alpha
    );
    assert!((e.fow_is_explored - 1.0).abs() < 1e-5);
    assert_eq!(
        e.fow_visibility_falloff,
        crate::fow_rendering::ObjectVisibility::FOGGED.visibility_falloff
    );
}

#[test]
fn canonical_kind_of_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("KoCh");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("RangerKo").kind_of("INFANTRY SELECTABLE HERO").health(100.0).walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("RangerKo", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    let bits = {
        let o = logic.host_objects().lookup(&oid).expect("o");
        o.presentation_kind_of_bits()
    };
    assert!(bits != 0, "bits {bits}");

    assert_ne!(
        bits & (1u32 << 10),
        0,
        "authored Hero template must expose its original presentation bit"
    );

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.kind_of_bits, bits | (1u32 << 10));
}

#[test]
fn canonical_identity_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("IdCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object IdU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("IdU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("IdU".into(), t);
    }
    let oid = logic
        .create_object("IdU", Team::USA, glam::Vec3::new(33.0, 0.0, 33.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.name = "ScriptRanger".into();
        o.team_color = [0.1, 0.2, 0.3, 1.0];
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.display_name.clear();
        e.team_color = [1.0, 1.0, 1.0, 1.0];
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.display_name, "ScriptRanger");
    assert!((e.team_color[0] - 0.1).abs() < 1e-5);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.display_name = "ScriptRanger".into();
        e.team_color = [0.1, 0.2, 0.3, 1.0];
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(o.name, "ScriptRanger");
    assert!((o.team_color[0] - 0.1).abs() < 1e-5);
}

#[test]
fn canonical_building_type_is_projected_from_receiving_owner() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BtCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object BtU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("BtU");
        t.set_health(400.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("BtU".into(), t);
    }
    let oid = logic
        .create_object("BtU", Team::USA, glam::Vec3::new(32.0, 0.0, 32.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.building_data = Some(BuildingData::new(BuildingType::Barracks));
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_building = false;
        e.building_type_ordinal = 255;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.is_building);
    assert_eq!(e.building_type_ordinal, 1);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_building = true;
        e.building_type_ordinal = 1;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(
        o.building_data.as_ref().map(|b| b.building_type),
        Some(BuildingType::Barracks)
    );
}

#[test]
fn canonical_crush_vision_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CvCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object CvU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("CvU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("CvU".into(), t);
    }
    let oid = logic
        .create_object("CvU", Team::USA, glam::Vec3::new(30.0, 0.0, 30.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.crusher_level = 2;
        o.crushable_level = 1;
        o.vision_range = 175.0;
        o.shroud_clearing_range = 200.0;
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.crusher_level = 0;
        e.crushable_level = 0;
        e.vision_range = 0.0;
        e.shroud_clearing_range = 0.0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.crusher_level, 2);
    assert_eq!(e.crushable_level, 1);
    assert!((e.vision_range - 175.0).abs() < 1e-5);
    assert!((e.shroud_clearing_range - 200.0).abs() < 1e-5);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.crusher_level = 2;
        e.crushable_level = 1;
        e.vision_range = 175.0;
        e.shroud_clearing_range = 200.0;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(o.crusher_level, 2);
    assert_eq!(o.crushable_level, 1);
    assert!((o.vision_range - 175.0).abs() < 1e-5);
    assert!((o.shroud_clearing_range - 200.0).abs() < 1e-5);
}

#[test]
fn canonical_mine_cheer_is_projected_from_receiving_owner() {
    use crate::game_logic::{
        KindOf, Team, ThingTemplate, host_mines::HostMineData, host_mines::HostMineKind,
    };
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DmcCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object DmcU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("DmcU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("DmcU".into(), t);
    }
    let oid = logic
        .create_object("DmcU", Team::GLA, glam::Vec3::new(29.0, 0.0, 29.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.demo_suicided_detonating = true;
        o.cheer_timer = 2.5;
        o.mine_data = Some(HostMineData::new(HostMineKind::LandMine));
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.demo_suicided_detonating = false;
        e.has_mine_data = false;
        e.cheer_timer = 0.0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.demo_suicided_detonating && e.has_mine_data);
    assert!((e.cheer_timer - 2.5).abs() < 1e-5);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.demo_suicided_detonating = true;
        e.has_mine_data = true;
        e.cheer_timer = 2.5;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!(o.demo_suicided_detonating);
    assert!((o.cheer_timer - 2.5).abs() < 1e-5);
    assert!(o.mine_data.is_some());
}

#[test]
fn canonical_model_condition_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("McCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object McU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("McU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("McU".into(), t);
    }
    let oid = logic
        .create_object("McU", Team::China, glam::Vec3::new(28.0, 0.0, 28.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.model_condition_bits = 0b1011;
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.model_condition_bits = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.model_condition_bits, 0b1011);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.model_condition_bits = 0b1011;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(o.model_condition_bits, 0b1011);
}
