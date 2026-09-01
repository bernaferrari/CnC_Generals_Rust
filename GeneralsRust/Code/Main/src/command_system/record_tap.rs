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
    Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D, ObjectID,
    is_network_command_message,
};
use game_engine::common::recorder::{init_recorder, with_recorder, with_recorder_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Camera pose carried by `MSG_SET_REPLAY_CAMERA`.
/// C++ `LookAtXlat.cpp:463-467` / `GameLogicDispatch.cpp:1807-1815`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayCameraPose {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    /// C++ `TheMouse->getMouseCursor()` integer recorded with the pose.
    pub cursor: i32,
    /// C++ `LookAtTranslator::m_currentPos` pixel.
    pub pixel: (i32, i32),
    /// C++ `GameMessage::getPlayerIndex()` — original recorder / issuing player.
    pub player_index: i32,
}

/// Playback `MSG_CREATE/SELECT/ADD_TEAM*` for the live host control groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayTeamOp {
    Create {
        player_index: i32,
        slot: u8,
        ids: Vec<ObjectId>,
    },
    Select {
        player_index: i32,
        slot: u8,
    },
    Add {
        player_index: i32,
        slot: u8,
    },
}

fn intern_name(name: &str) -> u32 {
    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name)
}

fn resolve_name(id: u32) -> String {
    game_engine::common::name_key_generator::NameKeyGenerator::key_to_name(id).unwrap_or_default()
}

fn special_maps() -> &'static Mutex<(
    HashMap<SpecialPowerType, u32>,
    HashMap<u32, SpecialPowerType>,
)> {
    static MAPS: std::sync::LazyLock<
        Mutex<(
            HashMap<SpecialPowerType, u32>,
            HashMap<u32, SpecialPowerType>,
        )>,
    > = std::sync::LazyLock::new(|| Mutex::new((HashMap::new(), HashMap::new())));
    &MAPS
}

fn intern_special(power: &SpecialPowerType) -> u32 {
    let mut maps = special_maps().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(id) = maps.0.get(power).copied() {
        return id;
    }
    let id = intern_name(&format!("SP::{power:?}"));
    maps.0.insert(power.clone(), id);
    maps.1.insert(id, power.clone());
    id
}

fn resolve_special(id: u32) -> SpecialPowerType {
    special_maps()
        .lock()
        .ok()
        .and_then(|maps| maps.1.get(&id).cloned())
        .unwrap_or(SpecialPowerType::Invalid)
}

fn weapon_slot_to_id(slot: &WeaponSlot) -> u32 {
    match slot {
        WeaponSlot::Primary => 0,
        WeaponSlot::Secondary => 1,
        WeaponSlot::Tertiary => 2,
        WeaponSlot::AntiAir => 3,
        WeaponSlot::Slot(n) => *n,
    }
}

fn weapon_slot_from_id(id: u32) -> WeaponSlot {
    match id {
        0 => WeaponSlot::Primary,
        1 => WeaponSlot::Secondary,
        2 => WeaponSlot::Tertiary,
        3 => WeaponSlot::AntiAir,
        n => WeaponSlot::Slot(n),
    }
}

fn append_game_message_to_stream(msg: &GameMessage) {
    use game_engine::common::message_stream::get_message_stream;
    let stream_lock = get_message_stream();
    let Ok(mut stream) = stream_lock.write() else {
        return;
    };
    let dest = stream.append_message(msg.get_type().clone());
    dest.set_player_index(msg.get_player_index());
    for arg in msg.get_arguments() {
        match &arg.data {
            GameMessageArgumentType::Integer(v) => dest.append_integer_argument(*v),
            GameMessageArgumentType::Real(v) => dest.append_real_argument(*v),
            GameMessageArgumentType::Boolean(v) => dest.append_boolean_argument(*v),
            GameMessageArgumentType::ObjectID(v) => dest.append_object_id_argument(*v),
            GameMessageArgumentType::DrawableID(v) => dest.append_drawable_id_argument(*v),
            GameMessageArgumentType::TeamID(v) | GameMessageArgumentType::SquadID(v) => {
                dest.append_team_id_argument(*v)
            }
            GameMessageArgumentType::Location(v) => dest.append_location_argument(v.clone()),
            GameMessageArgumentType::Pixel(v) => dest.append_pixel_argument(v.clone()),
            GameMessageArgumentType::PixelRegion(v) => dest.append_pixel_region_argument(v.clone()),
            GameMessageArgumentType::Timestamp(v) => dest.append_timestamp_argument(*v),
            GameMessageArgumentType::WideChar(v) => dest.append_wide_char_argument(*v),
            GameMessageArgumentType::String(v) => dest.append_string_argument(v.clone()),
        }
    }
}

static PENDING_REPLAY_COMMANDS: Mutex<Vec<GameCommand>> = Mutex::new(Vec::new());
static PENDING_REPLAY_CAMERA: Mutex<Option<ReplayCameraPose>> = Mutex::new(None);
static PENDING_REPLAY_TEAMS: Mutex<Vec<ReplayTeamOp>> = Mutex::new(Vec::new());
static PENDING_REPLAY_REMIRROR: Mutex<Vec<i32>> = Mutex::new(Vec::new());
static BRIDGES_INSTALLED: AtomicBool = AtomicBool::new(false);
static HOST_LOGIC_FRAME: AtomicU32 = AtomicU32::new(0);
static LAST_LOGIC_CRC: AtomicU32 = AtomicU32::new(0);
static LAST_LOGIC_CRC_FRAME: AtomicU32 = AtomicU32::new(u32::MAX);

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
    let leftover_cursor = game_client::helpers::TheInGameUI::get_mouse_cursor() as i32;
    #[cfg(not(feature = "game_client"))]
    let leftover_cursor = 0;
    (leftover_cursor, ICoord2D { x: 0, y: 0 })
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
    let command_sink: Arc<dyn Fn(GameMessage) + Send + Sync> = Arc::new(|msg| {
        // C++ playbackFile/stopPlayback use TheMessageStream for these two.
        match msg.get_type() {
            GameMessageType::NewGame | GameMessageType::ClearGameData => {
                append_game_message_to_stream(&msg);
            }
            _ => append_to_command_list(msg),
        }
    });
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
    let difficulty = gamelogic::helpers::TheScriptEngine::get_global_difficulty();
    let rank = gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start();
    let max_fps = game_engine::common::global_data::read()
        .writable
        .frames_per_second_limit;
    let mut message = GameMessage::new(GameMessageType::NewGame);
    message.append_integer_argument(game_mode_to_new_game_code(mode));
    message.append_integer_argument(difficulty);
    message.append_integer_argument(rank);
    message.append_integer_argument(if max_fps != 0 { max_fps } else { 30 });
    append_to_command_list(message);
}

/// C++ `LookAtXlat.cpp:459-469`: emit `MSG_SET_REPLAY_CAMERA` onto the list
/// the recorder snapshots (loc, angle, pitch, zoom, cursor, pixel).
pub fn tap_replay_camera_for_recorder(pose: ReplayCameraPose) {
    install_host_replay_bridges();
    let coord = coord_from_vec3(pose.pos);
    let mut message = GameMessage::with_player(
        GameMessageType::SetReplayCamera(coord.clone(), pose.yaw, pose.zoom),
        pose.player_index,
    );
    let (cursor, pixel) = if pose.pixel != (0, 0) || pose.cursor != 0 {
        (
            pose.cursor,
            ICoord2D {
                x: pose.pixel.0,
                y: pose.pixel.1,
            },
        )
    } else {
        host_replay_mouse_snapshot()
    };
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

fn leftover_logic_crc() -> u32 {
    gamelogic::get_game_logic()
        .try_lock()
        .ok()
        .map(|logic| logic.get_crc(gamelogic::CrcMode::Recalc))
        .unwrap_or(0)
}

fn logic_crc_due(frame: u32) -> bool {
    let interval = game_engine::common::crc_debug::replay_crc_interval();
    interval > 0 && frame % (interval as u32) == 0
}

/// C++ `GameLogic.cpp:3625-3654`: every `REPLAY_CRC_INTERVAL` frames compute
/// `getCRC(CRC_RECALC)` and append `MSG_LOGIC_CRC` so `updateRecord` writes it.
pub fn post_host_logic_crc_if_due(frame: u32, host_fold: u32) -> Option<u32> {
    if !logic_crc_due(frame) {
        return None;
    }
    if LAST_LOGIC_CRC_FRAME.load(Ordering::Relaxed) == frame {
        return Some(LAST_LOGIC_CRC.load(Ordering::Relaxed));
    }

    let leftover = leftover_logic_crc();
    let mut hasher = game_engine::common::crc::Crc::new();
    hasher.compute_crc(&leftover.to_le_bytes());
    hasher.compute_crc(&host_fold.to_le_bytes());
    let crc = hasher.get();

    let playback = host_recorder_is_playback();
    let mut message = GameMessage::new(GameMessageType::LogicCRC(crc));
    message.append_boolean_argument(playback);
    append_to_command_list(message);

    LAST_LOGIC_CRC.store(crc, Ordering::Relaxed);
    LAST_LOGIC_CRC_FRAME.store(frame, Ordering::Relaxed);
    Some(crc)
}

/// True when the live recorder is in `RECORDERMODETYPE_PLAYBACK`.
pub fn host_recorder_is_playback() -> bool {
    install_host_replay_bridges();
    with_recorder(|recorder| recorder.is_playback()).unwrap_or(false)
}

/// C++ `TheControlBar->getObserverLookAtPlayer()` index.
pub fn host_observer_look_at_player_index() -> Option<i32> {
    #[cfg(feature = "game_client")]
    {
        if let Some(index) =
            game_client::helpers::TheControlBar::get_observer_look_at_player_index()
        {
            return Some(index);
        }
        return game_client::gui::control_bar::control_bar_observer::observer_look_at_player_index(
        );
    }
    #[cfg(not(feature = "game_client"))]
    None
}

/// C++ `getObserverLookAtPlayer() == thisPlayer`.
pub fn host_replay_observer_matches_player(player_index: i32) -> bool {
    host_observer_look_at_player_index() == Some(player_index)
}

/// C++ GameLogicDispatch.cpp:1803 playback + useCamera + observer==thisPlayer.
pub fn host_should_apply_replay_camera(player_index: i32) -> bool {
    host_recorder_is_playback()
        && game_engine::common::global_data::read().use_camera_in_replay
        && host_replay_observer_matches_player(player_index)
}

/// C++ GameLogicDispatch.cpp:1970 same gate as SET_REPLAY_CAMERA.
pub fn host_should_remirror_observer_selection(player_index: i32) -> bool {
    host_should_apply_replay_camera(player_index)
}

/// Take the most recent playback `MSG_SET_REPLAY_CAMERA` pose.
pub fn take_pending_replay_camera() -> Option<ReplayCameraPose> {
    PENDING_REPLAY_CAMERA
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

/// C++ SelectionXlat.cpp:1047 MSG_CREATE/SELECT/ADD_TEAM0+group.
/// `kind`: 0=create, 1=select, 2=add.
pub fn tap_host_team_slot_for_recorder(slot: u8, kind: u8, ids: &[ObjectId]) {
    install_host_replay_bridges();
    if with_recorder(|recorder| recorder.is_playback()).unwrap_or(false) {
        return;
    }
    let message_type = match kind {
        0 => GameMessageType::CreateTeamSlot(slot),
        1 => GameMessageType::SelectTeamSlot(slot),
        _ => GameMessageType::AddTeamSlot(slot),
    };
    let mut message = GameMessage::with_player(message_type, host_local_player_index());
    if kind == 0 {
        for id in ids {
            message.append_object_id_argument(object_id_to_message(*id));
        }
    }
    append_to_command_list(message);
}

/// Take playback team ops for the live host control-group table.
pub fn take_pending_replay_team_ops() -> Vec<ReplayTeamOp> {
    PENDING_REPLAY_TEAMS
        .lock()
        .map(|mut pending| pending.drain(..).collect())
        .unwrap_or_default()
}

/// Queue C++ post-dispatch remirror of `thisPlayer` onto observer InGameUI.
pub fn queue_replay_selection_remirror(player_index: i32) {
    if let Ok(mut pending) = PENDING_REPLAY_REMIRROR.lock() {
        if !pending.contains(&player_index) {
            pending.push(player_index);
        }
    }
}

/// Take issuing-player indices that should remirror onto observer InGameUI.
pub fn take_pending_replay_selection_remirror() -> Vec<i32> {
    PENDING_REPLAY_REMIRROR
        .lock()
        .map(|mut pending| pending.drain(..).collect())
        .unwrap_or_default()
}

/// Leftover `Player::get_current_selection_ids` for the issuing replay player.
pub fn leftover_player_current_selection_ids(player_index: i32) -> Vec<ObjectId> {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return Vec::new();
    };
    let Some(player_arc) = list.get_player(player_index).cloned() else {
        return Vec::new();
    };
    drop(list);
    let Ok(player) = player_arc.read() else {
        return Vec::new();
    };
    player
        .get_current_selection_ids()
        .into_iter()
        .map(object_id_from_message)
        .collect()
}

fn host_local_player_index() -> i32 {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return 0;
    };
    let index = list.get_local_player_index();
    if index == gamelogic::player::PLAYER_INDEX_INVALID {
        0
    } else {
        index
    }
}

/// C++ `GameLogic::update` ticks `TheRecorder` then `processCommandList`.
/// Recording: write CommandList then drop it (host already queued the order).
/// Playback: playback sink fills CommandList; convert into the live host queue.
pub fn flush_recorder_and_replay_authority(host_queue: &mut VecDeque<GameCommand>) {
    install_host_replay_bridges();
    let playback = host_recorder_is_playback();
    let frame = host_logic_frame();
    // C++ posts MSG_LOGIC_CRC onto the stream before TheRecorder->update().
    let posted = post_host_logic_crc_if_due(frame, 0);
    let _ = with_recorder_mut(|recorder| {
        recorder.set_current_frame(frame);
        recorder.update();
        if playback {
            if let Some(crc) = posted {
                // C++ GameLogicDispatch.cpp:1940-1946 — compare only in playback.
                recorder.notify_logic_crc(crc, 0);
            }
        }
    });

    if playback {
        // C++ cullBadCommands drops user network orders before processCommandList.
        host_queue.retain(|cmd| game_command_to_message(cmd).is_none());
        let messages = take_command_list_messages();
        apply_replay_messages_to_host(&messages);
    } else {
        // Record mode: drop stale network user orders but keep the
        // MSG_LOGIC_CRC just posted above — C++ GameLogic.cpp:3625-3654 posts
        // the CRC before TheRecorder->update() so updateRecord writes it to
        // the .rep stream; a blanket clear would silently eat it.
        cull_host_command_list();
    }

    for command in take_pending_replay_commands() {
        host_queue.push_back(command);
    }
}

fn object_ids_from_message(message: &GameMessage) -> Vec<ObjectId> {
    (0..message.get_argument_count())
        .filter_map(|index| match message.get_argument(index) {
            Some(GameMessageArgumentType::ObjectID(id)) => Some(object_id_from_message(*id)),
            _ => None,
        })
        .collect()
}

fn push_pending_replay_team(op: ReplayTeamOp) {
    if let Ok(mut pending) = PENDING_REPLAY_TEAMS.lock() {
        pending.push(op);
    }
}

fn apply_replay_team_to_leftover_player(player_index: i32, slot: u8, kind: u8, ids: &[ObjectId]) {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return;
    };
    let Some(player_arc) = list.get_player(player_index).cloned() else {
        return;
    };
    drop(list);
    let Ok(mut player) = player_arc.write() else {
        return;
    };
    let object_ids: Vec<u32> = ids.iter().map(|id| id.0).collect();
    match kind {
        0 => player.process_create_team_game_message(slot as i32, &object_ids),
        1 => player.process_select_team_game_message(slot as i32),
        _ => player.process_add_team_game_message(slot as i32),
    }
}

fn apply_replay_messages_to_host(messages: &[GameMessage]) {
    for message in messages {
        let player_index = message.get_player_index();
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
                let cursor = match message.get_argument(4) {
                    Some(GameMessageArgumentType::Integer(value)) => *value,
                    _ => 0,
                };
                let pixel = match message.get_argument(5) {
                    Some(GameMessageArgumentType::Pixel(value)) => (value.x, value.y),
                    _ => (0, 0),
                };
                if let Ok(mut slot) = PENDING_REPLAY_CAMERA.lock() {
                    *slot = Some(ReplayCameraPose {
                        pos: vec3_from_coord(coord),
                        yaw: angle,
                        pitch,
                        zoom: zoom_v,
                        cursor,
                        pixel,
                        player_index,
                    });
                }
            }
            GameMessageType::NewGame | GameMessageType::ClearGameData => {
                // C++ GameLogicDispatch.cpp:396-440 prepareNewGame/clearGameData.
                append_game_message_to_stream(message);
            }
            GameMessageType::CreateTeamSlot(slot) => {
                let ids = object_ids_from_message(message);
                apply_replay_team_to_leftover_player(player_index, *slot, 0, &ids);
                push_pending_replay_team(ReplayTeamOp::Create {
                    player_index,
                    slot: *slot,
                    ids,
                });
                queue_replay_selection_remirror(player_index);
            }
            GameMessageType::SelectTeamSlot(slot) => {
                apply_replay_team_to_leftover_player(player_index, *slot, 1, &[]);
                push_pending_replay_team(ReplayTeamOp::Select {
                    player_index,
                    slot: *slot,
                });
                queue_replay_selection_remirror(player_index);
            }
            GameMessageType::AddTeamSlot(slot) => {
                apply_replay_team_to_leftover_player(player_index, *slot, 2, &[]);
                push_pending_replay_team(ReplayTeamOp::Add {
                    player_index,
                    slot: *slot,
                });
                queue_replay_selection_remirror(player_index);
            }
            GameMessageType::LogicCRC(_) => {}
            _ => {
                if let Some(command) = game_message_to_host_command(message) {
                    push_pending_replay_command(command);
                    queue_replay_selection_remirror(player_index);
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
        Build {
            template_name,
            location,
        }
        | DozerConstruct {
            template_name,
            location,
            ..
        } => GameMessageType::DozerConstruct(
            intern_name(template_name),
            coord_from_vec3(*location),
            match &command.command_type {
                DozerConstruct { orientation, .. } => *orientation,
                _ => 0.0,
            },
        ),
        DozerConstructLine {
            template_name,
            start,
            end,
        } => GameMessageType::DozerConstructLine(
            intern_name(template_name),
            coord_from_vec3(*start),
            coord_from_vec3(*end),
            0.0,
        ),
        DozerCancelConstruct { object_id } => {
            GameMessageType::DozerCancelConstruct(object_id_to_message(*object_id))
        }
        Sell { object_id } => GameMessageType::Sell(object_id_to_message(*object_id)),
        QueueUnitCreate {
            template_name,
            quantity,
        } => GameMessageType::QueueUnitCreate(intern_name(template_name), *quantity),
        CancelUnitCreate { template_name } => {
            GameMessageType::CancelUnitCreate(intern_name(template_name))
        }
        QueueUpgrade { upgrade_name } => GameMessageType::QueueUpgrade(intern_name(upgrade_name)),
        CancelUpgrade { upgrade_name } => GameMessageType::CancelUpgrade(intern_name(upgrade_name)),
        PurchaseScience { science_name } => {
            GameMessageType::PurchaseScience(intern_name(science_name))
        }
        DoSpecialPower { power_type, target } => {
            let power_id = intern_special(power_type);
            match target {
                PowerTarget::None => GameMessageType::DoSpecialPower(power_id, 0, 0),
                PowerTarget::Object(id) => GameMessageType::DoSpecialPowerAtObject(
                    power_id,
                    object_id_to_message(*id),
                    0,
                    0,
                ),
                PowerTarget::Location(pos) => GameMessageType::DoSpecialPowerAtLocation(
                    power_id,
                    coord_from_vec3(*pos),
                    0.0,
                    0,
                    0,
                    0,
                ),
                PowerTarget::LocationFacing { pos, angle } => {
                    GameMessageType::DoSpecialPowerAtLocation(
                        power_id,
                        coord_from_vec3(*pos),
                        *angle,
                        0,
                        0,
                        0,
                    )
                }
            }
        }
        DoWeapon {
            weapon_slot,
            target,
            ..
        } => {
            let slot = weapon_slot_to_id(weapon_slot);
            match target {
                WeaponTarget::Location(pos) => {
                    GameMessageType::DoWeaponAtLocation(slot, coord_from_vec3(*pos))
                }
                WeaponTarget::Object(id) => {
                    GameMessageType::DoWeaponAtObject(slot, object_id_to_message(*id))
                }
            }
        }
        Evacuate => GameMessageType::Evacuate,
        CombatDrop {
            target: DropTarget::Location(pos),
        } => GameMessageType::CombatDropAtLocation(coord_from_vec3(*pos)),
        CombatDrop {
            target: DropTarget::Object(id),
        } => GameMessageType::CombatDropAtObject(object_id_to_message(*id)),
        SetRallyPoint { location } => {
            let unit = command
                .selected_units
                .first()
                .copied()
                .map(object_id_to_message)
                .unwrap_or(0);
            GameMessageType::SetRallyPoint(unit, coord_from_vec3(*location))
        }
        Cheer => GameMessageType::DoCheer,
        PlaceBeacon { location, .. } => GameMessageType::PlaceBeacon(coord_from_vec3(*location)),
        RemoveBeacon => GameMessageType::RemoveBeacon(Coord3D::new(0.0, 0.0, 0.0)),
        SetBeaconText { text } => {
            GameMessageType::SetBeaconText(Coord3D::new(0.0, 0.0, 0.0), text.clone())
        }
        ExecuteRailedTransport => GameMessageType::ExecuteRailedTransport,
        HackInternet => GameMessageType::InternetHack,
        ToggleOvercharge => GameMessageType::ToggleOvercharge,
        SwitchWeapons { slot } => GameMessageType::SwitchWeapons(u32::from(*slot)),
        DestroySelectedGroup { team_id } => GameMessageType::DestroySelectedGroup(*team_id),
        RemoveFromSelectedGroup { units } => GameMessageType::RemoveFromSelectedGroup(
            units.iter().copied().map(object_id_to_message).collect(),
        ),
        CreateFormation => GameMessageType::CreateFormation(
            command
                .selected_units
                .iter()
                .copied()
                .map(object_id_to_message)
                .collect(),
        ),
        Exit => GameMessageType::Exit(0),
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
        CreateSelectedGroup(create_new, units) | CreateSelectedGroupNoSound(create_new, units) => {
            CommandType::CreateSelectedGroup {
                create_new: *create_new,
                units: units.iter().copied().map(object_id_from_message).collect(),
            }
        }
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
        DozerConstruct(building_type, coord, angle) => CommandType::DozerConstruct {
            template_name: resolve_name(*building_type),
            location: vec3_from_coord(coord),
            orientation: *angle,
        },
        DozerConstructLine(building_type, start, end, _angle) => CommandType::DozerConstructLine {
            template_name: resolve_name(*building_type),
            start: vec3_from_coord(start),
            end: vec3_from_coord(end),
        },
        DozerCancelConstruct(id) => CommandType::DozerCancelConstruct {
            object_id: object_id_from_message(*id),
        },
        Sell(id) => CommandType::Sell {
            object_id: object_id_from_message(*id),
        },
        QueueUnitCreate(unit_type_id, quantity) => CommandType::QueueUnitCreate {
            template_name: resolve_name(*unit_type_id),
            quantity: *quantity,
        },
        CancelUnitCreate(unit_type_id) => CommandType::CancelUnitCreate {
            template_name: resolve_name(*unit_type_id),
        },
        QueueUpgrade(upgrade_id) => CommandType::QueueUpgrade {
            upgrade_name: resolve_name(*upgrade_id),
        },
        CancelUpgrade(upgrade_id) => CommandType::CancelUpgrade {
            upgrade_name: resolve_name(*upgrade_id),
        },
        PurchaseScience(science_id) => CommandType::PurchaseScience {
            science_name: resolve_name(*science_id),
        },
        DoSpecialPower(power_id, _options, _source) => CommandType::DoSpecialPower {
            power_type: resolve_special(*power_id),
            target: PowerTarget::None,
        },
        DoSpecialPowerAtLocation(power_id, coord, angle, ..) => CommandType::DoSpecialPower {
            power_type: resolve_special(*power_id),
            target: PowerTarget::from_location_and_angle(vec3_from_coord(coord), *angle),
        },
        DoSpecialPowerAtObject(power_id, target, ..) => CommandType::DoSpecialPower {
            power_type: resolve_special(*power_id),
            target: PowerTarget::Object(object_id_from_message(*target)),
        },
        DoWeaponAtLocation(slot, coord) => CommandType::DoWeapon {
            weapon_slot: weapon_slot_from_id(*slot),
            max_shots_to_fire: -1,
            target: WeaponTarget::Location(vec3_from_coord(coord)),
        },
        DoWeaponAtObject(slot, id) => CommandType::DoWeapon {
            weapon_slot: weapon_slot_from_id(*slot),
            max_shots_to_fire: -1,
            target: WeaponTarget::Object(object_id_from_message(*id)),
        },
        Evacuate | EvacuateAtLocation(_) => CommandType::Evacuate,
        CombatDropAtLocation(coord) => CommandType::CombatDrop {
            target: DropTarget::Location(vec3_from_coord(coord)),
        },
        CombatDropAtObject(id) => CommandType::CombatDrop {
            target: DropTarget::Object(object_id_from_message(*id)),
        },
        SetRallyPoint(_unit, coord) => CommandType::SetRallyPoint {
            location: vec3_from_coord(coord),
        },
        DoCheer => CommandType::Cheer,
        PlaceBeacon(coord) => CommandType::PlaceBeacon {
            location: vec3_from_coord(coord),
            text: String::new(),
        },
        RemoveBeacon(_) => CommandType::RemoveBeacon,
        SetBeaconText(_coord, text) => CommandType::SetBeaconText { text: text.clone() },
        ExecuteRailedTransport => CommandType::ExecuteRailedTransport,
        InternetHack => CommandType::HackInternet,
        ToggleOvercharge => CommandType::ToggleOvercharge,
        SwitchWeapons(slot) => CommandType::SwitchWeapons {
            slot: u8::try_from(*slot).unwrap_or(0),
        },
        DestroySelectedGroup(team_id) => CommandType::DestroySelectedGroup { team_id: *team_id },
        RemoveFromSelectedGroup(units) => CommandType::RemoveFromSelectedGroup {
            units: units.iter().copied().map(object_id_from_message).collect(),
        },
        CreateFormation(_) => CommandType::CreateFormation,
        Exit(_) => CommandType::Exit,
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

        let restored =
            game_message_to_host_command(&message).expect("playback must restore MoveTo");
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
            cursor: 4,
            pixel: (12, 34),
            player_index: 2,
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
            Some(GameMessageArgumentType::Integer(cursor)) => assert_eq!(*cursor, 4),
            other => panic!("expected cursor int, got {other:?}"),
        }
        match camera.get_argument(5) {
            Some(GameMessageArgumentType::Pixel(pixel)) => {
                assert_eq!(pixel.x, 12);
                assert_eq!(pixel.y, 34);
            }
            other => panic!("expected pixel, got {other:?}"),
        }
        assert_eq!(camera.get_player_index(), 2);
    }

    fn host_command(command_type: CommandType) -> GameCommand {
        GameCommand {
            command_type,
            player_id: 2,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(9)],
            modifier_keys: ModifierKeys::default(),
        }
    }

    #[test]
    fn dozer_construct_round_trips_through_replay_tap() {
        // C++ Recorder.cpp:488-492 writes MSG_DOZER_CONSTRUCT from TheCommandList.
        let command = host_command(CommandType::DozerConstruct {
            template_name: "AmericaBarracks".to_string(),
            location: Vec3::new(40.0, 0.0, 8.0),
            orientation: 1.25,
        });
        let message = game_command_to_message(&command).expect("dozer construct must record");
        assert!(matches!(
            message.get_type(),
            GameMessageType::DozerConstruct(_, coord, angle)
                if (coord.x - 40.0).abs() < f32::EPSILON && (*angle - 1.25).abs() < f32::EPSILON
        ));
        match game_message_to_host_command(&message)
            .expect("playback must restore DozerConstruct")
            .command_type
        {
            CommandType::DozerConstruct {
                template_name,
                location,
                orientation,
            } => {
                assert_eq!(template_name, "AmericaBarracks");
                assert!((location.z - 8.0).abs() < f32::EPSILON);
                assert!((orientation - 1.25).abs() < f32::EPSILON);
            }
            other => panic!("expected DozerConstruct, got {other:?}"),
        }
    }

    #[test]
    fn queue_unit_and_special_power_round_trip() {
        // C++ MessageStream.h:462-584 includes queue unit + special power network IDs.
        let queue = host_command(CommandType::QueueUnitCreate {
            template_name: "AmericaInfantryRanger".to_string(),
            quantity: 3,
        });
        let queue_msg = game_command_to_message(&queue).expect("queue unit must record");
        match game_message_to_host_command(&queue_msg)
            .expect("playback must restore QueueUnitCreate")
            .command_type
        {
            CommandType::QueueUnitCreate {
                template_name,
                quantity,
            } => {
                assert_eq!(template_name, "AmericaInfantryRanger");
                assert_eq!(quantity, 3);
            }
            other => panic!("expected QueueUnitCreate, got {other:?}"),
        }

        let power = host_command(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ParticleCannon,
            target: PowerTarget::Location(Vec3::new(15.0, 0.0, 4.0)),
        });
        let power_msg = game_command_to_message(&power).expect("special power must record");
        match game_message_to_host_command(&power_msg)
            .expect("playback must restore DoSpecialPower")
            .command_type
        {
            CommandType::DoSpecialPower { power_type, target } => {
                assert_eq!(power_type, SpecialPowerType::ParticleCannon);
                match target {
                    PowerTarget::Location(pos) => {
                        assert!((pos.x - 15.0).abs() < f32::EPSILON);
                    }
                    other => panic!("expected location target, got {other:?}"),
                }
            }
            other => panic!("expected DoSpecialPower, got {other:?}"),
        }
    }

    #[test]
    fn special_power_location_facing_round_trip() {
        let power = host_command(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SneakAttack,
            target: PowerTarget::LocationFacing {
                pos: Vec3::new(15.0, 0.0, 4.0),
                angle: 1.25,
            },
        });
        let power_msg = game_command_to_message(&power).expect("facing special must record");
        match game_message_to_host_command(&power_msg)
            .expect("playback must restore facing")
            .command_type
        {
            CommandType::DoSpecialPower { power_type, target } => {
                assert_eq!(power_type, SpecialPowerType::SneakAttack);
                assert!((target.location_pos().unwrap().x - 15.0).abs() < f32::EPSILON);
                assert!((target.location_angle() - 1.25).abs() < 1.0e-5);
            }
            other => panic!("expected DoSpecialPower, got {other:?}"),
        }
    }

    #[test]
    fn apply_replay_new_game_posts_to_the_message_stream() {
        // C++ GameLogicDispatch.cpp:396-421 MSG_NEW_GAME starts the match.
        use game_engine::common::message_stream::get_message_stream;
        {
            let stream = get_message_stream();
            stream
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .clear_messages();
        }
        let mut message = GameMessage::new(GameMessageType::NewGame);
        message.append_integer_argument(3);
        message.append_integer_argument(2);
        message.append_integer_argument(10);
        message.append_integer_argument(45);
        apply_replay_messages_to_host(&[message]);
        let stream = get_message_stream();
        let guard = stream.read().unwrap_or_else(|e| e.into_inner());
        let found = guard
            .get_messages()
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::NewGame));
        assert!(found, "playback NewGame must land on TheMessageStream");
    }

    #[test]
    fn tap_create_team_slot_records_object_ids() {
        // C++ SelectionXlat.cpp:1047 MSG_CREATE_TEAM0+group carries object IDs.
        tap_host_team_slot_for_recorder(3, 0, &[ObjectId(11), ObjectId(12)]);
        let snap = snapshot_command_list();
        let team = snap
            .iter()
            .rev()
            .find(|msg| matches!(msg.get_type(), GameMessageType::CreateTeamSlot(3)))
            .expect("create team must append MSG_CREATE_TEAM3");
        let ids = object_ids_from_message(team);
        assert_eq!(ids, vec![ObjectId(11), ObjectId(12)]);
    }

    #[test]
    fn observer_mismatch_blocks_replay_camera_apply() {
        // C++ GameLogicDispatch.cpp:1803 requires getObserverLookAtPlayer()==thisPlayer.
        #[cfg(feature = "game_client")]
        {
            game_client::gui::control_bar::control_bar_observer::set_observer_look_at_player(Some(
                1,
            ));
            assert!(host_replay_observer_matches_player(1));
            assert!(!host_replay_observer_matches_player(0));
            game_client::gui::control_bar::control_bar_observer::set_observer_look_at_player(None);
            assert!(!host_replay_observer_matches_player(1));
        }
        #[cfg(not(feature = "game_client"))]
        {
            assert!(!host_replay_observer_matches_player(0));
        }
    }

    #[test]
    fn playback_move_queues_observer_selection_remirror() {
        // C++ GameLogicDispatch.cpp:1970-1984 remirrors after every network command.
        let _ = take_pending_replay_selection_remirror();
        let message =
            GameMessage::with_player(GameMessageType::DoMoveTo(Coord3D::new(4.0, 0.0, 1.0)), 4);
        apply_replay_messages_to_host(&[message]);
        assert_eq!(take_pending_replay_selection_remirror(), vec![4]);
    }

    #[test]
    fn set_replay_camera_does_not_queue_selection_remirror() {
        let _ = take_pending_replay_selection_remirror();
        let pose = ReplayCameraPose {
            pos: Vec3::new(1.0, 2.0, 3.0),
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
            cursor: 0,
            pixel: (0, 0),
            player_index: 3,
        };
        tap_replay_camera_for_recorder(pose);
        let snap = snapshot_command_list();
        let camera = snap
            .iter()
            .rev()
            .find(|msg| matches!(msg.get_type(), GameMessageType::SetReplayCamera(..)))
            .cloned()
            .expect("camera tap");
        apply_replay_messages_to_host(&[camera]);
        assert!(take_pending_replay_selection_remirror().is_empty());
        let stored = take_pending_replay_camera().expect("pose stored");
        assert_eq!(stored.player_index, 3);
    }

    fn reset_logic_crc_cadence() {
        LAST_LOGIC_CRC_FRAME.store(u32::MAX, Ordering::Relaxed);
        LAST_LOGIC_CRC.store(0, Ordering::Relaxed);
    }

    #[test]
    fn live_host_posts_logic_crc_every_replay_interval() {
        // C++ GameLogic.cpp:3634 — (m_frame % REPLAY_CRC_INTERVAL) == 0.
        reset_logic_crc_cadence();
        stamp_host_logic_frame(100);
        let posted = post_host_logic_crc_if_due(100, 0xABCD_0001).expect("frame 100 is due");
        let snap = snapshot_command_list();
        let crc_msg = snap
            .iter()
            .rev()
            .find(|msg| matches!(msg.get_type(), GameMessageType::LogicCRC(_)))
            .expect("MSG_LOGIC_CRC must land on TheCommandList");
        match crc_msg.get_type() {
            GameMessageType::LogicCRC(value) => assert_eq!(*value, posted),
            other => panic!("expected LogicCRC, got {other:?}"),
        }
        assert!(matches!(
            crc_msg.get_argument(0),
            Some(GameMessageArgumentType::Boolean(_))
        ));

        assert!(
            post_host_logic_crc_if_due(101, 0xABCD_0002).is_none(),
            "off-interval frames must not emit LogicCRC"
        );
    }

    #[test]
    fn flush_recorder_posts_logic_crc_before_update() {
        reset_logic_crc_cadence();
        stamp_host_logic_frame(200);
        let mut queue = VecDeque::new();
        flush_recorder_and_replay_authority(&mut queue);
        let snap = snapshot_command_list();
        assert!(
            snap.iter()
                .any(|msg| matches!(msg.get_type(), GameMessageType::LogicCRC(_))),
            "flush must post MSG_LOGIC_CRC so updateRecord can write .rep entries"
        );
    }
}
