//! Phase 3 host produce / AI / path tests — drive shipped GameLogic functions.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn barracks_command_set_allows_ranger_rejects_crusader() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_NO_PREREQ, CANMAKE_OK, command_set_allows_unit,
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
        CANMAKE_NO_PREREQ, CANMAKE_OK, command_set_allows_unit,
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
        CONSTRUCTION_COMPLETE_PERCENT, CONSTRUCTION_SELL_PERCENT,
        cpp_construction_percent_to_host_fraction, host_fraction_to_cpp_construction_percent,
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
fn exclusive_dozer_does_not_stack_build_rate() {
    // C++ DozerAIUpdate.cpp:305 — getBuilderID() != dozer refuses a second builder.
    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    // C++ ThingTemplate::calcTimeToBuild (ThingTemplate.cpp:1541-1558): a
    // 0/0 energy grid returns supply ratio 0 → build time ×2 (min 0.5
    // speed). The exclusivity contract asserts the 1× rate, so satisfy the
    // grid instead of eating the low-power penalty.
    logic.players.get_mut(&0).expect("player").power_produced = 10;
    let sid = logic
        .create_object_under_construction("TestBuilding", Team::USA, glam::Vec3::ZERO)
        .expect("scaffold");
    if let Some(o) = logic.host_object_mut(sid) {
        o.thing.template.build_time = 10.0;
        o.construction_percent = 0.0;
    }
    let d1 = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer1");
    let d2 = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0))
        .expect("dozer2");
    assert!(logic.resume_construction(&[d1], sid));
    if let Some(d) = logic.host_object_mut(d2) {
        d.set_target(Some(sid));
        d.set_ai_state(AIState::Constructing);
    }
    assert_eq!(
        logic.host_object(sid).and_then(|o| o.builder_id),
        Some(d1),
        "structure must keep the first exclusive builder"
    );
    assert!(!logic.can_resume_construction_of(d2, sid));
    logic.update_construction(&[sid], 1.0);
    let after = logic
        .host_object(sid)
        .map(|o| o.construction_percent)
        .unwrap_or(0.0);
    assert!(
        (after - 0.1).abs() < 0.02,
        "two targeting dozers must not stack; expected ~0.1, got {after}"
    );
    assert!(
        after < 0.15,
        "stacked builders would approach 0.2, got {after}"
    );
}

#[test]
fn under_construction_starts_at_one_hp_and_gains_linearly() {
    // C++ DozerAIUpdate.cpp:1708 start 1 HP; :526 +maxHealth/framesToBuild per frame.
    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let sid = logic
        .create_object_under_construction("TestBuilding", Team::USA, glam::Vec3::ZERO)
        .expect("scaffold");
    let max_hp = logic
        .host_object(sid)
        .map(|o| o.health.maximum)
        .unwrap_or(0.0);
    let start_hp = logic
        .host_object(sid)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (start_hp - 1.0).abs() < 1e-4,
        "under-construction HP must start at 1, got {start_hp} (max={max_hp})"
    );
    assert!(
        (start_hp - max_hp * 0.1).abs() > 1.0,
        "must not start at 10% max ({})",
        max_hp * 0.1
    );
    if let Some(o) = logic.host_object_mut(sid) {
        o.thing.template.build_time = 10.0; // 300 frames
    }
    let did = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer");
    assert!(logic.resume_construction(&[did], sid));
    logic.update_construction(&[sid], 1.0);
    let after = logic
        .host_object(sid)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let expected = 1.0 + max_hp / 300.0 * 30.0;
    assert!(
        (after - expected).abs() < 0.05,
        "linear per-frame HP gain expected ~{expected}, got {after}"
    );
}

#[test]
fn factory_door_phases_use_ini_door_times() {
    // C++ ProductionUpdate.cpp:113-115 DoorOpeningTime/DoorWaitOpenTime/DoorCloseTime.
    use crate::game_logic::host_production_buildable_command_residual::{
        producer_door_phase_duration, producer_door_phase_frames,
    };
    use crate::game_logic::host_structure_economy_residual::structure_economy_ms_to_frames;
    assert_eq!(
        producer_door_phase_frames("AmericaWarFactory"),
        (
            structure_economy_ms_to_frames(3250),
            structure_economy_ms_to_frames(3000),
            structure_economy_ms_to_frames(4000),
        )
    );
    assert_eq!(
        producer_door_phase_duration("AmericaWarFactory", 1),
        structure_economy_ms_to_frames(3250)
    );
    assert_eq!(
        producer_door_phase_duration("AmericaCommandCenter", 1),
        crate::game_logic::host_structure_economy_residual::USA_CC_DOOR_OPENING_FRAMES
    );
    assert_eq!(producer_door_phase_frames("TestBarracks"), (0, 0, 0));

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut wf = ThingTemplate::new("AmericaWarFactory");
    wf.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .set_health(1500.0)
        .set_cost(2000, -1);
    logic.templates.insert("AmericaWarFactory".into(), wf);
    let id = logic
        .create_object("AmericaWarFactory", Team::USA, glam::Vec3::ZERO)
        .expect("wf");
    if let Some(o) = logic.host_object_mut(id) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.start_production_door_cycle(0);
        assert_eq!(o.production_door_phase, 1);
        assert_eq!(
            o.production_door_phase_end_frame,
            structure_economy_ms_to_frames(3250)
        );
        assert_ne!(
            o.production_door_phase_end_frame, 15,
            "must not use hardcoded 15f opening"
        );
    }
}

#[test]
fn queue_head_allowed_to_build_recheck_cancels_script_disallowed_unit() {
    // C++ ProductionUpdate.cpp:671-682: allowedToBuild re-check; dozers stay.
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    let bid = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("brx");
    if let Some(o) = logic.host_object_mut(bid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(logic.enqueue_production(bid, "TestInfantry".into()));
    if let Some(p) = logic.get_player_mut(0) {
        p.set_can_build_units(false);
    }
    logic.cancel_script_disallowed_production_queue_heads();
    let qlen = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.len())
        .unwrap_or(99);
    assert_eq!(qlen, 0, "script-disallowed unit head must cancel");

    if let Some(p) = logic.get_player_mut(0) {
        p.set_can_build_units(true);
    }
    ensure_test_command_center_template(&mut logic);
    let cc = logic
        .create_object(
            "TestCommandCenter",
            Team::USA,
            glam::Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("cc");
    if let Some(o) = logic.host_object_mut(cc) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(logic.enqueue_production(cc, "TestDozer".into()));
    if let Some(p) = logic.get_player_mut(0) {
        p.set_can_build_units(false);
    }
    logic.cancel_script_disallowed_production_queue_heads();
    let qlen = logic
        .host_object(cc)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.len())
        .unwrap_or(0);
    assert_eq!(
        qlen, 1,
        "dozer queue head must survive allowedToBuild false"
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
fn start_production_door_cycle_preserves_opening_deadline() {
    // C++ ProductionUpdate.cpp:746-773: already-OPENING does not rewrite
    // DoorOpeningTime. A second start must not push the deadline out.
    use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut wf = ThingTemplate::new("AmericaWarFactory");
    wf.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .set_health(1500.0)
        .set_cost(2000, -1);
    logic.templates.insert("AmericaWarFactory".into(), wf);
    let id = logic
        .create_object("AmericaWarFactory", Team::USA, glam::Vec3::ZERO)
        .expect("wf");
    let open = producer_door_phase_duration("AmericaWarFactory", 1);
    assert!(open > 1, "fixture must use a multi-frame DoorOpeningTime");
    if let Some(o) = logic.host_object_mut(id) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.start_production_door_cycle(10);
        assert_eq!(o.production_door_phase, 1);
        assert_eq!(o.production_door_phase_end_frame, 10 + open);
        o.start_production_door_cycle(11);
        assert_eq!(o.production_door_phase, 1);
        assert_eq!(
            o.production_door_phase_end_frame,
            10 + open,
            "already-OPENING must preserve DoorOpeningTime"
        );
        assert!(!o.tick_production_door(10 + open));
        assert_eq!(o.production_door_phase, 2, "door must reach WAITING_OPEN");
    }
}

#[test]
fn door_animated_factory_releases_after_opening_time() {
    // C++ ProductionUpdate.cpp:746-773 + 795: spawn when WAITING_OPEN after
    // DoorOpeningTime. Host must not restart OPENING every blocked frame.
    use crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut wf = ThingTemplate::new("AmericaWarFactory");
    wf.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .set_health(1500.0)
        .set_cost(2000, -1);
    logic.templates.insert("AmericaWarFactory".into(), wf);
    let mut crusader = ThingTemplate::new("AmericaTankCrusader");
    crusader
        .add_kind_of(KindOf::Vehicle)
        .set_health(400.0)
        .set_cost(0, 0);
    crusader.build_time = 0.01;
    logic
        .templates
        .insert("AmericaTankCrusader".into(), crusader);
    let bid = logic
        .create_object("AmericaWarFactory", Team::USA, glam::Vec3::ZERO)
        .expect("wf");
    if let Some(o) = logic.host_object_mut(bid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(logic.enqueue_production(bid, "AmericaTankCrusader".into()));
    if let Some(o) = logic.host_object_mut(bid) {
        if let Some(b) = o.building_data.as_mut() {
            if let Some(item) = b.production_queue.first_mut() {
                item.progress = item.total_time;
                item.construction_frames = 10_000;
            }
        }
    }
    let open = producer_door_phase_duration("AmericaWarFactory", 1);
    assert!(open > 1, "fixture must use a multi-frame DoorOpeningTime");
    let ids = [bid];
    logic.frame = logic.frame.max(1);
    logic.update_construction(&ids, 1.0 / 30.0);
    logic.update_production(1.0 / 30.0);
    let (phase0, end0) = {
        let o = logic.host_object(bid).expect("wf");
        (o.production_door_phase, o.production_door_phase_end_frame)
    };
    assert_eq!(phase0, 1, "closed door starts OPENING once");
    for _ in 0..3 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_construction(&ids, 1.0 / 30.0);
        logic.update_production(1.0 / 30.0);
    }
    let (phase1, end1, units_mid) = {
        let units = logic
            .host_objects()
            .values()
            .filter(|o| o.template_name == "AmericaTankCrusader")
            .count();
        let o = logic.host_object(bid).expect("wf");
        (
            o.production_door_phase,
            o.production_door_phase_end_frame,
            units,
        )
    };
    assert_eq!(phase1, 1);
    assert_eq!(end1, end0, "already-OPENING must preserve DoorOpeningTime");
    assert_eq!(units_mid, 0, "must not spawn before WAITING_OPEN");
    let remaining = end0.saturating_sub(logic.frame);
    for _ in 0..=remaining {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_construction(&ids, 1.0 / 30.0);
        logic.update_production(1.0 / 30.0);
    }
    let phase2 = logic
        .host_object(bid)
        .map(|o| o.production_door_phase)
        .unwrap_or(0);
    let units = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "AmericaTankCrusader")
        .count();
    assert_eq!(phase2, 2, "door must reach WAITING_OPEN");
    assert_eq!(
        units, 1,
        "factory must release the completed unit once WAITING_OPEN"
    );
}

#[test]
fn closing_factory_door_pops_waiting_open_for_next_unit() {
    // C++ ProductionUpdate.cpp:762-776 + 795: CLOSING (`m_doorClosedFrame != 0`)
    // pops to WAITING_OPEN and the completed unit exits this frame.
    use crate::game_logic::host_enum_table_residual::{
        door_1_waiting_open_model_bit, host_model_condition_has,
    };
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut wf = ThingTemplate::new("AmericaWarFactory");
    wf.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .set_health(1500.0)
        .set_cost(2000, -1);
    logic.templates.insert("AmericaWarFactory".into(), wf);
    let mut crusader = ThingTemplate::new("AmericaTankCrusader");
    crusader
        .add_kind_of(KindOf::Vehicle)
        .set_health(400.0)
        .set_cost(0, 0);
    crusader.build_time = 0.01;
    logic
        .templates
        .insert("AmericaTankCrusader".into(), crusader);
    let bid = logic
        .create_object("AmericaWarFactory", Team::USA, glam::Vec3::ZERO)
        .expect("wf");
    let door_end_frame = logic.frame.saturating_add(90);
    if let Some(o) = logic.host_object_mut(bid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.production_door_phase = 4;
        o.production_door_phases[0] = 4;
        o.production_door_active_index = 0;
        o.production_door_phase_end_frame = door_end_frame;
        o.production_door_phase_end_frames[0] = door_end_frame;
    }
    assert!(logic.enqueue_production(bid, "AmericaTankCrusader".into()));
    if let Some(o) = logic.host_object_mut(bid) {
        if let Some(b) = o.building_data.as_mut() {
            if let Some(item) = b.production_queue.first_mut() {
                item.progress = item.total_time;
                item.construction_frames = 10_000;
            }
        }
    }
    let ids = [bid];
    logic.frame = logic.frame.max(1);
    logic.update_construction(&ids, 1.0 / 30.0);
    logic.update_production(1.0 / 30.0);
    let phase = logic
        .host_object(bid)
        .map(|o| o.production_door_phase)
        .unwrap_or(0);
    let waiting = logic.host_object(bid).is_some_and(|o| {
        host_model_condition_has(o.model_condition_bits, door_1_waiting_open_model_bit())
    });
    let units = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "AmericaTankCrusader")
        .count();
    assert_eq!(phase, 2, "CLOSING must pop to WAITING_OPEN, phase={phase}");
    assert!(waiting, "WAITING_OPEN model bit must be set");
    assert_eq!(
        units, 1,
        "next unit must exit this frame, not wait out DoorCloseTime"
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
    // C++ DefaultProductionExitUpdate.cpp:88-94 pushes the ADJUSTED custom
    // rally: Pathfinder::adjustDestination (AIPathfind.cpp:5331) spiral +
    // checkForAdjust → adjustCoordToCell (AIPathfind.cpp:8936-8948) snaps to
    // the pathfind cell. The destination is that snapped rally, never the
    // raw coordinate.
    let dest = unit
        .movement
        .target_position
        .expect("rally destination installed");
    assert!(
        (dest - rally).length() < 10.0,
        "destination must be the cell-adjusted rally {rally}, got {dest}"
    );
    assert_eq!(unit.movement.path.last().copied(), Some(dest));
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
    let src = concat!(
        include_str!("../world_save.rs"),
        include_str!("../world_save/world_subsystems.rs"),
        include_str!("../world_save/world_paths.rs"),
        include_str!("../world_save/world_runtime.rs"),
        include_str!("../world_save/world_players.rs"),
        include_str!("../world_save/world_load.rs"),
    );
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

#[test]
fn dozer_dock_plays_under_construction_loop_and_stops_on_complete() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_building_loops, clear_test_template_voices, set_test_per_unit_sound,
    };
    clear_test_template_voices();
    clear_building_loops();
    set_test_per_unit_sound(
        "TestBuilding",
        "UnderConstruction",
        "BuildingConstructionLoop",
    );
    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let sid = logic
        .create_object_under_construction("TestBuilding", Team::USA, glam::Vec3::ZERO)
        .expect("scaffold");
    if let Some(o) = logic.host_object_mut(sid) {
        o.thing.template.build_time = 10.0;
        o.construction_percent = 0.0;
    }
    let did = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer");
    assert!(logic.resume_construction(&[did], sid));
    logic.queued_audio_events.clear();
    logic.update_construction(&[sid], 1.0);
    assert!(
        logic.queued_audio_events.iter().any(|e| {
            e.event_type == "BuildingConstructionLoop"
                && e.is_looping
                && !e.stop
                && e.object_id == Some(sid)
        }),
        "docked dozer must start UnderConstruction loop: {:?}",
        logic.queued_audio_events
    );
    logic.queued_audio_events.clear();
    logic.update_construction(&[sid], 1.0);
    assert!(
        !logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "BuildingConstructionLoop" && e.is_looping && !e.stop }),
        "already-playing loop must not restart: {:?}",
        logic.queued_audio_events
    );
    if let Some(o) = logic.host_object_mut(sid) {
        o.construction_percent = 0.99;
    }
    logic.queued_audio_events.clear();
    logic.update_construction(&[sid], 1.0);
    assert!(
        logic
            .host_object(sid)
            .is_some_and(|o| !o.status.under_construction),
        "scaffold must complete"
    );
    assert!(
        logic.queued_audio_events.iter().any(|e| {
            e.event_type == "BuildingConstructionLoop" && e.stop && e.object_id == Some(sid)
        }),
        "complete must finishBuildingSound: {:?}",
        logic.queued_audio_events
    );
    clear_test_template_voices();
    clear_building_loops();
}

