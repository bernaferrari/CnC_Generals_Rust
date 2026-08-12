//! Phase 3 host produce / AI / path tests — drive shipped GameLogic functions.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn barracks_command_set_allows_ranger_rejects_crusader() {
    use crate::game_logic::host_production_buildable_command_residual::{
        command_set_allows_unit, CANMAKE_NO_PREREQ, CANMAKE_OK,
    };
    assert_eq!(
        command_set_allows_unit("AmericaBarracks", "AmericaInfantryRanger"),
        Some(true)
    );
    assert_eq!(
        command_set_allows_unit("AmericaBarracks", "AmericaTankCrusader"),
        Some(false)
    );
    assert_eq!(
        command_set_allows_unit("TestBarracks", "TestInfantry"),
        None
    );
    assert_eq!(
        command_set_allows_unit("Nuke_AmericaBarracks", "AmericaInfantryRanger"),
        None,
        "fallback must not authorize a general variant through a suffix match"
    );
    assert_eq!(
        command_set_allows_unit("AmericaBarracks", "americainfantryranger"),
        Some(false),
        "fallback target identities stay exact as well"
    );

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0)
        .set_cost(600, -1);
    logic.templates.insert("AmericaBarracks".into(), barracks);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(150, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut tank = ThingTemplate::new("AmericaTankCrusader");
    tank.add_kind_of(KindOf::Vehicle)
        .set_health(400.0)
        .set_cost(900, 0);
    logic.templates.insert("AmericaTankCrusader".into(), tank);

    let bid = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert_eq!(
        logic.can_make_unit(bid, "AmericaInfantryRanger"),
        CANMAKE_OK
    );
    assert_eq!(
        logic.can_make_unit(bid, "AmericaTankCrusader"),
        CANMAKE_NO_PREREQ
    );
    assert!(logic.enqueue_production(bid, "AmericaInfantryRanger".into()));
    assert!(!logic.enqueue_production(bid, "AmericaTankCrusader".into()));
}

#[test]
fn china_and_gla_barracks_command_sets_allow_retail_infantry() {
    use crate::game_logic::host_production_buildable_command_residual::{
        command_set_allows_unit, CANMAKE_NO_PREREQ, CANMAKE_OK,
    };
    assert_eq!(
        command_set_allows_unit("ChinaBarracks", "ChinaInfantryRedguard"),
        Some(true)
    );
    assert_eq!(
        command_set_allows_unit("ChinaBarracks", "AmericaInfantryRanger"),
        Some(false)
    );
    assert_eq!(
        command_set_allows_unit("GLABarracks", "GLAInfantryRebel"),
        Some(true)
    );
    assert_eq!(
        command_set_allows_unit("GLABarracks", "AmericaInfantryRanger"),
        Some(false)
    );
    assert_eq!(
        command_set_allows_unit("ChinaWarFactory", "ChinaTankBattleMaster"),
        Some(true)
    );
    assert_eq!(
        command_set_allows_unit("GLAArmsDealer", "GLAVehicleTechnical"),
        Some(true)
    );

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_player_for_team(&mut logic, Team::GLA);

    let mut china_brx = ThingTemplate::new("ChinaBarracks");
    china_brx
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0)
        .set_cost(500, -1);
    logic.templates.insert("ChinaBarracks".into(), china_brx);
    let mut redguard = ThingTemplate::new("ChinaInfantryRedguard");
    redguard
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(80, 0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".into(), redguard);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(150, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let mut gla_brx = ThingTemplate::new("GLABarracks");
    gla_brx
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(900.0)
        .set_cost(200, -1);
    logic.templates.insert("GLABarracks".into(), gla_brx);
    let mut rebel = ThingTemplate::new("GLAInfantryRebel");
    rebel
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(80, 0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel);

    let china_id = logic
        .create_object("ChinaBarracks", Team::China, glam::Vec3::ZERO)
        .expect("china barracks");
    if let Some(o) = logic.host_object_mut(china_id) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert_eq!(
        logic.can_make_unit(china_id, "ChinaInfantryRedguard"),
        CANMAKE_OK
    );
    assert_eq!(
        logic.can_make_unit(china_id, "AmericaInfantryRanger"),
        CANMAKE_NO_PREREQ
    );
    assert!(logic.enqueue_production(china_id, "ChinaInfantryRedguard".into()));
    assert!(!logic.enqueue_production(china_id, "AmericaInfantryRanger".into()));

    let gla_id = logic
        .create_object("GLABarracks", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("gla barracks");
    if let Some(o) = logic.host_object_mut(gla_id) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert_eq!(logic.can_make_unit(gla_id, "GLAInfantryRebel"), CANMAKE_OK);
    assert_eq!(
        logic.can_make_unit(gla_id, "AmericaInfantryRanger"),
        CANMAKE_NO_PREREQ
    );
    assert!(logic.enqueue_production(gla_id, "GLAInfantryRebel".into()));
    assert!(!logic.enqueue_production(gla_id, "AmericaInfantryRanger".into()));
}

#[test]
fn construction_percent_cpp_scale_and_exclusive_dozer() {
    use crate::game_logic::host_production_buildable_command_residual::{
        cpp_construction_percent_to_host_fraction, host_fraction_to_cpp_construction_percent,
        CONSTRUCTION_COMPLETE_PERCENT, CONSTRUCTION_SELL_PERCENT,
    };
    assert_eq!(
        host_fraction_to_cpp_construction_percent(1.0, false, false),
        CONSTRUCTION_COMPLETE_PERCENT
    );
    assert_eq!(
        host_fraction_to_cpp_construction_percent(0.5, true, false),
        50
    );
    assert_eq!(
        host_fraction_to_cpp_construction_percent(0.4, true, true),
        CONSTRUCTION_SELL_PERCENT
    );
    assert!((cpp_construction_percent_to_host_fraction(-1) - 1.0).abs() < 1e-6);
    assert!((cpp_construction_percent_to_host_fraction(25) - 0.25).abs() < 1e-6);

    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let sid = logic
        .create_object("TestBuilding", Team::USA, glam::Vec3::ZERO)
        .expect("bldg");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(true);
        o.construction_percent = 0.1;
        o.thing.template.build_time = 10.0;
    }
    let before = logic
        .host_object(sid)
        .map(|o| o.construction_percent)
        .unwrap_or(0.0);
    let _ = logic.update_with_dt(1.0);
    let after_idle = logic
        .host_object(sid)
        .map(|o| o.construction_percent)
        .unwrap_or(0.0);
    assert!(
        (after_idle - before).abs() < 1e-5,
        "zero assigned dozers must not ghost-progress (before={before} after={after_idle})"
    );

    let did = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("dozer");
    if let Some(d) = logic.host_object_mut(did) {
        d.set_target(Some(sid));
        d.set_ai_state(AIState::Constructing);
    }
    let _ = logic.update_with_dt(1.0);
    let after_dock = logic
        .host_object(sid)
        .map(|o| o.construction_percent)
        .unwrap_or(0.0);
    assert!(
        after_dock > after_idle,
        "exclusive docked dozer must advance construction ({after_idle} -> {after_dock})"
    );
    let cpp = logic
        .host_authoritative_construction_cpp(sid)
        .expect("cpp pct");
    assert!(cpp.1, "still under construction");
    assert!(
        cpp.0 >= 0 && cpp.0 < 100,
        "cpp percent 0-100, got {}",
        cpp.0
    );
}

#[test]
fn door_gated_spawn_waits_for_waiting_open() {
    use crate::game_logic::host_production_buildable_command_residual::{
        producer_num_door_animations, production_door_allows_spawn,
    };
    assert_eq!(producer_num_door_animations("AmericaBarracks"), 1);
    assert_eq!(producer_num_door_animations("AmericaAirfield"), 4);
    assert_eq!(producer_num_door_animations("TestBarracks"), 0);
    assert!(!production_door_allows_spawn(1, 0));
    assert!(!production_door_allows_spawn(1, 1));
    assert!(production_door_allows_spawn(1, 2));
    assert!(production_door_allows_spawn(0, 0));

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0)
        .set_cost(600, -1);
    logic.templates.insert("AmericaBarracks".into(), barracks);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(0, 0);
    ranger.build_time = 0.01;
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let bid = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("brx");
    if let Some(o) = logic.host_object_mut(bid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(logic.enqueue_production(bid, "AmericaInfantryRanger".into()));
    // Fast-forward queue head to complete without opening the door.
    if let Some(o) = logic.host_object_mut(bid) {
        if let Some(b) = o.building_data.as_mut() {
            if let Some(item) = b.production_queue.first_mut() {
                item.progress = item.total_time;
            }
        }
        o.production_door_phase = 0;
    }
    let before_units = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "AmericaInfantryRanger")
        .count();
    let _ = logic.update_with_dt(1.0 / 30.0);
    let after_units = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "AmericaInfantryRanger")
        .count();
    assert_eq!(
        after_units, before_units,
        "spawn must wait for WAITING_OPEN when NumDoorAnimations > 0"
    );
    let phase = logic
        .host_object(bid)
        .map(|o| o.production_door_phase)
        .unwrap_or(0);
    assert!(
        phase == 1 || phase == 2,
        "door cycle must start when head is ready, phase={phase}"
    );
}

#[test]
fn completed_production_preserves_factory_identity_exit_facing_and_rally() {
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let producer_id = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("producer");
    let exit_facing = 1.25;
    let rally = glam::Vec3::new(48.0, 0.0, -36.0);
    {
        let producer = logic.host_object_mut(producer_id).expect("producer object");
        producer.set_status_under_construction(false);
        producer.construction_percent = 1.0;
        producer.set_orientation(exit_facing);
    }

    assert!(logic.enqueue_production(producer_id, "TestInfantry".into()));
    {
        let producer = logic.host_object_mut(producer_id).expect("queued producer");
        let building = producer.building_data.as_mut().expect("building data");
        building.rally_point = Some(rally);
        let head = building.production_queue.first_mut().expect("queued unit");
        head.progress = head.total_time;
    }

    // This drives the accepted queue through completion, spawn, and the exit
    // rally handoff; production runs after movement, so its initial facing is
    // directly observable in this logic frame.
    let _ = logic.update_with_dt(1.0 / 30.0);

    let unit = logic
        .host_objects()
        .values()
        .find(|object| object.template_name == "TestInfantry")
        .expect("completed unit");
    assert_eq!(unit.producer_id, Some(producer_id));
    assert!(
        (unit.get_orientation() - exit_facing).abs() < 1e-6,
        "unit must inherit the producer exit facing"
    );
    assert!(unit.is_selectable(), "completed unit must be selectable");
    assert_eq!(unit.ai_state, AIState::Moving);
    assert_eq!(unit.movement.target_position, Some(rally));
    assert_eq!(unit.movement.path.last().copied(), Some(rally));
}

#[test]
fn assign_unit_path_fail_closed_and_retail_ai_names() {
    let skirmish = include_str!("../world_skirmish_tests.rs");
    assert!(
        !skirmish.contains("USA_CommandCenter"),
        "skirmish start must use retail AmericaCommandCenter"
    );
    assert!(skirmish.contains("AmericaCommandCenter"));
    assert!(skirmish.contains("ChinaCommandCenter"));

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let id = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("inf");
    // Same-cell / already-there is not a wall march.
    let _ = logic.assign_unit_path(id, glam::Vec3::ZERO, &[]);
    let src = include_str!("../world_save.rs");
    let body = src
        .split("pub fn assign_unit_path(")
        .nth(1)
        .expect("assign_unit_path");
    assert!(
        body.contains("find_path_ex") && body.contains("return false"),
        "assign_unit_path must fail-close on A* miss"
    );
    assert!(
        !body.contains("full_path.push(destination)") || body.contains("refuse fail-open"),
        "blocked hops must not invent a straight walk-through"
    );
}
