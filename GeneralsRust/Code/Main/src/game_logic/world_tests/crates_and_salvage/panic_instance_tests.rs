//! Same-ID instance isolation for the native Panic continuation.
use super::*;

const SEED_A: u32 = 0xA11C_E001;
const SEED_B: u32 = 0xB22D_F002;

#[derive(Debug, PartialEq)]
struct PanicObservation {
    ai_state: AIState,
    panic: Option<crate::game_logic::object::PanicRuntime>,
    path: Vec<Vec3>,
    target: Option<Vec3>,
    current_path_index: usize,
    requested_destination: Option<Vec3>,
    path_goal: Option<Vec3>,
    waiting_for_path: bool,
    path_timestamp: u32,
    adjust_destinations: bool,
    path_extra_distance: f32,
    retry_path: bool,
    try_one_more_repath: bool,
    is_blocked_and_stuck: bool,
    num_frames_blocked: u32,
    position: Vec3,
    velocity: Vec3,
    frame: u32,
    rng_words: [u32; 6],
}

fn seeded_panic_logic(seed: u32) -> GameLogic {
    let mut logic = panic_logic();
    // New-match updates first wake at frame 1. Admit the authored order
    // after the initial frame so the next real step executes Movement.
    logic.update_with_dt_budget(1.0 / 30.0, 1);
    logic.logic_random.seed_random(seed);
    logic.logic_base_seed = game_engine::common::random_value::get_game_logic_random_seed();
    // This fixture issues its authored request outside the ordinary script
    // phase. Give that synchronous operation the same owned stream as a tick.
    let mut random = std::mem::take(&mut logic.logic_random);
    game_engine::common::random_value::with_logic_rng_owner(&mut random, || {
        issue_authored_team_panic(&mut logic);
    });
    logic.logic_random = random;
    let unit = logic.objects.get_mut(&PANIC_UNIT).expect("panic unit");
    unit.wander_width_factor = 30.0;
    let path_end = *unit.movement.path.last().expect("initial waypoint path");
    unit.set_position(path_end);
    assert_panic_arrival_preconditions(unit);
    logic
}

fn panic_observation(logic: &GameLogic) -> PanicObservation {
    let unit = logic.objects.get(&PANIC_UNIT).expect("panic unit");
    PanicObservation {
        ai_state: unit.ai_state.clone(),
        panic: unit.panic_runtime.clone(),
        path: unit.movement.path.clone(),
        target: unit.movement.target_position,
        current_path_index: unit.movement.current_path_index,
        requested_destination: unit.requested_destination,
        path_goal: unit.path_goal_position,
        waiting_for_path: unit.waiting_for_path,
        path_timestamp: unit.path_timestamp,
        adjust_destinations: unit.adjust_destinations,
        path_extra_distance: unit.path_extra_distance,
        retry_path: unit.retry_path,
        try_one_more_repath: unit.try_one_more_repath,
        is_blocked_and_stuck: unit.is_blocked_and_stuck,
        num_frames_blocked: unit.num_frames_blocked,
        position: unit.get_position(),
        velocity: unit.movement.velocity,
        frame: logic.get_frame(),
        rng_words: logic.logic_random.seed_words(),
    }
}

fn isolated_observations(seed: u32) -> [PanicObservation; 2] {
    let mut logic = seeded_panic_logic(seed);
    logic.update_with_dt_budget(1.0 / 30.0, 1);
    let first = panic_observation(&logic);
    let panic = first.panic.as_ref().expect("panic remains active");
    assert_ne!(
        panic.current_waypoint_id, WAYPOINT_START,
        "the fixed step must select a linked waypoint"
    );
    assert_ne!(
        panic.group_offset,
        glam::Vec2::ZERO,
        "the waypoint transition must exercise seeded wander draws"
    );
    logic.update_with_dt_budget(1.0 / 30.0, 1);
    [first, panic_observation(&logic)]
}

#[test]
fn same_id_panic_worlds_interleave_and_restore_without_crossing_state_or_rng() {
    let _terrain = install_panic_fork_path();
    // Capture actual one-world baselines before either interleaved owner is
    // created. Keep plain observations, not cloned mutable GameLogic worlds.
    let baseline_a = isolated_observations(SEED_A);
    let baseline_b = isolated_observations(SEED_B);

    let mut world_a = seeded_panic_logic(SEED_A);
    let a_initial = panic_observation(&world_a);
    let mut world_b = seeded_panic_logic(SEED_B);
    let b_initial = panic_observation(&world_b);

    assert_eq!(
        world_a.objects[&PANIC_UNIT].id,
        world_b.objects[&PANIC_UNIT].id
    );
    assert_ne!(
        world_a.logic_random.seed_words(),
        world_b.logic_random.seed_words()
    );
    let _unrelated_world = GameLogic::new();
    assert_eq!(
        panic_observation(&world_a),
        a_initial,
        "constructing another world does not replace an existing same-ID continuation"
    );
    assert_eq!(
        panic_observation(&world_b),
        b_initial,
        "constructing another world does not alter its existing same-ID continuation"
    );

    // Each world reaches the first waypoint on its own real fixed step. The
    // selected branch and offset consume that world's scoped RNG stream.
    world_a.update_with_dt_budget(1.0 / 30.0, 1);
    assert_eq!(panic_observation(&world_a), baseline_a[0], "first A step");
    assert_eq!(
        panic_observation(&world_b),
        b_initial,
        "A step leaves B untouched"
    );
    world_b.update_with_dt_budget(1.0 / 30.0, 1);
    assert_eq!(panic_observation(&world_b), baseline_b[0], "first B step");

    // Restore A while both same-ID owners remain live. The restored world is
    // another independent owner and must resume the exact saved continuation.
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&world_a)
        .expect("snapshot world A");
    let encoded = bincode_legacy::serialize(&snapshot).expect("encode world A");
    let decoded: crate::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&encoded).expect("decode world A");
    let mut restored_a = GameLogic::new();
    restored_a.templates = world_a.templates.clone();
    builder
        .restore_from_snapshot(&decoded, &mut restored_a)
        .expect("restore world A with world B still alive");
    assert_eq!(panic_observation(&restored_a), baseline_a[0], "restored A");
    assert_eq!(
        panic_observation(&world_b),
        baseline_b[0],
        "restoring A does not alter live B"
    );

    world_a.update_with_dt_budget(1.0 / 30.0, 1);
    world_b.update_with_dt_budget(1.0 / 30.0, 1);
    restored_a.update_with_dt_budget(1.0 / 30.0, 1);
    assert_eq!(panic_observation(&world_a), baseline_a[1], "second A step");
    assert_eq!(
        panic_observation(&restored_a),
        baseline_a[1],
        "restored A's next step matches its isolated owner"
    );
    assert_eq!(panic_observation(&world_b), baseline_b[1], "second B step");
}

#[test]
fn panic_snapshot_rejects_invalid_internal_move_layer_ordinals() {
    let _terrain = install_panic_waypoint_path();
    let mut source = panic_logic();
    issue_authored_team_panic(&mut source);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("capture panic state");
    let encoded = bincode_legacy::serialize(&snapshot).expect("encode panic snapshot");

    // C++ AIFollowWaypointPathState::computeGoal emits ground or wall for a
    // waypoint. Exercise both the invalid sentinel and an out-of-range byte.
    for invalid_layer in [0u8, 255u8] {
        let mut malformed: crate::save_load::snapshot::WorldSnapshot =
            bincode_legacy::deserialize(&encoded).expect("decode fresh panic snapshot");
        let Some(crate::save_load::snapshot::ModuleSnapshot::AIUpdate(module)) = malformed
            .objects
            .get_mut(&PANIC_UNIT)
            .and_then(|object| object.modules.get_mut("AIUpdate"))
        else {
            panic!("panic module snapshot exists");
        };
        let encoded_state = module
            .state_machine_data
            .get("AIPanicState")
            .expect("panic continuation exists");
        let mut state: crate::game_logic::object::PanicSaveState =
            serde_json::from_str(encoded_state).expect("decode valid panic continuation");
        state.panic.goal_layer = invalid_layer;
        module.state_machine_data.insert(
            "AIPanicState".into(),
            serde_json::to_string(&state).expect("encode malformed panic layer"),
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        assert!(
            builder
                .restore_from_snapshot(&malformed, &mut restored)
                .is_err(),
            "invalid C++ goal-layer value {invalid_layer} must not be restored"
        );
    }
}

#[test]
fn panic_snapshot_rejects_unreachable_timer_and_nonfinite_width_capsules() {
    let _terrain = install_panic_waypoint_path();
    let mut source = panic_logic();
    issue_authored_team_panic(&mut source);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("capture panic state");
    let encoded = bincode_legacy::serialize(&snapshot).expect("encode panic snapshot");

    let valid_wait = 10 + (PANIC_UNIT.0 & 0x7) as i32;
    let malformed_cases = [
        (i32::MIN, valid_wait),
        (-1, valid_wait),
        (valid_wait + 1, valid_wait),
        (0, valid_wait + 1),
    ];
    for (timer, wait_frames) in malformed_cases {
        let mut malformed: crate::save_load::snapshot::WorldSnapshot =
            bincode_legacy::deserialize(&encoded).expect("decode fresh panic snapshot");
        let Some(crate::save_load::snapshot::ModuleSnapshot::AIUpdate(module)) = malformed
            .objects
            .get_mut(&PANIC_UNIT)
            .and_then(|object| object.modules.get_mut("AIUpdate"))
        else {
            panic!("panic module snapshot exists");
        };
        let encoded_state = module
            .state_machine_data
            .get("AIPanicState")
            .expect("panic continuation exists");
        let mut state: crate::game_logic::object::PanicSaveState =
            serde_json::from_str(encoded_state).expect("decode valid panic continuation");
        state.panic.timer = timer;
        state.panic.wait_frames = wait_frames;
        module.state_machine_data.insert(
            "AIPanicState".into(),
            serde_json::to_string(&state).expect("encode malformed panic timer"),
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        assert!(
            builder
                .restore_from_snapshot(&malformed, &mut restored)
                .is_err(),
            "unreachable panic timer state ({timer}, {wait_frames}) must be rejected"
        );
    }

    // A finite JSON number can overflow f32 on decode. Reject it before
    // it can enter waypoint offset arithmetic; absent widths remain valid.
    let mut malformed = snapshot;
    let Some(crate::save_load::snapshot::ModuleSnapshot::AIUpdate(module)) = malformed
        .objects
        .get_mut(&PANIC_UNIT)
        .and_then(|object| object.modules.get_mut("AIUpdate"))
    else {
        panic!("panic module snapshot exists");
    };
    let mut state: serde_json::Value = serde_json::from_str(
        module
            .state_machine_data
            .get("AIPanicState")
            .expect("panic continuation exists"),
    )
    .expect("decode valid panic continuation");
    state["wander_width_factor"] = serde_json::json!(1e39);
    module
        .state_machine_data
        .insert("AIPanicState".into(), state.to_string());
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    assert!(
        builder
            .restore_from_snapshot(&malformed, &mut restored)
            .is_err()
    );
}

fn moving_path_snapshot() -> crate::save_load::snapshot::WorldSnapshot {
    let mut source = panic_logic();
    let unit = source.objects.get_mut(&PANIC_UNIT).expect("moving unit");
    unit.set_ai_state(AIState::Moving);
    unit.movement.path = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(30.0, 0.0, 0.0),
        Vec3::new(60.0, 0.0, 0.0),
    ];
    unit.movement.current_path_index = 1;
    unit.movement.target_position = Some(Vec3::new(60.0, 0.0, 0.0));
    unit.requested_destination = Some(Vec3::new(75.0, 0.0, 0.0));
    crate::save_load::snapshot::SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .expect("capture ordinary moving path")
}

#[test]
fn complete_world_restore_keeps_full_ordinary_moving_path_and_index() {
    let snapshot = moving_path_snapshot();
    let mut restored = GameLogic::new();
    restored.templates = panic_logic().templates.clone();
    crate::save_load::snapshot::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore ordinary moving path");

    let unit = restored.objects.get(&PANIC_UNIT).expect("restored unit");
    assert_eq!(unit.ai_state, AIState::Moving);
    assert_eq!(
        unit.movement.path,
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(30.0, 0.0, 0.0),
            Vec3::new(60.0, 0.0, 0.0),
        ],
        "ObjectSnapshot restores the full path, not TMAI's remaining suffix"
    );
    assert_eq!(unit.movement.current_path_index, 1);
    assert_eq!(
        unit.movement.target_position,
        Some(Vec3::new(60.0, 0.0, 0.0))
    );
    assert_eq!(
        unit.requested_destination,
        Some(Vec3::new(75.0, 0.0, 0.0)),
        "TMAI still supplies the AI-only requested destination"
    );
}

#[test]
fn complete_world_restore_does_not_resurrect_authoritative_empty_movement() {
    let mut snapshot = moving_path_snapshot();
    let object = snapshot
        .objects
        .get_mut(&PANIC_UNIT)
        .expect("snapshot object");
    object.movement.path.clear();
    object.movement.current_path_index = 0;
    object.movement.target_position = None;
    // UnitSnapshot retains its legacy nonempty waypoints to prove that it
    // cannot override the authoritative ObjectSnapshot Movement either.
    match &mut object.object_type {
        crate::save_load::snapshot::ObjectTypeSnapshot::Unit(unit) => {
            assert!(
                !unit.waypoints.is_empty(),
                "legacy waypoint mirror is present"
            );
        }
        _ => panic!("fixture is a unit"),
    }
    snapshot.pathfinding_cache.cached_paths.clear();
    snapshot.pathfinding_cache.cache_timestamps.clear();

    let mut restored = GameLogic::new();
    restored.templates = panic_logic().templates.clone();
    crate::save_load::snapshot::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore authoritative empty movement");

    let unit = restored.objects.get(&PANIC_UNIT).expect("restored unit");
    assert!(
        unit.movement.path.is_empty(),
        "TMAI must not resurrect a path"
    );
    assert_eq!(unit.movement.current_path_index, 0);
    assert_eq!(unit.movement.target_position, None);
    assert_eq!(
        unit.requested_destination,
        Some(Vec3::new(75.0, 0.0, 0.0)),
        "the non-Movement TMAI residual remains available"
    );
}
