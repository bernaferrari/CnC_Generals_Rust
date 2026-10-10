//! Live-path recorder tap: host orders enter `TheCommandList` so
//! `RecorderClass::updateRecord` (Recorder.cpp:455) can see them, and
//! playback sink messages become Main `GameCommand`s.
//!
//! C++ writes every network message from `TheCommandList`. Main previously
//! queued `GameCommand`s only on `GameLogic.command_queue`, so
//! `update_record` saw an empty GameClient list.

use super::*;
use crate::game_logic::GameMode;
use crossbeam::channel::{self, Receiver, Sender};
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

fn intern_special(power: &SpecialPowerType) -> u32 {
    intern_name(&format!("SP::{power:?}"))
}

fn resolve_special(id: u32) -> SpecialPowerType {
    let name = resolve_name(id);
    let Some(variant) = name.strip_prefix("SP::") else {
        return SpecialPowerType::Invalid;
    };
    // All host power variants are unit variants. Deserialize their existing
    // replay names directly, so IDs follow the receiving name registry even
    // after it resets; no separate registration or mutable reverse cache.
    let variant = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(variant);
    SpecialPowerType::deserialize(variant).unwrap_or(SpecialPowerType::Invalid)
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

/// Transient command and replay handoff for one logical game. C++ propagates
/// client messages before `RecorderClass::update`, which appends playback
/// messages before `GameLogic::processCommandList`. The callback channel
/// crosses the client/logic boundary; decoded state stays on the game instance.
pub struct ReplayPendingState {
    incoming: Receiver<Vec<GameMessage>>,
    sender: Sender<Vec<GameMessage>>,
    commands: Vec<GameCommand>,
    camera: Option<ReplayCameraPose>,
    teams: Vec<ReplayTeamOp>,
    remirror: Vec<i32>,
    host_logic_frame: Arc<AtomicU32>,
    last_recorder_update_frame: Option<u32>,
    last_logic_crc: u32,
    last_logic_crc_frame: u32,
}

impl Default for ReplayPendingState {
    fn default() -> Self {
        let (sender, incoming) = channel::unbounded();
        Self {
            incoming,
            sender,
            commands: Vec::new(),
            camera: None,
            teams: Vec::new(),
            remirror: Vec::new(),
            host_logic_frame: Arc::new(AtomicU32::new(0)),
            last_recorder_update_frame: None,
            last_logic_crc: 0,
            last_logic_crc_frame: u32::MAX,
        }
    }
}

impl ReplayPendingState {
    pub fn clear(&mut self) {
        // Disconnect a callback that captured the previous world/session's
        // sender, so a late delivery cannot enter the reset game.
        (self.sender, self.incoming) = channel::unbounded();
        self.commands.clear();
        self.camera = None;
        self.teams.clear();
        self.remirror.clear();
        self.host_logic_frame.store(0, Ordering::Relaxed);
        self.last_recorder_update_frame = None;
        self.last_logic_crc = 0;
        self.last_logic_crc_frame = u32::MAX;
    }

    pub fn take_camera(&mut self) -> Option<ReplayCameraPose> {
        self.camera.take()
    }

    pub fn take_team_ops(&mut self) -> Vec<ReplayTeamOp> {
        std::mem::take(&mut self.teams)
    }

    pub fn queue_selection_remirror(&mut self, player_index: i32) {
        if !self.remirror.contains(&player_index) {
            self.remirror.push(player_index);
        }
    }

    pub fn take_selection_remirror(&mut self) -> Vec<i32> {
        std::mem::take(&mut self.remirror)
    }

    fn take_incoming(&mut self) -> Vec<GameMessage> {
        let mut pending = Vec::new();
        while let Ok(messages) = self.incoming.try_recv() {
            pending.extend(messages);
        }
        pending
    }
}

static BRIDGES_INSTALLED: AtomicBool = AtomicBool::new(false);

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

/// GameClient routes a completed stream batch to Main before GameLogic runs.
/// Direct taps already waiting from an earlier UI operation retain their
/// place; the new stream batch precedes this logic frame's MSG_LOGIC_CRC.
fn merge_routed_messages_into_command_list(messages: &[GameMessage]) {
    #[cfg(feature = "game_client")]
    {
        if messages.is_empty() {
            return;
        }
        if let Ok(mut list) = game_client::message_stream::command_list::get_command_list().write()
        {
            let (crc_taps, direct_taps): (Vec<_>, Vec<_>) = list
                .snapshot_messages()
                .into_iter()
                .partition(|message| matches!(message.get_type(), GameMessageType::LogicCRC(_)));
            list.retain_messages(|_| false);
            list.append_message_list(direct_taps);
            list.append_message_list(messages.to_vec());
            list.append_message_list(crc_taps);
        }
    }
    #[cfg(not(feature = "game_client"))]
    let _ = messages;
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

fn host_logic_frame(state: &ReplayPendingState) -> u32 {
    state.host_logic_frame.load(Ordering::Relaxed)
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
    let _ = with_recorder_mut(|recorder| {
        recorder.set_command_source(Some(command_source));
        recorder.set_command_sink(Some(command_sink));
        recorder.set_command_cull(Some(command_cull));
    });
}

/// Bind the GameClient command-router callback to the active world's queue.
/// The closure retains only a sender: dropping a world disconnects its queue.
pub fn bind_host_replay_authority(state: &ReplayPendingState) {
    install_host_replay_bridges();
    let host_frame = Arc::clone(&state.host_logic_frame);
    let _ = with_recorder_mut(|recorder| {
        recorder.set_frame_provider(Some(Arc::new(move || host_frame.load(Ordering::Relaxed))));
    });
    #[cfg(feature = "game_client")]
    {
        let sender = state.sender.clone();
        game_client::message_stream::command_router::set_host_command_authority(Some(Arc::new(
            move |messages| {
                let _ = sender.send(messages.to_vec());
            },
        )));
    }
    #[cfg(not(feature = "game_client"))]
    let _ = state;
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
pub fn tap_host_new_game_for_recorder(mode: GameMode, selected_seed: u32) {
    install_host_replay_bridges();
    let _ = with_recorder_mut(|recorder| recorder.set_recording_seed(selected_seed));
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
pub fn stamp_host_logic_frame(state: &mut ReplayPendingState, frame: u32) {
    state.host_logic_frame.store(frame, Ordering::Relaxed);
}

pub(crate) fn logic_crc_due(frame: u32) -> bool {
    let interval = game_engine::common::crc_debug::replay_crc_interval();
    // C++ GameLogic.cpp:3634 — m_frame > 0 && m_frame % REPLAY_CRC_INTERVAL == 0.
    interval > 0 && frame > 0 && frame % (interval as u32) == 0
}

/// C++ `GameLogic.cpp:3625-3654`: append the driving simulation's checksum
/// unchanged before recorder update. Main supplies its owned Rust checksum;
/// numeric equivalence to the original full `getCRC` is not established.
pub fn post_host_logic_crc_if_due(
    state: &mut ReplayPendingState,
    frame: u32,
    state_crc: u32,
) -> Option<u32> {
    if !logic_crc_due(frame) {
        return None;
    }
    if state.last_logic_crc_frame == frame {
        return Some(state.last_logic_crc);
    }

    let crc = state_crc;
    log::trace!("MSG_LOGIC_CRC frame={} state_crc=0x{:08X}", frame, crc);

    let playback = host_recorder_is_playback();
    // C++ GameLogicDispatch.cpp:1904-1946 threads the local player index.
    let mut message =
        GameMessage::with_player(GameMessageType::LogicCRC(crc), host_local_player_index());
    message.append_boolean_argument(playback);
    append_to_command_list(message);

    state.last_logic_crc = crc;
    state.last_logic_crc_frame = frame;
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
        // Unresolvable (no player list yet): fall back to C++ player index 0.
        return 0;
    };
    let index = list.get_local_player_index();
    if index == gamelogic::player::PLAYER_INDEX_INVALID {
        // Unresolvable (no local player bound yet): fall back to 0.
        0
    } else {
        index
    }
}

/// C++ `GameLogic::update` resets `TheCommandList` after recorder update and
/// command dispatch (GameLogic.cpp:3765). Main has already queued live orders
/// in `host_queue`, so the recorded snapshot must be discarded before a
/// second host command pass or the next logic frame. Retaining `MSG_NEW_GAME`
/// starts the same replay again; retaining CRCs or orders writes them twice.
fn reset_recorded_command_list() {
    #[cfg(feature = "game_client")]
    {
        if let Ok(mut list) = game_client::message_stream::command_list::get_command_list().write()
        {
            list.retain_messages(|_| false);
            list.reset_frame_counter();
        }
    }
}

/// C++ GameState::loadGame calls GameEngine::reset, which resets Recorder and
/// CommandList before loading an ordinary save. The Rust host commits a
/// staged world atomically, so close the old replay only after commit succeeds.
pub(crate) fn reset_host_recorder_after_successful_load(state: &mut ReplayPendingState) {
    let _ = with_recorder_mut(|recorder| recorder.reset());
    reset_recorded_command_list();
    state.clear();
}

/// C++ `GameLogic::update` ticks `TheRecorder` then `processCommandList`.
/// Recording: write CommandList then drop it (host already queued the order).
/// Playback: playback sink fills CommandList; convert into the live host queue.
pub fn flush_recorder_and_replay_authority(
    state: &mut ReplayPendingState,
    host_queue: &mut VecDeque<GameCommand>,
    state_crc: u32,
) {
    install_host_replay_bridges();
    let frame = host_logic_frame(state);
    if state.last_recorder_update_frame == Some(frame) {
        // GameLogic has a second post-AI command pass in this logic frame.
        // Its orders still drain through process_commands. A synchronous UI
        // command may also arrive after the first pass; keep its direct tap
        // for the next recorder frame instead of dropping it here.
        return;
    }

    let routed = state.take_incoming();
    merge_routed_messages_into_command_list(&routed);
    let playback = host_recorder_is_playback();
    // C++ posts MSG_LOGIC_CRC onto the stream before TheRecorder->update().
    let posted = post_host_logic_crc_if_due(state, frame, state_crc);
    let _ = with_recorder_mut(|recorder| {
        recorder.set_current_frame(frame);
        recorder.update();
        if playback {
            if let Some(crc) = posted {
                // C++ GameLogicDispatch.cpp:1940-1946 — compare only in
                // playback, attributed to the local player.
                recorder.notify_logic_crc(crc, host_local_player_index());
            }
        }
    });
    state.last_recorder_update_frame = Some(frame);

    if playback {
        // C++ cullBadCommands drops user network orders before processCommandList.
        host_queue.retain(|cmd| game_command_to_message(cmd).is_none());
        let messages = take_command_list_messages();
        apply_replay_messages_to_host(state, &messages);
    } else {
        // The client delivered this batch to its legacy queue already. Main
        // executes each translated host order once after Recorder snapshots it.
        apply_replay_messages_to_host(state, &routed);
        // C++ clears all messages at the end of the logic frame. Main already
        // owns the executable orders in `host_queue`; leaving lifecycle
        // messages here makes a second pass call startRecording again.
        reset_recorded_command_list();
    }

    for command in std::mem::take(&mut state.commands) {
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

fn apply_replay_messages_to_host(state: &mut ReplayPendingState, messages: &[GameMessage]) {
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
                state.camera = Some(ReplayCameraPose {
                    pos: vec3_from_coord(coord),
                    yaw: angle,
                    pitch,
                    zoom: zoom_v,
                    cursor,
                    pixel,
                    player_index,
                });
            }
            GameMessageType::NewGame | GameMessageType::ClearGameData => {
                // C++ GameLogicDispatch.cpp:396-440 prepareNewGame/clearGameData.
                append_game_message_to_stream(message);
            }
            GameMessageType::CreateTeamSlot(slot) => {
                let ids = object_ids_from_message(message);
                apply_replay_team_to_leftover_player(player_index, *slot, 0, &ids);
                state.teams.push(ReplayTeamOp::Create {
                    player_index,
                    slot: *slot,
                    ids,
                });
                state.queue_selection_remirror(player_index);
            }
            GameMessageType::SelectTeamSlot(slot) => {
                apply_replay_team_to_leftover_player(player_index, *slot, 1, &[]);
                state.teams.push(ReplayTeamOp::Select {
                    player_index,
                    slot: *slot,
                });
                state.queue_selection_remirror(player_index);
            }
            GameMessageType::AddTeamSlot(slot) => {
                apply_replay_team_to_leftover_player(player_index, *slot, 2, &[]);
                state.teams.push(ReplayTeamOp::Add {
                    player_index,
                    slot: *slot,
                });
                state.queue_selection_remirror(player_index);
            }
            GameMessageType::LogicCRC(_) => {}
            _ => {
                if let Some(command) = game_message_to_host_command(message) {
                    state.commands.push(command);
                    state.queue_selection_remirror(player_index);
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
#[path = "record_tap_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "record_tap_special_power_tests.rs"]
mod special_power_tests;
