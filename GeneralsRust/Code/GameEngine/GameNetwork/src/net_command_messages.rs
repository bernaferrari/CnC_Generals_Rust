use crate::network_defs::NetCommandType;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// BASE NET COMMAND MESSAGE
// Matches C++ NetCommandMsg.h lines 15-46
// ============================================================================

/// Base network command message
/// Matches C++ class NetCommandMsg
pub struct NetCommandMsg {
    /// Timestamp when message was created
    timestamp: u32,
    /// Frame when this command should execute
    execution_frame: u32,
    /// Player ID who sent this command
    player_id: u32,
    /// Unique command ID
    id: u16,
    /// Type of network command
    command_type: NetCommandType,
    /// Reference count for memory management
    reference_count: u32,
}

impl NetCommandMsg {
    /// Create a new base command message
    pub fn new() -> Self {
        Self {
            timestamp: get_timestamp(),
            execution_frame: 0,
            player_id: 0,
            id: 0,
            command_type: NetCommandType::Unknown,
            reference_count: 1, // start at 1, matches C++ constructor
        }
    }

    // Getters
    pub fn get_timestamp(&self) -> u32 {
        self.timestamp
    }
    pub fn get_execution_frame(&self) -> u32 {
        self.execution_frame
    }
    pub fn get_player_id(&self) -> u32 {
        self.player_id
    }
    pub fn get_id(&self) -> u16 {
        self.id
    }
    pub fn get_net_command_type(&self) -> NetCommandType {
        self.command_type
    }

    // Setters
    pub fn set_timestamp(&mut self, timestamp: u32) {
        self.timestamp = timestamp;
    }
    pub fn set_execution_frame(&mut self, frame: u32) {
        self.execution_frame = frame;
    }
    pub fn set_player_id(&mut self, player_id: u32) {
        self.player_id = player_id;
    }
    pub fn set_id(&mut self, id: u16) {
        self.id = id;
    }
    pub fn set_net_command_type(&mut self, command_type: NetCommandType) {
        self.command_type = command_type;
    }

    /// Get sort number for ordered list
    /// Matches C++ NetCommandMsg::getSortNumber()
    pub fn get_sort_number(&self) -> i32 {
        self.id as i32
    }

    /// Increment reference count
    /// Matches C++ NetCommandMsg::attach()
    pub fn attach(&mut self) {
        self.reference_count += 1;
    }

    /// Decrement reference count
    /// Matches C++ NetCommandMsg::detach()
    pub fn detach(&mut self) -> bool {
        self.reference_count -= 1;
        self.reference_count == 0
    }
}

// ============================================================================
// GAME COMMAND MESSAGE
// Matches C++ NetCommandMsg.h lines 49-73
// ============================================================================

/// Game command message
/// Matches C++ class NetGameCommandMsg
pub struct NetGameCommandMsg {
    base: NetCommandMsg,
    num_args: i32,
    arg_size: i32,
    message_type: u32, // GameMessage::Type
    arg_list: Vec<GameMessageArgument>,
}

impl NetGameCommandMsg {
    /// Create new game command message
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::GameCommand);
        Self {
            base,
            num_args: 0,
            arg_size: 0,
            message_type: 0,
            arg_list: Vec::new(),
        }
    }

    /// Add an argument to this command
    /// Matches C++ NetGameCommandMsg::addArgument()
    pub fn add_argument(
        &mut self,
        arg_type: GameMessageArgumentDataType,
        arg: GameMessageArgumentType,
    ) {
        self.arg_list.push(GameMessageArgument {
            data_type: arg_type,
            data: arg,
        });
        self.num_args += 1;
    }

    /// Set the game message type
    pub fn set_game_message_type(&mut self, msg_type: u32) {
        self.message_type = msg_type;
    }

    /// Get base message
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

// ============================================================================
// ACK MESSAGE TYPES
// Matches C++ NetCommandMsg.h lines 75-145
// ============================================================================

/// ACK Both command message
/// Matches C++ class NetAckBothCommandMsg
pub struct NetAckBothCommandMsg {
    base: NetCommandMsg,
    command_id: u16,
    original_player_id: u8,
}

impl NetAckBothCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::AckBoth);
        Self {
            base,
            command_id: 0,
            original_player_id: 0,
        }
    }

    pub fn get_command_id(&self) -> u16 {
        self.command_id
    }
    pub fn set_command_id(&mut self, id: u16) {
        self.command_id = id;
    }
    pub fn get_original_player_id(&self) -> u8 {
        self.original_player_id
    }
    pub fn set_original_player_id(&mut self, id: u8) {
        self.original_player_id = id;
    }

    pub fn get_sort_number(&self) -> i32 {
        self.command_id as i32
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// ACK Stage 1 command message
/// Matches C++ class NetAckStage1CommandMsg
pub struct NetAckStage1CommandMsg {
    base: NetCommandMsg,
    command_id: u16,
    original_player_id: u8,
}

impl NetAckStage1CommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::AckStage1);
        Self {
            base,
            command_id: 0,
            original_player_id: 0,
        }
    }

    pub fn get_command_id(&self) -> u16 {
        self.command_id
    }
    pub fn set_command_id(&mut self, id: u16) {
        self.command_id = id;
    }
    pub fn get_original_player_id(&self) -> u8 {
        self.original_player_id
    }
    pub fn set_original_player_id(&mut self, id: u8) {
        self.original_player_id = id;
    }

    pub fn get_sort_number(&self) -> i32 {
        self.command_id as i32
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// ACK Stage 2 command message
/// Matches C++ class NetAckStage2CommandMsg
pub struct NetAckStage2CommandMsg {
    base: NetCommandMsg,
    command_id: u16,
    original_player_id: u8,
}

impl NetAckStage2CommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::AckStage2);
        Self {
            base,
            command_id: 0,
            original_player_id: 0,
        }
    }

    pub fn get_command_id(&self) -> u16 {
        self.command_id
    }
    pub fn set_command_id(&mut self, id: u16) {
        self.command_id = id;
    }
    pub fn get_original_player_id(&self) -> u8 {
        self.original_player_id
    }
    pub fn set_original_player_id(&mut self, id: u8) {
        self.original_player_id = id;
    }

    pub fn get_sort_number(&self) -> i32 {
        self.command_id as i32
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

// ============================================================================
// FRAME INFO MESSAGE
// Matches C++ NetCommandMsg.h lines 147-160
// ============================================================================

/// Frame info command message
/// Matches C++ class NetFrameCommandMsg
pub struct NetFrameCommandMsg {
    base: NetCommandMsg,
    command_count: u16,
}

impl NetFrameCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::FrameInfo);
        Self {
            base,
            command_count: 0,
        }
    }

    pub fn get_command_count(&self) -> u16 {
        self.command_count
    }
    pub fn set_command_count(&mut self, count: u16) {
        self.command_count = count;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

// ============================================================================
// PLAYER LEAVE MESSAGE
// Matches C++ NetCommandMsg.h lines 162-175
// ============================================================================

/// Player leave command message
/// Matches C++ class NetPlayerLeaveCommandMsg
pub struct NetPlayerLeaveCommandMsg {
    base: NetCommandMsg,
    leaving_player_id: u8,
}

impl NetPlayerLeaveCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::PlayerLeave);
        Self {
            base,
            leaving_player_id: 0,
        }
    }

    pub fn get_leaving_player_id(&self) -> u8 {
        self.leaving_player_id
    }
    pub fn set_leaving_player_id(&mut self, id: u8) {
        self.leaving_player_id = id;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

// ============================================================================
// RUN AHEAD MESSAGES
// Matches C++ NetCommandMsg.h lines 177-212
// ============================================================================

/// Run ahead metrics command message
/// Matches C++ class NetRunAheadMetricsCommandMsg
pub struct NetRunAheadMetricsCommandMsg {
    base: NetCommandMsg,
    average_latency: f32,
    average_fps: i32,
}

impl NetRunAheadMetricsCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::RunAheadMetrics);
        Self {
            base,
            average_latency: 0.0,
            average_fps: 0,
        }
    }

    pub fn get_average_latency(&self) -> f32 {
        self.average_latency
    }
    pub fn set_average_latency(&mut self, lat: f32) {
        self.average_latency = lat;
    }
    pub fn get_average_fps(&self) -> i32 {
        self.average_fps
    }
    pub fn set_average_fps(&mut self, fps: i32) {
        self.average_fps = fps;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Run ahead command message
/// Matches C++ class NetRunAheadCommandMsg
pub struct NetRunAheadCommandMsg {
    base: NetCommandMsg,
    run_ahead: u16,
    frame_rate: u8,
}

impl NetRunAheadCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::RunAhead);
        Self {
            base,
            run_ahead: 0,
            frame_rate: 0,
        }
    }

    pub fn get_run_ahead(&self) -> u16 {
        self.run_ahead
    }
    pub fn set_run_ahead(&mut self, ra: u16) {
        self.run_ahead = ra;
    }
    pub fn get_frame_rate(&self) -> u8 {
        self.frame_rate
    }
    pub fn set_frame_rate(&mut self, fr: u8) {
        self.frame_rate = fr;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

// ============================================================================
// OTHER COMMAND MESSAGES
// Matches C++ NetCommandMsg.h lines 214-501
// ============================================================================

/// Destroy player command message
/// Matches C++ class NetDestroyPlayerCommandMsg
pub struct NetDestroyPlayerCommandMsg {
    base: NetCommandMsg,
    player_index: u32,
}

impl NetDestroyPlayerCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::DestroyPlayer);
        Self {
            base,
            player_index: 0,
        }
    }

    pub fn get_player_index(&self) -> u32 {
        self.player_index
    }
    pub fn set_player_index(&mut self, idx: u32) {
        self.player_index = idx;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Keep alive command message
/// Matches C++ class NetKeepAliveCommandMsg
pub struct NetKeepAliveCommandMsg {
    base: NetCommandMsg,
}

impl NetKeepAliveCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::KeepAlive);
        Self { base }
    }

    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Disconnect keep alive command message
/// Matches C++ class NetDisconnectKeepAliveCommandMsg
pub struct NetDisconnectKeepAliveCommandMsg {
    base: NetCommandMsg,
}

impl NetDisconnectKeepAliveCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::DisconnectKeepAlive);
        Self { base }
    }

    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Disconnect player command message
/// Matches C++ class NetDisconnectPlayerCommandMsg
pub struct NetDisconnectPlayerCommandMsg {
    base: NetCommandMsg,
    disconnect_slot: u8,
    disconnect_frame: u32,
}

impl NetDisconnectPlayerCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::DisconnectPlayer);
        Self {
            base,
            disconnect_slot: 0,
            disconnect_frame: 0,
        }
    }

    pub fn get_disconnect_slot(&self) -> u8 {
        self.disconnect_slot
    }
    pub fn set_disconnect_slot(&mut self, slot: u8) {
        self.disconnect_slot = slot;
    }
    pub fn get_disconnect_frame(&self) -> u32 {
        self.disconnect_frame
    }
    pub fn set_disconnect_frame(&mut self, frame: u32) {
        self.disconnect_frame = frame;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Chat command message
/// Matches C++ class NetChatCommandMsg
pub struct NetChatCommandMsg {
    base: NetCommandMsg,
    text: String,
    player_mask: i32,
}

impl NetChatCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::Chat);
        Self {
            base,
            text: String::new(),
            player_mask: 0,
        }
    }

    pub fn get_text(&self) -> &str {
        &self.text
    }
    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }
    pub fn get_player_mask(&self) -> i32 {
        self.player_mask
    }
    pub fn set_player_mask(&mut self, mask: i32) {
        self.player_mask = mask;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Progress command message
/// Matches C++ class NetProgressCommandMsg
pub struct NetProgressCommandMsg {
    base: NetCommandMsg,
    percent: u8,
}

impl NetProgressCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::Progress);
        Self { base, percent: 0 }
    }

    pub fn get_percentage(&self) -> u8 {
        self.percent
    }
    pub fn set_percentage(&mut self, p: u8) {
        self.percent = p;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Wrapper command message (for large commands)
/// Matches C++ class NetWrapperCommandMsg
pub struct NetWrapperCommandMsg {
    base: NetCommandMsg,
    data: Vec<u8>,
    data_offset: u32,
    total_data_length: u32,
    chunk_number: u32,
    num_chunks: u32,
    wrapped_command_id: u16,
}

impl NetWrapperCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::Wrapper);
        Self {
            base,
            data: Vec::new(),
            data_offset: 0,
            total_data_length: 0,
            chunk_number: 0,
            num_chunks: 0,
            wrapped_command_id: 0,
        }
    }

    pub fn get_data(&self) -> &[u8] {
        &self.data
    }
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }
    pub fn get_chunk_number(&self) -> u32 {
        self.chunk_number
    }
    pub fn set_chunk_number(&mut self, num: u32) {
        self.chunk_number = num;
    }
    pub fn get_num_chunks(&self) -> u32 {
        self.num_chunks
    }
    pub fn set_num_chunks(&mut self, num: u32) {
        self.num_chunks = num;
    }
    pub fn get_data_length(&self) -> usize {
        self.data.len()
    }
    pub fn get_total_data_length(&self) -> u32 {
        self.total_data_length
    }
    pub fn set_total_data_length(&mut self, len: u32) {
        self.total_data_length = len;
    }
    pub fn get_data_offset(&self) -> u32 {
        self.data_offset
    }
    pub fn set_data_offset(&mut self, offset: u32) {
        self.data_offset = offset;
    }
    pub fn get_wrapped_command_id(&self) -> u16 {
        self.wrapped_command_id
    }
    pub fn set_wrapped_command_id(&mut self, id: u16) {
        self.wrapped_command_id = id;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// File command message
/// Matches C++ class NetFileCommandMsg
pub struct NetFileCommandMsg {
    base: NetCommandMsg,
    portable_filename: String,
    data: Vec<u8>,
}

impl NetFileCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::File);
        Self {
            base,
            portable_filename: String::new(),
            data: Vec::new(),
        }
    }

    pub fn get_portable_filename(&self) -> &str {
        &self.portable_filename
    }
    pub fn set_portable_filename(&mut self, name: String) {
        self.portable_filename = name;
    }
    pub fn get_file_data(&self) -> &[u8] {
        &self.data
    }
    pub fn set_file_data(&mut self, data: Vec<u8>) {
        self.data = data;
    }
    pub fn get_file_length(&self) -> usize {
        self.data.len()
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// File announce command message
/// Matches C++ class NetFileAnnounceCommandMsg
pub struct NetFileAnnounceCommandMsg {
    base: NetCommandMsg,
    portable_filename: String,
    file_id: u16,
    player_mask: u8,
}

impl NetFileAnnounceCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::FileAnnounce);
        Self {
            base,
            portable_filename: String::new(),
            file_id: 0,
            player_mask: 0,
        }
    }

    pub fn get_portable_filename(&self) -> &str {
        &self.portable_filename
    }
    pub fn set_portable_filename(&mut self, name: String) {
        self.portable_filename = name;
    }
    pub fn get_file_id(&self) -> u16 {
        self.file_id
    }
    pub fn set_file_id(&mut self, id: u16) {
        self.file_id = id;
    }
    pub fn get_player_mask(&self) -> u8 {
        self.player_mask
    }
    pub fn set_player_mask(&mut self, mask: u8) {
        self.player_mask = mask;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// File progress command message
/// Matches C++ class NetFileProgressCommandMsg
pub struct NetFileProgressCommandMsg {
    base: NetCommandMsg,
    file_id: u16,
    progress: i32,
}

impl NetFileProgressCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::FileProgress);
        Self {
            base,
            file_id: 0,
            progress: 0,
        }
    }

    pub fn get_file_id(&self) -> u16 {
        self.file_id
    }
    pub fn set_file_id(&mut self, id: u16) {
        self.file_id = id;
    }
    pub fn get_progress(&self) -> i32 {
        self.progress
    }
    pub fn set_progress(&mut self, prog: i32) {
        self.progress = prog;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

/// Frame resend request command message
/// Matches C++ class NetFrameResendRequestCommandMsg
pub struct NetFrameResendRequestCommandMsg {
    base: NetCommandMsg,
    frame_to_resend: u32,
}

impl NetFrameResendRequestCommandMsg {
    pub fn new() -> Self {
        let mut base = NetCommandMsg::new();
        base.set_net_command_type(NetCommandType::FrameResendRequest);
        Self {
            base,
            frame_to_resend: 0,
        }
    }

    pub fn get_frame_to_resend(&self) -> u32 {
        self.frame_to_resend
    }
    pub fn set_frame_to_resend(&mut self, frame: u32) {
        self.frame_to_resend = frame;
    }
    pub fn base(&self) -> &NetCommandMsg {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NetCommandMsg {
        &mut self.base
    }
}

// ============================================================================
// HELPER TYPES AND FUNCTIONS
// ============================================================================

/// Game message argument data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMessageArgumentDataType {
    Integer,
    Real,
    Boolean,
    ObjectId,
    DrawableId,
    TeamId,
    Location,
    Pixel,
    PixelRegion,
    Timestamp,
    WideChar,
}

/// Game message argument type (union-like)
pub union GameMessageArgumentType {
    pub integer: i32,
    pub real: f32,
    pub boolean: bool,
    pub object_id: u64,
    pub drawable_id: u64,
    pub team_id: u32,
}

impl Clone for GameMessageArgumentType {
    fn clone(&self) -> Self {
        Self {
            integer: unsafe { self.integer },
        }
    }
}

impl Copy for GameMessageArgumentType {}

impl std::fmt::Debug for GameMessageArgumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameMessageArgumentType").finish()
    }
}

/// Game message argument
pub struct GameMessageArgument {
    data_type: GameMessageArgumentDataType,
    data: GameMessageArgumentType,
}

/// Get current timestamp in milliseconds
fn get_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u32
}

/// Global command ID generator
/// Matches C++ GenerateNextCommandID() function
static NEXT_COMMAND_ID: AtomicU16 = AtomicU16::new(64000);

pub fn generate_next_command_id() -> u16 {
    NEXT_COMMAND_ID.fetch_add(1, Ordering::SeqCst)
}

/// Check if command type requires a command ID
/// Matches C++ DoesCommandRequireACommandID()
pub fn does_command_require_command_id(cmd_type: NetCommandType) -> bool {
    matches!(
        cmd_type,
        NetCommandType::GameCommand
            | NetCommandType::FrameInfo
            | NetCommandType::PlayerLeave
            | NetCommandType::DestroyPlayer
            | NetCommandType::RunAheadMetrics
            | NetCommandType::RunAhead
            | NetCommandType::Chat
            | NetCommandType::DisconnectVote
            | NetCommandType::LoadComplete
            | NetCommandType::TimeoutStart
            | NetCommandType::Wrapper
            | NetCommandType::File
            | NetCommandType::FileAnnounce
            | NetCommandType::FileProgress
            | NetCommandType::DisconnectPlayer
            | NetCommandType::DisconnectFrame
            | NetCommandType::DisconnectScreenOff
            | NetCommandType::FrameResendRequest
    )
}

/// Check if command requires acknowledgment
/// Matches C++ CommandRequiresAck()
pub fn command_requires_ack(cmd_type: NetCommandType) -> bool {
    matches!(
        cmd_type,
        NetCommandType::GameCommand
            | NetCommandType::FrameInfo
            | NetCommandType::PlayerLeave
            | NetCommandType::DestroyPlayer
            | NetCommandType::RunAheadMetrics
            | NetCommandType::RunAhead
            | NetCommandType::Chat
            | NetCommandType::DisconnectVote
            | NetCommandType::DisconnectPlayer
            | NetCommandType::LoadComplete
            | NetCommandType::TimeoutStart
            | NetCommandType::Wrapper
            | NetCommandType::File
            | NetCommandType::FileAnnounce
            | NetCommandType::FileProgress
            | NetCommandType::DisconnectFrame
            | NetCommandType::DisconnectScreenOff
            | NetCommandType::FrameResendRequest
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_id_generation() {
        let id1 = generate_next_command_id();
        let id2 = generate_next_command_id();
        assert!(id2 > id1);
    }

    #[test]
    fn test_message_creation() {
        let msg = NetCommandMsg::new();
        assert_eq!(msg.get_execution_frame(), 0);
        assert_eq!(msg.get_player_id(), 0);
        assert_eq!(msg.get_net_command_type(), NetCommandType::Unknown);
    }

    #[test]
    fn test_game_command_message() {
        let mut cmd = NetGameCommandMsg::new();
        assert_eq!(
            cmd.base().get_net_command_type(),
            NetCommandType::GameCommand
        );
        cmd.set_game_message_type(42);
        assert_eq!(cmd.message_type, 42);
    }

    #[test]
    fn test_ack_messages() {
        let mut ack = NetAckBothCommandMsg::new();
        ack.set_command_id(100);
        ack.set_original_player_id(2);
        assert_eq!(ack.get_command_id(), 100);
        assert_eq!(ack.get_original_player_id(), 2);
        assert_eq!(ack.get_sort_number(), 100);
    }
}
