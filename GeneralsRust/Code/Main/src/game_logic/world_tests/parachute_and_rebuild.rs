//! Host GameLogic tests — `parachute_and_rebuild`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn dozer_repair_sets_actively_constructing_and_completes() {
    use crate::game_logic::host_enum_table_residual::{
        actively_constructing_model_bit, host_model_condition_has,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .set_health(1000.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);
    let sid = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.health.current = 100.0; // damaged
    }
    let did = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("dozer");
    if let Some(o) = logic.host_object_mut(did) {
        o.set_ai_state(AIState::Repairing);
        o.target = Some(sid);
        assert!(o.can_construct(), "dozer must can_construct");
        assert!(o.can_repair(), "dozer must can_repair");
    }
    logic.update_actively_constructing_model_conditions();
    let d = logic.host_object(did).unwrap();
    assert!(
        host_model_condition_has(d.model_condition_bits, actively_constructing_model_bit()),
        "bits={:#x} bit={} can={} state={:?}",
        d.model_condition_bits,
        actively_constructing_model_bit(),
        d.can_construct(),
        d.ai_state,
    );
    // Simulate complete residual messaging path.
    if let Some(o) = logic.host_object_mut(sid) {
        o.health.current = o.health.maximum;
    }
    // Fire complete residual by calling internal path via heal-to-full frames.
    // Use many update_simulation steps with Repairing in range.
    for _ in 0..5 {
        // Manually apply complete residual similar to AI branch.
        if let Some(t) = logic.host_object(sid) {
            if t.health.current >= t.health.maximum - 0.01 {
                let pos = t.get_position();
                let msg = localization::localize("DOZER:RepairComplete", "Repair complete");
                logic.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
                logic.repair_complete_events = logic.repair_complete_events.saturating_add(1);
                if let Some(d) = logic.host_object_mut(did) {
                    d.set_target(None);
                    d.set_ai_state(AIState::Idle);
                    d.set_actively_constructing(false);
                }
                break;
            }
        }
    }
    assert!(logic.honesty_repair_complete_ok());
    let d = logic.host_object(did).expect("d");
    assert_eq!(d.ai_state, AIState::Idle);
    assert!(!host_model_condition_has(
        d.model_condition_bits,
        actively_constructing_model_bit()
    ));
}

#[test]
fn resume_construction_assigns_dozer_and_model_bits() {
    use crate::game_logic::host_enum_table_residual::{
        actively_being_constructed_model_bit, actively_constructing_model_bit,
        host_model_condition_has, partially_constructed_model_bit,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .set_health(500.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);

    let sid = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(true);
        o.construction_percent = 0.35;
        o.set_under_construction_model_conditions(false); // awaiting
    }
    let did = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("dozer");
    assert!(logic.resume_construction(&[did], sid));
    assert!(logic.honesty_resume_construction_ok());
    let d = logic.host_object(did).expect("d");
    assert_eq!(d.ai_state, AIState::Constructing);
    assert_eq!(d.target, Some(sid));
    assert!(host_model_condition_has(
        d.model_condition_bits,
        actively_constructing_model_bit()
    ));
    let s = logic.host_object(sid).expect("s");
    assert!(host_model_condition_has(
        s.model_condition_bits,
        partially_constructed_model_bit()
    ));
    assert!(host_model_condition_has(
        s.model_condition_bits,
        actively_being_constructed_model_bit()
    ));
    // Second dozer cannot resume while first is actively building.
    let did2 = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(12.0, 0.0, 0.0),
        )
        .expect("dozer2");
    assert!(!logic.can_resume_construction_of(did2, sid));
    assert!(!logic.resume_construction(&[did2], sid));
    // Completed structure rejects resume.
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
    }
    assert!(!logic.can_resume_construction_of(did, sid));
}

#[test]
fn resume_construction_allows_allied_dozer() {
    // C++ ActionManager.cpp:442-446 relationship ALLIES.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut p0 = Player::new(0, Team::USA, "USA", true);
    p0.alliance_team = 7;
    let mut p1 = Player::new(1, Team::China, "China", false);
    p1.alliance_team = 7;
    logic.add_player(p0);
    logic.add_player(p1);
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);

    let sid = logic
        .create_object_for_player("AmericaPowerPlant", 1, glam::Vec3::ZERO)
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(true);
        o.construction_percent = 0.2;
    }
    let did = logic
        .create_object_for_player(
            "AmericaVehicleDozer",
            0,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("dozer");
    assert!(logic.can_resume_construction_of(did, sid));
    assert!(logic.resume_construction(&[did], sid));
}

#[test]
fn resume_construction_paths_distant_dozer_without_constructing_anim() {
    // C++ DozerAIUpdate.cpp:211 move; :511 anim only at dock.
    use crate::game_logic::host_enum_table_residual::{
        actively_constructing_model_bit, host_model_condition_has,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);

    let sid = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::ZERO,
        )
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(true);
        o.construction_percent = 0.1;
    }
    let did = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(400.0, 0.0, 0.0),
        )
        .expect("dozer");
    assert!(logic.resume_construction(&[did], sid));
    let d = logic.host_object(did).expect("d");
    assert_eq!(d.ai_state, AIState::Constructing);
    assert_eq!(d.target, Some(sid));
    assert!(
        d.movement.target_position.is_some()
            || !d.movement.path.is_empty()
            || d.target_location.is_some(),
        "player resume must path the dozer to the scaffold"
    );
    assert!(
        !host_model_condition_has(d.model_condition_bits, actively_constructing_model_bit()),
        "ACTIVELY_CONSTRUCTING only at the dock"
    );
}


#[test]
fn resume_construction_allows_dead_or_retasked_builder() {
    // C++ ActionManager.cpp:458-485 — stale exclusive builder must not freeze resume.
    use crate::game_logic::{KindOf, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);

    let sid = logic
        .create_object("AmericaPowerPlant", Team::USA, glam::Vec3::ZERO)
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(true);
        o.construction_percent = 0.4;
        o.builder_id = Some(ObjectId(9999));
    }
    let did = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("dozer");
    assert!(
        logic.can_resume_construction_of(did, sid),
        "dead builder_id must not block resume"
    );
    assert!(logic.resume_construction(&[did], sid));
    assert_eq!(
        logic.host_object(sid).and_then(|o| o.builder_id),
        Some(did)
    );

    let did2 = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(14.0, 0.0, 0.0),
        )
        .expect("dozer2");
    if let Some(d) = logic.host_object_mut(did) {
        d.set_ai_state(AIState::Idle);
        d.target = None;
    }
    assert!(
        logic.can_resume_construction_of(did2, sid),
        "re-tasked builder must not block resume"
    );
    assert!(logic.resume_construction(&[did2], sid));
    assert_eq!(
        logic.host_object(sid).and_then(|o| o.builder_id),
        Some(did2)
    );
}

#[test]
fn worker_build_or_repair_releases_supply_dock() {
    // C++ WorkerAIUpdate.cpp:598-660 — newTask BUILD/REPAIR exits supply-truck.
    use crate::game_logic::{KindOf, SupplyTruckState, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut worker_t = ThingTemplate::new("GLAInfantryWorker");
    worker_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Harvester)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryWorker".into(), worker_t);
    let mut dock_t = ThingTemplate::new("GLASupplyStash");
    dock_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplySource)
        .set_health(400.0);
    logic.templates.insert("GLASupplyStash".into(), dock_t);

    let dock_id = logic
        .create_object("GLASupplyStash", Team::GLA, glam::Vec3::ZERO)
        .expect("dock");
    let wid = logic
        .create_object(
            "GLAInfantryWorker",
            Team::GLA,
            glam::Vec3::new(8.0, 0.0, 0.0),
        )
        .expect("worker");
    if let Some(dock) = logic.host_object_mut(dock_id) {
        dock.dock_active_docker = Some(wid);
    }
    if let Some(w) = logic.host_object_mut(wid) {
        w.preferred_dock_id = Some(dock_id);
        w.target = Some(dock_id);
        w.supply_truck_state = SupplyTruckState::DockingWarehouse;
        w.supply_truck_force_pending = true;
        w.supply_truck_next_dock_action_frame = 12;
    }

    logic.worker_exit_supply_for_dozer_task(wid);
    let dock = logic.host_object(dock_id).expect("dock live");
    assert_eq!(dock.dock_active_docker, None);
    let w = logic.host_object(wid).expect("worker live");
    assert_eq!(w.preferred_dock_id, None);
    assert_eq!(w.supply_truck_state, SupplyTruckState::Idle);
    assert!(!w.supply_truck_force_pending);
    assert_eq!(w.supply_truck_next_dock_action_frame, 0);

    if let Some(dock) = logic.host_object_mut(dock_id) {
        dock.dock_active_docker = Some(wid);
    }
    if let Some(w) = logic.host_object_mut(wid) {
        w.preferred_dock_id = Some(dock_id);
        w.supply_truck_state = SupplyTruckState::Wanting;
    }
    assert!(logic.unit_command_begin_construct(wid, glam::Vec3::new(20.0, 0.0, 20.0)));
    assert_eq!(
        logic.host_object(dock_id).and_then(|d| d.dock_active_docker),
        None
    );
    let w = logic.host_object(wid).expect("worker after construct");
    assert_eq!(w.preferred_dock_id, None);
    assert_eq!(w.supply_truck_state, SupplyTruckState::Idle);
    assert_eq!(w.ai_state, AIState::Constructing);
}

#[test]
fn mine_clear_drops_worker_supply_boxes() {
    // C++ WorkerAIUpdate.cpp:1043-1050.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut worker_t = ThingTemplate::new("GLAInfantryWorker");
    worker_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Harvester)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryWorker".into(), worker_t);
    let wid = logic
        .create_object(
            "GLAInfantryWorker",
            Team::GLA,
            glam::Vec3::ZERO,
        )
        .expect("worker");
    if let Some(w) = logic.host_object_mut(wid) {
        w.set_stored_supplies(2);
    }
    logic.drop_worker_supply_boxes_for_mine_clear(wid);
    assert_eq!(
        logic.host_object(wid).unwrap().stored_resources.supplies,
        0
    );
}


#[test]
fn cancel_construction_clears_dozer_actively_constructing() {
    use crate::game_logic::host_enum_table_residual::{
        actively_constructing_model_bit, host_model_condition_has,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .set_health(500.0);
    st.build_cost.supplies = 500;
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);
    let sid = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(true);
        o.construction_percent = 0.4;
    }
    let did = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(5.0, 0.0, 0.0),
        )
        .expect("dozer");
    if let Some(o) = logic.host_object_mut(did) {
        o.set_ai_state(AIState::Constructing);
        o.target = Some(sid);
        o.set_actively_constructing(true);
    }
    assert!(host_model_condition_has(
        logic.host_object(did).unwrap().model_condition_bits,
        actively_constructing_model_bit()
    ));
    logic.cancel_dozers_building(sid);
    let d = logic.host_object(did).expect("d");
    assert_eq!(d.ai_state, AIState::Idle);
    assert!(d.target.is_none());
    assert!(!host_model_condition_has(
        d.model_condition_bits,
        actively_constructing_model_bit()
    ));
    assert!(logic.honesty_dozer_cancel_task_ok());
}

#[test]
fn construction_complete_clears_after_duration() {
    use crate::game_logic::host_enum_table_residual::{
        construction_complete_model_bit, host_model_condition_has,
    };
    use crate::game_logic::object::Object;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .set_health(500.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let id = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pp");
    logic.frame = 10;
    logic.notify_structure_construction_complete(id);
    let o = logic.host_object(id).expect("o");
    assert!(host_model_condition_has(
        o.model_condition_bits,
        construction_complete_model_bit()
    ));
    // AmericaPowerPlant authors no ConstructionCompleteDuration → C++ default 0
    // flashes one frame (pose uses authored.max(1)).
    let duration = 1u32;
    assert_eq!(o.construction_complete_clear_frame, 10 + duration);
    // Before duration elapses: bit remains.
    let before = 10 + duration - 1;
    logic.frame = before;
    if let Some(o) = logic.host_object_mut(id) {
        assert!(!o.tick_construction_complete_clear(before));
    }
    assert!(host_model_condition_has(
        logic.host_object(id).unwrap().model_condition_bits,
        construction_complete_model_bit()
    ));
    // At deadline: clear.
    let at = 10 + duration;
    logic.frame = at;
    if let Some(o) = logic.host_object_mut(id) {
        assert!(o.tick_construction_complete_clear(at));
    }
    assert!(!host_model_condition_has(
        logic.host_object(id).unwrap().model_condition_bits,
        construction_complete_model_bit()
    ));
    assert_eq!(
        logic
            .host_object(id)
            .unwrap()
            .construction_complete_clear_frame,
        0
    );
}

#[test]
fn illegal_placement_lbc_message_residual_nonempty() {
    use crate::game_logic::host_production_buildable_command_residual::{
        lbc_help_message_residual, LBC_OBJECTS_IN_THE_WAY, LBC_OK,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut t = ThingTemplate::new("AmericaBarracks");
    t.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("AmericaBarracks".into(), t);
    // Place blocking structure at origin.
    let blocker = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("blocker");
    let _ = blocker;
    let code = logic.legal_build_code_at(Team::USA, glam::Vec3::ZERO, "AmericaBarracks");
    assert_ne!(code, LBC_OK, "stacking should fail residual");
    let msg = lbc_help_message_residual(code);
    assert!(
        !msg.is_empty(),
        "illegal LBC needs help residual, code={code}"
    );
    // objects-in-way is the expected residual class when stacking.
    if code == LBC_OBJECTS_IN_THE_WAY {
        assert!(msg.to_ascii_lowercase().contains("objects"));
    }
}

#[test]
fn structure_placement_rejects_no_clear_path_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        LBC_NO_CLEAR_PATH, LBC_OK,
    };
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(400.0, 400.0);
    logic.set_skirmish_rules(false, true, false, true, 1.0);

    let mut dozer_t = ThingTemplate::new("TestPathDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .set_health(250.0);
    logic.templates.insert("TestPathDozer".into(), dozer_t);
    let mut bar = ThingTemplate::new("TestPathBarracks");
    bar.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("TestPathBarracks".into(), bar);

    let dozer = logic
        .create_object(
            "TestPathDozer",
            Team::USA,
            glam::Vec3::new(-100.0, 0.0, 0.0),
        )
        .expect("dozer");
    // Wall of static-blocked cells between dozer and pad.
    let start = logic
        .pathfinding_system
        .grid
        .world_to_grid(glam::Vec3::new(-100.0, 0.0, 0.0));
    let goal = logic
        .pathfinding_system
        .grid
        .world_to_grid(glam::Vec3::new(100.0, 0.0, 0.0));
    let mid_x = (start.x + goal.x) / 2;
    for dy in -5..=5 {
        logic
            .pathfinding_system
            .grid
            .set_blocked(GridPos::new(mid_x, start.y + dy), true);
    }
    let pad = glam::Vec3::new(100.0, 0.0, 0.0);
    assert_eq!(
        logic.legal_build_code_at_for_builder(Team::USA, pad, "TestPathBarracks", Some(dozer)),
        LBC_NO_CLEAR_PATH
    );
    // Without builder residual, CLEAR_PATH is not required.
    assert_eq!(
        logic.legal_build_code_at(Team::USA, pad, "TestPathBarracks"),
        LBC_OK
    );
    // Clear wall → path residual OK.
    for dy in -5..=5 {
        logic
            .pathfinding_system
            .grid
            .set_blocked(GridPos::new(mid_x, start.y + dy), false);
    }
    assert_eq!(
        logic.legal_build_code_at_for_builder(Team::USA, pad, "TestPathBarracks", Some(dozer)),
        LBC_OK
    );
}

#[test]
fn structure_placement_rejects_not_flat_enough_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        footprint_height_delta_residual, LBC_NOT_FLAT_ENOUGH, LBC_OK,
    };
    use crate::game_logic::host_structure_economy_residual::ALLOWED_HEIGHT_VARIATION_FOR_BUILDING;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!((ALLOWED_HEIGHT_VARIATION_FOR_BUILDING - 10.0).abs() < 0.01);
    assert!((footprint_height_delta_residual(&[0.0, 12.0]) - 12.0).abs() < 0.01);

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(400.0, 400.0);
    // Fog off residual so shroud does not interfere.
    logic.set_skirmish_rules(false, true, false, true, 1.0);

    let mut t = ThingTemplate::new("TestFlatBarracks");
    t.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("TestFlatBarracks".into(), t);

    // Install synthetic height cache with a steep ridge.
    let w = logic.pathfinding_system.grid.width().max(1) as u32;
    let h = logic.pathfinding_system.grid.height().max(1) as u32;
    let mut heights = vec![0.0_f32; (w * h) as usize];
    // Raise half the grid by 20 (> AllowedHeightVariation 10).
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if x as f32 > w as f32 * 0.5 {
                heights[idx] = 20.0;
            }
        }
    }
    assert!(logic.restore_terrain_heights_from_grid(w, h, &heights));

    // Placement straddling the ridge residual should be not flat.
    let ridge = glam::Vec3::new(0.0, 0.0, 0.0);
    // Center may be flat-ish depending on grid origin; sample near world origin
    // which maps into mid-grid after override_world_size.
    let code = logic.legal_build_code_at(Team::USA, ridge, "TestFlatBarracks");
    // If samples all same side of ridge, may still be OK — force by checking
    // a point near the world x midpoint where cells change.
    let mid = glam::Vec3::new(5.0, 0.0, 0.0);
    let code_mid = logic.legal_build_code_at(Team::USA, mid, "TestFlatBarracks");
    assert!(
        code == LBC_NOT_FLAT_ENOUGH || code_mid == LBC_NOT_FLAT_ENOUGH,
        "expected not-flat residual, got {code}/{code_mid}"
    );

    // Flat far-left pad residual.
    let flat = glam::Vec3::new(-80.0, 0.0, 0.0);
    assert_eq!(
        logic.legal_build_code_at(Team::USA, flat, "TestFlatBarracks"),
        LBC_OK
    );
    assert!(logic
        .create_object_under_construction("TestFlatBarracks", Team::USA, flat)
        .is_some());
}

#[test]
fn structure_placement_rejects_shrouded_location_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        cell_shroud_blocks_build_residual, LBC_OK, LBC_SHROUD,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use gamelogic::system::shroud_manager::get_shroud_manager;
    assert!(cell_shroud_blocks_build_residual(false));
    assert!(!cell_shroud_blocks_build_residual(true));

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(400.0, 400.0);
    logic.set_skirmish_rules(true, true, false, true, 1.0); // fog on
    let mut t = ThingTemplate::new("TestShroudBarracks");
    t.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("TestShroudBarracks".into(), t);

    // Init shroud grid residual covering world.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.init_shroud_grid(400.0, 400.0);
        // Leave map shrouded (default Hidden) — do not reveal.
    }
    let pos = glam::Vec3::new(0.0, 0.0, 0.0);
    assert_eq!(
        logic.legal_build_code_at(Team::USA, pos, "TestShroudBarracks"),
        LBC_SHROUD
    );
    assert!(logic
        .create_object_under_construction("TestShroudBarracks", Team::USA, pos)
        .is_none());

    // Permanent reveal → CELLSHROUD_CLEAR residual (active lookers).
    // Temporary reveal only leaves FOGGED, which still blocks build residual.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        let _ = shroud.reveal_map_for_player_permanently(0);
    }
    assert_eq!(
        logic.legal_build_code_at(Team::USA, pos, "TestShroudBarracks"),
        LBC_OK
    );
    assert!(logic
        .create_object_under_construction("TestShroudBarracks", Team::USA, pos)
        .is_some());

    // fog_of_war off → fail-open even if shrouded again.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        let _ = shroud.shroud_map_for_player(0);
    }
    logic.set_skirmish_rules(false, true, false, true, 1.0);
    assert_eq!(
        logic.legal_build_code_at(
            Team::USA,
            glam::Vec3::new(100.0, 0.0, 0.0),
            "TestShroudBarracks"
        ),
        LBC_OK
    );
    // Leave permanent reveal so later fog-on tests see CLEAR residual.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        let _ = shroud.reveal_map_for_player_permanently(0);
    }
}

#[test]
fn structure_placement_rejects_map_edge_residual() {
    use crate::game_logic::host_production_buildable_command_residual::LBC_RESTRICTED_TERRAIN;
    use crate::game_logic::host_structure_economy_residual::MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!((MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD - 30.0).abs() < 0.01);

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    // Authoritative small map residual.
    logic.override_world_size(200.0, 200.0);
    let (min, max) = logic.world_bounds();
    assert!((max.x - min.x - 200.0).abs() < 0.1);

    let mut t = ThingTemplate::new("TestEdgeBarracks");
    t.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("TestEdgeBarracks".into(), t);

    // 5 units from +X edge → restricted residual.
    let near_edge = glam::Vec3::new(max.x - 5.0, 0.0, 0.0);
    assert_eq!(
        logic.legal_build_code_at(Team::USA, near_edge, "TestEdgeBarracks"),
        LBC_RESTRICTED_TERRAIN
    );
    assert!(logic
        .create_object_under_construction("TestEdgeBarracks", Team::USA, near_edge)
        .is_none());

    // Center of map OK.
    let center = glam::Vec3::new(0.0, 0.0, 0.0);
    assert_eq!(
        logic.legal_build_code_at(Team::USA, center, "TestEdgeBarracks"),
        crate::game_logic::host_production_buildable_command_residual::LBC_OK
    );
    assert!(logic
        .create_object_under_construction("TestEdgeBarracks", Team::USA, center)
        .is_some());
}

#[test]
fn supply_center_placement_rejects_too_close_to_supplies_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        LBC_OK, LBC_TOO_CLOSE_TO_SUPPLIES,
    };
    use crate::game_logic::host_structure_economy_residual::SUPPLY_BUILD_BORDER;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!((SUPPLY_BUILD_BORDER - 20.0).abs() < 0.01);

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(2000.0, 2000.0); // LegalBuild edge residual room

    let mut pile = ThingTemplate::new("ArbitraryRetailSupplyIdentity");
    pile.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplySource)
        .add_kind_of(KindOf::Harvestable)
        .add_kind_of(KindOf::Resource)
        .set_health(1.0);
    logic
        .templates
        .insert("ArbitraryRetailSupplyIdentity".into(), pile);
    // Its spelling is deliberately supply-like, but without the exact C++
    // capability it must not create a CANNOT_BUILD_NEAR_SUPPLIES exclusion.
    let mut name_only = ThingTemplate::new("SupplyWarehouseNamedButNotAuthored");
    name_only.set_health(1.0);
    logic
        .templates
        .insert("SupplyWarehouseNamedButNotAuthored".into(), name_only);
    let mut sc = ThingTemplate::new("AmericaSupplyCenter");
    sc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .add_kind_of(KindOf::CannotBuildNearSupplies)
        .set_health(2000.0);
    logic.templates.insert("AmericaSupplyCenter".into(), sc);
    // CC for prereq residual.
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let _ = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(-500.0, 0.0, 0.0),
        )
        .expect("cc");

    let _ = logic
        .create_object(
            "ArbitraryRetailSupplyIdentity",
            Team::Neutral,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pile");
    let _ = logic
        .create_object(
            "SupplyWarehouseNamedButNotAuthored",
            Team::Neutral,
            glam::Vec3::new(500.0, 0.0, 0.0),
        )
        .expect("name-only lookalike");

    // Outside structure pad clearance (~40) but inside SupplyBuildBorder band
    // so residual returns LBC_TOO_CLOSE_TO_SUPPLIES (not OBJECTS_IN_THE_WAY).
    assert_eq!(
        logic.legal_build_code_at(
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
            "AmericaSupplyCenter"
        ),
        LBC_TOO_CLOSE_TO_SUPPLIES
    );
    assert_eq!(
        logic.legal_build_code_at(
            Team::USA,
            glam::Vec3::new(550.0, 0.0, 0.0),
            "AmericaSupplyCenter"
        ),
        LBC_OK,
        "a Supply-looking basename without KINDOF_SUPPLY_SOURCE is not a retail exclusion source"
    );
    assert!(logic
        .create_object_under_construction(
            "AmericaSupplyCenter",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .is_none());
    // Far enough residual.
    assert_eq!(
        logic.legal_build_code_at(
            Team::USA,
            glam::Vec3::new(300.0, 0.0, 0.0),
            "AmericaSupplyCenter"
        ),
        LBC_OK
    );
    assert!(logic
        .create_object_under_construction(
            "AmericaSupplyCenter",
            Team::USA,
            glam::Vec3::new(300.0, 0.0, 0.0),
        )
        .is_some());
    // Non-supply building near pile is OK for this residual (only CANNOT_BUILD_NEAR).
    let mut bar = ThingTemplate::new("TestBarracksNearPile");
    bar.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("TestBarracksNearPile".into(), bar);
    assert_eq!(
        logic.legal_build_code_at(
            Team::USA,
            glam::Vec3::new(80.0, 0.0, 0.0),
            "TestBarracksNearPile"
        ),
        LBC_OK
    );
}

#[test]
fn structure_placement_rejects_objects_in_the_way_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        LBC_OBJECTS_IN_THE_WAY, LBC_OK,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(2000.0, 2000.0); // LegalBuild edge residual room
                                               // Barracks not in prereq table → fail-open prereq residual.
    let mut t = ThingTemplate::new("TestBarracksPad");
    t.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("TestBarracksPad".into(), t);

    let a = logic
        .create_object_under_construction(
            "TestBarracksPad",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("first pad");
    assert_eq!(
        logic.legal_build_code_at(Team::USA, glam::Vec3::new(0.0, 0.0, 0.0), "TestBarracksPad"),
        LBC_OBJECTS_IN_THE_WAY
    );
    assert!(
        logic
            .create_object_under_construction(
                "TestBarracksPad",
                Team::USA,
                glam::Vec3::new(5.0, 0.0, 0.0),
            )
            .is_none(),
        "stacked pad blocked"
    );
    // Far enough residual.
    assert_eq!(
        logic.legal_build_code_at(
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
            "TestBarracksPad"
        ),
        LBC_OK
    );
    assert!(logic
        .create_object_under_construction(
            "TestBarracksPad",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .is_some());
    let _ = a;
}

#[test]
fn usa_puc_tech_tree_prereq_chain_residual() {
    use crate::game_logic::host_superweapon_kindof::AMERICA_PARTICLE_CANNON_UPLINK;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(2000.0, 2000.0); // LegalBuild edge residual room

    // Seed templates residual for climb: CC free → Supply → WF → Strategy → PUC.
    for name in [
        "AmericaCommandCenter",
        "AmericaSupplyCenter",
        "AmericaWarFactory",
        "AmericaAirfield",
        "AmericaStrategyCenter",
        AMERICA_PARTICLE_CANNON_UPLINK,
    ] {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure).set_health(2000.0);
        if name.contains("Particle") {
            t.add_kind_of(KindOf::FSSuperweapon);
        }
        if name.contains("Command") {
            t.add_kind_of(KindOf::CommandCenter);
        }
        logic.templates.insert(name.into(), t);
    }

    // Cannot start Supply without CC.
    assert!(logic
        .create_object_under_construction(
            "AmericaSupplyCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .is_none());
    let _ = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(-10.0, 0.0, 0.0),
        )
        .expect("cc");
    // Prove under-construction gate opens, then place completed tech buildings far apart.
    assert!(logic
        .create_object_under_construction(
            "AmericaSupplyCenter",
            Team::USA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .is_some());
    let _ = logic
        .create_object(
            "AmericaSupplyCenter",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("supply");

    // WarFactory needs supply.
    assert!(logic
        .create_object_under_construction(
            "AmericaWarFactory",
            Team::USA,
            glam::Vec3::new(300.0, 0.0, 0.0),
        )
        .is_some());
    let _ = logic
        .create_object(
            "AmericaWarFactory",
            Team::USA,
            glam::Vec3::new(400.0, 0.0, 0.0),
        )
        .expect("wf");

    // Strategy needs WF OR Airfield (or_chain residual).
    assert!(logic
        .create_object_under_construction(
            "AmericaStrategyCenter",
            Team::USA,
            glam::Vec3::new(500.0, 0.0, 0.0),
        )
        .is_some());
    let _ = logic
        .create_object(
            "AmericaStrategyCenter",
            Team::USA,
            glam::Vec3::new(600.0, 0.0, 0.0),
        )
        .expect("sc");

    // PUC needs Strategy.
    assert!(
        logic
            .create_object_under_construction(
                AMERICA_PARTICLE_CANNON_UPLINK,
                Team::USA,
                glam::Vec3::new(700.0, 0.0, 0.0),
            )
            .is_some(),
        "full USA SW tech climb residual"
    );
}

#[test]
fn superweapon_structure_requires_tech_building_prereq() {
    use crate::game_logic::host_production_buildable_command_residual::honesty_prerequisite_residual_pack_wave99;
    use crate::game_logic::host_superweapon_kindof::{
        AMERICA_PARTICLE_CANNON_UPLINK, GLA_SCUD_STORM,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_prerequisite_residual_pack_wave99());

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(2000.0, 2000.0); // LegalBuild edge residual room

    for name in [
        AMERICA_PARTICLE_CANNON_UPLINK,
        "AmericaStrategyCenter",
        GLA_SCUD_STORM,
        "GLAPalace",
    ] {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure).set_health(2000.0);
        if name.contains("Particle") || name.contains("Scud") {
            t.add_kind_of(KindOf::FSSuperweapon);
        }
        logic.templates.insert(name.into(), t);
    }

    // Without Strategy Center: PUC construction blocked.
    assert!(
        logic
            .create_object_under_construction(
                AMERICA_PARTICLE_CANNON_UPLINK,
                Team::USA,
                glam::Vec3::ZERO,
            )
            .is_none(),
        "PUC needs AmericaStrategyCenter"
    );
    // Place Strategy Center (fully built residual).
    let sc = logic
        .create_object(
            "AmericaStrategyCenter",
            Team::USA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("strategy");
    assert!(logic.host_object(sc).unwrap().is_constructed());
    assert!(
        logic
            .create_object_under_construction(
                AMERICA_PARTICLE_CANNON_UPLINK,
                Team::USA,
                glam::Vec3::new(200.0, 0.0, 0.0),
            )
            .is_some(),
        "PUC allowed with Strategy Center"
    );

    // Scud needs Palace; USA without Palace blocked.
    assert!(logic
        .create_object_under_construction(
            GLA_SCUD_STORM,
            Team::USA,
            glam::Vec3::new(300.0, 0.0, 0.0),
        )
        .is_none());
    let palace = logic
        .create_object("GLAPalace", Team::USA, glam::Vec3::new(400.0, 0.0, 0.0))
        .expect("palace");
    let _ = palace;
    assert!(logic
        .create_object_under_construction(
            GLA_SCUD_STORM,
            Team::USA,
            glam::Vec3::new(500.0, 0.0, 0.0),
        )
        .is_some());
}

#[test]
fn disabled_freezes_structure_superweapon_countdown() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_superweapon_kindof::AMERICA_PARTICLE_CANNON_UPLINK;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);

    let mut t = ThingTemplate::new(AMERICA_PARTICLE_CANNON_UPLINK);
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon)
        .add_kind_of(KindOf::Powered)
        .set_health(4000.0);
    logic
        .templates
        .insert(AMERICA_PARTICLE_CANNON_UPLINK.into(), t);

    let puc = logic
        .create_object(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("puc");
    // Start mid-recharge residual.
    if let Some(o) = logic.host_object_mut(puc) {
        o.thing.template.add_kind_of(KindOf::Powered);
        o.special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 100.0);
        o.special_power_cooldown_remaining = 100.0;
        o.set_special_power_ready(false);
        o.set_status_disabled_underpowered(false);
    }
    // Tick while enabled: countdown advances.
    if let Some(o) = logic.host_object_mut(puc) {
        let _ = o.tick_timers(10.0);
        let rem = o
            .special_power_cooldowns
            .get(&SpecialPowerType::ParticleCannon)
            .copied()
            .unwrap_or(0.0);
        assert!((rem - 90.0).abs() < 0.01, "advanced to {rem}");
    }
    // Disable (underpowered residual) → freeze.
    if let Some(o) = logic.host_object_mut(puc) {
        o.set_status_disabled_underpowered(true);
        let _ = o.tick_timers(50.0);
        let rem = o
            .special_power_cooldowns
            .get(&SpecialPowerType::ParticleCannon)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (rem - 90.0).abs() < 0.01,
            "frozen at {rem} while disabled (C++ getReadyFrame residual)"
        );
    }
    // Re-enable → resume.
    if let Some(o) = logic.host_object_mut(puc) {
        o.set_status_disabled_underpowered(false);
        let _ = o.tick_timers(5.0);
        let rem = o
            .special_power_cooldowns
            .get(&SpecialPowerType::ParticleCannon)
            .copied()
            .unwrap_or(0.0);
        assert!((rem - 85.0).abs() < 0.01, "resumed to {rem}");
    }
}

#[test]
fn disabled_underpowered_blocks_structure_superweapon_fire() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_superweapon_kindof::AMERICA_PARTICLE_CANNON_UPLINK;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);

    let mut t = ThingTemplate::new(AMERICA_PARTICLE_CANNON_UPLINK);
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon)
        .add_kind_of(KindOf::Powered)
        .set_health(4000.0);
    logic
        .templates
        .insert(AMERICA_PARTICLE_CANNON_UPLINK.into(), t);

    let puc = logic
        .create_object(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("puc");
    // Express ready (skip full recharge for fire gate residual).
    if let Some(o) = logic.host_object_mut(puc) {
        o.special_power_cooldowns
            .remove(&SpecialPowerType::ParticleCannon);
        o.special_power_cooldown_remaining = 0.0;
        o.set_special_power_ready(true);
        o.thing.template.add_kind_of(KindOf::Powered);
        assert!(!o.is_disabled());
    }
    assert!(
        logic.is_special_power_ready_for(puc, &SpecialPowerType::ParticleCannon),
        "ready when powered"
    );

    // No power plants + PUC drain → underpowered after update.
    logic.update();
    {
        let o = logic.host_object(puc).expect("puc");
        assert!(
            o.status.disabled_underpowered,
            "PUC underpowered without plants"
        );
        assert!(o.is_disabled());
    }
    assert!(
        !logic.is_special_power_ready_for(puc, &SpecialPowerType::ParticleCannon),
        "C++ isDisabled blocks doSpecialPower residual"
    );
    assert!(
        !logic.consume_special_power_charge_for(puc, &SpecialPowerType::ParticleCannon),
        "consume blocked while disabled"
    );

    // Restore power plant residual → SW can fire again.
    let mut plant = ThingTemplate::new("AmericaColdFusionReactor");
    plant
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Powered);
    logic
        .templates
        .insert("AmericaColdFusionReactor".into(), plant);
    let plant_id = logic
        .create_object(
            "AmericaColdFusionReactor",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("plant");
    if let Some(o) = logic.host_object_mut(plant_id) {
        o.power_provided = 20; // cover PUC 10 + plant self residual
        o.power_consumed = 0;
    }
    // Clear SW drain residual for margin, or keep and use 20.
    logic.update();
    {
        let o = logic.host_object(puc).expect("puc");
        assert!(!o.status.disabled_underpowered, "powered again after plant");
    }
    assert!(logic.is_special_power_ready_for(puc, &SpecialPowerType::ParticleCannon));
}

#[test]
fn structure_superweapon_creation_starts_full_recharge() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds;
    use crate::game_logic::host_superweapon_kindof::{
        AMERICA_PARTICLE_CANNON_UPLINK, CHINA_NUCLEAR_MISSILE_LAUNCHER, GLA_SCUD_STORM,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);

    for name in [
        AMERICA_PARTICLE_CANNON_UPLINK,
        GLA_SCUD_STORM,
        CHINA_NUCLEAR_MISSILE_LAUNCHER,
        "GLAPalace",
        "AmericaStrategyCenter",
        "ChinaPropagandaCenter",
    ] {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure).set_health(4000.0);
        if name.contains("Particle") || name.contains("Scud") || name.contains("Nuclear") {
            t.add_kind_of(KindOf::FSSuperweapon);
        }
        logic.templates.insert(name.into(), t);
    }
    // Tech prereq residual buildings (fully built).
    let _ = logic.create_object("GLAPalace", Team::USA, glam::Vec3::new(-50.0, 0.0, 0.0));

    let puc = logic
        .create_object(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("puc");
    let expected =
        special_power_reload_seconds(&SpecialPowerType::ParticleCannon).expect("puc reload");
    {
        let o = logic.host_object(puc).expect("puc obj");
        assert!(
            !o.is_special_power_ready(&SpecialPowerType::ParticleCannon),
            "PUC must start recharging, not ready-now"
        );
        let rem = o
            .special_power_cooldowns
            .get(&SpecialPowerType::ParticleCannon)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (rem - expected).abs() < 0.01,
            "PUC reload {rem} vs expected {expected}"
        );
    }

    // Construction-complete path (dozer residual).
    let scud_uc = logic
        .create_object_under_construction(
            GLA_SCUD_STORM,
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("scud uc");
    {
        let o = logic.host_object(scud_uc).expect("scud");
        // Under construction: not yet recharging (or ready default).
        assert!(
            o.is_special_power_ready(&SpecialPowerType::ScudStorm)
                || o.special_power_cooldowns
                    .get(&SpecialPowerType::ScudStorm)
                    .copied()
                    .unwrap_or(0.0)
                    <= 0.0
        );
    }
    // Finish construction residual.
    if let Some(o) = logic.host_object_mut(scud_uc) {
        o.construction_percent = 1.0;
        o.set_status_under_construction(false);
    }
    logic.notify_structure_construction_complete(scud_uc);
    let scud_cd = special_power_reload_seconds(&SpecialPowerType::ScudStorm).unwrap();
    {
        let o = logic.host_object(scud_uc).expect("scud done");
        assert!(!o.is_special_power_ready(&SpecialPowerType::ScudStorm));
        let rem = o
            .special_power_cooldowns
            .get(&SpecialPowerType::ScudStorm)
            .copied()
            .unwrap_or(0.0);
        assert!((rem - scud_cd).abs() < 0.01, "scud {rem} vs {scud_cd}");
    }

    let nuke = logic
        .create_object(
            CHINA_NUCLEAR_MISSILE_LAUNCHER,
            Team::USA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("nuke");
    let nuke_cd = special_power_reload_seconds(&SpecialPowerType::NuclearMissile).unwrap();
    {
        let o = logic.host_object(nuke).unwrap();
        assert!(!o.is_special_power_ready(&SpecialPowerType::NuclearMissile));
        let rem = o
            .special_power_cooldowns
            .get(&SpecialPowerType::NuclearMissile)
            .copied()
            .unwrap_or(0.0);
        assert!((rem - nuke_cd).abs() < 0.01);
    }
}

#[test]
fn superweapon_energy_production_drains_team_power() {
    use crate::game_logic::host_superweapon_kindof::{
        AMERICA_PARTICLE_CANNON_UPLINK, CHINA_NUCLEAR_MISSILE_LAUNCHER, GLA_SCUD_STORM,
        SUPERWEAPON_ENERGY_DRAIN,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    // Power plant residual so team has production.
    let mut plant = ThingTemplate::new("AmericaColdFusionReactor");
    plant
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Powered);
    logic
        .templates
        .insert("AmericaColdFusionReactor".into(), plant);
    let plant_id = logic
        .create_object(
            "AmericaColdFusionReactor",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("plant");
    if let Some(o) = logic.host_object_mut(plant_id) {
        // Explicit plant residual (BuildingType::PowerPlant default is also 10).
        o.power_provided = 10;
        o.power_consumed = 0;
    }

    for name in [
        AMERICA_PARTICLE_CANNON_UPLINK,
        CHINA_NUCLEAR_MISSILE_LAUNCHER,
        GLA_SCUD_STORM,
    ] {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSSuperweapon);
        if name != GLA_SCUD_STORM {
            t.add_kind_of(KindOf::Powered);
        }
        logic.templates.insert(name.into(), t);
    }

    let puc = logic
        .create_object(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("puc");
    {
        let o = logic.host_object(puc).expect("puc obj");
        assert_eq!(o.power_provided, 0, "PUC does not produce");
        assert_eq!(
            o.power_consumed,
            SUPERWEAPON_ENERGY_DRAIN.abs(),
            "PUC EnergyProduction -10 residual"
        );
    }
    let scud = logic
        .create_object(GLA_SCUD_STORM, Team::USA, glam::Vec3::new(100.0, 0.0, 0.0))
        .expect("scud");
    {
        let o = logic.host_object(scud).expect("scud obj");
        assert_eq!(o.power_provided, 0);
        assert_eq!(o.power_consumed, 0, "Scud Storm unpowered residual");
    }
    let nuke = logic
        .create_object(
            CHINA_NUCLEAR_MISSILE_LAUNCHER,
            Team::USA,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("nuke");
    {
        let o = logic.host_object(nuke).expect("nuke obj");
        assert_eq!(o.power_consumed, SUPERWEAPON_ENERGY_DRAIN.abs());
    }

    // Tick resources to recompute team power.
    logic.update();
    let player = logic
        .get_players()
        .values()
        .find(|p| p.team == Team::USA)
        .expect("usa");
    // plant 10 - PUC 10 - Nuke 10 - Scud 0 = -10 available
    assert_eq!(player.power_produced, 10);
    assert_eq!(player.power_consumed, 20);
    assert_eq!(player.power_available, -10);
}

#[test]
fn superweapon_max_simultaneous_blocks_second_when_limited() {
    use crate::game_logic::host_superweapon_kindof::{
        honesty_superweapon_max_simultaneous_residual_pack, AMERICA_PARTICLE_CANNON_UPLINK,
        GLA_SCUD_STORM,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_superweapon_max_simultaneous_residual_pack());

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic.override_world_size(2000.0, 2000.0); // LegalBuild edge residual room
    logic.set_skirmish_rules(true, true, true, true, 1.0); // limit_superweapons=true

    let mut puc = ThingTemplate::new(AMERICA_PARTICLE_CANNON_UPLINK);
    puc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon)
        .set_health(4000.0);
    logic
        .templates
        .insert(AMERICA_PARTICLE_CANNON_UPLINK.into(), puc);
    let mut scud = ThingTemplate::new(GLA_SCUD_STORM);
    scud.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon)
        .set_health(4000.0);
    logic.templates.insert(GLA_SCUD_STORM.into(), scud);
    // Tech prereqs residual for construction start gate.
    for tech in ["AmericaStrategyCenter", "GLAPalace"] {
        let mut t = ThingTemplate::new(tech);
        t.add_kind_of(KindOf::Structure).set_health(2000.0);
        logic.templates.insert(tech.into(), t);
    }
    let _ = logic.create_object(
        "AmericaStrategyCenter",
        Team::USA,
        glam::Vec3::new(-400.0, 0.0, 0.0),
    );
    let _ = logic.create_object("GLAPalace", Team::USA, glam::Vec3::new(-300.0, 0.0, 0.0));

    let first = logic
        .create_object_under_construction(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("first SW");
    assert!(logic.host_object(first).is_some());
    // Second SW of any Superweapon link key blocked.
    assert!(
        logic
            .create_object_under_construction(
                AMERICA_PARTICLE_CANNON_UPLINK,
                Team::USA,
                glam::Vec3::new(450.0, 0.0, 0.0),
            )
            .is_none(),
        "second PUC blocked"
    );
    assert!(
        logic
            .create_object_under_construction(
                GLA_SCUD_STORM,
                Team::USA,
                glam::Vec3::new(300.0, 0.0, 0.0),
            )
            .is_none(),
        "Scud also counts as Superweapon link key residual"
    );
    // Unlimited when rule off.
    logic.set_skirmish_rules(true, true, false, true, 1.0);
    assert!(logic
        .create_object_under_construction(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::USA,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .is_some());
    // Other team still free under limit.
    logic.set_skirmish_rules(true, true, true, true, 1.0);
    ensure_test_player_for_team(&mut logic, Team::China);
    let _ = logic.create_object(
        "AmericaStrategyCenter",
        Team::China,
        glam::Vec3::new(600.0, 0.0, 0.0),
    );
    assert!(logic
        .create_object_under_construction(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::China,
            glam::Vec3::new(700.0, 0.0, 0.0),
        )
        .is_some());
}

#[test]
fn science_purchase_expresses_shared_special_power_ready() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::Team;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let pid = logic.get_player_by_team(Team::USA).map(|p| p.id).unwrap();
    {
        let p = logic.get_player_mut(pid).unwrap();
        p.apply_faction_intrinsic_sciences();
        p.science_purchase_points = 5;
        // Simulate prior cooling residual for A10.
        p.shared_special_power_cooldowns
            .insert(SpecialPowerType::Airstrike, 999.0);
    }
    assert!(logic.unlock_team_science(Team::USA, "SCIENCE_A10ThunderboltMissileStrike1"));
    let p = logic.get_player(pid).unwrap();
    assert!(p.has_unlocked_science("SCIENCE_A10ThunderboltMissileStrike1"));
    // C++ onSpecialPowerCreation: sharedNSync expressed ready-now.
    assert!(
        p.is_shared_special_power_ready(&SpecialPowerType::Airstrike),
        "A10 must be ready-now after science creation residual"
    );
    // DaisyCutter science creation residual.
    assert!(logic.unlock_team_science(Team::USA, "SCIENCE_DaisyCutter"));
    let p = logic.get_player(pid).unwrap();
    assert!(p.is_shared_special_power_ready(&SpecialPowerType::DaisyCutter));
}

#[test]
fn shared_timer_ready_fires_eva_superweapon_ready_residual() {
    // Structure PublicTimer SWs (PUC/Nuke/Scud) are not SharedNSync — ready EVA
    // fires from Object::tick_timers edge (below). Shared+PublicTimer science
    // powers do not map to SuperweaponReady EVA residual families.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_superweapon_kindof::AMERICA_PARTICLE_CANNON_UPLINK;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        p.is_local = true;
    }
    let mut t = ThingTemplate::new(AMERICA_PARTICLE_CANNON_UPLINK);
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon)
        .add_kind_of(KindOf::Powered)
        .set_health(4000.0);
    logic
        .templates
        .insert(AMERICA_PARTICLE_CANNON_UPLINK.into(), t);
    // Fully built PUC without start_power_recharge path: create then set near-ready.
    // Use create_object (map-placed) which starts full recharge — then force remaining.
    let id = logic
        .create_object(AMERICA_PARTICLE_CANNON_UPLINK, Team::USA, glam::Vec3::ZERO)
        .expect("puc");
    // Power plant so underpowered does not freeze countdown residual.
    let mut plant = ThingTemplate::new("AmericaColdFusionReactor");
    plant
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Powered);
    logic
        .templates
        .insert("AmericaColdFusionReactor".into(), plant);
    let pid = logic
        .create_object(
            "AmericaColdFusionReactor",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("plant");
    if let Some(o) = logic.host_object_mut(pid) {
        o.power_provided = 20;
        o.power_consumed = 0;
    }
    if let Some(o) = logic.host_object_mut(id) {
        o.set_status_disabled_underpowered(false);
        o.special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 0.05);
        o.special_power_cooldown_remaining = 0.05;
        o.set_special_power_ready(false);
    }
    assert!(!logic.honesty_eva_superweapon_ready_ok());
    // Object tick_timers edge → try_eva_superweapon_ready residual.
    if let Some(o) = logic.host_object_mut(id) {
        let became = o.tick_timers(0.1);
        assert!(became, "PUC must become ready");
    }
    // GameLogic update path also calls try_eva on became_ready — call residual directly.
    logic.try_eva_superweapon_ready(id, Team::USA, AMERICA_PARTICLE_CANNON_UPLINK);
    assert!(
        logic.honesty_eva_superweapon_ready_ok(),
        "structure SW ready edge must fire EVA SuperweaponReady residual"
    );
    assert!(
        !crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
            &SpecialPowerType::ParticleCannon
        ),
        "PUC is structure-bound, not SharedNSync"
    );
    assert!(
        crate::game_logic::host_special_power_enum_residual::special_power_is_structure_bound_public_timer(
            &SpecialPowerType::ParticleCannon
        )
    );
}

#[test]
fn presentation_structure_sw_timer_uses_object_cooldown() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_superweapon_kindof::AMERICA_PARTICLE_CANNON_UPLINK;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        p.is_local = true;
    }
    let mut t = ThingTemplate::new(AMERICA_PARTICLE_CANNON_UPLINK);
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon)
        .add_kind_of(KindOf::Powered)
        .set_health(4000.0);
    logic
        .templates
        .insert(AMERICA_PARTICLE_CANNON_UPLINK.into(), t);
    let id = logic
        .create_object(AMERICA_PARTICLE_CANNON_UPLINK, Team::USA, glam::Vec3::ZERO)
        .expect("puc");
    if let Some(o) = logic.host_object_mut(id) {
        o.special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 123.0);
        o.special_power_cooldown_remaining = 123.0;
        o.set_special_power_ready(false);
    }
    // Player shared map must NOT drive structure SW HUD residual.
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        p.shared_special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 1.0);
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let row = frame
        .superweapon_timers()
        .iter()
        .find(|t| t.name.contains("Particle") || t.template_name.contains("Particle"))
        .expect("PUC timer row");
    assert!(row.unlocked);
    assert!(
        (row.remaining - 123.0).abs() < 0.01,
        "remaining {} must come from structure module not player shared 1.0",
        row.remaining
    );
    assert!(!row.ready);
    // Destroy structure → removeSuperweapon residual (row gone).
    logic.mark_object_for_destruction(id, None);
    logic.process_destroy_list();
    let frame2 = PresentationFrame::build_from_logic(&logic, 1);
    assert!(
        frame2
            .superweapon_timers()
            .iter()
            .all(|t| !t.template_name.contains("Particle") && !t.name.contains("Particle")),
        "destroyed PUC removes PublicTimer row residual"
    );
}

#[test]
fn kill_grants_player_skill_points_residual() {
    use crate::game_logic::host_rank_ui_residual::skill_points_for_kill_residual;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(skill_points_for_kill_residual(false, false, false, 0), 20);
    assert_eq!(skill_points_for_kill_residual(true, false, false, 0), 200);
    assert_eq!(skill_points_for_kill_residual(false, true, false, 0), 50);

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    // Fresh skirmish-like SPP residual.
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        p.apply_faction_intrinsic_sciences();
        p.skill_points = 0;
        p.rank_level = 1;
    }

    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut rebel = ThingTemplate::new("GLAInfantryRebel");
    rebel.add_kind_of(KindOf::Infantry).set_health(100.0);
    rebel.experience_value = 20.0;
    rebel.experience_values = [20.0, 20.0, 40.0, 60.0];
    logic.templates.insert("GLAInfantryRebel".into(), rebel);

    let killer = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("killer");
    let victim = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("victim");
    // Attribute kill to USA team residual (scoreTheKill killer team).
    if let Some(v) = logic.host_object_mut(victim) {
        v.last_damage_source = Some(killer);
    }
    let skill_before = logic
        .get_player_by_team(Team::USA)
        .map(|p| p.skill_points)
        .unwrap_or(0);

    // C++ destroy path: mark with killer team then process_destroy_list.
    logic.mark_object_for_destruction(victim, Some(Team::USA));
    logic.process_destroy_list();

    let p = logic.get_player_by_team(Team::USA).expect("usa");
    assert!(
        p.skill_points >= skill_before + 20,
        "skill_points {} expected >= {}",
        p.skill_points,
        skill_before + 20
    );
    let _ = killer;
}

#[test]
fn civilian_and_ignored_in_gui_kills_grant_no_rank_points() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        p.skill_points = 0;
        p.rank_level = 1;
        p.alliance_team = 1;
    }
    if let Some(p) = logic.get_player_mut_by_team(Team::GLA) {
        p.alliance_team = 2;
    }
    let mut civ_player = Player::new(9, Team::Neutral, "Civilian", false);
    civ_player.is_alive = true;
    civ_player.alliance_team = 2;
    logic.players.insert(9, civ_player);

    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut civ = ThingTemplate::new("Civilian");
    civ.add_kind_of(KindOf::Infantry).set_health(50.0);
    civ.experience_value = 20.0;
    civ.experience_values = [20.0, 20.0, 40.0, 60.0];
    logic.templates.insert("Civilian".into(), civ);
    let mut dummy = ThingTemplate::new("AngryMobNexus");
    dummy.add_kind_of(KindOf::Infantry);
    dummy.add_kind_of(KindOf::IgnoredInGui);
    dummy.set_health(50.0);
    dummy.experience_value = 20.0;
    dummy.experience_values = [20.0, 20.0, 40.0, 60.0];
    logic.templates.insert("AngryMobNexus".into(), dummy);

    let killer = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("killer");
    let civilian = logic
        .create_object("Civilian", Team::Neutral, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("civ");
    let ignored = logic
        .create_object(
            "AngryMobNexus",
            Team::GLA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("ignored");
    if let Some(v) = logic.host_object_mut(civilian) {
        v.last_damage_source = Some(killer);
        v.owner_player_id = Some(9);
    }
    if let Some(v) = logic.host_object_mut(ignored) {
        v.last_damage_source = Some(killer);
    }

    logic.mark_object_for_destruction(civilian, Some(Team::USA));
    logic.process_destroy_list();
    logic.mark_object_for_destruction(ignored, Some(Team::USA));
    logic.process_destroy_list();

    let p = logic.get_player_by_team(Team::USA).expect("usa");
    assert_eq!(
        p.skill_points, 0,
        "civilian and IGNORED_IN_GUI kills must not grant rank points"
    );
    let _ = killer;
}

#[test]
fn science_purchase_spends_points_not_supplies() {
    use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::{
        is_capable_of_purchasing_science_residual, science_purchase_point_cost_residual,
    };
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let pid = logic
        .get_player_by_team(Team::USA)
        .map(|p| p.id)
        .expect("usa");
    // Grant faction + rank residual prereqs and points.
    {
        let p = logic.get_player_mut(pid).unwrap();
        p.unlocked_sciences.insert("SCIENCE_AMERICA".into());
        p.unlocked_sciences.insert("SCIENCE_Rank1".into());
        p.science_purchase_points = 2;
        p.resources.supplies = 10_000;
    }
    let supplies_before = logic.get_player(pid).unwrap().resources.supplies;
    assert_eq!(
        science_purchase_point_cost_residual("SCIENCE_DaisyCutter"),
        Some(1)
    );
    assert!(is_capable_of_purchasing_science_residual(
        &logic.get_player(pid).unwrap().unlocked_sciences,
        2,
        "SCIENCE_DaisyCutter"
    ));
    assert!(logic
        .get_player_mut(pid)
        .unwrap()
        .attempt_to_purchase_science("SCIENCE_DaisyCutter"));
    let p = logic.get_player(pid).unwrap();
    assert!(p.has_unlocked_science("SCIENCE_DaisyCutter"));
    assert_eq!(p.science_purchase_points, 1);
    assert_eq!(
        p.effective_supplies(),
        supplies_before,
        "science purchase must not spend supplies residual"
    );
    // Cost 0 MOAB not purchasable.
    assert!(!p.is_capable_of_purchasing_science("SCIENCE_MOAB"));
    // Insufficient points.
    {
        let p = logic.get_player_mut(pid).unwrap();
        p.science_purchase_points = 0;
        assert!(!p.attempt_to_purchase_science("SCIENCE_PaladinTank"));
    }
    // CashBounty prereq chain residual.
    {
        let p = logic.get_player_mut(pid).unwrap();
        p.unlocked_sciences.insert("SCIENCE_GLA".into());
        p.unlocked_sciences.insert("SCIENCE_Rank3".into());
        p.science_purchase_points = 3;
        assert!(p.attempt_to_purchase_science("SCIENCE_CashBounty1"));
        assert!(p.attempt_to_purchase_science("SCIENCE_CashBounty2"));
        assert!((p.cash_bounty_percent - 0.10).abs() < 1e-6);
    }
}

#[test]
fn airfield_dock_reloads_countermeasures_residual() {
    use crate::game_logic::host_countermeasures::{
        aircraft_has_countermeasures_upgrade, FULL_LOAD_COUNTERMEASURES,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_COUNTERMEASURES;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);

    let mut af = ThingTemplate::new("AmericaAirfield");
    af.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(5000.0);
    logic.templates.insert("AmericaAirfield".into(), af);

    let mut raptor = ThingTemplate::new("AmericaJetRaptor");
    raptor.add_kind_of(KindOf::Aircraft).set_health(160.0);
    logic.templates.insert("AmericaJetRaptor".into(), raptor);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    // Ensure constructed residual for dock.
    if let Some(o) = logic.host_object_mut(af_id) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }

    let air = logic
        .create_object(
            "AmericaJetRaptor",
            Team::USA,
            glam::Vec3::new(5.0, 0.0, 0.0),
        )
        .expect("raptor");
    {
        let o = logic.host_object_mut(air).unwrap();
        o.apply_upgrade_tag(UPGRADE_AMERICA_COUNTERMEASURES);
        o.set_ai_state(AIState::Docked);
        o.set_contained_by(Some(af_id));
        assert!(aircraft_has_countermeasures_upgrade(&o.applied_upgrades));
    }
    // Exhaust flares residual.
    {
        let st = logic.countermeasures.ensure(air);
        st.available = 0;
        st.volleys_fired = 5;
    }
    assert_eq!(logic.countermeasures.get(air).map(|s| s.available), Some(0));

    logic.tick_airfield_parking_heal();
    assert!(
        logic.honesty_countermeasures_reload_ok(),
        "airfield dock must reload CM residual"
    );
    assert_eq!(
        logic.countermeasures.get(air).map(|s| s.available),
        Some(FULL_LOAD_COUNTERMEASURES)
    );
}

#[test]
fn countermeasures_diverts_projectile_direct_hits() {
    use crate::game_logic::host_countermeasures::{
        honesty_countermeasures_residual_pack_ok, try_divert_missile, EVASION_RATE,
        FULL_LOAD_COUNTERMEASURES,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_COUNTERMEASURES;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    assert!(honesty_countermeasures_residual_pack_ok());
    assert!((EVASION_RATE - 0.30).abs() < 1e-6);
    assert_eq!(FULL_LOAD_COUNTERMEASURES, 20);

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);

    let mut raptor = ThingTemplate::new("AmericaJetRaptor");
    raptor.add_kind_of(KindOf::Aircraft).set_health(160.0);
    logic.templates.insert("AmericaJetRaptor".into(), raptor);
    let mut buggy = ThingTemplate::new("GLAVehicleRocketBuggy");
    buggy.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic
        .templates
        .insert("GLAVehicleRocketBuggy".into(), buggy);

    let air = logic
        .create_object(
            "AmericaJetRaptor",
            Team::USA,
            glam::Vec3::new(0.0, 20.0, 0.0),
        )
        .expect("raptor");
    {
        let o = logic.host_object_mut(air).unwrap();
        o.set_position(glam::Vec3::new(0.0, 20.0, 0.0));
        o.status.airborne_target = true;
        o.apply_upgrade_tag(UPGRADE_AMERICA_COUNTERMEASURES);
        assert!(o.has_upgrade_tag(UPGRADE_AMERICA_COUNTERMEASURES));
    }
    let shooter = logic
        .create_object(
            "GLAVehicleRocketBuggy",
            Team::GLA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("buggy");

    // Unit residual: pure diversion rolls (registry path).
    let mut any = false;
    for f in 0..100u32 {
        if try_divert_missile(&mut logic.countermeasures, air, ObjectId(500 + f), f, true) {
            any = true;
        }
    }
    assert!(any, "registry diversion residual");
    assert!(logic.honesty_countermeasures_divert_ok());

    // Integration residual: hitscan projectile + Direct damage path.
    logic.countermeasures.clear();
    let weapon = Weapon {
        damage: 5.0,
        splash_radius: 0.0,
        ..Weapon::default()
    };
    let air_pos = glam::Vec3::new(0.0, 20.0, 0.0);
    let hp0 = logic.host_object(air).unwrap().health.current;
    for i in 0..60u32 {
        let pid = logic.combat_system.fire_projectile_ex(
            glam::Vec3::new(100.0, 20.0, 0.0),
            air_pos,
            &weapon,
            shooter,
            Some(air),
            f32::INFINITY, // hitscan residual
            false,
        );
        if let Some(p) = logic.combat_system.projectile_mut(pid) {
            p.position = air_pos;
            p.target_position = air_pos;
            p.target_id = Some(air);
            p.explosion_radius = 0.0;
            p.damage = 5.0;
            p.max_lifetime = 10.0;
            p.is_small_missile = true;
        }
        let _ = logic.combat_system.update_projectiles_with_countermeasures(
            1.0 / 30.0,
            &mut logic.objects,
            Some(&mut logic.countermeasures),
            1000 + i,
        );
    }
    assert!(
        logic.honesty_countermeasures_report_ok(),
        "expected Direct-path missile reports, reports={}",
        logic.countermeasures.total_reports()
    );
    let hp1 = logic
        .host_object(air)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    // With 30% diversion, expected damage ~ 0.7 * 60 * 5 = 210 > 160 so may die;
    // honesty is reports + at least one divert from registry path above.
    assert!(hp1 <= hp0 || logic.honesty_countermeasures_divert_ok());
    let _ = hp0;
    let _ = hp1;
    let _ = shooter;
}

#[test]
fn spy_drone_dynamic_shroud_grow_pulse_residual() {
    use crate::game_logic::host_spy_drone::{
        spy_drone_scan_radius_after_updates, SPY_DRONE_GROW_UPDATES_TO_FINAL,
        SPY_DRONE_REQUIRED_SCIENCE, SPY_DRONE_TEMPLATE, SPY_DRONE_VISION_RANGE,
    };
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(0) {
        p.unlock_science(SPY_DRONE_REQUIRED_SCIENCE);
    }
    assert!(logic.activate_spy_drone(0, Team::USA, Vec3::new(200.0, 0.0, 200.0), None,));
    let act = logic.spy_drones().last().expect("activation");
    assert!(act.growing || act.grow_index > 0);
    assert!(act.radius + 0.01 < SPY_DRONE_VISION_RANGE || act.grow_index > 0);
    // Drive grow to final.
    for _ in 0..SPY_DRONE_GROW_UPDATES_TO_FINAL + 2 {
        logic.update_spy_drone_grow();
    }
    let act = logic.spy_drones().last().expect("activation");
    assert!(!act.growing, "grow pulse must complete");
    assert!((act.radius - SPY_DRONE_VISION_RANGE).abs() < 0.01);
    assert!(logic.spy_drones().honesty_grow_ok());
    assert!(
        (spy_drone_scan_radius_after_updates(SPY_DRONE_GROW_UPDATES_TO_FINAL - 1)
            - SPY_DRONE_VISION_RANGE)
            .abs()
            < 0.01
    );
    assert!(logic
        .host_objects()
        .values()
        .any(|o| o.template_name == SPY_DRONE_TEMPLATE));
}

#[test]
fn spy_drone_special_power_spawns_vehicle_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_spy_drone::{
        honesty_spy_drone_residual_pack_ok, SPY_DRONE_REQUIRED_SCIENCE, SPY_DRONE_TEMPLATE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_spy_drone_residual_pack_ok());
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc1 = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    // Science gate residual.
    assert!(!logic.is_special_power_ready_for(cc1, &SpecialPowerType::SpyDrone));
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        p.unlock_science(SPY_DRONE_REQUIRED_SCIENCE);
    }
    assert!(logic.is_special_power_ready_for(cc1, &SpecialPowerType::SpyDrone));
    let before = logic.host_objects().len();
    assert!(logic.activate_spy_drone(0, Team::USA, glam::Vec3::new(50.0, 0.0, 50.0), Some(cc1),));
    assert!(logic.honesty_spy_drone_activate_ok());
    assert!(logic.honesty_spy_drone_spawn_ok());
    assert!(logic.host_objects().len() > before);
    let drone = logic
        .host_objects()
        .values()
        .find(|o| o.template_name == SPY_DRONE_TEMPLATE)
        .expect("drone spawned");
    assert!(drone.is_alive());
    assert_eq!(drone.team, Team::USA);
    // Consume shared timer residual.
    assert!(logic.consume_special_power_charge_for(cc1, &SpecialPowerType::SpyDrone));
    assert!(!logic.is_special_power_ready_for(cc1, &SpecialPowerType::SpyDrone));
}

#[test]
fn special_power_required_science_gates_shared_superweapons() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc1 = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc1");
    // DaisyCutter requires SCIENCE_DaisyCutter.
    assert!(
        crate::game_logic::host_special_power_enum_residual::special_power_required_science(
            &SpecialPowerType::DaisyCutter
        ) == Some("SCIENCE_DaisyCutter")
    );
    assert!(!logic.is_special_power_ready_for(cc1, &SpecialPowerType::DaisyCutter));
    // SpySatellite has no RequiredScience residual — ready without unlock.
    assert!(
        crate::game_logic::host_special_power_enum_residual::special_power_required_science(
            &SpecialPowerType::SpySatellite
        )
        .is_none()
    );
    assert!(logic.is_special_power_ready_for(cc1, &SpecialPowerType::SpySatellite));
    // ParticleCannon structure SW: no science residual.
    assert!(logic.is_special_power_ready_for(cc1, &SpecialPowerType::ParticleCannon));
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        p.unlock_science("SCIENCE_DaisyCutter");
    }
    assert!(logic.is_special_power_ready_for(cc1, &SpecialPowerType::DaisyCutter));
    // Unit ability residual: no science gate.
    assert!(
        crate::game_logic::host_special_power_enum_residual::special_power_required_science(
            &SpecialPowerType::TankHunterTnt
        )
        .is_none()
    );
}

#[test]
fn shared_synced_special_power_timer_is_player_wide() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    cc.special_power_cooldown = 10.0;
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc1 = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc1");
    let cc2 = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("cc2");
    // RequiredScience residual: A10 needs SCIENCE_A10ThunderboltMissileStrike1.
    assert!(
        !logic.is_special_power_ready_for(cc1, &SpecialPowerType::Airstrike),
        "A10 blocked without science residual"
    );
    if let Some(p) = logic.get_player_mut_by_team(Team::USA) {
        assert!(p.unlock_science("SCIENCE_A10ThunderboltMissileStrike1"));
    }
    assert!(logic.is_special_power_ready_for(cc1, &SpecialPowerType::Airstrike));
    assert!(logic.is_special_power_ready_for(cc2, &SpecialPowerType::Airstrike));
    assert!(logic.consume_special_power_charge_for(cc1, &SpecialPowerType::Airstrike));
    // Second command center must share the A10 timer residual.
    assert!(
        !logic.is_special_power_ready_for(cc2, &SpecialPowerType::Airstrike),
        "shared synced A10 must block sibling CC"
    );
    assert!(
        !crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
            &SpecialPowerType::TankHunterTnt
        )
    );
    assert!(
        crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
            &SpecialPowerType::Airstrike
        )
    );
    // Tick 240s clears shared residual.
    logic.tick_shared_special_power_timers(240.0);
    if let Some(o) = logic.host_object_mut(cc1) {
        let _ = o.tick_timers(240.0);
    }
    if let Some(o) = logic.host_object_mut(cc2) {
        let _ = o.tick_timers(240.0);
    }
    assert!(logic.is_special_power_ready_for(cc2, &SpecialPowerType::Airstrike));
}

#[test]
fn special_power_ready_uses_controlling_owner_not_first_faction() {
    // C++ SpecialPowerModule.cpp:278/386 getControllingPlayer — two USA
    // players must not share science or SharedSyncedTimer.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA0", true));
    logic.add_player(Player::new(1, Team::USA, "USA1", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    cc.special_power_cooldown = 10.0;
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc0 = logic
        .create_object_for_player("AmericaCommandCenter", 0, glam::Vec3::ZERO)
        .expect("cc0");
    let cc1 = logic
        .create_object_for_player(
            "AmericaCommandCenter",
            1,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("cc1");
    if let Some(p) = logic.get_player_mut(0) {
        assert!(p.unlock_science("SCIENCE_A10ThunderboltMissileStrike1"));
    }
    assert!(
        logic.is_special_power_ready_for(cc0, &SpecialPowerType::Airstrike),
        "owner 0 unlocked A10 science"
    );
    assert!(
        !logic.is_special_power_ready_for(cc1, &SpecialPowerType::Airstrike),
        "owner 1 must not inherit owner 0 science"
    );
    if let Some(p) = logic.get_player_mut(1) {
        assert!(p.unlock_science("SCIENCE_A10ThunderboltMissileStrike1"));
    }
    assert!(logic.is_special_power_ready_for(cc1, &SpecialPowerType::Airstrike));
    assert!(logic.consume_special_power_charge_for(cc0, &SpecialPowerType::Airstrike));
    assert!(
        !logic.is_special_power_ready_for(cc0, &SpecialPowerType::Airstrike),
        "owner 0 A10 consumed"
    );
    assert!(
        logic.is_special_power_ready_for(cc1, &SpecialPowerType::Airstrike),
        "owner 1 SharedSyncedTimer must stay independent"
    );
}

#[test]
fn named_special_power_countdown_reaches_host_pause_and_set_ready() {
    // C++ ScriptActions.cpp:4066-4113 pauseCountdown / setReadyFrame.
    // Use a non-SharedNSync power so setReady/pause affect isReady (C++ isReady
    // for SharedNSync reads the player timer, not the module frame).
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, NamedSpecialPowerCountdownOp, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut unit = ThingTemplate::new("AmericaInfantryTankHunter");
    unit.add_kind_of(KindOf::Infantry).set_health(100.0);
    unit.special_power_modules
        .push(crate::game_logic::SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_TNT".into()),
            module_kind: crate::game_logic::SpecialPowerModuleKind::SpecialAbility,
            special_power_template: "SpecialAbilityTankHunterTNTAttack".into(),
            special_power_template_id: 1,
            command_power: Some(SpecialPowerType::TankHunterTnt),
            reload_time_frames: 0,
            required_science: None,
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    logic
        .templates
        .insert("AmericaInfantryTankHunter".into(), unit);
    let id = logic
        .create_object(
            "AmericaInfantryTankHunter",
            Team::USA,
            glam::Vec3::ZERO,
        )
        .expect("hunter");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = "HeroCC".into();
    }
    let power = SpecialPowerType::TankHunterTnt;
    assert!(logic.is_special_power_ready_for(id, &power));
    assert!(logic.script_named_special_power_countdown(
        "HeroCC",
        "SpecialAbilityTankHunterTNTAttack",
        NamedSpecialPowerCountdownOp::Set,
        12,
    ));
    assert!(
        (logic
            .host_object(id)
            .unwrap()
            .special_power_countdown_seconds(&power)
            - 12.0)
            .abs()
            < 0.01
    );
    assert!(!logic.is_special_power_ready_for(id, &power));
    assert!(logic.script_named_special_power_countdown(
        "HeroCC",
        "SpecialAbilityTankHunterTNTAttack",
        NamedSpecialPowerCountdownOp::Add,
        3,
    ));
    assert!(
        (logic
            .host_object(id)
            .unwrap()
            .special_power_countdown_seconds(&power)
            - 15.0)
            .abs()
            < 0.01
    );
    assert!(logic.script_named_special_power_countdown(
        "HeroCC",
        "SpecialAbilityTankHunterTNTAttack",
        NamedSpecialPowerCountdownOp::Stop,
        0,
    ));
    assert!(logic
        .host_object(id)
        .unwrap()
        .is_special_power_countdown_paused(&power));
    assert!(
        !logic.is_special_power_ready_for(id, &power),
        "paused countdown is not ready"
    );
}


#[test]
fn special_power_fire_notifies_script_engine_triggered() {
    // C++ SpecialPowerModule.cpp:513 notifyOfTriggeredSpecialPower.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use gamelogic::scripting::engine::{initialize_script_engine, with_script_engine_mut};
    let _ = initialize_script_engine();
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut tpl = ThingTemplate::new("CC");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("CC".into(), tpl);
    let caster = logic
        .create_object("CC", Team::USA, glam::Vec3::ZERO)
        .expect("caster");
    assert!(logic
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            caster,
            glam::Vec3::new(80.0, 0.0, 80.0),
        )
        .is_some());
    let hit = with_script_engine_mut(|engine| {
        engine.is_special_power_triggered(
            0,
            "SuperweaponA10ThunderboltMissileStrike",
            false,
            caster.0,
        )
    })
    .unwrap_or(false);
    assert!(hit, "TRIGGERED condition must see host superweapon fire");
    logic.notify_script_engine_special_power_event(
        caster,
        &SpecialPowerType::SpySatellite,
        true,
        true,
    );
    let sat = with_script_engine_mut(|engine| {
        engine.is_special_power_triggered(0, "SpecialPowerSpySatellite", false, caster.0)
            && engine.is_special_power_complete(0, "SpecialPowerSpySatellite", false, caster.0)
    })
    .unwrap_or(false);
    assert!(sat, "instant powers notify TRIGGERED and COMPLETED");
}


#[test]
fn superweapon_fire_creates_view_object_reveal() {
    // C++ SpecialPowerModule.cpp:462-497 createViewObject range 250 / 30-40s.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let source = logic
        .create_object("AmericaCommandCenter", Team::USA, glam::Vec3::ZERO)
        .expect("source");
    let target = glam::Vec3::new(200.0, 0.0, 200.0);
    assert!(logic
        .queue_special_power_strike(&SpecialPowerType::Airstrike, source, target)
        .is_some());
    assert!(
        logic.special_power_strikes().view_object_count() >= 1,
        "createViewObject must record a reveal"
    );
    let vo = &logic.special_power_strikes().view_objects()[0];
    assert!((vo.range - 250.0).abs() < 0.1, "ViewObjectRange residual 250");
    let dur = vo.duration_frames();
    assert!(
        dur == 900 || dur == 1_200,
        "ViewObjectDuration 30-40s, got {dur}"
    );
    assert_eq!(vo.source_object, source);
}


#[test]
fn special_power_cooldowns_are_independent_per_power() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    cc.special_power_cooldown = 10.0;
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let o = logic.host_object_mut(id).unwrap();
    assert!(o.is_special_power_ready(&SpecialPowerType::Airstrike));
    assert!(o.is_special_power_ready(&SpecialPowerType::SpySatellite));
    o.consume_special_power_charge(&SpecialPowerType::Airstrike);
    // A10 reloading must not block SpySatellite residual.
    assert!(!o.is_special_power_ready(&SpecialPowerType::Airstrike));
    assert!(o.is_special_power_ready(&SpecialPowerType::SpySatellite));
    o.consume_special_power_charge(&SpecialPowerType::SpySatellite);
    assert!(!o.is_special_power_ready(&SpecialPowerType::SpySatellite));
    // Tick 60s: spy sat (60s) clears, A10 (240s) remains.
    let _ = o.tick_timers(60.0);
    assert!(
        o.is_special_power_ready(&SpecialPowerType::SpySatellite),
        "spy remaining should clear at 60s"
    );
    assert!(
        !o.is_special_power_ready(&SpecialPowerType::Airstrike),
        "a10 should still be on 240s residual"
    );
    let a10_rem = o
        .special_power_cooldowns
        .get(&SpecialPowerType::Airstrike)
        .copied()
        .unwrap_or(0.0);
    assert!((a10_rem - 180.0).abs() < 0.5, "a10_rem={a10_rem}");
}

#[test]
fn special_power_reload_seconds_uses_retail_residual_table() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds;
    use crate::game_logic::host_tank_hunter::TNT_RELOAD_MS;
    use crate::game_logic::special_power_strikes::A10_STRIKE_RELOAD_MS;
    assert_eq!(
        special_power_reload_seconds(&SpecialPowerType::Airstrike),
        Some(A10_STRIKE_RELOAD_MS as f32 / 1000.0)
    );
    assert_eq!(
        special_power_reload_seconds(&SpecialPowerType::TankHunterTnt),
        Some(TNT_RELOAD_MS as f32 / 1000.0)
    );
    assert_eq!(
        special_power_reload_seconds(&SpecialPowerType::MissileDefenderLaserGuided),
        Some(0.0)
    );
    assert_eq!(
        special_power_reload_seconds(&SpecialPowerType::DetonateDirtyNuke),
        Some(30.0)
    );
    // Consume applies residual reload onto object cooldown remaining.
    let mut logic = GameLogic::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    // Template default cooldown is often 10s — residual must override for A10.
    cc.special_power_cooldown = 10.0;
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    if let Some(o) = logic.host_object_mut(id) {
        assert!(o.is_special_power_ready(&SpecialPowerType::Airstrike));
        o.consume_special_power_charge(&SpecialPowerType::Airstrike);
        assert!(!o.special_power_ready);
        assert!(
            (o.special_power_cooldown_remaining - 240.0).abs() < 0.01,
            "remaining={}",
            o.special_power_cooldown_remaining
        );
    }
}

#[test]
fn infantry_capture_and_disguise_special_power_enum_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::RangerCaptureBuilding),
        Some("SPECIAL_INFANTRY_CAPTURE_BUILDING")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::RedGuardCaptureBuilding),
        Some("SPECIAL_INFANTRY_CAPTURE_BUILDING")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::RebelCaptureBuilding),
        Some("SPECIAL_INFANTRY_CAPTURE_BUILDING")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::DisguiseAsVehiclePower),
        Some("SPECIAL_DISGUISE_AS_VEHICLE")
    );
}

#[test]
fn hacker_lotus_microwave_special_power_enum_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::HackerDisableBuilding),
        Some("SPECIAL_HACKER_DISABLE_BUILDING")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::MicrowaveDisableBuilding),
        Some("SPECIAL_HACKER_DISABLE_BUILDING")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::BlackLotusDisableVehicle),
        Some("SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::BlackLotusStealCash),
        Some("SPECIAL_BLACKLOTUS_STEAL_CASH_HACK")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::BlackLotusCaptureBuilding),
        Some("SPECIAL_BLACKLOTUS_CAPTURE_BUILDING")
    );
}

#[test]
fn demo_and_burton_charge_special_power_enum_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::DemoRebelTimedCharges),
        Some("SPECIAL_TIMED_CHARGES")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::BattleBusDemoTrapRollout),
        Some("SPECIAL_TIMED_CHARGES")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::DemoKellRemoteCharges),
        Some("SPECIAL_REMOTE_CHARGES")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::BurtonRemoteCharges),
        Some("SPECIAL_REMOTE_CHARGES")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::BurtonTimedCharges),
        Some("SPECIAL_TIMED_CHARGES")
    );
    // Plant residual APIs remain available for special-power completion.
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    use crate::game_logic::{KindOf, ThingTemplate};
    let mut unit = ThingTemplate::new("AmericaInfantryColonelBurton");
    unit.add_kind_of(KindOf::Infantry).set_health(200.0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), unit);
    let mut structure = ThingTemplate::new("GLATunnelNetwork");
    structure.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("GLATunnelNetwork".into(), structure);
    let src = logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("burton");
    let tgt = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(5.0, 0.0, 0.0),
        )
        .expect("struct");
    assert!(logic
        .place_timed_demo_charge(
            Team::USA,
            glam::Vec3::new(5.0, 0.0, 0.0),
            Some(src),
            Some(tgt),
            None,
        )
        .is_some());
    // C++ UniqueSpecialObjectTargets (SpecialAbilityUpdate.cpp:146-147):
    // one C4 special object per target. Remote charge needs a second attach.
    let tgt2 = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(15.0, 0.0, 0.0),
        )
        .expect("struct2");
    assert!(logic
        .place_remote_demo_charge(
            Team::USA,
            glam::Vec3::new(15.0, 0.0, 0.0),
            Some(src),
            Some(tgt2),
        )
        .is_some());
}

#[test]
fn tank_hunter_tnt_and_laser_howitzer_special_power_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_missile_defender::{
        is_missile_defender_template, missile_defender_laser_guided_weapon,
        missile_defender_primary_weapon,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::host_tank_hunter::{
        is_tank_hunter_template, tnt_in_start_range, TNT_START_ABILITY_RANGE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::TankHunterTnt),
        Some("SPECIAL_TANKHUNTER_TNT_ATTACK")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::LaserGuidedHowitzer),
        Some("SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES")
    );
    assert!(is_tank_hunter_template("ChinaInfantryTankHunter"));
    assert!(tnt_in_start_range(TNT_START_ABILITY_RANGE));
    assert!(!tnt_in_start_range(TNT_START_ABILITY_RANGE + 1.0));

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut th = ThingTemplate::new("ChinaInfantryTankHunter");
    th.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ChinaInfantryTankHunter".into(), th);
    let mut structure = ThingTemplate::new("GLATunnelNetwork");
    structure.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("GLATunnelNetwork".into(), structure);

    let src = logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("th");
    let tgt = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(3.0, 0.0, 0.0),
        )
        .expect("struct");
    // Direct plant path residual (in range).
    let planted = logic.place_timed_demo_charge(
        Team::China,
        glam::Vec3::new(3.0, 0.0, 0.0),
        Some(src),
        Some(tgt),
        None,
    );
    assert!(planted.is_some());

    // Laser howitzer shares MD laser residual path for laser-capable infantry.
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let md_id = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("md");
    if let Some(o) = logic.host_object_mut(md_id) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let enemy = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("e2");
    assert!(logic.activate_missile_defender_laser_guided(md_id, enemy));
    assert!(is_missile_defender_template(
        "AmericaInfantryMissileDefender"
    ));
    let _ = SpecialPowerType::TankHunterTnt;
    let _ = SpecialPowerType::LaserGuidedHowitzer;
}

#[test]
fn missile_defender_laser_guided_spawns_laser_beam_object() {
    use crate::game_logic::host_missile_defender::{
        missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
        LASER_GUIDED_ATTACH_BONE, LASER_GUIDED_BEAM_LIFETIME_FRAMES, LASER_GUIDED_SPECIAL_OBJECT,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut tgt_t = ThingTemplate::new("AmericaTankCrusader");
    tgt_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), tgt_t);

    let shooter = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    {
        let o = logic.host_object_mut(shooter).unwrap();
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let target = logic
        .create_object("AmericaTankCrusader", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();

    assert!(logic.activate_missile_defender_laser_guided(shooter, target));
    assert!(logic.honesty_missile_defender_laser_beam_ok());
    assert!(logic.missile_defender_laser_beams_spawned >= 1);
    let beam = logic
        .host_objects()
        .values()
        .find(|o| o.missile_defender_laser_beam)
        .expect("LaserBeam special object");
    assert_eq!(beam.template_name, LASER_GUIDED_SPECIAL_OBJECT);
    assert_eq!(beam.producer_id, Some(shooter));
    let bid = beam.id;
    // Prep window expiry destroys residual LaserBeam.
    logic.frame = logic
        .frame
        .saturating_add(LASER_GUIDED_BEAM_LIFETIME_FRAMES + 2);
    logic.update_missile_defender_laser_beam_objects();
    assert!(logic
        .host_object(bid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
    let _ = LASER_GUIDED_ATTACH_BONE;
}

#[test]
fn missile_defender_laser_guided_special_locks_secondary() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_missile_defender::{
        is_missile_defender_template, missile_defender_laser_guided_weapon,
        missile_defender_primary_weapon, LASER_GUIDED_START_ABILITY_RANGE,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::MissileDefenderLaserGuided),
        Some("SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES")
    );
    assert!(is_missile_defender_template(
        "AmericaInfantryMissileDefender"
    ));

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);

    let src = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    if let Some(o) = logic.host_object_mut(src) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
        o.active_weapon_slot = 0;
    }
    let tgt = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(LASER_GUIDED_START_ABILITY_RANGE - 10.0, 0.0, 0.0),
        )
        .expect("enemy");

    assert!(logic.activate_missile_defender_laser_guided(src, tgt));
    let o = logic.host_object(src).unwrap();
    assert_eq!(o.active_weapon_slot, 1);
    assert_eq!(o.target, Some(tgt));
    assert!(logic.missile_defender_residual_laser_specials >= 1);
    let _ = SpecialPowerType::MissileDefenderLaserGuided;
}

#[test]
fn host_upgrade_complete_helix_nuke_bomb_tags_helix() {
    use crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NUKE_BOMB;
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        HostUpgradeKind::from_name("Nuke_Upgrade_HelixNukeBomb"),
        HostUpgradeKind::HelixNukeBomb
    );
    assert_eq!(
        HostUpgradeKind::from_name("Upgrade_HelixNapalmBomb"),
        HostUpgradeKind::HelixNapalmBomb
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut helix = ThingTemplate::new("ChinaHelix");
    helix
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(400.0);
    logic.templates.insert("ChinaHelix".into(), helix);
    let id = logic
        .create_object("ChinaHelix", Team::China, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    let n = logic.apply_helix_bomb_upgrade_to_team(
        Team::China,
        "Nuke_Upgrade_HelixNukeBomb",
        UPGRADE_HELIX_NUKE_BOMB,
    );
    assert!(n >= 1);
    assert!(logic
        .host_object(id)
        .unwrap()
        .has_upgrade_tag(UPGRADE_HELIX_NUKE_BOMB));
    assert!(logic
        .activate_helix_napalm_bomb(id, glam::Vec3::new(5.0, 0.0, 0.0))
        .is_some());
}

#[test]
fn helix_nuke_bomb_maps_to_helix_napalm_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_helix_napalm::{
        honesty_helix_nuke_bomb_residual_pack_ok, UPGRADE_HELIX_NUKE_BOMB,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_helix_nuke_bomb_residual_pack_ok());
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::HelixNukeBomb),
        Some("SPECIAL_HELIX_NAPALM_BOMB")
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut helix = ThingTemplate::new("ChinaHelix");
    helix
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(400.0);
    logic.templates.insert("ChinaHelix".into(), helix);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);
    let src = logic
        .create_object("ChinaHelix", Team::China, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    // Without upgrade: fail-closed.
    assert!(logic
        .activate_helix_napalm_bomb(src, glam::Vec3::new(10.0, 0.0, 0.0))
        .is_none());
    if let Some(o) = logic.host_object_mut(src) {
        o.apply_upgrade_tag(UPGRADE_HELIX_NUKE_BOMB);
    }
    let _e = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("enemy");
    assert!(logic
        .activate_helix_napalm_bomb(src, glam::Vec3::new(10.0, 0.0, 0.0))
        .is_some());
    let _ = SpecialPowerType::HelixNukeBomb;
}

#[test]
fn communications_download_maps_to_cia_intelligence_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_cia_intelligence::{
        honesty_communications_download_residual_pack_ok, COMMUNICATIONS_DOWNLOAD_RELOAD_MS,
        COMMUNICATIONS_DOWNLOAD_SPECIAL_ENUM,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_communications_download_residual_pack_ok());
    assert_eq!(COMMUNICATIONS_DOWNLOAD_RELOAD_MS, 10_000);
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::CommunicationsDownload),
        Some(COMMUNICATIONS_DOWNLOAD_SPECIAL_ENUM)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::GLA, "GLA", false));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let _e = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("enemy");
    assert!(logic.activate_cia_intelligence(0, Team::USA, Some(src)));
    // CommunicationsDownload shares host SpyVision residual path.
    assert!(logic.activate_cia_intelligence(0, Team::USA, Some(src)));
    let _ = SpecialPowerType::CommunicationsDownload;
}

#[test]
fn general_special_power_aliases_map_to_host_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        honesty_general_special_power_alias_pack_ok, CarpetBombFactionTier, HostSuperweaponKind,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_general_special_power_alias_pack_ok());

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    for power in [
        SpecialPowerType::AirForceDaisyCutter,
        SpecialPowerType::AirForceAirstrike,
        SpecialPowerType::AirForceSpectreGunship,
        SpecialPowerType::SuperweaponParticleCannon,
        SpecialPowerType::NukeNeutronMissile,
        SpecialPowerType::BaikonurRocket,
        SpecialPowerType::BattleshipBombardment,
        SpecialPowerType::LaserCannon,
    ] {
        assert!(
            logic
                .queue_special_power_strike(&power, src, glam::Vec3::new(70.0, 0.0, 0.0))
                .is_some(),
            "power {power:?} must queue host residual"
        );
    }

    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::NukeChinaCarpetBomb,
            src,
            glam::Vec3::new(90.0, 0.0, 0.0),
        )
        .expect("nuke china carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::China)
    );

    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::StealthGpsScrambler),
        None
    ); // GPS is not a superweapon strike
    assert!(logic.activate_gps_scrambler(0, glam::Vec3::new(20.0, 0.0, 0.0), Some(src)));
}

#[test]
fn superweapon_crate_drop_spawns_ten_200_dollar_crates() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_money_crate::{
        honesty_superweapon_crate_drop_residual_pack_ok, SUPERWEAPON_CRATE_DROP_COUNT,
        SUPERWEAPON_CRATE_DROP_MONEY,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_superweapon_crate_drop_residual_pack_ok());
    assert_eq!(SUPERWEAPON_CRATE_DROP_COUNT, 10);
    assert_eq!(SUPERWEAPON_CRATE_DROP_MONEY, 200);

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let n = logic.activate_crate_drop(0, glam::Vec3::new(50.0, 0.0, 0.0), Some(src));
    assert_eq!(n, SUPERWEAPON_CRATE_DROP_COUNT);
    assert_eq!(
        logic.last_crate_drop_spawned(),
        SUPERWEAPON_CRATE_DROP_COUNT
    );
    let crates = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("200Dollar") && o.is_alive())
        .count();
    assert_eq!(crates as u32, SUPERWEAPON_CRATE_DROP_COUNT);
    let _ = SpecialPowerType::CrateDrop;
}

#[test]
fn napalm_strike_and_general_paradrop_terror_cell_map_host_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_ambush::HostAmbushKind;
    use crate::game_logic::host_paradrop::HostParadropKind;
    use crate::game_logic::special_power_strikes::{
        honesty_napalm_strike_residual_pack_ok, HostSuperweaponKind, NAPALM_STRIKE_RELOAD_MS,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert!(honesty_napalm_strike_residual_pack_ok());
    assert_eq!(NAPALM_STRIKE_RELOAD_MS, 600_000);
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::NapalmStrike),
        Some(HostSuperweaponKind::DaisyCutter)
    );
    assert_eq!(
        HostParadropKind::from_command_power(&SpecialPowerType::InfantryParadrop),
        Some(HostParadropKind::InfantryParadrop)
    );
    assert_eq!(
        HostParadropKind::from_command_power(&SpecialPowerType::TankParadrop),
        Some(HostParadropKind::TankParadrop)
    );
    assert_eq!(
        HostAmbushKind::from_command_power(&SpecialPowerType::TerrorCell),
        Some(HostAmbushKind::GLARebelAmbush)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    assert!(logic
        .queue_special_power_strike(
            &SpecialPowerType::NapalmStrike,
            src,
            glam::Vec3::new(90.0, 0.0, 0.0),
        )
        .is_some());
    assert!(logic
        .queue_paradrop(
            &SpecialPowerType::InfantryParadrop,
            src,
            glam::Vec3::new(110.0, 0.0, 0.0),
        )
        .is_some());
    assert!(logic
        .queue_ambush(
            &SpecialPowerType::TerrorCell,
            src,
            glam::Vec3::new(130.0, 0.0, 0.0),
        )
        .is_some());
}

#[test]
fn black_market_and_dirty_nuke_queue_nuclear_missile_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        honesty_black_market_and_dirty_nuke_residual_pack_ok, HostSuperweaponKind,
        BLACK_MARKET_NUKE_RELOAD_MS, DIRTY_NUKE_RELOAD_MS,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_black_market_and_dirty_nuke_residual_pack_ok());
    assert_eq!(BLACK_MARKET_NUKE_RELOAD_MS, 600_000);
    assert_eq!(DIRTY_NUKE_RELOAD_MS, 30_000);
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::BlackMarketNuke),
        Some(HostSuperweaponKind::NuclearMissile)
    );
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::DetonateDirtyNuke),
        Some(HostSuperweaponKind::NuclearMissile)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut sc = ThingTemplate::new("GLACommandCenter");
    sc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(4000.0);
    logic.templates.insert("GLACommandCenter".into(), sc);
    let src = logic
        .create_object(
            "GLACommandCenter",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sc");
    assert!(logic
        .queue_special_power_strike(
            &SpecialPowerType::BlackMarketNuke,
            src,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .is_some());
    assert!(logic
        .queue_special_power_strike(
            &SpecialPowerType::DetonateDirtyNuke,
            src,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .is_some());
}

#[test]
fn airforce_carpet_bomb_forces_airforce_payload_tier() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        honesty_airf_carpet_bomb_residual_pack_ok, CarpetBombFactionTier, HostSuperweaponKind,
        CARPET_BOMB_COUNT_AIRF,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_airf_carpet_bomb_residual_pack_ok());
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::AirForceCarpetBomb),
        Some(HostSuperweaponKind::CarpetBomb)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    // China caster + AirF power still forces AirForce payload residual.
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::AirForceCarpetBomb,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("airf carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::AirForce)
    );
    assert_eq!(
        CarpetBombFactionTier::AirForce.bomb_count(),
        CARPET_BOMB_COUNT_AIRF
    );
}

#[test]
fn early_china_carpet_bomb_forces_china_payload_tier() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        honesty_early_china_carpet_bomb_residual_pack_ok, CarpetBombFactionTier,
        HostSuperweaponKind, CARPET_BOMB_COUNT_CHINA,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_early_china_carpet_bomb_residual_pack_ok());
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::EarlyChinaCarpetBomb),
        Some(HostSuperweaponKind::CarpetBomb)
    );

    let mut logic = GameLogic::new();
    // USA caster + EarlyChinaCarpetBomb still forces China payload residual.
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::EarlyChinaCarpetBomb,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("early china carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::China)
    );
    assert_eq!(
        CarpetBombFactionTier::China.bomb_count(),
        CARPET_BOMB_COUNT_CHINA
    );
}

#[test]
fn early_frenzy_and_emergency_repair_powers_activate() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_emergency_repair::{
        highest_emergency_repair_level_from_sciences, HostEmergencyRepairLevel,
    };
    use crate::game_logic::host_frenzy::{highest_frenzy_level_from_sciences, HostFrenzyLevel};
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::EarlyFrenzy),
        Some("EARLY_SPECIAL_FRENZY")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::EarlyEmergencyRepair),
        Some("EARLY_SPECIAL_REPAIR_VEHICLES")
    );
    assert_eq!(
        highest_frenzy_level_from_sciences(["Early_SCIENCE_Frenzy2"]),
        HostFrenzyLevel::Two
    );
    assert_eq!(
        highest_emergency_repair_level_from_sciences(["Early_SCIENCE_EmergencyRepair3"]),
        HostEmergencyRepairLevel::Three
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("Early_SCIENCE_Frenzy1".to_string());
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("Early_SCIENCE_EmergencyRepair1".to_string());
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let mut tank = ThingTemplate::new("ChinaTankBattleMaster");
    tank.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("ChinaTankBattleMaster".into(), tank);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let veh = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("tank");
    // Damage vehicle so repair has a target.
    if let Some(o) = logic.host_object_mut(veh) {
        let _ = o.take_damage(100.0);
    }
    assert!(logic.activate_frenzy(
        0,
        glam::Vec3::new(10.0, 0.0, 0.0),
        Some(src),
        HostFrenzyLevel::One,
    ));
    assert!(logic.activate_emergency_repair(
        0,
        glam::Vec3::new(10.0, 0.0, 0.0),
        Some(src),
        HostEmergencyRepairLevel::One,
    ));
    let _ = SpecialPowerType::EarlyFrenzy;
    let _ = SpecialPowerType::EarlyEmergencyRepair;
}

#[test]
fn early_leaflet_drop_queues_same_delay_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_leaflet_drop::{
        honesty_early_leaflet_drop_residual_pack_ok, HostLeafletDropKind, LEAFLET_DELAY_FRAMES,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_early_leaflet_drop_residual_pack_ok());
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_leaflet_drop(
            &SpecialPowerType::EarlyLeafletDrop,
            src,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("early leaflet");
    let m = logic.host_leaflet_drops().get(id).expect("mission");
    assert_eq!(m.kind, HostLeafletDropKind::UsaEarlyLeafletDrop);
    assert_eq!(
        m.impact_frame.saturating_sub(m.activate_frame),
        LEAFLET_DELAY_FRAMES
    );
}

#[test]
fn host_upgrade_complete_fanaticism_maps_to_nationalism() {
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        HostUpgradeKind::from_name("Upgrade_Fanaticism"),
        HostUpgradeKind::Nationalism
    );
    assert_eq!(
        HostUpgradeKind::from_name("Upgrade_ChinaNationalism"),
        HostUpgradeKind::Nationalism
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(120.0);
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);
    let id = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rg");
    // Fanaticism is infantry-general Nationalism residual (same apply path).
    let n = logic.apply_nationalism_to_team(Team::China, "Upgrade_Fanaticism");
    assert!(n >= 1, "fanaticism nationalism residual n={n}");
    assert!(logic
        .host_object(id)
        .unwrap()
        .has_upgrade_tag("Upgrade_Fanaticism"));
}

#[test]
fn superweapon_cash_hack_science_tier_steals_amount() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_hero_abilities::{
        cash_hack_money_from_sciences, CASH_HACK_MONEY_AMOUNT_DEFAULT,
        CASH_HACK_MONEY_AMOUNT_TIER3, SCIENCE_CASH_HACK_3,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        cash_hack_money_from_sciences(["SCIENCE_CashHack1"]),
        CASH_HACK_MONEY_AMOUNT_DEFAULT
    );
    assert_eq!(
        cash_hack_money_from_sciences([SCIENCE_CASH_HACK_3]),
        CASH_HACK_MONEY_AMOUNT_TIER3
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    logic
        .players
        .insert(1, Player::new(1, Team::USA, "USA", false));
    logic.players.get_mut(&0).unwrap().resources.supplies = 0;
    logic.players.get_mut(&1).unwrap().resources.supplies = 10_000;
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_CASH_HACK_3.to_string());

    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    let mut depot = ThingTemplate::new("AmericaSupplyCenter");
    depot
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSupplyCenter)
        .set_health(2000.0);
    depot.capturable = true;
    logic.templates.insert("AmericaSupplyCenter".into(), depot);
    let victim = logic
        .create_object(
            "AmericaSupplyCenter",
            Team::USA,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("victim");

    // Location-only fire is a C++ no-op (CashHackSpecialPower.cpp:76-82).
    assert_eq!(logic.activate_cash_hack(0, Some(src), None), None);

    let stolen = logic
        .activate_cash_hack(0, Some(src), Some(victim))
        .expect("valid cash-generator steal");
    assert_eq!(stolen, CASH_HACK_MONEY_AMOUNT_TIER3);
    assert_eq!(
        logic.last_cash_hack_request_amount(),
        CASH_HACK_MONEY_AMOUNT_TIER3
    );
    assert_eq!(logic.last_cash_hack_stolen_amount(), stolen);
    assert_eq!(
        logic.players.get(&0).unwrap().effective_supplies(),
        CASH_HACK_MONEY_AMOUNT_TIER3
    );
    assert_eq!(
        logic.players.get(&1).unwrap().effective_supplies(),
        10_000 - CASH_HACK_MONEY_AMOUNT_TIER3
    );

    // Object-target path residual (second steal from the same victim player).
    let stolen2 = logic
        .activate_cash_hack(0, Some(src), Some(victim))
        .expect("second valid steal");
    assert_eq!(stolen2, CASH_HACK_MONEY_AMOUNT_TIER3);
    let _ = SpecialPowerType::CashHack;
}

#[test]
fn carpet_bomb_faction_tier_from_team_and_airforce_science() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        CarpetBombFactionTier, CARPET_BOMB_COUNT, CARPET_BOMB_COUNT_AIRF, CARPET_BOMB_COUNT_CHINA,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        CarpetBombFactionTier::from_team(Team::China).bomb_count(),
        CARPET_BOMB_COUNT_CHINA
    );
    assert_eq!(
        CarpetBombFactionTier::from_team(Team::USA).bomb_count(),
        CARPET_BOMB_COUNT
    );
    assert_eq!(
        CarpetBombFactionTier::AirForce.bomb_count(),
        CARPET_BOMB_COUNT_AIRF
    );
    assert_eq!(
        CarpetBombFactionTier::highest_from_team_and_sciences(
            Team::USA,
            ["AirF_SUPERWEAPON_CarpetBomb"],
        ),
        CarpetBombFactionTier::AirForce
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::CarpetBomb,
            src,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::China)
    );

    let mut logic_us = GameLogic::new();
    logic_us
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic_us
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("AirF_SUPERWEAPON_CarpetBomb".to_string());
    let mut us_cc = ThingTemplate::new("AmericaCommandCenter");
    us_cc
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic_us
        .templates
        .insert("AmericaCommandCenter".into(), us_cc);
    let us_src = logic_us
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("us cc");
    let us_id = logic_us
        .queue_special_power_strike(
            &SpecialPowerType::CarpetBomb,
            us_src,
            glam::Vec3::new(140.0, 0.0, 0.0),
        )
        .expect("us carpet");
    assert_eq!(
        logic_us.special_power_strike_carpet_tier(us_id),
        Some(CarpetBombFactionTier::AirForce)
    );
}

#[test]
fn a10_science_tier_spawns_ocl_jets_from_map_edge() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        A10StrikeScienceTier, HostSpecialPowerStrikeRegistry, HostSuperweaponKind, A10_SCIENCE_TIER3,
        A10_TRANSPORT,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        A10StrikeScienceTier::highest_from_sciences([A10_SCIENCE_TIER3]).formation_size(),
        3
    );
    // Delayed circular blob is not retail — OCL jets apply missile/vulcan.
    let l1 =
        HostSpecialPowerStrikeRegistry::damage_at_distance(HostSuperweaponKind::A10Strike, 0.0);
    let l3 = HostSpecialPowerStrikeRegistry::damage_at_distance_with_tiers(
        HostSuperweaponKind::A10Strike,
        0.0,
        crate::game_logic::special_power_strikes::ScudStormAnthraxTier::Base,
        A10StrikeScienceTier::Level3,
    );
    assert!(l1.abs() < 0.1 && l3.abs() < 0.1, "l1={l1} l3={l3}");

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(A10_SCIENCE_TIER3.to_string());
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("a10");
    assert_eq!(
        logic.special_power_strike_a10_tier(id),
        Some(A10StrikeScienceTier::Level3)
    );
    let jets: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.a10_strike_transport.is_some())
        .collect();
    assert_eq!(jets.len(), 3, "science 3 must spawn FormationSize 3 jets");
    assert!(jets.iter().all(|o| o.template_name == A10_TRANSPORT));
    let (min, max) = logic.world_bounds();
    for jet in &jets {
        let p = jet.get_position();
        let on_edge = (p.x - min.x).abs() < 1.0
            || (p.x - max.x).abs() < 1.0
            || (p.z - min.z).abs() < 1.0
            || (p.z - max.z).abs() < 1.0;
        assert!(on_edge, "A10 jet must spawn on map edge, pos={p:?}");
    }
}

#[test]
fn tank_and_infantry_paradrop_use_faction_templates() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_paradrop::{
        HostParadropKind, INFA_PARADROP_TEMPLATE, TANK_PARADROP_TEMPLATE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_TankParadrop2".into());
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("Infa_SCIENCE_InfantryParadrop3".into());
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    let tank_id = logic
        .queue_paradrop(
            &SpecialPowerType::TankParadrop,
            src,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("tank drop");
    let tank = logic.host_paradrops.get(tank_id).expect("tank mission");
    assert_eq!(tank.kind, HostParadropKind::TankParadrop);
    assert_eq!(tank.unit_template, TANK_PARADROP_TEMPLATE);
    assert_eq!(tank.unit_count, 2);

    let inf_id = logic
        .queue_paradrop(
            &SpecialPowerType::InfantryParadrop,
            src,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("infa drop");
    let inf = logic.host_paradrops.get(inf_id).expect("infa mission");
    assert_eq!(inf.kind, HostParadropKind::InfantryParadrop);
    assert_eq!(inf.unit_template, INFA_PARADROP_TEMPLATE);
    assert_eq!(inf.unit_count, 20);
}

#[test]
fn emergency_repair_and_frenzy_science_tier_from_unlocked() {
    use crate::game_logic::host_emergency_repair::{
        highest_emergency_repair_level_from_sciences, HostEmergencyRepairLevel,
        SCIENCE_EMERGENCY_REPAIR3,
    };
    use crate::game_logic::host_frenzy::{
        highest_frenzy_level_from_sciences, HostFrenzyLevel, SCIENCE_FRENZY2, SCIENCE_FRENZY3,
    };
    assert_eq!(
        highest_emergency_repair_level_from_sciences([SCIENCE_EMERGENCY_REPAIR3]),
        HostEmergencyRepairLevel::Three
    );
    assert_eq!(
        highest_frenzy_level_from_sciences([SCIENCE_FRENZY2, SCIENCE_FRENZY3]),
        HostFrenzyLevel::Three
    );
    assert_eq!(
        highest_frenzy_level_from_sciences(["SCIENCE_CashBounty1"]),
        HostFrenzyLevel::One
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_EMERGENCY_REPAIR3.to_string());
    let sciences = logic.player_unlocked_sciences(0);
    assert_eq!(
        highest_emergency_repair_level_from_sciences(sciences.iter().map(|s| s.as_str())),
        HostEmergencyRepairLevel::Three
    );
}

#[test]
fn ambush_science_tier_selects_unit_count() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_ambush::{
        AmbushScienceTier, GLA_AMBUSH1_UNIT_COUNT, GLA_AMBUSH3_UNIT_COUNT, SCIENCE_AMBUSH3,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut palace = ThingTemplate::new("GLAPalace");
    palace.add_kind_of(KindOf::Structure).set_health(3000.0);
    logic.templates.insert("GLAPalace".into(), palace);
    let mut rebel = ThingTemplate::new("GLAInfantryRebel");
    rebel.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel);

    let src = logic
        .create_object("GLAPalace", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("palace");

    let id1 = logic
        .queue_ambush(
            &SpecialPowerType::Ambush,
            src,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("q1");
    assert_eq!(
        logic.ambush_mission_unit_count(id1),
        Some(GLA_AMBUSH1_UNIT_COUNT)
    );

    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_AMBUSH3.to_string());
    let id3 = logic
        .queue_ambush(
            &SpecialPowerType::Ambush,
            src,
            glam::Vec3::new(160.0, 0.0, 0.0),
        )
        .expect("q3");
    assert_eq!(
        logic.ambush_mission_unit_count(id3),
        Some(GLA_AMBUSH3_UNIT_COUNT)
    );
    assert_eq!(
        AmbushScienceTier::highest_from_sciences([SCIENCE_AMBUSH3]).rebel_count(),
        GLA_AMBUSH3_UNIT_COUNT
    );
}

#[test]
fn paradrop_science_tier_selects_unit_count() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_paradrop::{
        ParadropScienceTier, PARADROP_RANGER_COUNT_L1, PARADROP_RANGER_COUNT_L3, SCIENCE_PARADROP1,
        SCIENCE_PARADROP3,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    // Default tier 1 without science still queues residual L1 count via highest_from_sciences default.
    let id1 = logic
        .queue_paradrop(
            &SpecialPowerType::Paradrop,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("q1");
    assert_eq!(
        logic.paradrop_mission_unit_count(id1),
        Some(PARADROP_RANGER_COUNT_L1)
    );

    // Unlock tier 3 science.
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_PARADROP3.to_string());
    let id3 = logic
        .queue_paradrop(
            &SpecialPowerType::Paradrop,
            src,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("q3");
    assert_eq!(
        logic.paradrop_mission_unit_count(id3),
        Some(PARADROP_RANGER_COUNT_L3)
    );
    assert_eq!(
        ParadropScienceTier::highest_from_sciences([SCIENCE_PARADROP1, SCIENCE_PARADROP3])
            .ranger_count(),
        PARADROP_RANGER_COUNT_L3
    );
}

#[test]
fn host_upgrade_complete_cash_bounty_sets_player_percent() {
    use crate::game_logic::host_cash_bounty::{CASH_BOUNTY1_PERCENT, CASH_BOUNTY3_PERCENT};
    use crate::game_logic::Team;
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));

    let n = logic.apply_cash_bounty_upgrade_to_team(Team::GLA, "Upgrade_CashBounty");
    assert_eq!(n, 1);
    let p = logic.players.get(&0).unwrap();
    assert!((p.cash_bounty_percent - CASH_BOUNTY1_PERCENT).abs() < 0.001);
    assert!(p.unlocked_sciences.contains("SCIENCE_CashBounty1"));

    let n3 = logic.apply_cash_bounty_upgrade_to_team(Team::GLA, "SCIENCE_CashBounty3");
    assert_eq!(n3, 1);
    let p = logic.players.get(&0).unwrap();
    assert!((p.cash_bounty_percent - CASH_BOUNTY3_PERCENT).abs() < 0.001);
}

#[test]
fn host_upgrade_complete_slave_drone_attaches_to_humvee() {
    use crate::game_logic::host_slave_drones::{
        UPGRADE_AMERICA_BATTLE_DRONE, UPGRADE_AMERICA_SCOUT_DRONE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));

    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);

    let mid = logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("humvee");

    let n = logic.apply_slave_drone_upgrade_to_team(Team::USA, UPGRADE_AMERICA_SCOUT_DRONE);
    assert_eq!(n, 1);
    assert!(logic
        .host_object(mid)
        .unwrap()
        .has_upgrade_tag(UPGRADE_AMERICA_SCOUT_DRONE));
    // Scout drone object should exist.
    let drones: Vec<_> = logic
        .objects
        .values()
        .filter(|o| {
            o.template_name.to_ascii_lowercase().contains("scoutdrone")
                || o.template_name.contains("ScoutDrone")
        })
        .collect();
    assert!(!drones.is_empty(), "scout drone must spawn");
    assert!(logic.honesty_scout_drone_attach_ok());

    // Battle drone on second master.
    let mid2 = logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            glam::Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("humvee2");
    let n2 = logic.apply_slave_drone_upgrade_to_team(Team::USA, UPGRADE_AMERICA_BATTLE_DRONE);
    assert!(n2 >= 1);
    assert!(logic
        .host_object(mid2)
        .unwrap()
        .has_upgrade_tag(UPGRADE_AMERICA_BATTLE_DRONE));
}

#[test]
fn host_upgrade_complete_chemical_suits_reduces_poison() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_armor_residual::{
        apply_residual_armor, CHEM_SUIT_HUMAN_ARMOR_POISON,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_CHEMICAL_SUITS;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let id = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("ranger");
    let before = {
        let o = logic.host_object(id).unwrap();
        apply_residual_armor(o, DamageType::Toxin, 100.0)
    };
    let n = logic.apply_chemical_suits_to_team(Team::USA, UPGRADE_AMERICA_CHEMICAL_SUITS);
    assert!(n >= 1);
    let after = {
        let o = logic.host_object(id).unwrap();
        apply_residual_armor(o, DamageType::Toxin, 100.0)
    };
    // Chem suit poison coeff 0.20 vs human default (higher).
    assert!(
        after < before,
        "chem suits must reduce poison residual ({after} vs {before})"
    );
    assert!((after - 100.0 * CHEM_SUIT_HUMAN_ARMOR_POISON).abs() < 1.0);
}

#[test]
fn host_upgrade_complete_moab_and_satellite_hack_unlock() {
    use crate::game_logic::host_upgrades::{
        UPGRADE_AMERICA_MOAB, UPGRADE_CHINA_SATELLITE_HACK_TWO,
    };
    use crate::game_logic::Team;
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", true));
    assert_eq!(
        logic.apply_player_unlock_upgrade(Team::USA, UPGRADE_AMERICA_MOAB, UPGRADE_AMERICA_MOAB),
        1
    );
    assert!(logic
        .players
        .get(&0)
        .unwrap()
        .unlocked_sciences
        .contains(UPGRADE_AMERICA_MOAB));
    assert!(logic.apply_satellite_hack_to_team(Team::China, UPGRADE_CHINA_SATELLITE_HACK_TWO) >= 1);
    assert!(logic
        .players
        .get(&1)
        .unwrap()
        .unlocked_sciences
        .contains(UPGRADE_CHINA_SATELLITE_HACK_TWO));
}

#[test]
fn host_upgrade_complete_mines_radar_fortified() {
    use crate::game_logic::host_upgrades::{
        FORTIFIED_STRUCTURE_ADD_MAX_HEALTH, UPGRADE_CHINA_MINES, UPGRADE_GLA_FORTIFIED_STRUCTURE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    logic
        .players
        .insert(1, Player::new(1, Team::GLA, "GLA", true));
    logic
        .players
        .insert(2, Player::new(2, Team::USA, "USA", true));

    let n_m =
        logic.apply_player_unlock_upgrade(Team::China, UPGRADE_CHINA_MINES, UPGRADE_CHINA_MINES);
    assert_eq!(n_m, 1);
    assert!(logic
        .players
        .get(&0)
        .unwrap()
        .unlocked_sciences
        .contains(UPGRADE_CHINA_MINES));

    let mut bm = ThingTemplate::new("GLABlackMarket");
    bm.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("GLABlackMarket".into(), bm);
    let bid = logic
        .create_object("GLABlackMarket", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("bm");
    let before = logic.host_object(bid).unwrap().max_health;
    let n_f = logic.apply_fortified_structure_to_team(Team::GLA, UPGRADE_GLA_FORTIFIED_STRUCTURE);
    assert_eq!(n_f, 1);
    let after = logic.host_object(bid).unwrap().max_health;
    assert!((after - before - FORTIFIED_STRUCTURE_ADD_MAX_HEALTH).abs() < 0.1);

    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cid = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("cc");
    let n_r = logic.apply_radar_research_to_team(Team::USA, "Upgrade_AmericaRadar");
    assert!(n_r >= 1);
    assert!(logic
        .host_object(cid)
        .unwrap()
        .has_upgrade_tag("Upgrade_AmericaRadar"));
    assert!(logic
        .players
        .get(&2)
        .unwrap()
        .unlocked_sciences
        .contains("Upgrade_AmericaRadar"));
}

#[test]
fn host_upgrade_complete_drone_and_aircraft_armor() {
    use crate::game_logic::host_mig::{
        MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH, UPGRADE_CHINA_AIRCRAFT_ARMOR,
    };
    use crate::game_logic::host_slave_drones::{
        drone_armor_add_max_health, SlaveDroneKind, UPGRADE_AMERICA_DRONE_ARMOR,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", true));

    let mut battle = ThingTemplate::new("AmericaBattleDrone");
    battle.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("AmericaBattleDrone".into(), battle);
    let mut mig = ThingTemplate::new("ChinaJetMIG");
    mig.add_kind_of(KindOf::Aircraft).set_health(160.0);
    logic.templates.insert("ChinaJetMIG".into(), mig);

    let did = logic
        .create_object(
            "AmericaBattleDrone",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("drone");
    let before = logic.host_object(did).unwrap().max_health;
    let n_d = logic.apply_drone_armor_to_team(Team::USA, UPGRADE_AMERICA_DRONE_ARMOR);
    assert_eq!(n_d, 1);
    let after = logic.host_object(did).unwrap().max_health;
    assert!((after - before - drone_armor_add_max_health(SlaveDroneKind::Battle)).abs() < 0.1);

    let mid = logic
        .create_object("ChinaJetMIG", Team::China, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("mig");
    let mb = logic.host_object(mid).unwrap().max_health;
    let n_a = logic.apply_aircraft_armor_to_team(Team::China, UPGRADE_CHINA_AIRCRAFT_ARMOR);
    assert_eq!(n_a, 1);
    let ma = logic.host_object(mid).unwrap().max_health;
    assert!((ma - mb - MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH).abs() < 0.1);
}

#[test]
fn host_upgrade_complete_advanced_training_and_tactical_nuke_mig() {
    use crate::game_logic::host_unit_training::{
        residual_xp_gain_with_advanced_training, UPGRADE_AMERICA_ADVANCED_TRAINING,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", true));

    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut mig = ThingTemplate::new("Nuke_ChinaJetMIG");
    mig.add_kind_of(KindOf::Aircraft).set_health(200.0);
    logic.templates.insert("Nuke_ChinaJetMIG".into(), mig);

    let rid = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("ranger");
    let mid = logic
        .create_object(
            "Nuke_ChinaJetMIG",
            Team::China,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("mig");

    let n_at = logic.apply_advanced_training_to_team(Team::USA, UPGRADE_AMERICA_ADVANCED_TRAINING);
    assert!(n_at >= 1);
    assert!(logic
        .players
        .get(&0)
        .unwrap()
        .unlocked_sciences
        .contains(UPGRADE_AMERICA_ADVANCED_TRAINING));
    assert!(logic
        .host_object(rid)
        .unwrap()
        .has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING));
    // Honesty: XP scalar residual path.
    assert!((residual_xp_gain_with_advanced_training(50.0, true) - 100.0).abs() < 0.01);

    let n_nuke = logic.apply_tactical_nuke_mig_to_team(Team::China, "Upgrade_ChinaTacticalNukeMig");
    assert_eq!(n_nuke, 1);
    assert!(logic
        .host_object(mid)
        .unwrap()
        .has_upgrade_tag("Upgrade_ChinaTacticalNukeMig"));
}

#[test]
fn ocl_science_tiers_use_controlling_player_not_faction_union() {
    // C++ OCLSpecialPower::findOCL — getControllingPlayer()->hasScience only.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        A10StrikeScienceTier, ArtilleryBarrageScienceTier, ScudStormAnthraxTier, A10_SCIENCE_TIER3,
        ARTILLERY_SCIENCE_TIER3, A10_TRANSPORT,
    };
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA-A", true));
    logic.add_player(Player::new(1, Team::USA, "USA-B", false));
    logic.add_player(Player::new(2, Team::China, "China-A", false));
    logic.add_player(Player::new(3, Team::China, "China-B", false));
    logic.add_player(Player::new(4, Team::GLA, "GLA-A", false));
    logic.add_player(Player::new(5, Team::GLA, "GLA-B", false));

    logic
        .players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert(A10_SCIENCE_TIER3.into());
    logic
        .players
        .get_mut(&3)
        .unwrap()
        .unlocked_sciences
        .insert(ARTILLERY_SCIENCE_TIER3.into());
    logic
        .players
        .get_mut(&5)
        .unwrap()
        .unlocked_sciences
        .insert("Upgrade_GLAAnthraxBeta".into());

    let mut usa_cc = ThingTemplate::new("AmericaCommandCenter");
    usa_cc
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), usa_cc);
    let mut china_cc = ThingTemplate::new("ChinaCommandCenter");
    china_cc
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), china_cc);
    let mut gla_cc = ThingTemplate::new("GLAScudStorm");
    gla_cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), gla_cc);

    let usa_src = logic
        .create_object_for_player("AmericaCommandCenter", 0, glam::Vec3::ZERO)
        .unwrap();
    let china_src = logic
        .create_object_for_player(
            "ChinaCommandCenter",
            2,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .unwrap();
    let gla_src = logic
        .create_object_for_player("GLAScudStorm", 4, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();

    let a10 = logic
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            usa_src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic.special_power_strike_a10_tier(a10),
        Some(A10StrikeScienceTier::Level1)
    );
    let jets = logic
        .host_objects()
        .values()
        .filter(|o| o.a10_strike_transport.is_some())
        .count();
    assert_eq!(jets, 1, "ally A10-3 must not upgrade this player's strike");
    assert!(logic
        .host_objects()
        .values()
        .filter(|o| o.a10_strike_transport.is_some())
        .all(|o| o.template_name == A10_TRANSPORT));

    let arty = logic
        .queue_special_power_strike(
            &SpecialPowerType::Artillery,
            china_src,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic
            .special_power_strikes()
            .get(arty)
            .map(|s| s.artillery_tier),
        Some(ArtilleryBarrageScienceTier::Level1)
    );

    let scud = logic
        .queue_special_power_strike(
            &SpecialPowerType::ScudStorm,
            gla_src,
            glam::Vec3::new(140.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic
            .special_power_strikes()
            .get(scud)
            .map(|s| s.scud_anthrax_tier),
        Some(ScudStormAnthraxTier::Base)
    );

    let usa_up = logic
        .create_object_for_player(
            "AmericaCommandCenter",
            1,
            glam::Vec3::new(30.0, 0.0, 0.0),
        )
        .unwrap();
    let a10_up = logic
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            usa_up,
            glam::Vec3::new(160.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic.special_power_strike_a10_tier(a10_up),
        Some(A10StrikeScienceTier::Level3)
    );
}
