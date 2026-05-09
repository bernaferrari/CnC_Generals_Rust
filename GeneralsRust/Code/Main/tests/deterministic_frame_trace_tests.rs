use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::deterministic_trace::{
    calculate_frame_crc, first_trace_difference, run_trace_scenario, FrameTrace, TraceObject,
    TraceScenario,
};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, Weapon};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

fn command(
    command_id: u32,
    command_type: CommandType,
    selected_units: Vec<ObjectId>,
) -> GameCommand {
    GameCommand {
        command_type,
        player_id: 0,
        command_id,
        timestamp: UNIX_EPOCH + Duration::from_secs(command_id as u64),
        selected_units,
        modifier_keys: ModifierKeys::default(),
    }
}

fn seed() -> [u32; 6] {
    [
        0x12345678, 0x9abcdef0, 0x13579bdf, 0x2468ace0, 0xfedcba98, 0x76543210,
    ]
}

fn baseline_trace(command_order_reversed: bool) -> Vec<FrameTrace> {
    let mut frame_10_commands = vec![
        command(
            2,
            CommandType::AttackObject {
                target_id: ObjectId(20),
            },
            vec![ObjectId(10)],
        ),
        command(
            1,
            CommandType::MoveTo {
                destination: Vec3::new(128.0, 0.0, 256.0),
                waypoints: Vec::new(),
            },
            vec![ObjectId(10)],
        ),
    ];
    if command_order_reversed {
        frame_10_commands.reverse();
    }

    vec![
        FrameTrace::new(
            10,
            seed(),
            frame_10_commands,
            vec![
                trace_object(
                    ObjectId(20),
                    "GLAInfantryRebel",
                    Team::GLA,
                    256.0,
                    256.0,
                    100.0,
                ),
                trace_object(
                    ObjectId(10),
                    "AmericaVehicleHumvee",
                    Team::USA,
                    100.0,
                    200.0,
                    360.0,
                ),
            ],
            None,
        ),
        FrameTrace::new(
            11,
            seed(),
            Vec::new(),
            vec![
                trace_object(
                    ObjectId(10),
                    "AmericaVehicleHumvee",
                    Team::USA,
                    104.0,
                    204.0,
                    360.0,
                ),
                trace_object(
                    ObjectId(20),
                    "GLAInfantryRebel",
                    Team::GLA,
                    256.0,
                    256.0,
                    70.0,
                ),
            ],
            None,
        ),
    ]
}

fn trace_object(
    id: ObjectId,
    template: &str,
    team: Team,
    x: f32,
    z: f32,
    health: f32,
) -> TraceObject {
    TraceObject {
        id,
        template: template.to_string(),
        team,
        position: Vec3::new(x, 0.0, z),
        orientation: 0.0,
        health,
        max_health: health,
        status_bits: 0,
        ai_state: "Idle".to_string(),
        target: None,
        target_location: None,
        construction_percent: 1.0,
    }
}

fn test_template(name: &str, max_health: f32) -> ThingTemplate {
    let mut template = ThingTemplate::new(name);
    template
        .set_health(max_health)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Vehicle);
    template
}

fn traced_game_logic() -> (GameLogic, ObjectId, ObjectId) {
    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    game_logic.add_player(Player::new(1, Team::GLA, "GLA", false));
    game_logic.templates.insert(
        "TraceHumvee".to_string(),
        test_template("TraceHumvee", 360.0),
    );
    game_logic.templates.insert(
        "TraceTechnical".to_string(),
        test_template("TraceTechnical", 240.0),
    );

    let humvee = game_logic
        .create_object("TraceHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee should spawn");
    let technical = game_logic
        .create_object("TraceTechnical", Team::GLA, Vec3::new(35.0, 0.0, 0.0))
        .expect("technical should spawn");

    let humvee_weapon = Some(Weapon {
        damage: 25.0,
        range: 100.0,
        reload_time: 0.0,
        projectile_speed: 0.0,
        ..Weapon::default()
    });
    game_logic
        .get_objects_mut()
        .get_mut(&humvee)
        .expect("humvee exists")
        .weapon = humvee_weapon;

    (game_logic, humvee, technical)
}

#[test]
fn frame_trace_is_stable_across_command_and_object_ordering() {
    let trace_a = baseline_trace(false);
    let trace_b = baseline_trace(true);

    assert_eq!(trace_a, trace_b);
    assert_eq!(trace_a[0].commands[0].command_id, 1);
    assert_eq!(trace_a[0].objects[0].id, ObjectId(10));
}

#[test]
fn frame_trace_reports_first_divergent_frame() {
    let expected = baseline_trace(false);
    let mut actual = baseline_trace(false);
    actual[1].objects[1].health = 69.0;
    actual[1].crc = calculate_frame_crc(
        actual[1].frame,
        &actual[1].rng_seed,
        &actual[1].commands,
        &actual[1].objects,
        actual[1].victory_state.as_deref(),
    );

    let (expected_frame, actual_frame) =
        first_trace_difference(&expected, &actual).expect("frame 11 should diverge");

    assert_eq!(expected_frame.frame, 11);
    assert_eq!(actual_frame.frame, 11);
    assert_ne!(expected_frame.crc, actual_frame.crc);
}

#[test]
fn frame_trace_captures_real_game_logic_command_and_damage_frames() {
    let (mut game_logic, humvee, technical) = traced_game_logic();
    let attack = command(
        1,
        CommandType::AttackObject {
            target_id: technical,
        },
        vec![humvee],
    );

    game_logic.queue_command(attack.clone());
    game_logic.update();
    let frame_1 = FrameTrace::from_game_logic(&game_logic, seed(), vec![attack], None);

    game_logic.update();
    let frame_2 = FrameTrace::from_game_logic(&game_logic, seed(), Vec::new(), None);

    assert_eq!(frame_1.frame, 1);
    assert_eq!(frame_1.commands[0].command_id, 1);
    assert_ne!(frame_1.crc, frame_2.crc);

    let traced_technical = frame_2
        .objects
        .iter()
        .find(|object| object.id == technical)
        .expect("technical should be traced");
    assert!(traced_technical.health < 240.0);
}

#[test]
fn trace_scenario_runs_scheduled_commands_before_each_frame_capture() {
    let (mut game_logic, humvee, technical) = traced_game_logic();
    let scenario = TraceScenario::new(seed(), 3).with_commands(
        1,
        vec![command(
            7,
            CommandType::AttackObject {
                target_id: technical,
            },
            vec![humvee],
        )],
    );

    let trace = run_trace_scenario(&mut game_logic, &scenario);

    assert_eq!(trace.len(), 3);
    assert_eq!(trace[0].frame, 1);
    assert_eq!(trace[0].commands.len(), 1);
    assert_eq!(trace[0].commands[0].command_id, 7);
    assert!(trace[1].commands.is_empty());
    assert_ne!(trace[0].crc, trace[2].crc);

    let final_technical = trace[2]
        .objects
        .iter()
        .find(|object| object.id == technical)
        .expect("technical should be traced");
    assert!(final_technical.health < 240.0);
}
