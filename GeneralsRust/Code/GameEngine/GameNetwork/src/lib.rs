//! # GameNetwork - Modern Networking for Command & Conquer Generals Zero Hour
//!
//! This module provides a complete, modern networking system for multiplayer gaming,
//! replacing the legacy GameSpy-based networking with secure, high-performance
//! async Rust implementations.
//!
//! ## Features
//!
//! - **Async Transport Layer**: Built on tokio for high-performance async networking
//! - **Multi-Protocol Support**: TCP, UDP, WebSocket, and QUIC protocols
//! - **Secure Communication**: End-to-end encryption and authentication
//! - **Deterministic Networking**: Frame-based synchronization for RTS gameplay
//! - **Modern Matchmaking**: Replace GameSpy with cloud-based services
//! - **File Transfer**: Async map and mod distribution with resume support
//! - **Anti-Cheat**: Command validation and cheat detection
//! - **Cross-Platform**: Support for modern gaming platforms
//!
//! ## Architecture
//!
//! The networking system is built around several core components:
//!
//! - [`Transport`]: Low-level packet transport layer
//! - [`Connection`]: Per-player connection management
//! - [`NetCommand`]: Game command serialization and validation  
//! - [`FrameData`]: Deterministic frame synchronization
//! - [`FileTransfer`]: Async file distribution system
//! - [`Matchmaking`]: Modern lobby and matchmaking services

#![allow(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use async_trait::async_trait;
use game_engine::common::system::compression::{
    compress_data, get_preferred_compression, CompressionLevel,
};
use game_engine::get_game_state;
use parking_lot::{Mutex, RwLock as ParkingRwLock};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::{broadcast, watch, Mutex as AsyncMutex, OnceCell, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use crate::commands::{CommandPayload, FrameInfoData, ProgressType};
use crate::connection::{
    disconnect_voting::{
        DisconnectReason, DisconnectVotingCoordinator, VoteDecision, VoteEvent, VoteEvidence,
    },
    ConnectionManager as ConnManager,
};
use crate::error::{NetworkError as NetError, NetworkResult as NetResult};
use crate::frame_data::{
    FrameData as SyncFrameData, FrameDataManager as FrameManager, FrameExecutor,
};
use crate::nat::{NatBinding as NatBind, NatConfig as NatCfg, NatService as NatSvc};
use crate::observability::telemetry;
use crate::security::firewall::{FirewallConfig as FwConfig, FirewallHelper as FwHelper};
use crate::security::SecurityManager as SecManager;
use crate::transport::Transport as NetTransport;
use crate::utils::NetworkUtils;

// Public modules
pub mod bridge; // Network bridge connecting transport to GameLogic
pub mod command_types;
pub mod commands;
pub mod connection;
pub mod connection_state; // Connection state machine matching C++ NetLocalStatus lifecycle
pub mod desync_manager;
pub mod download_manager;
pub mod error;
pub mod file_transfer;
pub mod firewall_helper;
pub mod frame_data;
pub mod frame_manager; // Frame synchronization and runahead tracking
pub mod frame_resend; // Frame resend request handling for missing frame recovery
pub mod game_info; // Game setup state information (slots, settings, serialization)
pub mod game_message_parser;
pub mod gamespy;
pub mod gui_util;
pub mod integration; // Game logic integration layer
pub mod ip_enumeration;
pub mod keep_alive; // Keep-alive system for NAT mapping and connection health
pub mod lan_api;
pub mod matchmaking;
pub mod nat;
pub mod nat_traversal;
pub mod net_command_list;
pub mod net_command_messages;
pub mod net_command_msg;
pub mod net_command_ref;
pub mod net_command_wrapper_list;
pub mod net_message_stream;
pub mod net_packet;
pub mod network; // C++-style Network singleton wrapper
pub mod network_defs; // Network definitions and constants
pub mod network_metrics; // Network metrics and monitoring
pub mod network_util;
pub mod observability;
pub mod rank_point_value;
pub mod security;
pub mod sync;
pub mod time;
pub mod transport;
pub mod transport_udp; // Raw UDP transport matching C++ exactly
pub mod transport_unified; // Unified transport supporting both UDP and QUIC
pub mod udp; // Legacy UDP wrapper compatibility
pub mod utils;
pub mod wol_browser; // Deterministic lockstep game synchronization

// Re-exports
pub mod game_spy_thread;
pub mod gamespy_thread;
pub mod lan_player;
pub mod network_interface;
pub mod networkutil;
pub use command_types::NetCommandType as NetCommandTypeI32;
pub use commands::{NetCommand, NetCommandType};
pub use connection::{
    ChatEvent, Connection, ConnectionManager, FileAnnouncementEvent, FileAnnouncementState,
    FileProgressEvent, FileProgressState, FileProgressStatus, ManagedTransfer, TimeoutEvent,
};
pub use connection_state::{
    ConnectionInfo, ConnectionState, ConnectionStateMachine, ConnectionStatistics, StateTransition,
};
pub use desync_manager::{DesyncInfo, DesyncManager, DesyncMetrics};
pub use download_manager::{
    DownloadEvent, DownloadManager, DownloadProgress, QueuedDownload,
    DOWNLOADEVENT_COULDNOTCONNECT, DOWNLOADEVENT_DISCONNECTERROR,
    DOWNLOADEVENT_LOCALFILEOPENFAILED, DOWNLOADEVENT_LOGINFAILED, DOWNLOADEVENT_NOSUCHFILE,
    DOWNLOADEVENT_NOSUCHSERVER, DOWNLOADEVENT_TCPERROR, DOWNLOADSTATUS_CONNECTING,
    DOWNLOADSTATUS_DISCONNECTING, DOWNLOADSTATUS_DONE, DOWNLOADSTATUS_DOWNLOADING,
    DOWNLOADSTATUS_FINDINGFILE, DOWNLOADSTATUS_FINISHING, DOWNLOADSTATUS_GO,
    DOWNLOADSTATUS_LOGGINGIN, DOWNLOADSTATUS_NONE, DOWNLOADSTATUS_QUERYINGRESUME,
};
pub use error::{NetworkError, NetworkResult};
pub use file_transfer::FileMetadata;
pub use file_transfer::{
    bandwidth::{BandwidthManager, BandwidthThrottle},
    TransferType,
};
pub use frame_data::{FrameData, FrameDataManager};
pub use frame_manager::{
    FrameData as SyncFrame, FrameDataManager as SyncFrameManager, FrameManagerStats,
    FRAMES_TO_KEEP, FRAME_DATA_LENGTH, MAX_FRAMES_AHEAD, MIN_RUNAHEAD,
};
pub use frame_resend::{
    FrameResendCommand, FrameResendManager, FrameResendRequest, SerializableFrameResendRequest,
};
pub use game_info::{
    game_info_to_ascii_string, parse_ascii_string_to_game_info, FirewallBehaviorType, GameInfo,
    GameInfoSnapshot, GameSlot, GameSlotSnapshot, Money, SkirmishGameInfo, SlotState,
    PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM,
};
pub use gamespy::{GameSpyCommand, GameSpyEvent, GameSpyInterface, GameSpyStatus};
pub use keep_alive::{
    KeepAliveConfig, KeepAliveManager, KeepAliveMetrics, KeepAliveState, IDLE_TIMEOUT_SECS,
    KEEP_ALIVE_INTERVAL_SECS,
};
pub use lan_api::{DiscoveryConfig, GameDiscovery, LanApi, LanConfig, LanEvent, LanResult};
pub use matchmaking::lobby::Lobby;
pub use matchmaking::{GameMode, LobbyFilter, MatchmakingService};
pub use nat::{
    NatBinding, NatConfig, NatService, PortMapping, StunClient, StunConfig, StunNatType,
    UPnPClient, UPnPConfig, UPnPGateway,
};
pub use nat_traversal::{NatBehavior, NatTraversalManager, NatType, PortAllocationPattern};
pub use rank_point_value::{
    calculate_rank, get_favorite_side, get_rank_point_values, RankPoints, MAX_RANKS,
    RANK_BRIGADIER_GENERAL, RANK_CAPTAIN, RANK_COLONEL, RANK_COMMANDER_IN_CHIEF, RANK_CORPORAL,
    RANK_GENERAL, RANK_LIEUTENANT, RANK_MAJOR, RANK_PRIVATE, RANK_SERGEANT,
};
// PacketType removed - C++ has no transport-layer packet type
// All command differentiation happens at application layer via NetCommandType
pub use net_packet::{NetPacket, NetPacketHeader, PacketPayload};
pub use security::firewall::{FirewallConfig, FirewallHelper};
pub use security::SecurityManager;
pub use time::{NetworkClock, NetworkInstant};
pub use transport::{Transport, TransportMessage, TransportMetrics, TransportProtocol};
pub use transport_udp::{
    calculate_crc32, xor_decrypt, xor_encrypt, Transport as UdpTransport,
    TransportConfig as UdpConfig, GENERALS_MAGIC_NUMBER, MAX_PACKET_SIZE,
};
pub use wol_browser::{WolBrowser, WolBrowserCommand, WolBrowserEvent};

/// Network configuration constants
pub mod config {
    /// Maximum player index (0-based, matches C++ MAX_PLAYER)
    /// C++ defines MAX_PLAYER = 7 (0-7 inclusive)
    pub const MAX_PLAYER_INDEX: u8 = 7;

    /// Maximum number of player slots (matches C++ MAX_SLOTS = MAX_PLAYER + 1)
    /// C++ defines MAX_SLOTS = 8 (total slots 0-7)
    pub const MAX_SLOTS: usize = 8;

    /// Maximum number of players supported in a game (alias for MAX_SLOTS)
    pub const MAX_PLAYERS: u8 = 8;

    /// Maximum number of commands per frame
    pub const MAX_COMMANDS_PER_FRAME: usize = 256;

    /// Maximum packet size (accounting for UDP + IP headers)
    pub const MAX_PACKET_SIZE: usize = 476;

    /// Base port number for game networking
    pub const BASE_PORT: u16 = 8088;

    /// Magic number for identifying game packets
    pub const GENERALS_MAGIC: u16 = 0xF00D;

    /// Default timeout for network operations
    pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

    /// Keep-alive interval in milliseconds
    /// MUST match C++ NAT.cpp: 15000ms (15 seconds)
    pub const KEEPALIVE_INTERVAL_MS: u64 = 15000;

    // Frame synchronization constants (matching C++ NetworkDefs.h)

    /// Maximum frames ahead that can be buffered for commands
    /// This determines how far in the future players can send commands
    /// MUST match C++ NetworkUtil.cpp: Int MAX_FRAMES_AHEAD = 128;
    pub const MAX_FRAMES_AHEAD: u32 = 128;

    /// Minimum run-ahead frames to maintain between command submission and execution
    /// Must match C++ NetworkUtil.cpp: Int MIN_RUNAHEAD = 10;
    pub const MIN_RUNAHEAD: u32 = 10;

    /// Frame data buffer length (circular buffer size)
    /// CRITICAL: C++ comment explains: "needs to be MAX_FRAMES_AHEAD+1 because a player can send
    /// commands one beyond twice max runahead"
    /// Must match C++ NetworkUtil.cpp: Int FRAME_DATA_LENGTH = (128+1)*2 = 258
    pub const FRAME_DATA_LENGTH: usize = (MAX_FRAMES_AHEAD as usize + 1) * 2;

    /// Number of frames to keep in history for debugging and rollback
    /// MUST match C++ NetworkUtil.cpp: Int FRAMES_TO_KEEP = (128/2) + 1 = 65;
    pub const FRAMES_TO_KEEP: u32 = 65;

    /// Default target frames per second for game logic
    pub const TARGET_FPS: u32 = 30;

    /// Frame timeout in milliseconds (how long to wait for all player commands)
    pub const FRAME_TIMEOUT_MS: u64 = 5000;

    // Frame metrics tracking

    /// Number of FPS history samples to maintain for averaging
    pub const FPS_HISTORY_LENGTH: usize = 30;

    /// Number of latency history samples to maintain for averaging
    pub const LATENCY_HISTORY_LENGTH: usize = 200;

    /// Number of cushion history samples to track minimum cushion
    pub const CUSHION_HISTORY_LENGTH: usize = 10;

    /// Interval between run-ahead metrics calculations (milliseconds)
    pub const RUNAHEAD_METRICS_INTERVAL_MS: u64 = 5000;

    /// Percentage of slack to add to calculated run-ahead value
    pub const RUNAHEAD_SLACK_PERCENT: u32 = 20;

    /// Disconnect timeout in milliseconds (network stall before disconnect dialog)
    pub const DISCONNECT_TIMEOUT_MS: u64 = 5000;

    /// Player timeout in milliseconds (time without keep-alive before considered disconnected)
    pub const PLAYER_TIMEOUT_MS: u64 = 60000;
}

/// Network interface providing the main entry point for all networking operations
pub struct NetworkInterface {
    transport: Arc<NetTransport>,
    connection_manager: Arc<RwLock<ConnManager>>,
    frame_manager: Arc<RwLock<FrameManager>>,
    local_ip: AtomicU32,
    local_port: AtomicU16,
    local_player_id: u8,
    frame_sync: FrameSyncHandle,
    metrics_cache: Mutex<Option<MetricsSnapshot>>,
    nat: NatSvc,
    config: NetworkConfig,
    nat_monitor: Mutex<Option<JoinHandle<()>>>,
    security_manager: Option<Arc<SecManager>>,
    firewall: Option<Arc<FwHelper>>,
    disconnect_voting: Arc<AsyncMutex<DisconnectVotingCoordinator>>,
    player_names: Arc<AsyncMutex<HashMap<u8, String>>>,
    player_load_progress: Arc<AsyncMutex<HashMap<u8, u8>>>,
    file_transfer_map: Arc<AsyncMutex<HashMap<u16, Vec<(u8, Uuid)>>>>,
    file_announcements: Arc<AsyncMutex<HashMap<u16, connection::FileAnnouncementState>>>,
    file_progress: Arc<AsyncMutex<HashMap<u16, connection::FileProgressState>>>,
    load_progress_watch: watch::Sender<HashMap<u8, u8>>,
    file_announcement_tx: broadcast::Sender<FileAnnouncementEvent>,
    file_progress_tx: broadcast::Sender<connection::FileProgressEvent>,
    chat_tx: broadcast::Sender<connection::ChatEvent>,
    timeout_tx: broadcast::Sender<connection::TimeoutEvent>,
    command_tx: broadcast::Sender<ExecutedFrame>,
    frame_listeners: Arc<ParkingRwLock<HashMap<usize, FrameListener>>>,
    frame_listener_counter: AtomicUsize,
    executed_frames: Arc<Mutex<VecDeque<ExecutedFrame>>>,
    load_progress: Mutex<Option<u8>>,
}

/// Configuration for the network interface
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Local player ID
    pub player_id: u8,
    /// Maximum frames to keep in history
    pub max_frames_ahead: u32,
    /// Minimum run-ahead frames
    pub min_runahead: u32,
    /// Maximum run-ahead frames
    pub max_run_ahead: u32,
    /// Target frame rate for deterministic lockstep
    pub target_frame_rate: u32,
    /// Enable packet compression
    pub enable_compression: bool,
    /// Enable encryption
    pub enable_encryption: bool,
    /// Debug mode for development
    pub debug_mode: bool,
    /// NAT traversal settings
    pub nat: NatCfg,
    /// Firewall/UPnP configuration
    pub firewall: FwConfig,
}

#[derive(Clone, Copy)]
struct MetricsSnapshot {
    metrics: TransportMetrics,
    timestamp: NetworkInstant,
}

type FrameSyncHandle = Arc<Mutex<FrameSyncState>>;
pub type FrameListener = Arc<dyn Fn(&ExecutedFrame) + Send + Sync + 'static>;
pub type FrameListenerId = usize;

/// Description of a remote player discovered through lobby/user list parsing.
#[derive(Debug, Clone)]
pub struct PlayerEndpoint {
    /// Remote player identifier (0-7).
    pub player_id: u8,
    /// Network address for the remote player.
    pub address: SocketAddr,
    /// Optional human friendly display name.
    pub display_name: Option<String>,
    /// Preferred transport protocol.
    pub protocol: TransportProtocol,
}

impl PlayerEndpoint {
    /// Create a UDP endpoint with no display name.
    pub fn new(player_id: u8, address: SocketAddr) -> Self {
        Self {
            player_id,
            address,
            display_name: None,
            protocol: TransportProtocol::Udp,
        }
    }
}

/// Snapshot of the frame synchronization state exposed via [`NetworkStats`].
#[derive(Debug, Clone, Copy)]
pub struct FrameSyncSnapshot {
    pub game_frame: u32,
    pub execution_frame: u32,
    pub run_ahead: u32,
    pub min_run_ahead: u32,
    pub max_run_ahead: u32,
    pub frame_rate: u32,
    pub average_cushion_frames: f32,
    pub frames_ahead: u32,
    pub pending_frames: usize,
    pub saw_crc_mismatch: bool,
    pub pings_sent: i32,
    pub pings_received: i32,
}

/// Payload describing all commands executed for a given frame.
#[derive(Debug, Clone)]
pub struct ExecutedFrame {
    pub frame_number: u32,
    pub commands: Vec<NetCommand>,
}

#[derive(Debug)]
struct RollingAverage {
    window: VecDeque<f32>,
    max_len: usize,
    sum: f32,
}

impl RollingAverage {
    fn new(max_len: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(max_len.max(1)),
            max_len: max_len.max(1),
            sum: 0.0,
        }
    }

    fn reset(&mut self) {
        self.window.clear();
        self.sum = 0.0;
    }

    fn push(&mut self, value: f32) {
        if self.window.len() == self.max_len {
            if let Some(old) = self.window.pop_front() {
                self.sum -= old;
            }
        }
        self.window.push_back(value);
        self.sum += value;
    }

    fn average(&self) -> f32 {
        if self.window.is_empty() {
            0.0
        } else {
            self.sum / self.window.len() as f32
        }
    }
}

#[derive(Debug)]
struct FrameSyncState {
    enabled: bool,
    frame_rate: u32,
    run_ahead: u32,
    min_run_ahead: u32,
    max_run_ahead: u32,
    game_frame: u32,
    execution_frame: u32,
    last_tick: NetworkInstant,
    target_frame_duration: Duration,
    cushion: RollingAverage,
    pings_sent: i32,
    pings_received: i32,
    saw_crc_mismatch: bool,
    min_cushion_since_last: Option<f32>,
}

impl FrameSyncState {
    fn new(frame_rate: u32, min_run_ahead: u32, max_run_ahead: u32) -> Self {
        let frame_rate = frame_rate.max(1);
        let min_run_ahead = min_run_ahead.max(1);
        let max_run_ahead = max_run_ahead.max(min_run_ahead);
        let run_ahead = min_run_ahead;
        let target_frame_duration = Duration::from_secs_f64(1.0 / frame_rate as f64);

        Self {
            enabled: true,
            frame_rate,
            run_ahead,
            min_run_ahead,
            max_run_ahead,
            game_frame: 0,
            execution_frame: 0,
            last_tick: NetworkInstant::now(),
            target_frame_duration,
            cushion: RollingAverage::new(120),
            pings_sent: 0,
            pings_received: 0,
            saw_crc_mismatch: false,
            min_cushion_since_last: None,
        }
    }

    fn set_bounds(&mut self, min_run_ahead: u32, max_run_ahead: u32) {
        let min = min_run_ahead.max(1);
        let max = max_run_ahead.max(min);
        self.min_run_ahead = min;
        self.max_run_ahead = max;
        self.run_ahead = self.run_ahead.clamp(min, max);
    }

    fn advance_frames(&mut self, now: NetworkInstant) -> Vec<u32> {
        if !self.enabled {
            return Vec::new();
        }

        let mut advanced = Vec::new();
        while now.duration_since(self.last_tick) >= self.target_frame_duration {
            self.last_tick += self.target_frame_duration;
            self.game_frame = self.game_frame.wrapping_add(1);
            advanced.push(self.game_frame);
        }
        advanced
    }

    fn record_execution(&mut self, frame: u32, checksum_ok: bool) {
        self.execution_frame = frame;
        if !checksum_ok {
            self.saw_crc_mismatch = true;
        }
    }

    fn record_cushion(&mut self, cushion: f32) -> Option<u32> {
        self.cushion.push(cushion);
        self.min_cushion_since_last = Some(
            self.min_cushion_since_last
                .map(|current| current.min(cushion))
                .unwrap_or(cushion),
        );
        self.adjust_run_ahead()
    }

    fn adjust_run_ahead(&mut self) -> Option<u32> {
        let average = self.cushion.average();
        if average.is_nan() {
            return None;
        }

        let low_threshold = (self.run_ahead as f32 * 0.25).max(0.5);
        let high_threshold = (self.run_ahead as f32 * 0.75).max(1.0);

        if average < low_threshold && self.run_ahead < self.max_run_ahead {
            self.run_ahead += 1;
            Some(self.run_ahead)
        } else if average > high_threshold && self.run_ahead > self.min_run_ahead {
            self.run_ahead -= 1;
            Some(self.run_ahead)
        } else {
            None
        }
    }

    fn snapshot(&self, pending_frames: usize) -> FrameSyncSnapshot {
        let frames_ahead = self.game_frame.saturating_sub(self.execution_frame);
        FrameSyncSnapshot {
            game_frame: self.game_frame,
            execution_frame: self.execution_frame,
            run_ahead: self.run_ahead,
            min_run_ahead: self.min_run_ahead,
            max_run_ahead: self.max_run_ahead,
            frame_rate: self.frame_rate,
            average_cushion_frames: self.cushion.average(),
            frames_ahead,
            pending_frames,
            saw_crc_mismatch: self.saw_crc_mismatch,
            pings_sent: self.pings_sent,
            pings_received: self.pings_received,
        }
    }

    fn take_min_cushion(&mut self) -> f32 {
        self.min_cushion_since_last
            .take()
            .unwrap_or_else(|| self.cushion.average())
    }

    fn reset(&mut self) {
        self.enabled = true;
        self.game_frame = 0;
        self.execution_frame = 0;
        self.last_tick = NetworkInstant::now();
        self.cushion.reset();
        self.pings_sent = 0;
        self.pings_received = 0;
        self.saw_crc_mismatch = false;
        self.min_cushion_since_last = None;
    }
}

struct FrameExecutionTap {
    sync: FrameSyncHandle,
    command_tx: broadcast::Sender<ExecutedFrame>,
    listeners: Arc<ParkingRwLock<HashMap<usize, FrameListener>>>,
    executed_frames: Arc<Mutex<VecDeque<ExecutedFrame>>>,
}

impl FrameExecutionTap {
    fn new(
        sync: FrameSyncHandle,
        command_tx: broadcast::Sender<ExecutedFrame>,
        listeners: Arc<ParkingRwLock<HashMap<usize, FrameListener>>>,
        executed_frames: Arc<Mutex<VecDeque<ExecutedFrame>>>,
    ) -> Self {
        Self {
            sync,
            command_tx,
            listeners,
            executed_frames,
        }
    }
}

#[async_trait]
impl FrameExecutor for FrameExecutionTap {
    async fn execute_frame(&self, frame_data: &SyncFrameData) -> NetResult<()> {
        let commands = frame_data.get_all_commands_ordered();
        let executed = ExecutedFrame {
            frame_number: frame_data.frame_number,
            commands,
        };

        let _ = self.command_tx.send(executed.clone());

        {
            let listeners = self.listeners.read();
            for listener in listeners.values() {
                listener(&executed);
            }
        }

        {
            let mut queue = self.executed_frames.lock();
            if queue.len() >= 512 {
                queue.pop_front();
            }
            queue.push_back(executed);
        }

        let checksum_ok = frame_data.validate_checksum();
        let (run_ahead_change, run_ahead_value, average_cushion, frames_ahead_value, min_cushion) = {
            let mut guard = self.sync.lock();
            let frames_ahead = guard.game_frame.saturating_sub(frame_data.frame_number);
            let cushion = guard.run_ahead as f32 - frames_ahead as f32;
            let run_ahead_change = guard.record_cushion(cushion);
            guard.record_execution(frame_data.frame_number, checksum_ok);
            (
                run_ahead_change,
                guard.run_ahead,
                guard.cushion.average(),
                frames_ahead,
                guard.min_cushion_since_last.unwrap_or(cushion),
            )
        };

        if !checksum_ok {
            warn!(
                "Frame {} failed checksum validation during execution",
                frame_data.frame_number
            );
        }

        let telemetry_handle = telemetry();

        if let Some(ref telemetry) = telemetry_handle {
            telemetry.record_frame_processed(Duration::default());
            if average_cushion.is_finite() {
                telemetry.set_packet_cushion(average_cushion);
            }
            if min_cushion.is_finite() {
                telemetry.set_min_packet_cushion(min_cushion);
            }
            telemetry.set_frames_ahead(frames_ahead_value);
        }

        if let Some(new_run_ahead) = run_ahead_change {
            info!(
                run_ahead = new_run_ahead,
                "Adjusted run-ahead based on cushion analysis"
            );
            if let Some(ref telemetry) = telemetry_handle {
                telemetry.set_run_ahead(new_run_ahead);
            }
        } else if let Some(ref telemetry) = telemetry_handle {
            telemetry.set_run_ahead(run_ahead_value);
        }

        Ok(())
    }

    async fn handle_frame_error(&self, frame_number: u32, error: NetworkError) {
        warn!("Error executing frame {}: {}", frame_number, error);
        let mut guard = self.sync.lock();
        guard.saw_crc_mismatch = true;
        guard.execution_frame = frame_number;
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            player_id: 0,
            max_frames_ahead: 10,
            min_runahead: 2,
            max_run_ahead: 5,
            target_frame_rate: 30,
            enable_compression: true,
            enable_encryption: true,
            debug_mode: false,
            nat: NatCfg::default(),
            firewall: FwConfig::default(),
        }
    }
}

impl NetworkInterface {
    async fn sync_expected_players(&self) -> NetResult<()> {
        let manager = self.connection_manager.read().await;
        let mut players = manager.player_ids().await;
        drop(manager);

        if !players.contains(&self.local_player_id) {
            players.push(self.local_player_id);
        }
        players.sort_unstable();

        {
            let frame_manager = self.frame_manager.read().await;
            frame_manager.set_expected_players(players.clone()).await;
        }

        let active_players: HashSet<u8> = players.iter().copied().collect();
        {
            let coordinator = self.disconnect_voting.lock().await;
            coordinator.update_players(active_players).await;
        }
        Ok(())
    }

    fn spawn_nat_monitor(&self) {
        let mut guard = self.nat_monitor.lock();
        if guard.is_some() {
            return;
        }

        let mut updates = self.nat.subscribe();
        let connections = Arc::clone(&self.connection_manager);
        let firewall = self.firewall.clone();
        let internal_port = self.local_port.load(Ordering::SeqCst);

        let handle = tokio::spawn(async move {
            let mut last_endpoint = None;
            loop {
                let binding = updates.borrow().clone();
                let current_endpoint = binding.as_ref().map(|entry| entry.address);
                if current_endpoint != last_endpoint {
                    NetworkInterface::handle_nat_update(
                        Arc::clone(&connections),
                        firewall.clone(),
                        internal_port,
                        binding.clone(),
                    )
                    .await;
                    last_endpoint = current_endpoint;
                }

                if updates.changed().await.is_err() {
                    break;
                }
            }
        });

        *guard = Some(handle);
    }

    async fn handle_nat_update(
        connections: Arc<RwLock<ConnManager>>,
        firewall: Option<Arc<FwHelper>>,
        internal_port: u16,
        binding: Option<NatBind>,
    ) {
        let peers = ConnectionManager::player_ids_for(&connections).await;

        match (&binding, peers.is_empty()) {
            (Some(entry), false) => {
                info!(
                    address = %entry.address,
                    server = %entry.server,
                    peer_count = peers.len(),
                    peer_ids = ?peers,
                    "Public address updated; sending keep-alives to maintain peer sessions"
                );
                ConnectionManager::broadcast_keepalive_for(&connections).await;
            }
            (Some(entry), true) => {
                info!(
                    address = %entry.address,
                    server = %entry.server,
                    "Public address updated; no active peers to notify"
                );
            }
            (None, false) => {
                warn!(
                    peer_count = peers.len(),
                    peer_ids = ?peers,
                    "Public address lost; sending keep-alives to keep NAT bindings alive"
                );
                ConnectionManager::broadcast_keepalive_for(&connections).await;
            }
            (None, true) => {
                info!("Public address unavailable; no active connections");
            }
        }

        if let Some(helper) = firewall {
            match &binding {
                Some(entry) => {
                    if let Some(IpAddr::V4(local_v4)) = NetworkUtils::get_local_ip() {
                        if let Err(err) = helper
                            .ensure_mapping(
                                crate::transport::TransportProtocol::Udp,
                                local_v4,
                                internal_port,
                            )
                            .await
                        {
                            warn!("Firewall mapping refresh failed: {}", err);
                        }
                        info!(
                            external = %entry.address,
                            "Refreshed firewall mapping for updated NAT binding"
                        );
                    } else {
                        warn!("No local IPv4 address available for firewall mapping refresh");
                    }
                }
                None => helper.remove_mapping().await,
            }
        }
    }

    async fn rate_for<F>(&self, accessor: F) -> f32
    where
        F: Fn(TransportMetrics) -> u64,
    {
        let metrics = self.transport.metrics().await;
        let now = NetworkInstant::now();

        let mut cache = self.metrics_cache.lock();
        let rate = if let Some(previous) = *cache {
            let elapsed = previous.timestamp.elapsed().as_secs_f32();
            if elapsed > 0.0 {
                let delta = accessor(metrics).saturating_sub(accessor(previous.metrics));
                delta as f32 / elapsed
            } else {
                0.0
            }
        } else {
            0.0
        };

        *cache = Some(MetricsSnapshot {
            metrics,
            timestamp: now,
        });
        rate
    }
}

impl NetworkInterface {
    /// Create a new network interface
    pub async fn new(config: NetworkConfig) -> NetResult<Self> {
        Self::with_security(config, None).await
    }

    /// Create a new interface with an optional shared security manager.
    pub async fn with_security(
        config: NetworkConfig,
        security_manager: Option<Arc<SecManager>>,
    ) -> NetResult<Self> {
        info!("Initializing GameNetwork with config: {:?}", config);

        let player_names = Arc::new(AsyncMutex::new(HashMap::new()));
        let player_load_progress = Arc::new(AsyncMutex::new(HashMap::new()));
        let file_transfer_map = Arc::new(AsyncMutex::new(HashMap::new()));
        let file_announcements = Arc::new(AsyncMutex::new(HashMap::new()));
        let file_progress = Arc::new(AsyncMutex::new(HashMap::new()));
        let wrapper_reassembler = Arc::new(AsyncMutex::new(
            crate::commands::wrapper::WrapperReassembler::new(),
        ));
        let (load_progress_watch, _) = watch::channel(HashMap::new());
        let (file_announcement_tx, _) = broadcast::channel(64);
        let (file_progress_tx, _) = broadcast::channel(128);
        let (chat_tx, _) = broadcast::channel(256);
        let (timeout_tx, _) = broadcast::channel(64);
        let (command_tx, _) = broadcast::channel(256);
        let frame_listeners = Arc::new(ParkingRwLock::new(HashMap::new()));
        let frame_listener_counter = AtomicUsize::new(0);
        let executed_frames = Arc::new(Mutex::new(VecDeque::with_capacity(128)));

        let transport = Arc::new(NetTransport::new().await?);
        let connection_manager = Arc::new(RwLock::new(
            ConnManager::new_with_transport(transport.clone()).await?,
        ));
        {
            let mut manager = connection_manager.write().await;
            manager.configure_local_endpoint(
                config.player_id,
                config.enable_compression,
                config.enable_encryption,
            );
            manager.set_command_context(connection::CommandProcessorContext {
                local_player_id: config.player_id,
                connection_manager: connection_manager.clone(),
                wrapper_reassembler: wrapper_reassembler.clone(),
                player_names: player_names.clone(),
                player_load_progress: player_load_progress.clone(),
                file_announcements: file_announcements.clone(),
                file_transfer_map: file_transfer_map.clone(),
                load_progress_tx: load_progress_watch.clone(),
                file_announcement_tx: file_announcement_tx.clone(),
                file_progress: file_progress.clone(),
                file_progress_tx: file_progress_tx.clone(),
                chat_tx: chat_tx.clone(),
                timeout_tx: timeout_tx.clone(),
            });
            if let Some(security) = security_manager.clone() {
                manager.set_security_manager(security);
            }
        }
        // File transfer integration placeholder
        // File transfer coordination happens through the connection manager

        let min_run_ahead = config.min_runahead.max(1);
        let max_run_ahead = config
            .max_run_ahead
            .max(min_run_ahead)
            .min(config.max_frames_ahead.max(min_run_ahead));

        let frame_sync = Arc::new(Mutex::new(FrameSyncState::new(
            config.target_frame_rate,
            min_run_ahead,
            max_run_ahead,
        )));

        let frame_manager = Arc::new(RwLock::new(FrameManager::new(
            config.max_frames_ahead,
            config.min_runahead,
        )));
        {
            let mut guard = frame_manager.write().await;
            guard.set_target_fps(config.target_frame_rate);
            guard.set_frame_executor(Arc::new(FrameExecutionTap::new(
                Arc::clone(&frame_sync),
                command_tx.clone(),
                frame_listeners.clone(),
                executed_frames.clone(),
            )));
        }
        let nat = NatSvc::new(config.nat.clone());
        nat.start_auto_refresh(transport.clone()).await;
        let firewall_helper = if config.firewall.enabled {
            Some(Arc::new(FwHelper::new(config.firewall.clone())))
        } else {
            None
        };

        let mut disconnect_voting = DisconnectVotingCoordinator::new();
        disconnect_voting.start().await?;
        let disconnect_voting = Arc::new(AsyncMutex::new(disconnect_voting));

        let interface = Self {
            transport,
            connection_manager,
            frame_manager,
            local_ip: AtomicU32::new(u32::from(Ipv4Addr::new(127, 0, 0, 1))),
            local_port: AtomicU16::new(config::BASE_PORT),
            local_player_id: config.player_id,
            frame_sync,
            metrics_cache: Mutex::new(None),
            nat,
            config,
            nat_monitor: Mutex::new(None),
            security_manager,
            firewall: firewall_helper,
            disconnect_voting,
            player_names: player_names.clone(),
            player_load_progress: player_load_progress.clone(),
            file_transfer_map: file_transfer_map.clone(),
            file_announcements: file_announcements.clone(),
            file_progress: file_progress.clone(),
            load_progress_watch: load_progress_watch.clone(),
            file_announcement_tx: file_announcement_tx.clone(),
            file_progress_tx: file_progress_tx.clone(),
            chat_tx: chat_tx.clone(),
            timeout_tx: timeout_tx.clone(),
            command_tx: command_tx.clone(),
            frame_listeners: frame_listeners.clone(),
            frame_listener_counter,
            executed_frames: executed_frames.clone(),
            load_progress: Mutex::new(Some(0)),
        };

        {
            let mut names = player_names.lock().await;
            names.insert(
                interface.local_player_id,
                format!("Player{}", interface.local_player_id),
            );
        }

        {
            let mut progress = player_load_progress.lock().await;
            progress.insert(interface.local_player_id, 0);
        }

        interface.spawn_nat_monitor();
        interface.sync_expected_players().await?;
        Ok(interface)
    }

    /// Subscribe to NAT binding updates with metadata.
    pub fn nat_updates(&self) -> tokio::sync::watch::Receiver<Option<NatBind>> {
        self.nat.subscribe()
    }

    /// Obtain the current NAT binding snapshot if one is known.
    pub async fn nat_binding(&self) -> Option<NatBind> {
        self.nat.current_binding().await
    }

    /// Access the shared security manager, if one is attached.
    pub fn security_manager(&self) -> Option<Arc<SecManager>> {
        self.security_manager.clone()
    }

    /// Subscribe to new file transfer announcements.
    pub fn subscribe_file_announcements(
        &self,
    ) -> broadcast::Receiver<connection::FileAnnouncementEvent> {
        self.file_announcement_tx.subscribe()
    }

    /// Subscribe to per-command file progress updates.
    pub fn subscribe_file_progress(&self) -> broadcast::Receiver<connection::FileProgressEvent> {
        self.file_progress_tx.subscribe()
    }

    /// Subscribe to aggregated load progress snapshots (per player percentage).
    pub fn subscribe_load_progress(&self) -> watch::Receiver<HashMap<u8, u8>> {
        self.load_progress_watch.subscribe()
    }

    /// Subscribe to chat messages emitted by the networking layer.
    pub fn subscribe_chat_events(&self) -> broadcast::Receiver<connection::ChatEvent> {
        self.chat_tx.subscribe()
    }

    /// Subscribe to timeout start events broadcast during loading watchdog.
    pub fn subscribe_timeout_events(&self) -> broadcast::Receiver<connection::TimeoutEvent> {
        self.timeout_tx.subscribe()
    }

    /// Subscribe to executed frame notifications for gameplay integration.
    pub fn subscribe_executed_frames(&self) -> broadcast::Receiver<ExecutedFrame> {
        self.command_tx.subscribe()
    }

    /// Register an in-process listener that is invoked whenever a frame executes.
    pub fn register_frame_listener(&self, listener: FrameListener) -> FrameListenerId {
        let id = self.frame_listener_counter.fetch_add(1, Ordering::SeqCst);
        self.frame_listeners.write().insert(id, listener);
        id
    }

    /// Remove a previously registered frame listener.
    pub fn unregister_frame_listener(&self, id: FrameListenerId) -> bool {
        self.frame_listeners.write().remove(&id).is_some()
    }

    /// Drain any recorded executed frames accumulated since the last call.
    pub fn drain_executed_frames(&self) -> Vec<ExecutedFrame> {
        let mut queue = self.executed_frames.lock();
        queue.drain(..).collect()
    }

    /// Initialize networking for a local game
    pub async fn init_local(&mut self, port: u16) -> NetResult<()> {
        info!("Initializing local networking on port {}", port);

        if !self.transport.is_ready() {
            let local_ip = Ipv4Addr::from(self.local_ip.load(Ordering::SeqCst));
            let bind_address = SocketAddr::new(IpAddr::V4(local_ip), port);
            self.transport.set_bind_address(bind_address)?;
            self.transport.bind().await?;
        }

        self.local_port.store(port, Ordering::SeqCst);
        self.nat.refresh(&self.transport).await?;
        if let Some(helper) = &self.firewall {
            match NetworkUtils::get_local_ip() {
                Some(IpAddr::V4(ipv4)) => {
                    if let Err(err) = helper
                        .ensure_mapping(crate::transport::TransportProtocol::Udp, ipv4, port)
                        .await
                    {
                        warn!("Firewall mapping failed: {}", err);
                    }
                }
                Some(IpAddr::V6(_)) => {
                    warn!("Local interface reports IPv6 only; skipping firewall mapping");
                }
                None => warn!("Unable to determine local IPv4 address for firewall mapping"),
            }
        }
        self.sync_expected_players().await?;
        Ok(())
    }

    /// Initialize networking for multiplayer
    pub async fn init_multiplayer(&self, host: &str, port: u16) -> NetResult<()> {
        info!("Initializing multiplayer networking to {}:{}", host, port);
        let addr = format!("{}:{}", host, port).parse().map_err(|err| {
            NetError::transport(format!("Invalid address {}:{} ({})", host, port, err))
        })?;
        self.transport.connect(addr).await?;
        self.nat.refresh(&self.transport).await?;
        self.sync_expected_players().await?;
        Ok(())
    }

    /// Configure the local bind address prior to initialization.
    pub async fn set_local_address(&self, ip: Ipv4Addr, port: u16) -> NetResult<()> {
        if self.transport.is_ready() {
            return Err(NetError::transport(
                "Cannot change local address after transport is bound",
            ));
        }

        let bind_address = SocketAddr::new(IpAddr::V4(ip), port);
        self.transport.set_bind_address(bind_address)?;
        self.local_ip.store(u32::from(ip), Ordering::SeqCst);
        self.local_port.store(port, Ordering::SeqCst);
        Ok(())
    }

    /// Current run-ahead value used by the deterministic lockstep.
    pub fn run_ahead(&self) -> u32 {
        self.frame_sync.lock().run_ahead
    }

    /// Target frame rate for deterministic networking.
    pub fn frame_rate(&self) -> u32 {
        self.frame_sync.lock().frame_rate
    }

    /// Smallest packet arrival cushion observed since the previous query.
    pub fn packet_arrival_cushion(&self) -> f32 {
        self.frame_sync.lock().take_min_cushion()
    }

    /// Returns true when the next frame is ready for execution.
    pub async fn is_frame_data_ready(&self) -> bool {
        let manager = self.frame_manager.read().await;
        manager.has_ready_frame().await
    }

    /// Reset networking state for a new session.
    pub async fn reset_session(&self) -> NetResult<()> {
        {
            let mut manager = self.connection_manager.write().await;
            manager.reset().await?;
        }

        self.start_game().await?;
        Ok(())
    }

    /// Reset frame synchronization state and prepare for a new game session.
    pub async fn start_game(&self) -> NetResult<()> {
        {
            let mut manager = self.frame_manager.write().await;
            manager.reset().await?;
        }

        {
            let mut sync = self.frame_sync.lock();
            sync.reset();
        }

        *self.metrics_cache.lock() = None;
        *self.load_progress.lock() = Some(0);

        {
            let mut progress = self.player_load_progress.lock().await;
            progress.clear();
            progress.insert(self.local_player_id, 0);
        }

        self.file_transfer_map.lock().await.clear();
        self.file_announcements.lock().await.clear();
        self.file_progress.lock().await.clear();

        if let Some(telemetry) = telemetry() {
            telemetry.set_load_progress(0);
            telemetry.set_run_ahead(self.run_ahead());
        }

        self.sync_expected_players().await?;
        Ok(())
    }

    /// Current average FPS observed by the frame manager.
    pub async fn average_fps(&self) -> i32 {
        let manager = self.frame_manager.read().await;
        manager.current_fps().await.round() as i32
    }

    /// Average FPS for a specific slot (currently global).
    pub async fn slot_average_fps(&self, _slot: u8) -> i32 {
        self.average_fps().await
    }

    /// Update the known players with lobby/user-list information and connect to new peers.
    pub async fn parse_user_list(&self, players: &[PlayerEndpoint]) -> NetResult<()> {
        let mut discovered: HashSet<u8> = HashSet::new();
        {
            let mut names = self.player_names.lock().await;
            for endpoint in players.iter() {
                discovered.insert(endpoint.player_id);
                if let Some(name) = &endpoint.display_name {
                    names.insert(endpoint.player_id, name.clone());
                } else {
                    names
                        .entry(endpoint.player_id)
                        .or_insert_with(|| format!("Player{}", endpoint.player_id));
                }
            }
            discovered.insert(self.local_player_id);
            names.retain(|player, _| discovered.contains(player));
        }

        {
            let mut progress = self.player_load_progress.lock().await;
            for player_id in discovered.iter().copied() {
                if player_id == self.local_player_id {
                    continue;
                }
                progress.entry(player_id).or_insert(0);
            }
            progress.retain(|player, _| discovered.contains(player));
        }

        let mut existing: HashSet<u8> = {
            let manager = self.connection_manager.read().await;
            manager.player_ids().await.into_iter().collect()
        };

        for endpoint in players.iter() {
            if endpoint.player_id == self.local_player_id {
                continue;
            }

            if existing.contains(&endpoint.player_id) {
                continue;
            }

            if endpoint.protocol != TransportProtocol::Quic
                && endpoint.protocol != TransportProtocol::Udp
            {
                warn!(
                    "Skipping player {} with unsupported protocol {:?}",
                    endpoint.player_id, endpoint.protocol
                );
                continue;
            }

            if let Err(err) = self
                .connect_player(endpoint.player_id, endpoint.address)
                .await
            {
                match &err {
                    NetError::Connection { message } if message.contains("already connected") => {
                        debug!(
                            "Player {} already connected when parsing user list",
                            endpoint.player_id
                        );
                    }
                    _ => return Err(err),
                }
            } else {
                existing.insert(endpoint.player_id);
            }
        }

        self.sync_expected_players().await
    }

    /// Local player identifier.
    pub fn local_player_id(&self) -> u8 {
        self.local_player_id
    }

    /// Resolve a stored display name for the given player.
    pub async fn player_name(&self, player_id: u8) -> String {
        let names = self.player_names.lock().await;
        names
            .get(&player_id)
            .cloned()
            .unwrap_or_else(|| format!("Player{}", player_id))
    }

    /// Enumerate the known player identifiers including the local participant.
    pub async fn player_ids(&self) -> Vec<u8> {
        let manager = self.connection_manager.read().await;
        let mut ids = manager.player_ids().await;
        drop(manager);

        if !ids.contains(&self.local_player_id) {
            ids.push(self.local_player_id);
        }
        ids.sort_unstable();
        ids
    }

    /// Retrieve the last reported loading progress for a player, if known.
    pub async fn player_load_progress(&self, player_id: u8) -> Option<u8> {
        let progress = self.player_load_progress.lock().await;
        progress.get(&player_id).copied()
    }

    /// Retrieve transfer identifiers associated with a command id.
    pub async fn file_transfers_for_command(&self, command_id: u16) -> Option<Vec<(u8, Uuid)>> {
        let mapping = self.file_transfer_map.lock().await;
        mapping.get(&command_id).cloned()
    }

    /// Retrieve announced metadata for a given command id.
    pub async fn file_metadata_for_command(&self, command_id: u16) -> Option<FileMetadata> {
        let announcements = self.file_announcements.lock().await;
        announcements
            .get(&command_id)
            .map(|state| state.metadata.clone())
    }

    /// Snapshot the current file progress (percentage per player) keyed by command id.
    pub async fn file_progress_snapshot(&self) -> HashMap<u16, HashMap<u8, u8>> {
        let progress = self.file_progress.lock().await;
        progress
            .iter()
            .map(|(command_id, state)| (*command_id, state.progress.clone()))
            .collect()
    }

    /// Assign a display name to a player slot.
    pub async fn set_player_name(&self, player_id: u8, name: impl Into<String>) {
        let mut names = self.player_names.lock().await;
        names.insert(player_id, name.into());
    }

    /// Total number of players in the current session including the local player.
    pub async fn num_players(&self) -> usize {
        self.player_ids().await.len()
    }

    /// Record loading progress for UI/telemetry consumers.
    pub async fn update_load_progress(&self, percent: u8) {
        let clamped = percent.min(100);
        let should_broadcast = {
            let mut guard = self.load_progress.lock();
            let changed = guard.map_or(true, |prev| prev != clamped);
            *guard = Some(clamped);
            changed
        };

        {
            let mut progress = self.player_load_progress.lock().await;
            progress.insert(self.local_player_id, clamped);
        }

        if let Some(telemetry) = telemetry() {
            telemetry.set_load_progress(clamped);
        }

        if should_broadcast {
            let command =
                NetCommand::progress(self.local_player_id, ProgressType::Loading, clamped);
            if let Err(err) = self.send_command(command).await {
                warn!("Failed to broadcast load progress {}: {}", clamped, err);
            }
        }
    }

    /// Indicate that loading is complete.
    pub async fn load_progress_complete(&self) {
        *self.load_progress.lock() = Some(100);
        {
            let mut progress = self.player_load_progress.lock().await;
            progress.insert(self.local_player_id, 100);
        }
        if let Some(telemetry) = telemetry() {
            telemetry.set_load_progress(100);
            telemetry.mark_load_complete();
        }

        let command = NetCommand::load_complete(self.local_player_id);
        if let Err(err) = self.send_command(command).await {
            warn!("Failed to broadcast load completion: {}", err);
        }
    }

    /// Broadcast a timeout-start command to all peers.
    pub async fn send_timeout_game_start(&self) -> NetResult<()> {
        let command = NetCommand::timeout_start(self.local_player_id);
        self.send_command(command).await
    }

    /// Announce a pending file transfer and return a short-lived identifier.
    pub async fn send_file_announce<P: AsRef<Path>>(
        &self,
        path: P,
        player_mask: u8,
    ) -> NetResult<u16> {
        let path_ref = path.as_ref();
        let metadata = match fs::metadata(path_ref).await {
            Ok(meta) if meta.len() > 0 => meta,
            _ => {
                warn!(
                    "Not sending file announce for {:?} to mask {:X}",
                    path_ref, player_mask
                );
                return Ok(0);
            }
        };

        let command_id = crate::net_command_messages::generate_next_command_id();
        let portable_name = {
            let game_state = get_game_state();
            game_state.real_map_path_to_portable_map_path(&path_ref.to_string_lossy())
        };

        let file_metadata = FileMetadata {
            filename: portable_name.clone(),
            file_size: metadata.len(),
            checksum: [0u8; 32],
            transfer_type: TransferType::Generic,
        };

        {
            let mut announcements = self.file_announcements.lock().await;
            announcements.insert(
                command_id,
                FileAnnouncementState {
                    metadata: file_metadata.clone(),
                    player_mask,
                },
            );
        }

        {
            let mut progress = self.file_progress.lock().await;
            let entry = progress.entry(command_id).or_default();
            entry.progress.clear();
            entry.failures.clear();

            for slot in 0..config::MAX_PLAYERS as usize {
                let player = slot as u8;
                if (player_mask & (1u8 << player)) != 0 {
                    entry.progress.insert(player, 0);
                } else {
                    entry.progress.insert(player, 100);
                }
            }
        }

        let command =
            NetCommand::file_announce(self.local_player_id, command_id, player_mask, file_metadata);
        let manager = self.connection_manager.read().await;
        let announce_mask = 0xffu32 ^ (1u32 << self.local_player_id);
        manager.send_command_to_mask(command, announce_mask).await?;

        Ok(command_id)
    }

    /// Send a file payload to the specified player mask.
    pub async fn send_file<P: AsRef<Path>>(
        &self,
        path: P,
        player_mask: u8,
        command_id: u16,
    ) -> NetResult<()> {
        let path_ref = path.as_ref();
        let metadata = match fs::metadata(path_ref).await {
            Ok(meta) if meta.len() > 0 => meta,
            _ => {
                warn!("Not sending file {:?} to mask {:X}", path_ref, player_mask);
                return Ok(());
            }
        };
        let portable_name = {
            let game_state = get_game_state();
            game_state.real_map_path_to_portable_map_path(&path_ref.to_string_lossy())
        };

        let mut data = fs::read(path_ref).await.map_err(|err| {
            NetError::file_transfer(format!("failed to read file {:?}: {}", path_ref, err))
        })?;
        if data.len() as u64 != metadata.len() {
            warn!(
                "File size changed while sending {:?} (expected {}, read {})",
                path_ref,
                metadata.len(),
                data.len()
            );
        }

        if let Some(ext) = path_ref.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("tga") {
                let compression = get_preferred_compression();
                match compress_data(&data, compression, CompressionLevel::Default) {
                    Ok(compressed) => {
                        if !compressed.is_empty() {
                            debug!(
                                "Compressed '{}' from {} to {} bytes for transfer",
                                path_ref.display(),
                                data.len(),
                                compressed.len()
                            );
                            data = compressed;
                        }
                    }
                    Err(err) => {
                        warn!(
                            "Failed to compress '{}' for transfer: {}",
                            path_ref.display(),
                            err
                        );
                    }
                }
            }
        }

        let command =
            NetCommand::file_transfer(self.local_player_id, portable_name, data, command_id);
        let manager = self.connection_manager.read().await;
        manager
            .send_command_to_mask(command, player_mask as u32)
            .await
    }

    /// Query the progress of a file transfer for the specified player.
    pub async fn get_file_transfer_progress<P: AsRef<Path>>(&self, player_id: u8, path: P) -> i32 {
        self.file_transfer_progress_for(player_id, path).await
    }

    /// Approximate incoming packets per second.
    pub async fn incoming_packets_per_second(&self) -> f32 {
        self.rate_for(|metrics| metrics.packets_received).await
    }

    /// Approximate outgoing packets per second.
    pub async fn outgoing_packets_per_second(&self) -> f32 {
        self.rate_for(|metrics| metrics.packets_sent).await
    }

    /// Placeholder for legacy unknown-byte accounting.
    pub fn unknown_bytes_per_second(&self) -> f32 {
        0.0_f32
    }

    /// Placeholder for legacy unknown-packet accounting.
    pub fn unknown_packets_per_second(&self) -> f32 {
        0.0_f32
    }

    /// Returns true if this peer is acting as a packet router.
    pub fn is_packet_router(&self) -> bool {
        false
    }

    /// Whether a CRC mismatch was observed during synchronization.
    pub fn saw_crc_mismatch(&self) -> bool {
        self.frame_sync.lock().saw_crc_mismatch
    }

    /// Flag the current session as having observed a CRC mismatch.
    pub fn set_saw_crc_mismatch(&self) {
        self.frame_sync.lock().saw_crc_mismatch = true;
    }

    /// Highest frame that has been executed locally.
    pub fn execution_frame(&self) -> u32 {
        self.frame_sync.lock().execution_frame
    }

    /// Current network frame used for ping accountability.
    pub fn ping_frame(&self) -> u32 {
        self.frame_sync.lock().game_frame
    }

    /// Count of pings sent since last snapshot.
    pub fn pings_sent(&self) -> i32 {
        self.frame_sync.lock().pings_sent
    }

    /// Count of pings received since last snapshot.
    pub fn pings_received(&self) -> i32 {
        self.frame_sync.lock().pings_received
    }

    /// Check if a specific player is connected.
    pub async fn is_player_connected(&self, player_id: u8) -> bool {
        let manager = self.connection_manager.read().await;
        if let Some(connection) = manager.get_connection(player_id).await {
            connection.is_active().await
        } else {
            false
        }
    }

    /// Notify peers of the current frame we are on.
    pub async fn notify_others_of_current_frame(&self) -> NetResult<()> {
        let manager = self.frame_manager.read().await;
        let frame = manager.get_current_frame().await;
        drop(manager);
        self.notify_others_of_new_frame(frame).await
    }

    /// Notify peers that we are on a new frame.
    pub async fn notify_others_of_new_frame(&self, frame: u32) -> NetResult<()> {
        let (command_count, checksum) = {
            let manager = self.frame_manager.read().await;
            manager.frame_info(frame).await.unwrap_or((0, 0))
        };

        let command = NetCommand::new(
            NetCommandType::FrameInfo,
            self.local_player_id,
            0,
            CommandPayload::FrameInfo(FrameInfoData {
                frame,
                command_count,
                checksum,
            }),
        );
        self.send_command(command).await
    }

    /// Broadcast an in-game chat message to connected peers.
    pub async fn send_chat_message(
        &self,
        message: impl Into<String>,
        player_mask: u8,
    ) -> NetResult<()> {
        let text = message.into();
        if text.trim().is_empty() {
            return Err(NetError::invalid_command("chat message cannot be empty"));
        }

        let mask = if player_mask == 0 {
            -1i32 // Broadcast to all players
        } else {
            player_mask as i32
        };
        let command = NetCommand::chat(self.local_player_id, text, mask);
        self.send_command(command).await
    }

    /// Broadcast a disconnect-screen chat message.
    pub async fn send_disconnect_chat_message(
        &self,
        message: impl Into<String>,
        player_mask: u8,
    ) -> NetResult<()> {
        let text = message.into();
        if text.trim().is_empty() {
            return Err(NetError::invalid_command(
                "disconnect chat message cannot be empty",
            ));
        }

        let mask = if player_mask == 0 {
            -1i32 // Broadcast to all players
        } else {
            player_mask as i32
        };
        let command = NetCommand::disconnect_chat(self.local_player_id, text, mask);
        self.send_command(command).await
    }

    /// Return the current progress (0-100) for a peer receiving the specified file.
    /// A return value of `-1` indicates that the transfer failed.
    pub async fn file_transfer_progress_for<P: AsRef<Path>>(&self, player_id: u8, path: P) -> i32 {
        let file_name = path
            .as_ref()
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase());

        let announcements_snapshot = {
            let announcements = self.file_announcements.lock().await;
            announcements.clone()
        };

        {
            let progress = self.file_progress.lock().await;
            for (command_id, state) in progress.iter() {
                if let Some(announcement) = announcements_snapshot.get(command_id) {
                    let metadata_name = announcement.metadata.filename.to_ascii_lowercase();
                    if let Some(target_name) = &file_name {
                        if metadata_name != *target_name {
                            continue;
                        }
                    } else if metadata_name != path.as_ref().to_string_lossy().to_ascii_lowercase()
                    {
                        continue;
                    }

                    if let Some(reason) = state.failures.get(&player_id) {
                        warn!(
                            "File transfer command {} failed for player {}: {}",
                            command_id, player_id, reason
                        );
                        return -1;
                    }

                    if let Some(progress_value) = state.progress.get(&player_id) {
                        return *progress_value as i32;
                    }
                }
            }
        }

        let transfers = ConnManager::active_transfers_for(&self.connection_manager).await;
        for record in transfers {
            let metadata_name = record.progress.metadata.filename.to_ascii_lowercase();

            if let Some(target_name) = &file_name {
                if metadata_name != *target_name {
                    continue;
                }
            } else if metadata_name != path.as_ref().to_string_lossy().to_ascii_lowercase() {
                continue;
            }

            if !record.participants.is_empty() && !record.participants.contains(&player_id) {
                continue;
            }

            if let Some(reason) = &record.failure_reason {
                warn!(
                    "File transfer {} failed for player {}: {}",
                    record.progress.transfer_id, player_id, reason
                );
                return -1;
            }

            if let Some(status) = record.completion_status.get(&player_id) {
                if *status {
                    return 100;
                }
            }

            if record.progress.complete {
                return 100;
            }

            let total = record.progress.metadata.file_size.max(1);
            let pct = ((record.progress.bytes_transferred as f64 / total as f64) * 100.0)
                .round()
                .clamp(0.0, 100.0) as i32;
            return pct;
        }

        0
    }

    /// Determine whether all send/receive queues across connections are empty.
    pub async fn are_all_queues_empty(&self) -> bool {
        let manager = self.connection_manager.read().await;
        manager.queues_empty().await
    }

    /// Initiate a disconnect vote against the specified player slot.
    pub async fn initiate_disconnect_vote(
        &self,
        target_player: u8,
        reason: DisconnectReason,
        evidence: Vec<VoteEvidence>,
    ) -> NetResult<Uuid> {
        let coordinator = self.disconnect_voting.clone();
        let guard = coordinator.lock().await;
        guard
            .initiate_vote(target_player, reason, self.local_player_id, evidence)
            .await
    }

    /// Convenience wrapper that mirrors the legacy "vote for player disconnect" behaviour.
    pub async fn vote_for_player_disconnect(&self, target_player: u8) -> NetResult<Uuid> {
        self.initiate_disconnect_vote(
            target_player,
            DisconnectReason::PlayerRequest {
                reason: "Manual vote".to_string(),
            },
            Vec::new(),
        )
        .await
    }

    /// Cast a decision on an active disconnect vote.
    pub async fn cast_disconnect_vote(
        &self,
        vote_id: Uuid,
        decision: VoteDecision,
        comment: Option<String>,
    ) -> NetResult<()> {
        let coordinator = self.disconnect_voting.clone();
        let guard = coordinator.lock().await;
        guard
            .cast_vote(vote_id, self.local_player_id, decision, comment)
            .await
    }

    /// Get the currently active disconnect votes.
    pub async fn active_disconnect_votes(
        &self,
    ) -> Vec<connection::disconnect_voting::DisconnectVote> {
        let coordinator = self.disconnect_voting.clone();
        let guard = coordinator.lock().await;
        guard.get_active_votes().await
    }

    /// Subscribe to disconnect vote events.
    pub async fn subscribe_disconnect_vote_events(
        &self,
    ) -> tokio::sync::broadcast::Receiver<VoteEvent> {
        let coordinator = self.disconnect_voting.clone();
        let receiver = {
            let guard = coordinator.lock().await;
            guard.subscribe_events()
        };
        receiver
    }

    /// Gracefully leave the networked game.
    pub async fn quit_game(&self) -> NetResult<()> {
        self.shutdown().await
    }

    /// Disconnect the specified remote player.
    pub async fn self_destruct_player(&self, player_id: u8) -> NetResult<()> {
        self.disconnect_player(player_id).await
    }

    /// Process incoming network packets with concurrent operations
    pub async fn update(&self) -> NetResult<()> {
        // Use tokio::select! to handle multiple operations concurrently
        tokio::select! {
            // Process transport layer - highest priority for packet processing
            result = self.transport.update() => {
                if let Err(e) = result {
                    error!("Transport update error: {}", e);
                    return Err(e);
                }
            }

            // Process connections concurrently
            result = async {
                let connection_manager = self.connection_manager.write().await;
                connection_manager.update().await
            } => {
                if let Err(e) = result {
                    error!("Connection manager update error: {}", e);
                    return Err(e);
                }
            }

            // Process frame data concurrently
            result = async {
                let frame_manager = self.frame_manager.write().await;
                frame_manager.update().await
            } => {
                if let Err(e) = result {
                    error!("Frame manager update error: {}", e);
                    return Err(e);
                }
            }

            // Add timeout to prevent blocking indefinitely
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                // Timeout occurred - continue with other operations
                trace!("Network update timeout reached");
            }
        }

        self.advance_game_clock().await
    }

    async fn advance_game_clock(&self) -> NetResult<()> {
        let (advanced_frames, run_ahead) = {
            let mut sync = self.frame_sync.lock();
            let frames = sync.advance_frames(NetworkInstant::now());
            (frames, sync.run_ahead)
        };

        if advanced_frames.is_empty() {
            return Ok(());
        }

        let earliest = advanced_frames[0];
        let latest = *advanced_frames.last().unwrap();
        let future_limit = latest.saturating_add(run_ahead);

        let manager = self.frame_manager.read().await;
        manager.ensure_future_window(earliest, future_limit).await;

        Ok(())
    }

    /// Perform comprehensive network update with all concurrent operations
    pub async fn update_concurrent(&self) -> NetResult<()> {
        // Spawn concurrent tasks for all major network operations
        let transport_future = self.transport.update();
        let connection_future = async {
            let connection_manager = self.connection_manager.write().await;
            connection_manager.update().await
        };
        let frame_future = async {
            let frame_manager = self.frame_manager.write().await;
            frame_manager.update().await
        };

        // Await all network components concurrently
        let (_trans_res, _conn_res, _frame_res) =
            tokio::try_join!(transport_future, connection_future, frame_future)?;

        debug!("All network updates completed successfully");

        Ok(())
    }

    /// Send a command to all players
    pub async fn send_command(&self, command: NetCommand) -> NetResult<()> {
        let connection_manager = self.connection_manager.read().await;
        let result = connection_manager.broadcast_command(command).await;
        drop(connection_manager);

        if result.is_ok() {
            if let Some(telemetry) = telemetry() {
                telemetry.record_command_processed();
            }
        }

        result
    }

    /// Approximate incoming byte rate.
    pub async fn incoming_bytes_per_second(&self) -> f32 {
        self.rate_for(|metrics| metrics.bytes_received).await
    }

    /// Approximate outgoing byte rate.
    pub async fn outgoing_bytes_per_second(&self) -> f32 {
        self.rate_for(|metrics| metrics.bytes_sent).await
    }

    /// Connect to a remote player and integrate them into synchronization.
    pub async fn connect_player(&self, player_id: u8, addr: SocketAddr) -> NetResult<()> {
        let manager = self.connection_manager.read().await;
        manager
            .add_connection(player_id, addr, TransportProtocol::Quic)
            .await?;
        drop(manager);

        self.sync_expected_players().await?;
        Ok(())
    }

    /// Disconnect a remote player.
    pub async fn disconnect_player(&self, player_id: u8) -> NetResult<()> {
        if player_id == self.config.player_id {
            return Err(NetError::connection("cannot disconnect local player"));
        }

        let manager = self.connection_manager.read().await;
        manager.remove_connection(player_id).await?;
        drop(manager);

        self.sync_expected_players().await?;
        Ok(())
    }

    /// Reconfigure the run-ahead bounds used by the deterministic lockstep loop.
    pub fn configure_run_ahead(&mut self, min: u32, max: u32) {
        let min = min.max(1);
        let max = max.max(min).min(self.config.max_frames_ahead.max(min));

        {
            let mut sync = self.frame_sync.lock();
            sync.set_bounds(min, max);
        }

        self.config.min_runahead = min;
        self.config.max_run_ahead = max;
    }

    /// Shutdown the network interface
    pub async fn shutdown(&self) -> NetResult<()> {
        info!("Shutting down GameNetwork");

        // Gracefully disconnect all connections
        {
            let mut connection_manager = self.connection_manager.write().await;
            connection_manager.shutdown_all().await?;
        }

        // Shutdown transport
        self.transport.shutdown().await?;

        {
            let mut names = self.player_names.lock().await;
            names.retain(|player, _| *player == self.local_player_id);
        }
        {
            let mut progress = self.player_load_progress.lock().await;
            progress.retain(|player, _| *player == self.local_player_id);
        }
        self.file_transfer_map.lock().await.clear();
        self.file_announcements.lock().await.clear();
        self.file_progress.lock().await.clear();

        {
            let mut coordinator = self.disconnect_voting.lock().await;
            coordinator.shutdown().await?;
        }

        if let Some(handle) = self.nat_monitor.lock().take() {
            handle.abort();
        }

        if let Some(helper) = &self.firewall {
            helper.remove_mapping().await;
        }

        Ok(())
    }

    /// Get network statistics
    pub async fn get_stats(&self) -> NetworkStats {
        let connection_manager = self.connection_manager.read().await;
        let connected_players = connection_manager.active_connections().await;
        drop(connection_manager);

        if let Some(telemetry) = telemetry() {
            telemetry.set_active_connections(connected_players);
        }

        let frame_stats = {
            let manager = self.frame_manager.read().await;
            manager.get_stats().await
        };

        let frame_sync_snapshot = {
            let sync = self.frame_sync.lock();
            sync.snapshot(frame_stats.pending_frames)
        };

        let metrics = self.transport.metrics().await;
        NetworkStats {
            connected_players,
            packets_sent: metrics.packets_sent,
            packets_received: metrics.packets_received,
            bytes_sent: metrics.bytes_sent,
            bytes_received: metrics.bytes_received,
            public_address: self.transport.public_address(),
            frame_sync: frame_sync_snapshot,
        }
    }

    /// Public address discovered via NAT traversal (if any).
    pub fn public_address(&self) -> Option<SocketAddr> {
        self.transport.public_address()
    }

    /// Force a refresh of the public address via NAT traversal.
    pub async fn refresh_public_address(&self) -> NetResult<Option<SocketAddr>> {
        self.nat.refresh(&self.transport).await
    }
}

// File transfer progress bridge removed - coordination handled through connection manager

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    /// Number of currently connected players
    pub connected_players: usize,
    /// Total packets sent
    pub packets_sent: u64,
    /// Total packets received  
    pub packets_received: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Advertised public address if known
    pub public_address: Option<SocketAddr>,
    /// Snapshot of frame synchronization state
    pub frame_sync: FrameSyncSnapshot,
}

/// Global network interface singleton using tokio::sync::OnceCell
static NETWORK_INTERFACE: OnceCell<Arc<NetworkInterface>> = OnceCell::const_new();

/// Initialize the global network interface
pub async fn init_network(config: NetworkConfig) -> NetResult<()> {
    let interface = Arc::new(NetworkInterface::new(config).await?);

    NETWORK_INTERFACE
        .set(interface)
        .map_err(|_| NetError::generic("Network interface already initialized".to_string()))?;

    Ok(())
}

/// Get the global network interface
pub fn get_network() -> Option<Arc<NetworkInterface>> {
    NETWORK_INTERFACE.get().cloned()
}

/// Shutdown the global network interface
pub async fn shutdown_network() -> NetResult<()> {
    if let Some(network) = get_network() {
        network.shutdown().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_interface_creation() {
        let config = NetworkConfig::default();
        let interface = NetworkInterface::new(config).await.unwrap();

        let stats = interface.get_stats().await;
        assert_eq!(stats.connected_players, 0);
        assert_eq!(stats.packets_sent, 0);
        assert_eq!(stats.packets_received, 0);
        assert!(stats.public_address.is_none());
    }

    #[tokio::test]
    async fn test_global_network_init() {
        let config = NetworkConfig::default();
        init_network(config).await.unwrap();

        let network = get_network().unwrap();
        let stats = network.get_stats().await;
        assert_eq!(stats.connected_players, 0);
        assert!(stats.public_address.is_none());

        shutdown_network().await.unwrap();
    }

    #[tokio::test]
    async fn test_nat_monitor_starts() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let config = NetworkConfig::default();
        let interface = NetworkInterface::new(config).await.unwrap();

        assert!(interface.nat_monitor.lock().is_some());

        interface.shutdown().await.unwrap();
    }
}
