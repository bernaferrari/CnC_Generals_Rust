//! Wave 957: host_object/host_objects authority dual-read seal.
use crate::command_system::{CommandType, GameCommand};
use crate::game_logic::{AIState, GameLogic, Object, ObjectId, Player, Team};
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

/// Canonical per-player state used by deterministic gameplay trace comparisons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TracePlayer {
    pub id: i32,
    pub name: String,
    pub side: String,
    pub base_side: String,
    pub player_type: String,
    pub money: i32,
    pub power: i32,
    pub low_power: bool,
    pub has_radar: bool,
    pub is_dead: bool,
    pub rank_level: i32,
    pub skill_points: i32,
    pub science_purchase_points: i32,
    pub total_score: i32,
}

impl TracePlayer {
    pub fn from_player(player: &Player) -> Self {
        Self {
            id: player.id as i32,
            name: player.name.clone(),
            side: player.team.get_name().to_string(),
            base_side: player.team.get_name().to_string(),
            player_type: if player.is_local { "Human" } else { "Computer" }.to_string(),
            money: player.resources.supplies as i32,
            power: player.power_available,
            low_power: player.power_available < 0,
            has_radar: player.has_radar(),
            is_dead: !player.is_alive,
            rank_level: 0,
            skill_points: 0,
            science_purchase_points: player.unlocked_sciences.len() as i32,
            total_score: player.statistics.units_destroyed as i32
                + player.statistics.structures_destroyed as i32
                + player.statistics.resources_collected as i32
                - player.statistics.units_lost as i32
                - player.statistics.structures_lost as i32,
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

/// Ordered observable event emitted during a frame. C++ and Rust producers use
/// the same compact representation so ordering differences are not hidden by
/// a frame CRC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEvent {
    pub kind: String,
    pub source: Option<ObjectId>,
    pub target: Option<ObjectId>,
    pub payload: String,
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
    #[serde(default)]
    pub players: Vec<TracePlayer>,
    #[serde(default)]
    pub events: Vec<TraceEvent>,
    /// Canonical snapshot/Xfer bytes captured at this frame when the scenario
    /// requests serialization evidence.
    #[serde(default)]
    pub xfer_bytes: Vec<u8>,
    pub victory_state: Option<String>,
    pub crc: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceDifference {
    FrameCrc {
        index: usize,
        left_frame: u32,
        right_frame: u32,
        left_crc: u32,
        right_crc: u32,
    },
    Length {
        matching_frames: usize,
        left_len: usize,
        right_len: usize,
    },
}

/// First semantic difference inside a mismatching frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceFieldDifference {
    pub index: usize,
    pub left_frame: u32,
    pub right_frame: u32,
    pub category: String,
    pub path: String,
    pub left: serde_json::Value,
    pub right: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFrameCommands {
    pub frame: u32,
    pub commands: Vec<GameCommand>,
}

impl TraceFrameCommands {
    pub fn new(frame: u32, commands: Vec<GameCommand>) -> Self {
        Self { frame, commands }
    }
}

/// Scripted deterministic trace input. Commands are queued before the target
/// frame is advanced, matching the C++ command-list phase ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceScenario {
    pub rng_seed: [u32; 6],
    pub final_frame: u32,
    pub commands: Vec<TraceFrameCommands>,
}

impl TraceScenario {
    pub fn new(rng_seed: [u32; 6], final_frame: u32) -> Self {
        Self {
            rng_seed,
            final_frame,
            commands: Vec::new(),
        }
    }

    pub fn with_commands(mut self, frame: u32, commands: Vec<GameCommand>) -> Self {
        self.commands.push(TraceFrameCommands::new(frame, commands));
        self
    }
}

impl FrameTrace {
    pub fn new(
        frame: u32,
        rng_seed: [u32; 6],
        commands: Vec<GameCommand>,
        objects: Vec<TraceObject>,
        victory_state: Option<String>,
    ) -> Self {
        Self::new_with_players(
            frame,
            rng_seed,
            commands,
            objects,
            Vec::new(),
            victory_state,
        )
    }

    pub fn new_with_players(
        frame: u32,
        rng_seed: [u32; 6],
        commands: Vec<GameCommand>,
        objects: Vec<TraceObject>,
        players: Vec<TracePlayer>,
        victory_state: Option<String>,
    ) -> Self {
        Self::new_with_diagnostics(
            frame,
            rng_seed,
            commands,
            objects,
            players,
            Vec::new(),
            Vec::new(),
            victory_state,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_diagnostics(
        frame: u32,
        rng_seed: [u32; 6],
        commands: Vec<GameCommand>,
        objects: Vec<TraceObject>,
        players: Vec<TracePlayer>,
        events: Vec<TraceEvent>,
        xfer_bytes: Vec<u8>,
        victory_state: Option<String>,
    ) -> Self {
        let mut commands: Vec<TraceCommand> = commands
            .into_iter()
            .map(TraceCommand::from_command)
            .collect();
        commands.sort_by_key(|command| (command.command_id, command.player_id));

        let mut objects = objects;
        objects.sort_by_key(|object| object.id);

        let mut players = players;
        players.sort_by_key(|player| player.id);

        let crc = calculate_frame_crc_with_diagnostics(
            frame,
            &rng_seed,
            &commands,
            &objects,
            &players,
            &events,
            &xfer_bytes,
            victory_state.as_deref(),
        );

        Self {
            frame,
            rng_seed,
            commands,
            objects,
            players,
            events,
            xfer_bytes,
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
            .host_objects()
            .values()
            .map(TraceObject::from_object)
            .collect();
        let players = game_logic
            .get_players()
            .values()
            .map(TracePlayer::from_player)
            .collect();

        Self::new_with_players(
            game_logic.get_frame(),
            rng_seed,
            commands,
            objects,
            players,
            victory_state,
        )
    }
}

pub fn run_trace_scenario(game_logic: &mut GameLogic, scenario: &TraceScenario) -> Vec<FrameTrace> {
    let mut commands_by_frame = scenario.commands.clone();
    commands_by_frame.sort_by_key(|entry| entry.frame);

    let mut command_index = 0usize;
    let mut trace = Vec::new();

    while game_logic.get_frame() < scenario.final_frame {
        let next_frame = game_logic.get_frame() + 1;
        let mut frame_commands = Vec::new();

        while command_index < commands_by_frame.len()
            && commands_by_frame[command_index].frame == next_frame
        {
            for command in commands_by_frame[command_index].commands.iter().cloned() {
                game_logic.queue_command(command.clone());
                frame_commands.push(command);
            }
            command_index += 1;
        }

        game_logic.update();
        trace.push(FrameTrace::from_game_logic(
            game_logic,
            scenario.rng_seed,
            frame_commands,
            None,
        ));
    }

    trace
}

pub fn first_trace_difference<'a>(
    left: &'a [FrameTrace],
    right: &'a [FrameTrace],
) -> Option<(&'a FrameTrace, &'a FrameTrace)> {
    left.iter()
        .zip(right.iter())
        .find(|(left_frame, right_frame)| left_frame.crc != right_frame.crc)
}

pub fn compare_frame_traces(
    left: &[FrameTrace],
    right: &[FrameTrace],
) -> Result<(), TraceDifference> {
    if let Some((index, (left_frame, right_frame))) = left
        .iter()
        .zip(right.iter())
        .enumerate()
        .find(|(_, (left_frame, right_frame))| left_frame.crc != right_frame.crc)
    {
        return Err(TraceDifference::FrameCrc {
            index,
            left_frame: left_frame.frame,
            right_frame: right_frame.frame,
            left_crc: left_frame.crc,
            right_crc: right_frame.crc,
        });
    }

    if left.len() != right.len() {
        return Err(TraceDifference::Length {
            matching_frames: left.len().min(right.len()),
            left_len: left.len(),
            right_len: right.len(),
        });
    }

    Ok(())
}

/// Report the first divergent frame and the first semantic field within it.
/// Category order is stable: frame, RNG, commands, object/player state,
/// events, Xfer bytes, victory, then CRC integrity.
pub fn first_trace_field_difference(
    left: &[FrameTrace],
    right: &[FrameTrace],
) -> Option<TraceFieldDifference> {
    for (index, (left_frame, right_frame)) in left.iter().zip(right).enumerate() {
        let candidates = [
            (
                "frame",
                serde_json::json!(left_frame.frame),
                serde_json::json!(right_frame.frame),
            ),
            (
                "rng",
                serde_json::json!(left_frame.rng_seed),
                serde_json::json!(right_frame.rng_seed),
            ),
            (
                "commands",
                serde_json::json!(left_frame.commands),
                serde_json::json!(right_frame.commands),
            ),
            (
                "objects",
                serde_json::json!(left_frame.objects),
                serde_json::json!(right_frame.objects),
            ),
            (
                "players",
                serde_json::json!(left_frame.players),
                serde_json::json!(right_frame.players),
            ),
            (
                "events",
                serde_json::json!(left_frame.events),
                serde_json::json!(right_frame.events),
            ),
            (
                "xfer",
                serde_json::json!(left_frame.xfer_bytes),
                serde_json::json!(right_frame.xfer_bytes),
            ),
            (
                "victory",
                serde_json::json!(left_frame.victory_state),
                serde_json::json!(right_frame.victory_state),
            ),
        ];
        for (category, left_value, right_value) in candidates {
            if let Some((path, left, right)) =
                first_json_difference(&left_value, &right_value, category.to_string())
            {
                return Some(TraceFieldDifference {
                    index,
                    left_frame: left_frame.frame,
                    right_frame: right_frame.frame,
                    category: category.to_string(),
                    path,
                    left,
                    right,
                });
            }
        }
        if left_frame.crc != right_frame.crc {
            return Some(TraceFieldDifference {
                index,
                left_frame: left_frame.frame,
                right_frame: right_frame.frame,
                category: "crc".to_string(),
                path: "crc".to_string(),
                left: serde_json::json!(left_frame.crc),
                right: serde_json::json!(right_frame.crc),
            });
        }
    }
    (left.len() != right.len()).then(|| TraceFieldDifference {
        index: left.len().min(right.len()),
        left_frame: left.last().map_or(0, |frame| frame.frame),
        right_frame: right.last().map_or(0, |frame| frame.frame),
        category: "trace".to_string(),
        path: "frames.length".to_string(),
        left: serde_json::json!(left.len()),
        right: serde_json::json!(right.len()),
    })
}

fn first_json_difference(
    left: &serde_json::Value,
    right: &serde_json::Value,
    path: String,
) -> Option<(String, serde_json::Value, serde_json::Value)> {
    if left == right {
        return None;
    }
    match (left, right) {
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            for index in 0..left.len().max(right.len()) {
                let left_value = left.get(index).cloned().unwrap_or(serde_json::Value::Null);
                let right_value = right.get(index).cloned().unwrap_or(serde_json::Value::Null);
                if let Some(difference) =
                    first_json_difference(&left_value, &right_value, format!("{path}[{index}]"))
                {
                    return Some(difference);
                }
            }
        }
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            let mut keys: Vec<&String> = left.keys().chain(right.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let left_value = left.get(key).cloned().unwrap_or(serde_json::Value::Null);
                let right_value = right.get(key).cloned().unwrap_or(serde_json::Value::Null);
                if let Some(difference) =
                    first_json_difference(&left_value, &right_value, format!("{path}.{key}"))
                {
                    return Some(difference);
                }
            }
        }
        _ => {}
    }
    Some((path, left.clone(), right.clone()))
}

pub fn calculate_frame_crc(
    frame: u32,
    rng_seed: &[u32; 6],
    commands: &[TraceCommand],
    objects: &[TraceObject],
    players: &[TracePlayer],
    victory_state: Option<&str>,
) -> u32 {
    calculate_frame_crc_with_diagnostics(
        frame,
        rng_seed,
        commands,
        objects,
        players,
        &[],
        &[],
        victory_state,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_frame_crc_with_diagnostics(
    frame: u32,
    rng_seed: &[u32; 6],
    commands: &[TraceCommand],
    objects: &[TraceObject],
    players: &[TracePlayer],
    events: &[TraceEvent],
    xfer_bytes: &[u8],
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

    hasher.update(b"PLAYERS");
    hasher.update(&(players.len() as u32).to_le_bytes());
    for player in players {
        hasher.update(&player.id.to_le_bytes());
        hash_str(&mut hasher, &player.name);
        hash_str(&mut hasher, &player.side);
        hash_str(&mut hasher, &player.base_side);
        hash_str(&mut hasher, &player.player_type);
        hasher.update(&player.money.to_le_bytes());
        hasher.update(&player.power.to_le_bytes());
        hasher.update(&[player.low_power as u8]);
        hasher.update(&[player.has_radar as u8]);
        hasher.update(&[player.is_dead as u8]);
        hasher.update(&player.rank_level.to_le_bytes());
        hasher.update(&player.skill_points.to_le_bytes());
        hasher.update(&player.science_purchase_points.to_le_bytes());
        hasher.update(&player.total_score.to_le_bytes());
    }

    hasher.update(b"EVENTS");
    hasher.update(&(events.len() as u32).to_le_bytes());
    for event in events {
        hash_str(&mut hasher, &event.kind);
        hash_object_id(&mut hasher, event.source);
        hash_object_id(&mut hasher, event.target);
        hash_str(&mut hasher, &event.payload);
    }

    hasher.update(b"XFER");
    hasher.update(&(xfer_bytes.len() as u32).to_le_bytes());
    hasher.update(xfer_bytes);

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
        CommandType::AttackMoveTo {
            destination,
            max_shots: -1,
        } => format_vec_command("AttackMoveTo", *destination),
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
            orientation,
        } => format!(
            "DozerConstruct:{}:{}:{:.3}",
            template_name,
            format_vec3(*location),
            orientation
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
        | ((status.detected as u32) << 11)
        | ((object.is_detector as u32) << 12)
        | ((status.disabled_unmanned as u32) << 13)
        | ((status.is_carbomb as u32) << 14)
        | ((status.hijacked as u32) << 15)
        | ((status.disabled_hacked as u32) << 16)
        | ((status.disabled_emp as u32) << 17)
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
