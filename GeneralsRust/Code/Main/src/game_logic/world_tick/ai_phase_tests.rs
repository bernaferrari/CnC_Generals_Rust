//! Exercise the ordinary match driver, including foreign compatibility state.

use super::*;
use crate::game_logic::{KindOf, Object, Player, Team, ThingTemplate};
use std::io::Read;
use std::process::{Command, Stdio};

// These tests deliberately seed compatibility singletons. An exact child
// process keeps that evidence independent of the rest of Main's test suite.
fn isolated(name: &str) -> bool {
    const MARKER: &str = "GENERALS_OWNED_AI_PHASE_TEST";
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::{name}");
    if std::env::var(MARKER).as_deref() == Ok(name.as_str()) {
        return false;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([&name, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, &name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output =
        readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
    assert!(!timed_out, "{name} exceeded deadline: {output:?}");
    assert!(status.success(), "{name}: {output:?}");
    assert!(
        output[0].contains("1 passed; 0 failed"),
        "exact child ran no regression: {output:?}"
    );
    true
}

fn foreign_ai() -> String {
    gamelogic::ai::integration::with_ai_integration(|manager| {
        format!(
            "{}:{}",
            manager.get_ai_player_count(),
            manager.has_ai_player(77)
        )
    })
    .expect("foreign AI manager")
}

fn seed_foreign_ai() {
    gamelogic::ai::integration::initialize_ai_integration().unwrap();
    gamelogic::ai::integration::with_ai_integration_mut(|manager| {
        manager.ensure_ai_player(77, false);
    })
    .unwrap();
}

fn match_with_player() -> GameLogic {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Human", true));
    logic.add_player(Player::new(1, Team::China, "Computer", false));
    logic
}

fn queued_mover(start: Vec3, destination: Vec3) -> GameLogic {
    let mut logic = match_with_player();
    logic.force_map_loaded_for_path_test(true);
    let mut template = ThingTemplate::new("AIPhaseInfantry");
    template.add_kind_of(KindOf::Infantry);
    let mut mover = Object::new(template, ObjectId(301), Team::USA);
    mover.owner_player_id = Some(0);
    mover.movement.max_speed = 30.0;
    mover.set_position(start);
    // Initial facing is already along the requested path. This regression
    // checks scheduling/isolation rather than the locomotor's turning delay.
    let heading = destination - start;
    mover.set_orientation(heading.z.atan2(heading.x));
    logic.objects.insert(mover.id, mover);
    assert!(logic.unit_command_move_to(ObjectId(301), destination));
    assert!(logic.objects[&ObjectId(301)].waiting_for_path);
    assert_eq!(logic.pathfinding_system.pending_path_count(), 1);
    logic
}

#[test]
fn match_construction_and_skirmish_setup_do_not_reset_foreign_ai() {
    if isolated("match_construction_and_skirmish_setup_do_not_reset_foreign_ai") {
        return;
    }
    seed_foreign_ai();
    let before = foreign_ai();
    let mut first = match_with_player();
    let mut second = match_with_player();
    assert_eq!(foreign_ai(), before, "constructors must be inert");
    first.setup_skirmish_ai(0);
    second.setup_skirmish_ai(0);
    assert_eq!(first.host_ai_player_count(), 1);
    assert_eq!(second.host_ai_player_count(), 1);
    assert_eq!(
        foreign_ai(),
        before,
        "a match's setup must not reset another AI manager"
    );
}

#[test]
fn skirmish_rules_and_wall_height_belong_to_the_driving_match() {
    if isolated("skirmish_rules_and_wall_height_belong_to_the_driving_match") {
        return;
    }
    let mut first = match_with_player();
    let mut second = match_with_player();
    for (logic, repulsed, wall) in [(&mut first, 40.0, 21.0), (&mut second, 90.0, 55.0)] {
        let mut rules = logic.engine_stores.ai_data().write().unwrap();
        rules.ensure_base();
        let data = rules.get_active_mut().unwrap();
        data.repulsed_distance = repulsed;
        data.wall_height = wall;
    }
    for (logic, repulsed, wall) in [(&mut first, 40.0, 21.0), (&mut second, 90.0, 55.0)] {
        logic.setup_skirmish_ai(0);
        {
            let ai = logic.engine_stores.ai().read().unwrap();
            assert_eq!(ai.get_ai_data().repulsed_distance, repulsed);
            assert_eq!(ai.get_ai_data().wall_height, wall);
        }
        let mut template = ThingTemplate::new("AIPhaseWall");
        template.add_kind_of(KindOf::WalkOnTopOfWall);
        logic.templates.insert(template.name.clone(), template);
        logic
            .create_object("AIPhaseWall", Team::Neutral, Vec3::ZERO)
            .unwrap();
        assert_eq!(logic.pathfinding_system.wall_height(), wall);
    }
    assert_eq!(first.pathfinding_system.wall_height(), 21.0);
    assert_eq!(second.pathfinding_system.wall_height(), 55.0);
}

#[test]
fn interleaved_matches_tick_only_their_owned_ai() {
    if isolated("interleaved_matches_tick_only_their_owned_ai") {
        return;
    }
    let mut first = match_with_player();
    let mut second = match_with_player();
    first.add_ai_opponent(1, Team::China, AIDifficulty::Easy);
    second.add_ai_opponent(1, Team::China, AIDifficulty::Hard);
    seed_foreign_ai();
    let foreign = foreign_ai();
    let second_initial = format!("{:?}", second.snapshot_host_ai_players_for_save());
    first.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_eq!(second.frame, 0);
    assert_eq!(
        format!("{:?}", second.snapshot_host_ai_players_for_save()),
        second_initial
    );
    assert_eq!(
        foreign_ai(),
        foreign,
        "ordinary ticks must not drive compatibility AI"
    );
    let first_initial = format!("{:?}", first.snapshot_host_ai_players_for_save());
    second.update_with_dt(LOGIC_FRAME_TIMESTEP);
    second.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_eq!(first.frame, 1);
    assert_eq!(second.frame, 2);
    assert_eq!(
        format!("{:?}", first.snapshot_host_ai_players_for_save()),
        first_initial
    );
    assert_eq!(first.ai_manager.ai_players[&1].last_update_time, 0.0);
    assert_eq!(
        second.ai_manager.ai_players[&1].last_update_time,
        LOGIC_FRAME_TIMESTEP
    );
    assert_eq!(foreign_ai(), foreign);
}

#[test]
fn frozen_steps_preserve_queued_paths_until_the_owned_ai_phase() {
    if isolated("frozen_steps_preserve_queued_paths_until_the_owned_ai_phase") {
        return;
    }
    let start = Vec3::new(-100.0, 0.0, 0.0);
    let mut logic = queued_mover(start, Vec3::new(-30.0, 0.0, 0.0));
    logic.set_script_time_frozen_for_test(true);
    logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_eq!(logic.frame, 0);
    assert_eq!(
        logic.pathfinding_system.pending_path_count(),
        1,
        "GameLogic.cpp:3616 returns before TheAI->UPDATE"
    );
    assert!(logic.objects[&ObjectId(301)].waiting_for_path);
    assert_eq!(logic.objects[&ObjectId(301)].get_position(), start);
    logic.set_script_time_frozen_for_test(false);
    logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_eq!(logic.frame, 1);
    assert_eq!(logic.pathfinding_system.pending_path_count(), 0);
    assert!(!logic.objects[&ObjectId(301)].waiting_for_path);
    assert_eq!(
        logic.objects[&ObjectId(301)].get_position(),
        start,
        "AI.cpp:338 drains after object movement, not before it"
    );
    logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_ne!(logic.objects[&ObjectId(301)].get_position(), start);
}

#[test]
fn frozen_steps_do_not_reissue_deferred_move_orders() {
    if isolated("frozen_steps_do_not_reissue_deferred_move_orders") {
        return;
    }
    let start = Vec3::new(-100.0, 0.0, 0.0);
    let destination = start + Vec3::new(70.0, 0.0, 0.0);
    let mut logic = queued_mover(start, destination);
    logic.pathfinding_system.take_pending_paths();
    let mover = logic.objects.get_mut(&ObjectId(301)).unwrap();
    mover.waiting_for_path = false;
    mover.pending_move = Some(destination);
    logic.set_script_time_frozen_for_test(true);
    logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_eq!(
        logic.objects[&ObjectId(301)].pending_move,
        Some(destination)
    );
    assert_eq!(logic.pathfinding_system.pending_path_count(), 0);
    logic.set_script_time_frozen_for_test(false);
    logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    let mover = &logic.objects[&ObjectId(301)];
    assert_eq!(mover.pending_move, None);
    assert!(!mover.waiting_for_path);
    assert!(!mover.movement.path.is_empty());
    assert_eq!(mover.get_position(), start);
}

#[test]
fn same_object_ids_keep_separate_paths_and_move_on_the_following_tick() {
    if isolated("same_object_ids_keep_separate_paths_and_move_on_the_following_tick") {
        return;
    }
    let a = Vec3::new(-100.0, 0.0, 0.0);
    let b = Vec3::new(100.0, 0.0, 0.0);
    let mut first = queued_mover(a, a + Vec3::new(70.0, 0.0, 0.0));
    let mut second = queued_mover(b, b - Vec3::new(70.0, 0.0, 0.0));
    first.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_eq!(first.objects[&ObjectId(301)].get_position(), a);
    assert_eq!(second.pathfinding_system.pending_path_count(), 1);
    assert_eq!(second.objects[&ObjectId(301)].get_position(), b);
    second.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert_eq!(second.objects[&ObjectId(301)].get_position(), b);
    first.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert!(first.objects[&ObjectId(301)].get_position().x > a.x);
    assert_eq!(second.objects[&ObjectId(301)].get_position(), b);
    second.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert!(second.objects[&ObjectId(301)].get_position().x < b.x);
}

#[test]
fn owned_ai_repair_commands_follow_cpp_player_index_order() {
    if isolated("owned_ai_repair_commands_follow_cpp_player_index_order") {
        return;
    }
    use crate::command_system::CommandType;
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

    fn scenario(order: &[u32]) -> Vec<u32> {
        let mut logic = GameLogic::new();
        let mut dozer = ThingTemplate::new("AIPhaseRepairDozer");
        dozer.add_kind_of(KindOf::Dozer);
        logic.templates.insert(dozer.name.clone(), dozer);
        let mut bridge = ThingTemplate::new("AIPhaseRepairBridge");
        bridge.add_kind_of(KindOf::Structure).set_health(100.0);
        logic.templates.insert(bridge.name.clone(), bridge);
        let mut expected = std::collections::HashMap::new();
        for &id in order {
            let position = Vec3::new(id as f32 * 20.0, 0.0, 0.0);
            logic.add_player(Player::new(id, Team::USA, "Repair AI", false));
            let dozer = logic
                .create_object("AIPhaseRepairDozer", Team::USA, position)
                .unwrap();
            logic.objects.get_mut(&dozer).unwrap().owner_player_id = Some(id);
            let bridge = logic
                .create_object("AIPhaseRepairBridge", Team::USA, position)
                .unwrap();
            let object = logic.objects.get_mut(&bridge).unwrap();
            object.owner_player_id = Some(id);
            object.health.current = 20.0;
            object.body_damage_state = HostBodyDamageType::ReallyDamaged;
            logic.add_ai_opponent(id, Team::USA, AIDifficulty::Easy);
            let mut ai = logic.ai_manager.ai_players.remove(&id).unwrap();
            // Exercise the actual AI request and callback. Other policy timers
            // are held in the future so this scenario isolates repair ordering.
            ai.next_building_time = 100.0;
            ai.next_team_queue_time = 100.0;
            ai.next_team_time = 100.0;
            ai.repair_structure(&logic, bridge);
            logic.ai_manager.ai_players.insert(id, ai);
            expected.insert(id, (dozer, bridge));
        }
        let rng = logic.logic_random.seed_words();
        logic.update_match_ai();
        let trace: Vec<_> = logic
            .command_queue
            .iter()
            .filter_map(|command| {
                if let CommandType::Repair { target_id } = command.command_type {
                    let &(dozer, bridge) = expected.get(&command.player_id).unwrap();
                    assert_eq!(target_id, bridge);
                    assert_eq!(command.selected_units, vec![dozer]);
                    Some(command.player_id)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            trace,
            vec![1, 2, 3, 4, 5, 6, 7, 8],
            "PlayerList.cpp:221-228 executes shared-world effects by ascending player index"
        );
        assert_eq!(
            logic.logic_random.seed_words(),
            rng,
            "repair scheduling introduces no RNG draws"
        );
        logic.process_commands();
        for &(dozer, bridge) in expected.values() {
            assert_eq!(
                logic.objects[&dozer].target,
                Some(bridge),
                "actual queued repair was applied"
            );
        }
        trace
    }

    let forward = scenario(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let reverse = scenario(&[8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(
        forward, reverse,
        "construction order must not choose the AI execution order"
    );
}

#[test]
fn idle_wings_circle_without_a_second_altitude_step() {
    if isolated("idle_wings_circle_without_a_second_altitude_step") {
        return;
    }
    // C++ Locomotor.cpp:2488-2524 circles in XY; maintainCurrentPosition
    // applies handleBehaviorZ once afterwards. Host coordinates are Y-up.
    let mut template = ThingTemplate::new("HqOwnedWingsAltitude");
    template.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(template, ObjectId(46300), Team::USA);
    jet.set_position(Vec3::new(80.0, 100.0, 80.0));
    jet.loco_appearance = LocomotorAppearance::Wings;
    jet.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
    jet.loco_preferred_height = 100.0;
    jet.loco_preferred_height_damping = 1.0;
    jet.max_lift = 300.0 / 900.0;
    jet.speed_limit_z = 1.0;
    jet.min_speed = 20.0;
    jet.circling_radius = 40.0;
    jet.movement.max_speed = 80.0;
    jet.movement.acceleration = 100.0;
    jet.movement.velocity = Vec3::new(20.0, 2.0, 0.0);
    jet.motive_frames_remaining = 10;
    jet.maintain_pos = Some(jet.get_position());
    jet.maintain_pos_valid = true;
    let start = jet.get_position();
    let mut altitude_only = jet.clone();
    GameLogic::apply_live_handle_behavior_z_for_test(&mut altitude_only, 0.0, Some(start.y));
    let _ = jet.loco_maintain_appearance(LOGIC_FRAME_TIMESTEP);
    assert_eq!(
        jet.get_position().y,
        start.y,
        "appearance maintenance is horizontal only"
    );
    assert!(
        Vec3::new(
            jet.get_position().x - start.x,
            0.0,
            jet.get_position().z - start.z
        )
        .length()
            > 0.05,
        "Wings must still circle horizontally"
    );
    GameLogic::apply_live_handle_behavior_z_for_test(&mut jet, 0.0, Some(start.y));
    assert!(
        (jet.get_position().y - altitude_only.get_position().y).abs() < 0.0001,
        "one altitude pass: actual={:?}, expected={:?}",
        jet.get_position(),
        altitude_only.get_position()
    );
}
