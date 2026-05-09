use crate::command_system::{CommandType, GameCommand};
use crate::game_logic::{AIState, GameLogic, Object, ObjectId, Team};
use crc32fast::Hasher;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Canonical per-object state used by deterministic gameplay trace comparisons.
///
/// The fields intentionally mirror gameplay-visible state instead of renderer
/// internals so the same schema can be produced by the C++ engine and by Rust.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceObject {
    pub id: ObjectId,
    pub template: String,
    pub team: Team,
    pub position: Vec3,
    pub orientation: f32,
    pub health: f32,
    pub max_health: f32,
    pub status_bits: u32,
    pub ai_state: String,
    pub target: Option<ObjectId>,
    pub target_location: Option<Vec3>,
    pub construction_percent: f32,
}

impl TraceObject {
    pub fn from_object(object: &Object) -> Self {
        Self {
            id: object.id,
            template: object.template_name.clone(),
            team: object.team,
            position: object.get_position(),
            orientation: object.get_orientation(),
            health: object.health.current,
            max_health: object.max_health,
            status_bits: object_status_bits(object),
            ai_state: summarize_ai_state(&object.ai_state),
            target: object.target,
            target_location: object.target_location,
            construction_percent: object.construction_percent,
        }
    }
}

/// Canonical command input for a frame. Selected units are sorted so equivalent
/// command batches produce the same hash even when built from unordered state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceCommand {
    pub player_id: u32,
    pub command_id: u32,
    pub command: String,
    pub selected_units: Vec<ObjectId>,
}

impl TraceCommand {
    pub fn from_command(command: GameCommand) -> Self {
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

/// One deterministic gameplay frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameTrace {
    pub frame: u32,
    pub rng_seed: [u32; 6],
    pub commands: Vec<TraceCommand>,
    pub objects: Vec<TraceObject>,
    pub victory_state: Option<String>,
    pub crc: u32,
}

impl FrameTrace {
    pub fn new(
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

    pub fn from_game_logic(
        game_logic: &GameLogic,
        rng_seed: [u32; 6],
        commands: Vec<GameCommand>,
        victory_state: Option<String>,
    ) -> Self {
        let objects = game_logic
            .get_objects()
            .values()
            .map(TraceObject::from_object)
            .collect();

        Self::new(
            game_logic.get_frame(),
            rng_seed,
            commands,
            objects,
            victory_state,
        )
    }
}

pub fn first_trace_difference<'a>(
    left: &'a [FrameTrace],
    right: &'a [FrameTrace],
) -> Option<(&'a FrameTrace, &'a FrameTrace)> {
    left.iter()
        .zip(right.iter())
        .find(|(left_frame, right_frame)| left_frame.crc != right_frame.crc)
}

pub fn calculate_frame_crc(
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
    hasher.update(&(commands.len() as u32).to_le_bytes());
    for command in commands {
        hasher.update(&command.player_id.to_le_bytes());
        hasher.update(&command.command_id.to_le_bytes());
        hash_str(&mut hasher, &command.command);
        hasher.update(&(command.selected_units.len() as u32).to_le_bytes());
        for unit in &command.selected_units {
            hasher.update(&unit.0.to_le_bytes());
        }
    }

    hasher.update(b"OBJECTS");
    hasher.update(&(objects.len() as u32).to_le_bytes());
    for object in objects {
        hasher.update(&object.id.0.to_le_bytes());
        hash_str(&mut hasher, &object.template);
        hash_str(&mut hasher, object.team.get_name());
        hash_vec3(&mut hasher, object.position);
        hasher.update(&object.orientation.to_le_bytes());
        hasher.update(&object.health.to_le_bytes());
        hasher.update(&object.max_health.to_le_bytes());
        hasher.update(&object.status_bits.to_le_bytes());
        hash_str(&mut hasher, &object.ai_state);
        hash_object_id(&mut hasher, object.target);
        hash_optional_vec3(&mut hasher, object.target_location);
        hasher.update(&object.construction_percent.to_le_bytes());
    }

    hasher.update(b"VICTORY");
    if let Some(victory_state) = victory_state {
        hash_str(&mut hasher, victory_state);
    }

    hasher.finalize()
}

pub fn summarize_command(command: &CommandType) -> String {
    match command {
        CommandType::Move { destination } => format_vec_command("Move", *destination),
        CommandType::MoveTo {
            destination,
            waypoints,
        } => {
            let mut result = format_vec_command("MoveTo", *destination);
            for waypoint in waypoints {
                result.push(':');
                result.push_str(&format_vec3(*waypoint));
            }
            result
        }
        CommandType::AttackMoveTo { destination } => {
            format_vec_command("AttackMoveTo", *destination)
        }
        CommandType::ForceMoveTo { destination } => format_vec_command("ForceMoveTo", *destination),
        CommandType::AddWaypoint { destination } => format_vec_command("AddWaypoint", *destination),
        CommandType::Attack { target_id } => format!("Attack:{}", target_id.0),
        CommandType::AttackObject { target_id } => format!("AttackObject:{}", target_id.0),
        CommandType::ForceAttackObject { target_id } => {
            format!("ForceAttackObject:{}", target_id.0)
        }
        CommandType::ForceAttackGround { location } => {
            format_vec_command("ForceAttackGround", *location)
        }
        CommandType::DozerConstruct {
            template_name,
            location,
        } => format!(
            "DozerConstruct:{}:{}",
            template_name,
            format_vec3(*location)
        ),
        CommandType::Build {
            template_name,
            location,
        } => format!("Build:{}:{}", template_name, format_vec3(*location)),
        other => format!("{other:?}"),
    }
}

fn summarize_ai_state(ai_state: &AIState) -> String {
    format!("{ai_state:?}")
}

fn object_status_bits(object: &Object) -> u32 {
    let status = &object.status;
    (status.destroyed as u32)
        | ((status.under_construction as u32) << 1)
        | ((status.selected as u32) << 2)
        | ((status.moving as u32) << 3)
        | ((status.attacking as u32) << 4)
        | ((status.airborne_target as u32) << 5)
        | ((status.stealthed as u32) << 6)
        | ((status.disabled_underpowered as u32) << 7)
        | ((object.selected as u32) << 8)
        | ((object.force_attack as u32) << 9)
        | ((object.overcharge_enabled as u32) << 10)
}

fn format_vec_command(name: &str, value: Vec3) -> String {
    format!("{name}:{}", format_vec3(value))
}

fn format_vec3(value: Vec3) -> String {
    format!("{:.3},{:.3},{:.3}", value.x, value.y, value.z)
}

fn hash_str(hasher: &mut Hasher, value: &str) {
    hasher.update(&(value.len() as u32).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_vec3(hasher: &mut Hasher, value: Vec3) {
    hasher.update(&value.x.to_le_bytes());
    hasher.update(&value.y.to_le_bytes());
    hasher.update(&value.z.to_le_bytes());
}

fn hash_optional_vec3(hasher: &mut Hasher, value: Option<Vec3>) {
    hasher.update(&[value.is_some() as u8]);
    if let Some(value) = value {
        hash_vec3(hasher, value);
    }
}

fn hash_object_id(hasher: &mut Hasher, value: Option<ObjectId>) {
    hasher.update(&[value.is_some() as u8]);
    if let Some(value) = value {
        hasher.update(&value.0.to_le_bytes());
    }
}
