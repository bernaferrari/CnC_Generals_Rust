use crc32fast::Hasher;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{ObjectId, Team};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
struct TraceObject {
    id: ObjectId,
    template: &'static str,
    team: Team,
    position: Vec3,
    health: u32,
    status_bits: u32,
}

#[derive(Debug, Clone, PartialEq)]
struct TraceCommand {
    player_id: u32,
    command_id: u32,
    command: String,
    selected_units: Vec<ObjectId>,
}

#[derive(Debug, Clone, PartialEq)]
struct FrameTrace {
    frame: u32,
    rng_seed: [u32; 6],
    commands: Vec<TraceCommand>,
    objects: Vec<TraceObject>,
    victory_state: Option<String>,
    crc: u32,
}

impl FrameTrace {
    fn new(
        frame: u32,
        rng_seed: [u32; 6],
        commands: Vec<GameCommand>,
        objects: Vec<TraceObject>,
        victory_state: Option<String>,
    ) -> Self {
        let mut commands: Vec<TraceCommand> = commands
            .into_iter()
            .map(TraceCommand::from_command)
            .collect();
        commands.sort_by_key(|command| (command.command_id, command.player_id));

        let mut objects = objects;
        objects.sort_by_key(|object| object.id);

        let crc = calculate_frame_crc(
            frame,
            &rng_seed,
            &commands,
            &objects,
            victory_state.as_deref(),
        );

        Self {
            frame,
            rng_seed,
            commands,
            objects,
            victory_state,
            crc,
        }
    }
}

impl TraceCommand {
    fn from_command(command: GameCommand) -> Self {
        let mut selected_units = command.selected_units;
        selected_units.sort();

        Self {
            player_id: command.player_id,
            command_id: command.command_id,
            command: summarize_command(&command.command_type),
            selected_units,
        }
    }
}

fn calculate_frame_crc(
    frame: u32,
    rng_seed: &[u32; 6],
    commands: &[TraceCommand],
    objects: &[TraceObject],
    victory_state: Option<&str>,
) -> u32 {
    let mut hasher = Hasher::new();

    hasher.update(b"FRAME");
    hasher.update(&frame.to_le_bytes());

    hasher.update(b"RNG");
    for seed in rng_seed {
        hasher.update(&seed.to_le_bytes());
    }

    hasher.update(b"COMMANDS");
    for command in commands {
        hasher.update(&command.player_id.to_le_bytes());
        hasher.update(&command.command_id.to_le_bytes());
        hasher.update(command.command.as_bytes());
        for unit in &command.selected_units {
            hasher.update(&unit.0.to_le_bytes());
        }
    }

    hasher.update(b"OBJECTS");
    for object in objects {
        hasher.update(&object.id.0.to_le_bytes());
        hasher.update(object.template.as_bytes());
        hasher.update(object.team.get_name().as_bytes());
        hasher.update(&object.position.x.to_le_bytes());
        hasher.update(&object.position.y.to_le_bytes());
        hasher.update(&object.position.z.to_le_bytes());
        hasher.update(&object.health.to_le_bytes());
        hasher.update(&object.status_bits.to_le_bytes());
    }

    hasher.update(b"VICTORY");
    if let Some(victory_state) = victory_state {
        hasher.update(victory_state.as_bytes());
    }

    hasher.finalize()
}

fn summarize_command(command: &CommandType) -> String {
    match command {
        CommandType::MoveTo { destination, .. } => format!(
            "MoveTo:{:.3},{:.3},{:.3}",
            destination.x, destination.y, destination.z
        ),
        CommandType::AttackObject { target_id } => format!("AttackObject:{}", target_id.0),
        CommandType::DozerConstruct {
            template_name,
            location,
        } => format!(
            "DozerConstruct:{}:{:.3},{:.3},{:.3}",
            template_name, location.x, location.y, location.z
        ),
        other => format!("{other:?}"),
    }
}

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

fn baseline_trace(command_order_reversed: bool) -> Vec<FrameTrace> {
    let seed = [
        0x12345678, 0x9abcdef0, 0x13579bdf, 0x2468ace0, 0xfedcba98, 0x76543210,
    ];

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
            seed,
            frame_10_commands,
            vec![
                TraceObject {
                    id: ObjectId(20),
                    template: "GLAInfantryRebel",
                    team: Team::GLA,
                    position: Vec3::new(256.0, 0.0, 256.0),
                    health: 100,
                    status_bits: 0,
                },
                TraceObject {
                    id: ObjectId(10),
                    template: "AmericaVehicleHumvee",
                    team: Team::USA,
                    position: Vec3::new(100.0, 0.0, 200.0),
                    health: 360,
                    status_bits: 0,
                },
            ],
            None,
        ),
        FrameTrace::new(
            11,
            seed,
            Vec::new(),
            vec![
                TraceObject {
                    id: ObjectId(10),
                    template: "AmericaVehicleHumvee",
                    team: Team::USA,
                    position: Vec3::new(104.0, 0.0, 204.0),
                    health: 360,
                    status_bits: 1,
                },
                TraceObject {
                    id: ObjectId(20),
                    template: "GLAInfantryRebel",
                    team: Team::GLA,
                    position: Vec3::new(256.0, 0.0, 256.0),
                    health: 70,
                    status_bits: 0,
                },
            ],
            None,
        ),
    ]
}

fn first_trace_difference<'a>(
    left: &'a [FrameTrace],
    right: &'a [FrameTrace],
) -> Option<(&'a FrameTrace, &'a FrameTrace)> {
    left.iter()
        .zip(right.iter())
        .find(|(left_frame, right_frame)| left_frame.crc != right_frame.crc)
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
    actual[1].objects[1].health = 69;
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
