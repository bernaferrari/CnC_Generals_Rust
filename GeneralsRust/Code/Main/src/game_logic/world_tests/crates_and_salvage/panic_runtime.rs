//! Authored TEAM_PANIC state, collision, and save continuation gates.
use super::*;
use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
use gamelogic::scripting::{request_host_team_loco_set, take_host_team_loco_set_requests};
use gamelogic::system::map_loader::{ICoord2D, MapData, MapWaypoint};
use gamelogic::terrain::get_terrain_logic;
use glam::Vec3;

#[path = "panic_instance_tests.rs"]
mod instance_tests;

const PANIC_UNIT: ObjectId = ObjectId(48_101);
const BLOCKER: ObjectId = ObjectId(48_102);
const WAYPOINT_START: u32 = 48_111;
const WAYPOINT_NEXT: u32 = 48_112;
const WAYPOINT_BRANCH: u32 = 48_113;

struct RestoreLogicRng([u32; 6]);

impl Drop for RestoreLogicRng {
    fn drop(&mut self) {
        game_engine::common::random_value::set_game_logic_random_seed_state(self.0);
    }
}

struct ResetGlobalTerrain;

impl Drop for ResetGlobalTerrain {
    fn drop(&mut self) {
        if let Ok(mut terrain) = get_terrain_logic().write() {
            terrain.reset();
        }
    }
}

fn panic_map_data() -> MapData {
    let mut data = MapData::new();
    data.width = 128;
    data.height = 128;
    data.heightmap = vec![0; 128 * 128];
    data.boundaries.push(ICoord2D::new(128, 128));
    data
}

fn install_panic_waypoint_path() -> ResetGlobalTerrain {
    install_panic_waypoint_path_with_terminal_x(200.0)
}

fn install_panic_waypoint_path_with_terminal_x(terminal_x: f32) -> ResetGlobalTerrain {
    let _cleanup = ResetGlobalTerrain;
    let mut data = panic_map_data();
    data.waypoints.push(MapWaypoint {
        id: WAYPOINT_START,
        name: "PanicStart".into(),
        location: gamelogic::system::map_loader::Coord3D::new(50.0, 0.0, 0.0),
        path_label1: "PanicPath".into(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });
    data.waypoints.push(MapWaypoint {
        id: WAYPOINT_NEXT,
        name: "PanicNext".into(),
        location: gamelogic::system::map_loader::Coord3D::new(terminal_x, 0.0, 0.0),
        path_label1: "PanicTerminal".into(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });
    data.waypoint_links.push((WAYPOINT_START, WAYPOINT_NEXT));
    let mut terrain = get_terrain_logic().write().expect("global terrain");
    terrain.reset();
    terrain.load_map_data(data);
    drop(terrain);
    _cleanup
}

fn install_panic_fork_path() -> ResetGlobalTerrain {
    let _cleanup = ResetGlobalTerrain;
    let mut data = panic_map_data();
    for (id, name, x, label) in [
        (WAYPOINT_START, "PanicStart", 50.0, "PanicPath"),
        (WAYPOINT_NEXT, "PanicBranchA", 200.0, "PanicTerminal"),
        (WAYPOINT_BRANCH, "PanicBranchB", 200.0, "PanicTerminal"),
    ] {
        data.waypoints.push(MapWaypoint {
            id,
            name: name.into(),
            location: gamelogic::system::map_loader::Coord3D::new(x, 0.0, 0.0),
            path_label1: label.into(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    }
    data.waypoint_links.extend([
        (WAYPOINT_START, WAYPOINT_NEXT),
        (WAYPOINT_START, WAYPOINT_BRANCH),
    ]);
    let mut terrain = get_terrain_logic().write().expect("global terrain");
    terrain.reset();
    terrain.load_map_data(data);
    drop(terrain);
    _cleanup
}

fn panic_logic() -> GameLogic {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("PanicRuntimeInfantry");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::CanBeRepulsed)
        .set_health(100.0);
    logic
        .templates
        .insert("PanicRuntimeInfantry".into(), template.clone());
    let mut unit = Object::new(template, PANIC_UNIT, Team::GLA);
    unit.set_position(Vec3::ZERO);
    unit.movement.max_speed = 20.0;
    unit.vision_range = 200.0;
    logic.objects.insert(PANIC_UNIT, unit);
    logic
}

fn issue_authored_team_panic(logic: &mut GameLogic) {
    issue_authored_team_panic_for(logic, "PanicPath");
}

fn issue_authored_team_panic_for(logic: &mut GameLogic, path_label: &str) {
    let _ = take_host_team_loco_set_requests();
    request_host_team_loco_set("GLA", "panic", Some(path_label));
    assert_eq!(
        take_host_team_loco_set_requests(),
        vec![("GLA".into(), "panic".into(), Some(path_label.into()))],
        "the authored TEAM_PANIC request carries the path label"
    );
    request_host_team_loco_set("GLA", "panic", Some(path_label));
    logic.apply_host_loco_set_script_requests();
}

fn assert_panic_state(state: &AIState) {
    // Compare the public state name so this OLD test compiles before AIState::Panic exists.
    assert_eq!(
        format!("{state:?}"),
        "Panic",
        "TEAM_PANIC must retain its distinct AI_PANIC state instead of generic Moving"
    );
}

fn assert_panic_runtime(unit: &Object, current: u32, prior: Option<u32>) {
    let panic = unit
        .panic_runtime
        .as_ref()
        .expect("owned panic continuation");
    assert_eq!(panic.current_waypoint_id, current);
    assert_eq!(panic.prior_waypoint_id, prior);
    assert_eq!(panic.wait_frames, 10 + (unit.id.0 & 7) as i32);
}

#[test]
fn authored_team_panic_enters_distinct_state_and_keeps_ordinary_move_control() {
    let _terrain = install_panic_waypoint_path();
    let mut logic = panic_logic();
    issue_authored_team_panic(&mut logic);

    let unit = &logic.objects[&PANIC_UNIT];
    assert_panic_state(&unit.ai_state);
    assert!(
        unit.is_panicking,
        "panic model condition remains independent"
    );
    assert!(
        unit.requested_destination.is_some() || unit.movement.target_position.is_some(),
        "panic state must install the authored waypoint path"
    );
    assert!(
        unit.movement.max_speed > 0.0,
        "panic locomotor selection must remain active"
    );
    if unit.panic_runtime.as_ref().unwrap().append_goal_position {
        assert!(unit.allow_invalid_position);
        assert!(
            !unit.adjust_destinations,
            "C++ FollowWaypointPath disables destination adjustment for an appended off-map goal"
        );
    }
    assert_panic_runtime(unit, WAYPOINT_START, None);

    let ordinary_id = ObjectId(48_103);
    let mut ordinary_template = ThingTemplate::new("OrdinaryMoveControl");
    ordinary_template
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0);
    let mut ordinary = Object::new(ordinary_template, ordinary_id, Team::USA);
    ordinary.movement.max_speed = 10.0;
    logic.objects.insert(ordinary_id, ordinary);
    assert!(logic.unit_command_move_to(ordinary_id, Vec3::new(100.0, 0.0, 0.0)));
    assert_eq!(logic.objects[&ordinary_id].ai_state, AIState::Moving);
}

#[test]
fn authored_panic_waypoint_on_wall_keeps_wall_height_and_layer() {
    let _terrain = install_panic_waypoint_path();
    let mut logic = panic_logic();
    let mut wall_template = ThingTemplate::new("PanicWallPiece");
    wall_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::WalkOnTopOfWall)
        .set_health(500.0);
    let mut wall = Object::new(wall_template, ObjectId(48_104), Team::USA);
    wall.selection_radius = 50.0;
    wall.set_position(Vec3::new(50.0, 0.0, 0.0));
    logic.pathfinding_system.set_wall_height(12.0);
    logic
        .pathfinding_system
        .add_wall_piece_from_object(&wall, 12.0);
    assert!(
        logic
            .pathfinding_system
            .is_point_on_wall(Vec3::new(50.0, 0.0, 0.0))
    );

    issue_authored_team_panic(&mut logic);
    let unit = &logic.objects[&PANIC_UNIT];
    assert_panic_state(&unit.ai_state);
    assert_eq!(unit.panic_runtime.as_ref().unwrap().goal_layer, 15);
    assert_eq!(unit.requested_destination.unwrap().y, 12.0);
}

#[test]
fn off_map_panic_entry_disables_destination_adjustment_without_waypoint_links() {
    let _terrain = install_panic_waypoint_path_with_terminal_x(1400.0);
    let mut logic = panic_logic();
    issue_authored_team_panic_for(&mut logic, "PanicTerminal");

    let unit = &logic.objects[&PANIC_UNIT];
    assert_panic_runtime(unit, WAYPOINT_NEXT, None);
    assert!(
        unit.panic_runtime.as_ref().unwrap().append_goal_position,
        "the no-link terminal waypoint lies outside this fixture's pathfind extent"
    );
    assert!(unit.allow_invalid_position);
    assert!(
        !unit.adjust_destinations,
        "C++ off-map goal handling overrides the no-links destination-adjustment default"
    );
}

#[test]
fn authored_team_panic_bounces_infantry_collision_before_blocked_state() {
    let _terrain = install_panic_waypoint_path();
    let mut logic = panic_logic();
    if let Some(unit) = logic.objects.get_mut(&PANIC_UNIT) {
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.movement.velocity = Vec3::new(2.0, 0.0, 0.0);
        unit.set_orientation(0.0);
        unit.selection_radius = 5.0;
    }
    issue_authored_team_panic(&mut logic);

    let mut blocker_template = ThingTemplate::new("PanicRuntimeBlocker");
    blocker_template
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0);
    let mut blocker = Object::new(blocker_template, BLOCKER, Team::USA);
    blocker.cur_locomotor_name = Some("BasicHumanLocomotor".into());
    blocker.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    blocker.set_position(Vec3::new(5.0, 0.0, 0.0));
    blocker.set_orientation(0.0);
    blocker.selection_radius = 5.0;
    logic.objects.insert(BLOCKER, blocker);

    assert!(
        crate::game_logic::pathfinding::PathfindingGrid::is_doing_ground_movement_full(
            &logic.objects[&PANIC_UNIT]
        )
    );
    assert!(
        crate::game_logic::pathfinding::PathfindingGrid::is_doing_ground_movement_full(
            &logic.objects[&BLOCKER]
        )
    );
    assert!(logic.try_physics_collide(PANIC_UNIT, BLOCKER, 5.0));
    let unit = &logic.objects[&PANIC_UNIT];
    assert_eq!(unit.last_collidee, Some(BLOCKER));
    assert!(
        !unit.is_blocked,
        "AI_PANIC collision returns bounce before setting m_isBlocked"
    );
    assert_panic_state(&unit.ai_state);
    assert_panic_runtime(unit, WAYPOINT_START, None);
}

#[test]
fn panic_state_and_waypoint_path_survive_world_snapshot_roundtrip() {
    let _terrain = install_panic_waypoint_path();
    let mut source = panic_logic();
    issue_authored_team_panic(&mut source);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("capture panicking world");
    let encoded = bincode_legacy::serialize(&snapshot).expect("encode panic snapshot");
    let decoded: crate::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&encoded).expect("decode panic snapshot");
    let decode_again = || {
        bincode_legacy::deserialize::<crate::save_load::snapshot::WorldSnapshot>(&encoded)
            .expect("decode panic snapshot for corrupted-input control")
    };

    let path_before = source.objects[&PANIC_UNIT].movement.path.clone();
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("restore panic snapshot");

    let restored_unit = &restored.objects[&PANIC_UNIT];
    assert_panic_state(&restored_unit.ai_state);
    assert_panic_runtime(restored_unit, WAYPOINT_START, None);
    assert_eq!(
        restored_unit.movement.path, path_before,
        "restored panic continuation must retain the canonical movement path"
    );
    assert_eq!(
        restored_unit.panic_runtime, source.objects[&PANIC_UNIT].panic_runtime,
        "waypoint IDs, group offset, and panic timer survive module snapshot restore"
    );
    let source_unit = &source.objects[&PANIC_UNIT];
    assert_eq!(
        restored_unit.requested_destination,
        source_unit.requested_destination
    );
    assert_eq!(
        restored_unit.path_goal_position,
        source_unit.path_goal_position
    );
    assert_eq!(restored_unit.waiting_for_path, source_unit.waiting_for_path);
    assert_eq!(restored_unit.path_timestamp, source_unit.path_timestamp);
    assert_eq!(
        restored_unit.adjust_destinations,
        source_unit.adjust_destinations
    );
    assert_eq!(
        restored_unit.path_extra_distance,
        source_unit.path_extra_distance
    );
    assert_eq!(restored_unit.retry_path, source_unit.retry_path);
    assert_eq!(
        restored_unit.try_one_more_repath,
        source_unit.try_one_more_repath
    );
    assert_eq!(
        restored_unit.is_blocked_and_stuck,
        source_unit.is_blocked_and_stuck
    );
    assert_eq!(
        restored_unit.num_frames_blocked,
        source_unit.num_frames_blocked
    );
    assert!(restored_unit.is_panicking);

    let mut missing = decode_again();
    if let Some(crate::save_load::snapshot::ModuleSnapshot::AIUpdate(module)) = missing
        .objects
        .get_mut(&PANIC_UNIT)
        .and_then(|object| object.modules.get_mut("AIUpdate"))
    {
        module.state_machine_data.remove("AIPanicState");
    }
    let mut missing_world = GameLogic::new();
    missing_world.templates = source.templates.clone();
    assert!(
        builder
            .restore_from_snapshot(&missing, &mut missing_world)
            .is_err()
    );

    let mut malformed = decode_again();
    if let Some(crate::save_load::snapshot::ModuleSnapshot::AIUpdate(module)) = malformed
        .objects
        .get_mut(&PANIC_UNIT)
        .and_then(|object| object.modules.get_mut("AIUpdate"))
    {
        module
            .state_machine_data
            .insert("AIPanicState".into(), "not-json".into());
    }
    let mut malformed_world = GameLogic::new();
    malformed_world.templates = source.templates.clone();
    assert!(
        builder
            .restore_from_snapshot(&malformed, &mut malformed_world)
            .is_err()
    );

    let mut missing_waypoint = decode_again();
    if let Some(crate::save_load::snapshot::ModuleSnapshot::AIUpdate(module)) = missing_waypoint
        .objects
        .get_mut(&PANIC_UNIT)
        .and_then(|object| object.modules.get_mut("AIUpdate"))
    {
        let encoded = module
            .state_machine_data
            .get("AIPanicState")
            .expect("panic state module");
        let mut state: crate::game_logic::object::PanicSaveState =
            serde_json::from_str(encoded).expect("valid panic save state");
        state.panic.current_waypoint_id = u32::MAX;
        module.state_machine_data.insert(
            "AIPanicState".into(),
            serde_json::to_string(&state).expect("encode corrupted waypoint"),
        );
    }
    let mut missing_waypoint_world = GameLogic::new();
    missing_waypoint_world.templates = source.templates.clone();
    assert!(
        builder
            .restore_from_snapshot(&missing_waypoint, &mut missing_waypoint_world)
            .is_err()
    );
}

#[test]
fn panic_follows_multiple_waypoints_and_finishes_after_real_locomotor_arrivals() {
    let _terrain = install_panic_waypoint_path();
    let mut logic = panic_logic();
    issue_authored_team_panic(&mut logic);

    let mut observed_next_waypoint = false;
    let mut moved_frames = 0;
    let mut previous_position = logic.objects[&PANIC_UNIT].get_position();
    for _ in 0..900 {
        logic.update_with_dt_budget(1.0 / 30.0, 1);
        let unit = &logic.objects[&PANIC_UNIT];
        let position = unit.get_position();
        if position.distance_squared(previous_position) > 1.0e-8 {
            moved_frames += 1;
        }
        previous_position = position;
        observed_next_waypoint |= unit
            .panic_runtime
            .as_ref()
            .is_some_and(|panic| panic.current_waypoint_id == WAYPOINT_NEXT);
        if unit
            .panic_runtime
            .as_ref()
            .is_some_and(|panic| panic.append_goal_position)
        {
            assert!(unit.allow_invalid_position);
            assert!(
                !unit.adjust_destinations,
                "the next off-map waypoint also disables destination adjustment"
            );
        }
        if unit.ai_state == AIState::Idle {
            break;
        }
    }

    let unit = &logic.objects[&PANIC_UNIT];
    assert!(
        observed_next_waypoint,
        "the first completed path selects its linked waypoint"
    );
    assert!(
        moved_frames > 1,
        "the route was traversed across multiple simulation frames"
    );
    assert_eq!(
        unit.ai_state,
        AIState::Idle,
        "the terminal path returns to Idle"
    );
    assert_eq!(unit.completed_waypoint_labels, vec!["PanicTerminal"]);
    assert!(
        unit.movement.target_position.is_none() && !unit.status.moving,
        "terminal InternalMove cleanup stops movement"
    );
    assert!(unit.movement.path.is_empty());
    assert_eq!(unit.movement.current_path_index, 0);
    assert!(unit.path_goal_position.is_some());
    assert_eq!(unit.path_goal_position, unit.requested_destination);
    assert_eq!(unit.queue_for_path_frames, 0);
    assert_eq!(unit.ignored_obstacle_id, None);
    assert!(!unit.waiting_for_path);
    assert!(!unit.is_panicking);
    let panicking_bit = crate::game_logic::host_enum_table_residual::panicking_model_bit();
    assert_eq!(unit.model_condition_bits & (1u128 << panicking_bit), 0);
}

#[test]
fn ordinary_move_command_interrupts_panic_and_retires_its_continuation() {
    let _terrain = install_panic_waypoint_path();
    let mut logic = panic_logic();
    issue_authored_team_panic(&mut logic);
    assert_panic_state(&logic.objects[&PANIC_UNIT].ai_state);

    assert!(logic.unit_command_move_to(PANIC_UNIT, Vec3::new(300.0, 0.0, 0.0)));
    let unit = &logic.objects[&PANIC_UNIT];
    assert_eq!(unit.ai_state, AIState::Moving);
    assert!(unit.panic_runtime.is_none());
    assert!(!unit.is_panicking);
}

#[test]
fn panic_failure_does_not_advance_the_waypoint_and_terminal_uses_terminal_labels() {
    let _terrain = install_panic_waypoint_path();
    let mut logic = panic_logic();
    issue_authored_team_panic(&mut logic);

    // AIInternalMoveToState::update reports FAILURE for a route that cannot
    // be recomputed; AIPanicState::update preserves its state and does not
    // advance the waypoint in that case.
    {
        let unit = logic.objects.get_mut(&PANIC_UNIT).unwrap();
        unit.movement.path.clear();
        unit.movement.target_position = None;
        unit.set_status_moving(false);
        unit.waiting_for_path = false;
        unit.retry_path = true;
    }
    logic.tick_host_panic_states(&[PANIC_UNIT]);
    assert_panic_state(&logic.objects[&PANIC_UNIT].ai_state);
    assert_panic_runtime(&logic.objects[&PANIC_UNIT], WAYPOINT_START, None);

    // Give the terminal InternalMove update a completed one-node path at the
    // current position. retry_path remains true: it is not a success gate.
    let unit = logic.objects.get_mut(&PANIC_UNIT).unwrap();
    unit.panic_runtime.as_mut().unwrap().current_waypoint_id = WAYPOINT_NEXT;
    let current = unit.get_position();
    unit.movement.path = vec![current];
    unit.movement.current_path_index = 0;
    unit.movement.target_position = None;
    unit.waiting_for_path = false;
    unit.retry_path = true;
    unit.path_goal_position = Some(current);
    unit.requested_destination = Some(current);
    unit.queue_for_path_frames = 7;
    unit.ignored_obstacle_id = Some(ObjectId(999_999));
    unit.try_one_more_repath = true;
    let rng_before = game_engine::common::random_value::get_game_logic_random_seed_state();
    let _restore_rng = RestoreLogicRng(rng_before);
    logic.tick_host_panic_states(&[PANIC_UNIT]);

    let unit = &logic.objects[&PANIC_UNIT];
    assert_eq!(unit.ai_state, AIState::Idle);
    assert_eq!(unit.completed_waypoint_labels, vec!["PanicTerminal"]);
    assert!(unit.movement.path.is_empty());
    assert_eq!(unit.movement.current_path_index, 0);
    assert_eq!(unit.movement.target_position, None);
    assert_eq!(unit.path_goal_position, Some(current));
    assert_eq!(unit.requested_destination, Some(current));
    assert_eq!(unit.queue_for_path_frames, 0);
    assert_eq!(unit.ignored_obstacle_id, None);
    assert!(!unit.waiting_for_path);
    assert!(unit.retry_path);
    assert!(unit.try_one_more_repath);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        rng_before,
        "RandomValue(0, -1) wraps to delta zero and consumes no draw"
    );
}

#[test]
fn panic_arrival_uses_selected_locomotor_close_enough_distance() {
    let _terrain = install_panic_fork_path();
    let mut logic = panic_logic();
    issue_authored_team_panic(&mut logic);

    let unit = logic.objects.get_mut(&PANIC_UNIT).unwrap();
    unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
    unit.close_enough_dist = None;
    let binding = crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(
        "BasicHumanLocomotor",
    )
    .expect("authored test locomotor binding");
    assert!(binding.close_enough_dist < 2.0);
    unit.path_extra_distance = 0.0;
    unit.panic_runtime.as_mut().unwrap().timer = 5;
    unit.panic_runtime.as_mut().unwrap().append_goal_position = false;
    unit.movement.path = vec![Vec3::new(binding.close_enough_dist + 0.25, 0.0, 0.0)];
    unit.movement.current_path_index = 0;
    unit.movement.target_position = None;
    unit.waiting_for_path = false;
    unit.is_blocked_and_stuck = false;
    unit.num_frames_blocked = 0;

    let rng_before = game_engine::common::random_value::get_game_logic_random_seed_state();
    let _restore_rng = RestoreLogicRng(rng_before);
    logic.tick_host_panic_states(&[PANIC_UNIT]);

    assert_panic_runtime(&logic.objects[&PANIC_UNIT], WAYPOINT_START, None);
    assert_eq!(
        logic.objects[&PANIC_UNIT]
            .panic_runtime
            .as_ref()
            .unwrap()
            .timer,
        4,
        "an arrival outside the selected locomotor's threshold does not advance, but AIPanic still ticks its timer"
    );
}

#[test]
fn blocked_panic_repath_runs_before_arrival_and_timer_processing() {
    let _terrain = install_panic_fork_path();
    let mut logic = panic_logic();
    issue_authored_team_panic(&mut logic);
    let terminal = *logic.objects[&PANIC_UNIT]
        .movement
        .path
        .last()
        .expect("authored panic route");
    {
        let unit = logic.objects.get_mut(&PANIC_UNIT).unwrap();
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.set_position(terminal);
        unit.movement.target_position = None;
        unit.waiting_for_path = false;
        unit.is_blocked_and_stuck = true;
        unit.num_frames_blocked = 2 * 30 + 1;
        unit.panic_runtime.as_mut().unwrap().timer = 5;
        unit.panic_runtime.as_mut().unwrap().append_goal_position = false;
    }

    let rng_before = game_engine::common::random_value::get_game_logic_random_seed_state();
    let _restore_rng = RestoreLogicRng(rng_before);
    logic.tick_host_panic_states(&[PANIC_UNIT]);

    let unit = &logic.objects[&PANIC_UNIT];
    assert_eq!(
        unit.panic_runtime.as_ref().unwrap().timer,
        4,
        "the AIPanic timer still runs after InternalMoveTo's blocked repath"
    );
    assert_ne!(
        unit.panic_runtime.as_ref().unwrap().current_waypoint_id,
        WAYPOINT_START,
        "a successful blocked recompute is followed by the canonical arrival check"
    );
}

#[test]
fn panic_waypoint_choice_and_next_rng_survive_save_load_continuation() {
    let _terrain = install_panic_fork_path();
    let mut source = panic_logic();
    issue_authored_team_panic(&mut source);
    {
        let unit = source.objects.get_mut(&PANIC_UNIT).unwrap();
        let last = *unit.movement.path.last().expect("authored fork path");
        unit.set_position(last);
    }

    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("capture forked panic path");
    let encoded = bincode_legacy::serialize(&snapshot).expect("encode panic path");
    let decoded: crate::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&encoded).expect("decode panic path");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("restore forked panic path");

    source.update_with_dt_budget(2.0 / 30.0, 2);
    restored.update_with_dt_budget(2.0 / 30.0, 2);
    let source_unit = &source.objects[&PANIC_UNIT];
    let restored_unit = &restored.objects[&PANIC_UNIT];
    assert_eq!(
        source_unit
            .panic_runtime
            .as_ref()
            .unwrap()
            .current_waypoint_id,
        restored_unit
            .panic_runtime
            .as_ref()
            .unwrap()
            .current_waypoint_id,
        "the two-link choice after load uses the saved world's next RNG draw"
    );
    assert_eq!(
        source.logic_random.seed_words(),
        restored.logic_random.seed_words(),
        "the continuation consumes the same owned RNG sequence after restore"
    );
}
