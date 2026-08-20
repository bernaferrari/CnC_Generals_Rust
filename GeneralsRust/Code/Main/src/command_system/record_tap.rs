//! Live-path recorder tap: host orders enter `TheCommandList` so
//! `RecorderClass::updateRecord` (Recorder.cpp:455) can see them, and
//! playback sink messages become Main `GameCommand`s.
//!
//! C++ writes every network message from `TheCommandList`. Main previously
//! queued `GameCommand`s only on `GameLogic.command_queue`, so
//! `update_record` saw an empty GameClient list.

use super::*;
use crate::game_logic::GameMode;
use game_engine::common::message_stream::{
    is_network_command_message, Coord3D, GameMessage, GameMessageArgumentType, GameMessageType,
    ICoord2D, ObjectID,
};
use game_engine::common::recorder::{init_recorder, with_recorder, with_recorder_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Camera pose carried by `MSG_SET_REPLAY_CAMERA`.
/// C++ `LookAtXlat.cpp:463-467` / `GameLogicDispatch.cpp:1807-1815`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayCameraPose {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

static PENDING_REPLAY_COMMANDS: Mutex<Vec<GameCommand>> = Mutex::new(Vec::new());
static PENDING_REPLAY_CAMERA: Mutex<Option<ReplayCameraPose>> = Mutex::new(None);
static BRIDGES_INSTALLED: AtomicBool = AtomicBool::new(false);
static HOST_LOGIC_FRAME: AtomicU32 = AtomicU32::new(0);

/// C++ `GameLogic.cpp` / `MessageStream.h` game-mode integers.
fn game_mode_to_new_game_code(mode: GameMode) -> i32 {
    match mode {
        GameMode::SinglePlayer => 0,
        GameMode::Multiplayer | GameMode::Lan => 1,
        GameMode::Skirmish => 2,
        GameMode::Replay => 3,
        GameMode::Shell => 4,
        GameMode::Internet => 5,
        GameMode::None => 6,
    }
}

fn object_id_from_message(id: ObjectID) -> ObjectId {
    ObjectId(id)
}

fn object_id_to_message(id: ObjectId) -> ObjectID {
    id.0
}

fn coord_from_vec3(pos: Vec3) -> Coord3D {
    Coord3D::new(pos.x, pos.y, pos.z)
}

fn vec3_from_coord(coord: &Coord3D) -> Vec3 {
    Vec3::new(coord.x, coord.y, coord.z)
}

fn append_to_command_list(message: GameMessage) {
    #[cfg(feature = "game_client")]
    {
        let _ = game_client::message_stream::command_list::append_command(message);
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = message;
    }
}

fn snapshot_command_list() -> Vec<GameMessage> {
    #[cfg(feature = "game_client")]
    {
        game_client::message_stream::command_list::get_command_list()
            .read()
            .map(|list| list.snapshot_messages())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "game_client"))]
    {
        Vec::new()
    }
}

fn take_command_list_messages() -> Vec<GameMessage> {
    #[cfg(feature = "game_client")]
    {
        let list = game_client::message_stream::command_list::get_command_list();
        match list.write() {
            Ok(mut guard) => {
                guard.reset_frame_counter();
                guard.get_all_commands()
            }
            Err(_) => Vec::new(),
        }
    }
    #[cfg(not(feature = "game_client"))]
    {
        Vec::new()
    }
}

fn clear_command_list() {
    #[cfg(feature = "game_client")]
    {
        if let Ok(mut guard) = game_client::message_stream::command_list::get_command_list().write()
        {
            guard.clear_all_commands();
            guard.reset_frame_counter();
        }
    }
}

fn keep_command_during_playback(msg: &GameMessage) -> bool {
    let ty = msg.get_type();
    !(is_network_command_message(ty) && !matches!(ty, GameMessageType::LogicCRC(_)))
}

fn host_logic_frame() -> u32 {
    let leftover = gamelogic::helpers::TheGameLogic::get_frame();
    if leftover != 0 {
        leftover
    } else {
        HOST_LOGIC_FRAME.load(Ordering::Relaxed)
    }
}

fn host_replay_mouse_snapshot() -> (i32, ICoord2D) {
    #[cfg(feature = "game_client")]
    {
        let cursor = game_client::helpers::TheInGameUI::get_mouse_cursor() as i32;
        (cursor, ICoord2D { x: 0, y: 0 })
    }
    #[cfg(not(feature = "game_client"))]
    {
        (0, ICoord2D { x: 0, y: 0 })
    }
}

fn cull_host_command_list() {
    #[cfg(feature = "game_client")]
    {
        if let Ok(mut list) = game_client::message_stream::command_list::get_command_list().write()
        {
            list.retain_messages(keep_command_during_playback);
        }
    }
}

fn push_pending_replay_command(command: GameCommand) {
    if let Ok(mut pending) = PENDING_REPLAY_COMMANDS.lock() {
        pending.push(command);
    }
}

fn take_pending_replay_commands() -> Vec<GameCommand> {
    PENDING_REPLAY_COMMANDS
        .lock()
        .map(|mut pending| pending.drain(..).collect())
        .unwrap_or_default()
}

/// Install CommandList source/sink + command_router host authority.
/// Safe to call repeatedly.
pub fn install_host_replay_bridges() {
    if BRIDGES_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    init_recorder();

    let command_source: Arc<dyn Fn() -> Vec<GameMessage> + Send + Sync> =
        Arc::new(snapshot_command_list);
    let command_sink: Arc<dyn Fn(GameMessage) + Send + Sync> = Arc::new(append_to_command_list);
    let command_cull: Arc<dyn Fn() + Send + Sync> = Arc::new(cull_host_command_list);
    let frame_provider: Arc<dyn Fn() -> u32 + Send + Sync> = Arc::new(host_logic_frame);

    let _ = with_recorder_mut(|recorder| {
        recorder.set_command_source(Some(command_source));
        recorder.set_command_sink(Some(command_sink));
        recorder.set_command_cull(Some(command_cull));
        recorder.set_frame_provider(Some(frame_provider));
    });

    #[cfg(feature = "game_client")]
    {
        game_client::message_stream::command_router::set_host_command_authority(Some(Arc::new(
            |messages| apply_replay_messages_to_host(messages),
        )));
    }
}

/// Convert a live host order into a `GameMessage` and append it to
/// `TheCommandList` so `RecorderClass::updateRecord` can write it.
pub fn tap_host_command_for_recorder(command: &GameCommand) {
    install_host_replay_bridges();
    if let Some(message) = game_command_to_message(command) {
        append_to_command_list(message);
    }
}

/// C++ `RecorderClass::updateRecord` starts a file when `MSG_NEW_GAME` is
/// not `GAME_SHELL` / `GAME_SINGLE_PLAYER` / `GAME_NONE`.
pub fn tap_host_new_game_for_recorder(mode: GameMode) {
    install_host_replay_bridges();
    let mut message = GameMessage::new(GameMessageType::NewGame);
    message.append_integer_argument(game_mode_to_new_game_code(mode));
    message.append_integer_argument(1); // DIFFICULTY_NORMAL
    message.append_integer_argument(0); // rank points
    message.append_integer_argument(30); // max FPS
    append_to_command_list(message);
}

/// C++ `LookAtXlat.cpp:459-469`: emit `MSG_SET_REPLAY_CAMERA` onto the list
/// the recorder snapshots (loc, angle, pitch, zoom, cursor, pixel).
pub fn tap_replay_camera_for_recorder(pose: ReplayCameraPose) {
    install_host_replay_bridges();
    let coord = coord_from_vec3(pose.pos);
    let mut message = GameMessage::new(GameMessageType::SetReplayCamera(
        coord.clone(),
        pose.yaw,
        pose.zoom,
    ));
    let (cursor, pixel) = host_replay_mouse_snapshot();
    message.append_location_argument(coord);
    message.append_real_argument(pose.yaw);
    message.append_real_argument(pose.pitch);
    message.append_real_argument(pose.zoom);
    message.append_integer_argument(cursor);
    message.append_pixel_argument(pixel);
    append_to_command_list(message);
}

/// Stamp live `TheGameLogic->getFrame()` for the next recorder write/playback.
pub fn stamp_host_logic_frame(frame: u32) {
    HOST_LOGIC_FRAME.store(frame, Ordering::Relaxed);
}

/// True when the live recorder is in `RECORDERMODETYPE_PLAYBACK`.
pub fn host_recorder_is_playback() -> bool {
    install_host_replay_bridges();
    with_recorder(|recorder| recorder.is_playback()).unwrap_or(false)
}

/// Take the most recent playback `MSG_SET_REPLAY_CAMERA` pose.
pub fn take_pending_replay_camera() -> Option<ReplayCameraPose> {
    PENDING_REPLAY_CAMERA.lock().ok().and_then(|mut slot| slot.take())
}

/// C++ `GameLogic::update` ticks `TheRecorder` then `processCommandList`.
/// Recording: write CommandList then drop it (host already queued the order).
/// Playback: playback sink fills CommandList; convert into the live host queue.
pub fn flush_recorder_and_replay_authority(host_queue: &mut VecDeque<GameCommand>) {
    install_host_replay_bridges();
    let playback = host_recorder_is_playback();
    let frame = host_logic_frame();
    let _ = with_recorder_mut(|recorder| {
        recorder.set_current_frame(frame);
        recorder.update();
    });

    if playback {
        // C++ cullBadCommands drops user network orders before processCommandList.
        host_queue.retain(|cmd| game_command_to_message(cmd).is_none());
        let messages = take_command_list_messages();
        apply_replay_messages_to_host(&messages);
    } else {
        clear_command_list();
    }

    for command in take_pending_replay_commands() {
        host_queue.push_back(command);
    }
}

fn apply_replay_messages_to_host(messages: &[GameMessage]) {
    for message in messages {
        match message.get_type() {
            GameMessageType::SetReplayCamera(coord, yaw, zoom) => {
                let angle = match message.get_argument(1) {
                    Some(GameMessageArgumentType::Real(value)) => *value,
                    _ => *yaw,
                };
                let pitch = match message.get_argument(2) {
                    Some(GameMessageArgumentType::Real(value)) => *value,
                    _ => 0.0,
                };
                let zoom_v = match message.get_argument(3) {
                    Some(GameMessageArgumentType::Real(value)) => *value,
                    _ => *zoom,
                };
                if let Ok(mut slot) = PENDING_REPLAY_CAMERA.lock() {
                    *slot = Some(ReplayCameraPose {
                        pos: vec3_from_coord(coord),
                        yaw: angle,
                        pitch,
                        zoom: zoom_v,
                    });
                }
            }
            GameMessageType::NewGame | GameMessageType::ClearGameData => {}
            _ => {
                if let Some(command) = game_message_to_host_command(message) {
                    push_pending_replay_command(command);
                }
            }
        }
    }
}

fn game_command_to_message(command: &GameCommand) -> Option<GameMessage> {
    use CommandType::*;
    let player = command.player_id as i32;
    let message_type = match &command.command_type {
        Move { destination } | MoveTo { destination, .. } | ForceMoveTo { destination } => {
            GameMessageType::DoMoveTo(coord_from_vec3(*destination))
        }
        AttackMoveTo { destination, .. } => {
            GameMessageType::DoAttackMoveTo(coord_from_vec3(*destination))
        }
        Attack { target_id } | AttackObject { target_id } => {
            GameMessageType::DoAttackObject(object_id_to_message(*target_id))
        }
        ForceAttackObject { target_id } => {
            GameMessageType::DoForceAttackObject(object_id_to_message(*target_id))
        }
        ForceAttackGround { location } => {
            GameMessageType::DoForceAttackGround(coord_from_vec3(*location))
        }
        Stop => GameMessageType::DoStop,
        Scatter => GameMessageType::DoScatter,
        Guard {
            target: GuardTarget::Position(pos),
            mode,
        } => GameMessageType::DoGuardPosition(coord_from_vec3(*pos), *mode as i32),
        Guard {
            target: GuardTarget::Object(id),
            mode,
        } => GameMessageType::DoGuardObject(object_id_to_message(*id), *mode as i32),
        AddWaypoint { destination } => GameMessageType::AddWaypoint(coord_from_vec3(*destination)),
        CreateSelectedGroup { create_new, units } => GameMessageType::CreateSelectedGroup(
            *create_new,
            units.iter().copied().map(object_id_to_message).collect(),
        ),
        Enter { target_id } => GameMessageType::Enter(0, object_id_to_message(*target_id)),
        Dock { target_id } => GameMessageType::Dock(object_id_to_message(*target_id)),
        Repair { target_id } => GameMessageType::DoRepair(object_id_to_message(*target_id)),
        GetRepaired { target_id } => GameMessageType::GetRepaired(object_id_to_message(*target_id)),
        GetHealed { target_id } => GameMessageType::GetHealed(object_id_to_message(*target_id)),
        ResumeConstruction { target_id } => {
            GameMessageType::ResumeConstruction(object_id_to_message(*target_id))
        }
        DoSalvage { destination } => GameMessageType::DoSalvage(coord_from_vec3(*destination)),
        EnableRetaliationMode {
            player_index,
            enabled,
        } => GameMessageType::EnableRetaliationMode(*player_index, *enabled),
        SelfDestruct { transfer_to_ally } => {
            GameMessageType::SelfDestruct(if *transfer_to_ally { 1 } else { 0 })
        }
        _ => return None,
    };
    Some(GameMessage::with_player(message_type, player))
}

fn game_message_to_host_command(message: &GameMessage) -> Option<GameCommand> {
    use GameMessageType::*;
    let command_type = match message.get_type() {
        DoMoveTo(coord) => CommandType::MoveTo {
            destination: vec3_from_coord(coord),
            waypoints: Vec::new(),
        },
        DoAttackMoveTo(coord) => CommandType::AttackMoveTo {
            destination: vec3_from_coord(coord),
            max_shots: -1,
        },
        DoForceMoveTO(coord) => CommandType::ForceMoveTo {
            destination: vec3_from_coord(coord),
        },
        DoAttackObject(id) => CommandType::AttackObject {
            target_id: object_id_from_message(*id),
        },
        DoForceAttackObject(id) => CommandType::ForceAttackObject {
            target_id: object_id_from_message(*id),
        },
        DoForceAttackGround(coord) => CommandType::ForceAttackGround {
            location: vec3_from_coord(coord),
        },
        DoStop => CommandType::Stop,
        DoScatter => CommandType::Scatter,
        DoGuardPosition(coord, mode) => CommandType::Guard {
            target: GuardTarget::Position(vec3_from_coord(coord)),
            mode: guard_mode_from_i32(*mode),
        },
        DoGuardObject(id, mode) => CommandType::Guard {
            target: GuardTarget::Object(object_id_from_message(*id)),
            mode: guard_mode_from_i32(*mode),
        },
        AddWaypoint(coord) => CommandType::AddWaypoint {
            destination: vec3_from_coord(coord),
        },
        CreateSelectedGroup(create_new, units) => CommandType::CreateSelectedGroup {
            create_new: *create_new,
            units: units.iter().copied().map(object_id_from_message).collect(),
        },
        Enter(_selector, id) => CommandType::Enter {
            target_id: object_id_from_message(*id),
        },
        Dock(id) => CommandType::Dock {
            target_id: object_id_from_message(*id),
        },
        DoRepair(id) => CommandType::Repair {
            target_id: object_id_from_message(*id),
        },
        GetRepaired(id) => CommandType::GetRepaired {
            target_id: object_id_from_message(*id),
        },
        GetHealed(id) => CommandType::GetHealed {
            target_id: object_id_from_message(*id),
        },
        ResumeConstruction(id) => CommandType::ResumeConstruction {
            target_id: object_id_from_message(*id),
        },
        DoSalvage(coord) => CommandType::DoSalvage {
            destination: vec3_from_coord(coord),
        },
        EnableRetaliationMode(player_index, enabled) => CommandType::EnableRetaliationMode {
            player_index: *player_index,
            enabled: *enabled,
        },
        SelfDestruct(flag) => CommandType::SelfDestruct {
            transfer_to_ally: *flag != 0,
        },
        _ => return None,
    };
    Some(GameCommand {
        command_type,
        player_id: message.get_player_index() as u32,
        command_id: 0,
        timestamp: SystemTime::now(),
        selected_units: Vec::new(),
        modifier_keys: ModifierKeys::default(),
    })
}

fn guard_mode_from_i32(mode: i32) -> crate::game_logic::GuardMode {
    match mode {
        1 => crate::game_logic::GuardMode::WithoutPursuit,
        2 => crate::game_logic::GuardMode::FlyingUnitsOnly,
        _ => crate::game_logic::GuardMode::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_command(destination: Vec3, player_id: u32) -> GameCommand {
        GameCommand {
            command_type: CommandType::MoveTo {
                destination,
                waypoints: Vec::new(),
            },
            player_id,
            command_id: 7,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(11)],
            modifier_keys: ModifierKeys::default(),
        }
    }

    #[test]
    fn host_move_round_trips_to_do_move_to() {
        // C++ Recorder.cpp:455-492 writes network messages from TheCommandList.
        let command = move_command(Vec3::new(12.0, 0.0, -4.0), 3);
        let message = game_command_to_message(&command).expect("move must be a network message");
        assert!(matches!(
            message.get_type(),
            GameMessageType::DoMoveTo(coord) if (coord.x - 12.0).abs() < f32::EPSILON
        ));
        assert_eq!(message.get_player_index(), 3);

        let restored = game_message_to_host_command(&message).expect("playback must restore MoveTo");
        match restored.command_type {
            CommandType::MoveTo { destination, .. } => {
                assert!((destination.x - 12.0).abs() < f32::EPSILON);
                assert!((destination.z + 4.0).abs() < f32::EPSILON);
            }
            other => panic!("expected MoveTo, got {other:?}"),
        }
        assert_eq!(restored.player_id, 3);
    }

    #[test]
    fn unknown_host_command_is_fail_closed() {
        let command = GameCommand {
            command_type: CommandType::Invalid,
            player_id: 0,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: Vec::new(),
            modifier_keys: ModifierKeys::default(),
        };
        assert!(game_command_to_message(&command).is_none());
    }

    #[test]
    fn new_game_skirmish_code_matches_cpp() {
        assert_eq!(game_mode_to_new_game_code(GameMode::Skirmish), 2);
        assert_eq!(game_mode_to_new_game_code(GameMode::SinglePlayer), 0);
        assert_eq!(game_mode_to_new_game_code(GameMode::Shell), 4);
    }

    #[test]
    fn set_replay_camera_message_uses_cpp_argument_layout() {
        tap_replay_camera_for_recorder(ReplayCameraPose {
            pos: Vec3::new(8.0, 1.0, 3.0),
            yaw: 0.25,
            pitch: 0.5,
            zoom: 1.5,
        });
        let snap = snapshot_command_list();
        let camera = snap
            .iter()
            .rev()
            .find(|msg| matches!(msg.get_type(), GameMessageType::SetReplayCamera(..)))
            .expect("tap must append MSG_SET_REPLAY_CAMERA");
        match camera.get_argument(0) {
            Some(GameMessageArgumentType::Location(coord)) => {
                assert!((coord.x - 8.0).abs() < f32::EPSILON);
            }
            other => panic!("expected location, got {other:?}"),
        }
        match camera.get_argument(1) {
            Some(GameMessageArgumentType::Real(yaw)) => {
                assert!((yaw - 0.25).abs() < f32::EPSILON);
            }
            other => panic!("expected yaw, got {other:?}"),
        }
        match camera.get_argument(2) {
            Some(GameMessageArgumentType::Real(pitch)) => {
                assert!((pitch - 0.5).abs() < f32::EPSILON);
            }
            other => panic!("expected pitch, got {other:?}"),
        }
        match camera.get_argument(3) {
            Some(GameMessageArgumentType::Real(zoom)) => {
                assert!((zoom - 1.5).abs() < f32::EPSILON);
            }
            other => panic!("expected zoom, got {other:?}"),
        }
        match camera.get_argument(4) {
            Some(GameMessageArgumentType::Integer(_)) => {}
            other => panic!("expected cursor int, got {other:?}"),
        }
        match camera.get_argument(5) {
            Some(GameMessageArgumentType::Pixel(_)) => {}
            other => panic!("expected pixel, got {other:?}"),
        }
    }
}
