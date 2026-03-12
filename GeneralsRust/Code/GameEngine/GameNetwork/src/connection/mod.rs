//! Connection management for multiplayer networking
//!
//! This module handles individual player connections, including
//! connection pooling, state management, and reliable message delivery.

use crate::commands::wrapper::WrapperReassembler;
use crate::commands::{CommandPayload, NetCommand, NetCommandType, ProgressType};
use crate::connection::reliability::{ReliabilityConfig, ReliabilityLayer};
use crate::error::{NetworkError, NetworkResult};
use crate::file_transfer::{FileMetadata, TransferProgress};
use crate::security::{
    encryption::{self, EncryptedPacket},
    SecurityManager,
};
use crate::time::NetworkInstant;
use crate::transport::{Transport, TransportMessage, TransportProtocol};
use chrono::{DateTime, Utc};
use game_engine::common::system::compression::{decompress_data, is_data_compressed};
use game_engine::get_game_state;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{broadcast, watch, Mutex as AsyncMutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

const MAX_QUIT_FLUSH_TIME_MS: u64 = 30_000;
const CONNECTION_LATENCY_HISTORY_LENGTH: usize = 200;

#[derive(Clone)]
pub struct CommandProcessorContext {
    pub local_player_id: u8,
    pub connection_manager: Arc<RwLock<ConnectionManager>>,
    pub wrapper_reassembler: Arc<AsyncMutex<WrapperReassembler>>,
    pub player_names: Arc<AsyncMutex<HashMap<u8, String>>>,
    pub player_load_progress: Arc<AsyncMutex<HashMap<u8, u8>>>,
    pub file_announcements: Arc<AsyncMutex<HashMap<u16, FileAnnouncementState>>>,
    pub file_transfer_map: Arc<AsyncMutex<HashMap<u16, Vec<(u8, Uuid)>>>>,
    pub load_progress_tx: watch::Sender<HashMap<u8, u8>>,
    pub file_announcement_tx: broadcast::Sender<FileAnnouncementEvent>,
    pub file_progress: Arc<AsyncMutex<HashMap<u16, FileProgressState>>>,
    pub file_progress_tx: broadcast::Sender<FileProgressEvent>,
    pub chat_tx: broadcast::Sender<ChatEvent>,
    pub timeout_tx: broadcast::Sender<TimeoutEvent>,
}

#[derive(Debug, Default)]
struct CommandContextResult {
    responses: Vec<NetCommand>,
    additional_commands: Vec<NetCommand>,
}

impl CommandContextResult {
    fn with_response(command: NetCommand) -> Self {
        Self {
            responses: vec![command],
            additional_commands: Vec::new(),
        }
    }

    fn push_additional(&mut self, command: NetCommand) {
        self.additional_commands.push(command);
    }

    fn extend_from(&mut self, other: CommandContextResult) {
        self.responses.extend(other.responses);
        self.additional_commands.extend(other.additional_commands);
    }
}

impl CommandProcessorContext {
    async fn publish_load_progress(&self) {
        let snapshot = {
            let load = self.player_load_progress.lock().await;
            load.clone()
        };
        let _ = self.load_progress_tx.send(snapshot);
    }

    async fn apply_file_progress(
        &self,
        command_id: u16,
        player_id: u8,
        status: FileProgressStatus,
    ) {
        {
            let mut map = self.file_progress.lock().await;
            let entry = map.entry(command_id).or_default();
            match &status {
                FileProgressStatus::Progress { percentage } => {
                    entry.failures.remove(&player_id);
                    entry.progress.insert(player_id, (*percentage).min(100));
                }
                FileProgressStatus::Completed => {
                    entry.failures.remove(&player_id);
                    entry.progress.insert(player_id, 100);
                }
                FileProgressStatus::Failed { reason } => {
                    entry.failures.insert(player_id, reason.clone());
                }
            }

            if entry.progress.is_empty() && entry.failures.is_empty() {
                map.remove(&command_id);
            }
        }

        let _ = self.file_progress_tx.send(FileProgressEvent {
            command_id,
            player_id,
            status,
        });
    }

    async fn handle_command(&self, player_id: u8, command: &NetCommand) -> CommandContextResult {
        let mut result = CommandContextResult::default();

        match &command.payload {
            CommandPayload::Progress(progress)
                if progress.progress_type == ProgressType::Loading =>
            {
                {
                    let mut load = self.player_load_progress.lock().await;
                    load.insert(player_id, progress.percentage.min(100));
                }
                self.publish_load_progress().await;
            }
            CommandPayload::FileAnnouncement(data) => {
                {
                    let mut announcements = self.file_announcements.lock().await;
                    announcements.insert(
                        data.command_id,
                        FileAnnouncementState {
                            metadata: data.metadata.clone(),
                            player_mask: data.player_mask,
                        },
                    );
                }
                {
                    let mut transfers = self.file_transfer_map.lock().await;
                    transfers.remove(&data.command_id);
                }
                let mut announced_players = Vec::new();
                {
                    let mut progress = self.file_progress.lock().await;
                    let entry = progress.entry(data.command_id).or_default();
                    entry.progress.clear();
                    entry.failures.clear();

                    for slot in 0..crate::config::MAX_PLAYERS as usize {
                        let player = slot as u8;
                        if (data.player_mask & (1u8 << player)) != 0 {
                            entry.progress.insert(player, 0);
                            announced_players.push(player);
                        } else {
                            entry.progress.insert(player, 100);
                        }
                    }
                }
                for target in announced_players {
                    let _ = self.file_progress_tx.send(FileProgressEvent {
                        command_id: data.command_id,
                        player_id: target,
                        status: FileProgressStatus::Progress { percentage: 0 },
                    });
                }
                let _ = self.file_announcement_tx.send(FileAnnouncementEvent {
                    command_id: data.command_id,
                    player_mask: data.player_mask,
                    metadata: data.metadata.clone(),
                });
            }
            CommandPayload::FileProgress(data) => {
                // progress is i32 in C++, convert to u8 for status
                let percentage = data.progress.clamp(0, 100) as u8;
                let status = if percentage >= 100 {
                    FileProgressStatus::Completed
                } else {
                    FileProgressStatus::Progress { percentage }
                };
                self.apply_file_progress(data.file_id, player_id, status)
                    .await;
            }
            CommandPayload::FileTransfer(data) => {
                let command_id = if data.file_id != 0 {
                    (data.file_id & 0xFFFF) as u16
                } else {
                    (command.id.as_u128() & 0xFFFF) as u16
                };

                let portable_path = data.filename.clone();
                let target_path = {
                    let game_state = get_game_state();
                    game_state.portable_map_path_to_real_map_path(&portable_path)
                };
                let target_path = PathBuf::from(&target_path);

                let mut payload = data.data.clone();
                if portable_path.to_ascii_lowercase().ends_with(".tga")
                    && is_data_compressed(&payload)
                {
                    match decompress_data(&payload) {
                        Ok(decompressed) => {
                            payload = decompressed;
                        }
                        Err(err) => {
                            warn!("Failed to decompress '{}' transfer: {}", portable_path, err);
                        }
                    }
                }

                {
                    let mut announcements = self.file_announcements.lock().await;
                    announcements
                        .entry(command_id)
                        .or_insert(FileAnnouncementState {
                            metadata: FileMetadata {
                                filename: portable_path.clone(),
                                file_size: payload.len() as u64,
                                checksum: [0u8; 32],
                                transfer_type: crate::file_transfer::TransferType::Generic,
                            },
                            player_mask: 1u8 << self.local_player_id,
                        });
                }

                let mut wrote_file = true;
                if let Some(parent) = target_path.parent() {
                    if let Err(err) = fs::create_dir_all(parent).await {
                        let reason =
                            format!("failed to create parent dir for {:?}: {}", target_path, err);
                        warn!("{}", reason);
                        wrote_file = false;
                    }
                }

                if wrote_file {
                    if let Err(err) = fs::write(&target_path, &payload).await {
                        let reason = format!("failed to write file {:?}: {}", target_path, err);
                        warn!("{}", reason);
                        wrote_file = false;
                    }
                }

                let status = FileProgressStatus::Completed;
                if !wrote_file {
                    warn!(
                        "File transfer command {} completed with write errors",
                        command_id
                    );
                }

                self.apply_file_progress(command_id, self.local_player_id, status)
                    .await;

                let progress_value = 100;
                let progress_command =
                    NetCommand::file_progress(self.local_player_id, command_id, progress_value);
                let mask = 0xffu32 ^ (1u32 << self.local_player_id);
                let manager = self.connection_manager.read().await;
                if let Err(err) = manager.send_command_to_mask(progress_command, mask).await {
                    warn!(
                        "Failed to broadcast file progress for command {}: {}",
                        command_id, err
                    );
                }
            }
            CommandPayload::Wrapper(wrapper) => {
                let wrapped_id = wrapper.wrapped_command_id;
                let orig_progress = {
                    let progress = self.file_progress.lock().await;
                    progress
                        .get(&wrapped_id)
                        .and_then(|entry| entry.progress.get(&self.local_player_id))
                        .copied()
                        .unwrap_or(0)
                };

                let mut reassembler = self.wrapper_reassembler.lock().await;
                let reassembled = match reassembler.add_chunk(wrapper.clone()) {
                    Ok(data) => data,
                    Err(err) => {
                        warn!(
                            "Failed to add wrapper chunk for command {}: {}",
                            wrapped_id, err
                        );
                        None
                    }
                };

                if self
                    .file_announcements
                    .lock()
                    .await
                    .contains_key(&wrapped_id)
                {
                    if let Some(new_progress) = reassembler.percent_complete(wrapped_id) {
                        if new_progress > orig_progress && new_progress < 100 {
                            self.apply_file_progress(
                                wrapped_id,
                                self.local_player_id,
                                FileProgressStatus::Progress {
                                    percentage: new_progress,
                                },
                            )
                            .await;

                            let progress_command = NetCommand::file_progress(
                                self.local_player_id,
                                wrapped_id,
                                new_progress as i32,
                            );
                            let mask = 0xffu32 ^ (1u32 << self.local_player_id);
                            let manager = self.connection_manager.read().await;
                            if let Err(err) =
                                manager.send_command_to_mask(progress_command, mask).await
                            {
                                warn!(
                                    "Failed to broadcast wrapper progress for command {}: {}",
                                    wrapped_id, err
                                );
                            }
                        }
                    }
                }

                drop(reassembler);

                if let Some(data) = reassembled {
                    match bincode::deserialize::<NetCommand>(&data) {
                        Ok(inner) => {
                            // Box the recursive call to avoid infinite future size
                            let inner_result =
                                Box::pin(self.handle_command(player_id, &inner)).await;
                            result.extend_from(inner_result);
                            result.push_additional(inner);
                        }
                        Err(err) => {
                            warn!(
                                "Failed to deserialize wrapped command {}: {}",
                                wrapped_id, err
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        match command.command_type {
            NetCommandType::PlayerLeave
            | NetCommandType::DisconnectPlayer
            | NetCommandType::DestroyPlayer => {
                self.remove_player(player_id).await;
            }
            NetCommandType::Chat | NetCommandType::DisconnectChat => {
                if let CommandPayload::Chat(chat) = &command.payload {
                    let event = ChatEvent {
                        player_id,
                        message: chat.message.clone(),
                        player_mask: chat.target_mask,
                        is_disconnect_chat: command.command_type == NetCommandType::DisconnectChat,
                    };
                    let _ = self.chat_tx.send(event);
                }
            }
            NetCommandType::TimeoutStart => {
                let _ = self.timeout_tx.send(TimeoutEvent { player_id });
            }
            _ => {}
        }

        result
    }

    async fn remove_player(&self, player_id: u8) {
        let mut names = self.player_names.lock().await;
        names.remove(&player_id);
        drop(names);

        {
            let mut load = self.player_load_progress.lock().await;
            let existed = load.remove(&player_id).is_some();
            let snapshot = if existed { Some(load.clone()) } else { None };
            drop(load);
            if let Some(snapshot) = snapshot {
                let _ = self.load_progress_tx.send(snapshot);
            }
        }

        let mut transfers = self.file_transfer_map.lock().await;
        transfers.retain(|_, entries| {
            entries.retain(|(pid, _)| *pid != player_id);
            !entries.is_empty()
        });
        drop(transfers);

        let mut disconnect_events = Vec::new();
        {
            let mut progress = self.file_progress.lock().await;
            let mut empty = Vec::new();
            for (&command_id, state) in progress.iter_mut() {
                if let Some(percent) = state.progress.remove(&player_id) {
                    if percent < 100 {
                        let reason = "player disconnected".to_string();
                        state.failures.insert(player_id, reason.clone());
                        disconnect_events.push(FileProgressEvent {
                            command_id,
                            player_id,
                            status: FileProgressStatus::Failed { reason },
                        });
                    } else {
                        state.failures.remove(&player_id);
                    }
                } else {
                    state.failures.remove(&player_id);
                }

                if state.progress.is_empty() && state.failures.is_empty() {
                    empty.push(command_id);
                }
            }
            for command_id in empty {
                progress.remove(&command_id);
            }
        }

        for event in disconnect_events {
            let _ = self.file_progress_tx.send(event);
        }
    }
}

pub mod connection;
pub mod connection_manager;
pub mod disconnect_manager;
pub mod disconnect_voting;
pub mod manager;
pub mod pool;
pub mod reliability;
pub mod state;
pub mod timeout;
pub mod user;

pub use timeout::{BandwidthMonitor, ConnectionHealth, ConnectionMonitor, TimeoutConfig};
pub use user::{ConnectionQuality, User, UserAuth, UserNetworkStats, UserSettings, UserState};

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ConnectionState {
    /// Initial state - not connected
    Disconnected = 0,
    /// Attempting to connect
    Connecting = 1,
    /// Connected and ready
    Connected = 2,
    /// Connection being authenticated
    Authenticating = 3,
    /// Connection authenticated and ready for game
    Authenticated = 4,
    /// Connection in game
    InGame = 5,
    /// Connection being gracefully disconnected
    Disconnecting = 6,
    /// Connection lost/error state
    Error = 7,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Connected => "Connected",
            Self::Authenticating => "Authenticating",
            Self::Authenticated => "Authenticated",
            Self::InGame => "InGame",
            Self::Disconnecting => "Disconnecting",
            Self::Error => "Error",
        };
        write!(f, "{}", name)
    }
}

/// Connection statistics
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ConnectionStats {
    /// Total packets sent
    pub packets_sent: u64,
    /// Total packets received
    pub packets_received: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Packets lost (not acknowledged)
    pub packets_lost: u64,
    /// Average round-trip time in milliseconds
    pub average_rtt_ms: f64,
    /// Current latency in milliseconds
    pub current_latency_ms: f64,
    /// Connection uptime
    pub uptime: Duration,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
}

/// Runtime tracking for active file transfers routed through the connection manager.
#[derive(Debug, Clone)]
pub struct ManagedTransfer {
    pub progress: TransferProgress,
    pub participants: Vec<u8>,
    pub completion_status: HashMap<u8, bool>,
    pub command_id: Option<u16>,
    pub failure_reason: Option<String>,
}

/// Status for per-player file transfer progress updates.
#[derive(Debug, Clone)]
pub enum FileProgressStatus {
    /// Transfer is advancing with the provided percentage.
    Progress { percentage: u8 },
    /// Transfer completed successfully.
    Completed,
    /// Transfer aborted with the associated reason.
    Failed { reason: String },
}

/// Event broadcast when a file transfer progresses, completes, or fails.
#[derive(Debug, Clone)]
pub struct FileProgressEvent {
    pub command_id: u16,
    pub player_id: u8,
    pub status: FileProgressStatus,
}

/// Announcement broadcast describing an impending file transfer.
#[derive(Debug, Clone)]
pub struct FileAnnouncementEvent {
    pub command_id: u16,
    pub player_mask: u8,
    pub metadata: FileMetadata,
}

/// Persistent state for announced transfers used to map metadata to command ids.
#[derive(Debug, Clone)]
pub struct FileAnnouncementState {
    pub metadata: FileMetadata,
    pub player_mask: u8,
}

/// Aggregated progress/failure tracking per transfer command.
#[derive(Debug, Clone, Default)]
pub struct FileProgressState {
    pub progress: HashMap<u8, u8>,
    pub failures: HashMap<u8, String>,
}

/// Network chat surfaced from remote peers or the local player.
#[derive(Debug, Clone)]
pub struct ChatEvent {
    pub player_id: u8,
    pub message: String,
    pub player_mask: i32, // Changed from u8 to i32 to match C++ format
    pub is_disconnect_chat: bool,
}

/// Event indicating the timeout watchdog has fired for a player.
#[derive(Debug, Clone)]
pub struct TimeoutEvent {
    pub player_id: u8,
}

/// Connection configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectionConfig {
    /// Maximum retry attempts for reliable messages
    pub max_retries: u32,
    /// Retry timeout in milliseconds
    pub retry_timeout_ms: u64,
    /// Minimum time between packet sends (ms). Mirrors C++ frame grouping.
    pub frame_grouping_ms: u64,
    /// Keep-alive interval
    pub keepalive_interval: Duration,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Maximum queue sizes
    pub max_send_queue: usize,
    pub max_receive_queue: usize,
    /// Enable reliability layer
    pub enable_reliability: bool,
    /// Enable compression
    pub enable_compression: bool,
    /// Enable authenticated encryption using session keys
    pub enable_encryption: bool,
    /// Local player identifier for commands originating from this node
    pub local_player_id: u8,
    /// Local slot index (0-7) - matches C++ m_localSlot
    pub local_slot: usize,
    /// Whether this connection is the packet router
    pub is_packet_router: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            retry_timeout_ms: 2000,
            frame_grouping_ms: 1,
            keepalive_interval: Duration::from_secs(20),
            connection_timeout: Duration::from_secs(30),
            max_send_queue: 1000,
            max_receive_queue: 1000,
            enable_reliability: true,
            enable_compression: true,
            enable_encryption: true,
            local_player_id: 0,
            local_slot: 0,
            is_packet_router: false,
        }
    }
}

/// Individual player connection
pub struct Connection {
    /// Unique connection identifier
    id: Uuid,
    /// Player ID (0-7)
    player_id: u8,
    /// Remote address
    remote_addr: SocketAddr,
    /// Connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Connection configuration
    config: ConnectionConfig,
    /// Connection statistics
    stats: Arc<RwLock<ConnectionStats>>,

    /// Transport protocol used
    protocol: TransportProtocol,
    /// Transport layer reference
    transport: Arc<Transport>,

    /// Message queues
    send_queue: Arc<RwLock<VecDeque<NetCommand>>>,
    receive_queue: Arc<RwLock<VecDeque<NetCommand>>>,
    /// Track last send time for frame grouping.
    last_time_sent: Arc<AsyncMutex<NetworkInstant>>,
    /// Minimum time between packet sends.
    frame_grouping: Arc<AsyncMutex<Duration>>,
    /// Retry interval for reliable commands.
    retry_time: Duration,
    /// Retry metrics
    num_retries: Arc<AsyncMutex<u32>>,
    retry_metrics_time: Arc<AsyncMutex<NetworkInstant>>,
    /// Latency tracking for ACKs.
    latencies: Arc<AsyncMutex<[f32; CONNECTION_LATENCY_HISTORY_LENGTH]>>,
    average_latency_ms: Arc<AsyncMutex<f32>>,
    /// Ack tracking for latency + retry.
    pending_ack_times: Arc<AsyncMutex<HashMap<Uuid, NetworkInstant>>>,
    /// Quit handling
    is_quitting: Arc<AsyncMutex<bool>>,
    quit_time: Arc<AsyncMutex<Option<NetworkInstant>>>,

    /// Reliability layer for acknowledgments, ordering, and retransmission
    reliability: Option<Arc<ReliabilityLayer>>,

    /// Shared security manager used for encryption and authentication metadata
    security: Option<Arc<SecurityManager>>,

    /// Timing
    created_at: DateTime<Utc>,
    last_keepalive: Arc<RwLock<NetworkInstant>>,
    last_activity: Arc<RwLock<NetworkInstant>>,

    /// Control channels
    shutdown_tx: broadcast::Sender<()>,
    task_handles: Vec<tokio::task::JoinHandle<()>>,
    command_context: Option<CommandProcessorContext>,
}

impl Connection {
    /// Create a new connection
    pub async fn new(
        player_id: u8,
        remote_addr: SocketAddr,
        protocol: TransportProtocol,
        transport: Arc<Transport>,
    ) -> NetworkResult<Self> {
        Self::with_config(
            player_id,
            remote_addr,
            protocol,
            transport,
            ConnectionConfig::default(),
            None,
        )
        .await
    }

    /// Create connection with custom configuration
    pub async fn with_config(
        player_id: u8,
        remote_addr: SocketAddr,
        protocol: TransportProtocol,
        transport: Arc<Transport>,
        config: ConnectionConfig,
        security: Option<Arc<SecurityManager>>,
    ) -> NetworkResult<Self> {
        let (shutdown_tx, _) = broadcast::channel(1);
        let now = NetworkInstant::now();
        // Extract values needed after config is moved
        let frame_grouping_ms = config.frame_grouping_ms;
        let retry_timeout_ms = config.retry_timeout_ms;
        let reliability = if config.enable_reliability {
            let mut reliability_config = ReliabilityConfig::default();
            reliability_config.max_retries = config.max_retries;
            reliability_config.initial_timeout = Duration::from_millis(config.retry_timeout_ms);
            reliability_config.max_timeout =
                Duration::from_millis(config.retry_timeout_ms.saturating_mul(8))
                    .min(Duration::from_secs(10));
            let layer = Arc::new(ReliabilityLayer::with_config(reliability_config));
            layer.set_local_player_id(config.local_player_id);
            Some(layer)
        } else {
            None
        };

        Ok(Self {
            id: Uuid::new_v4(),
            player_id,
            remote_addr,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            config: config.clone(),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
            protocol,
            transport,
            send_queue: Arc::new(RwLock::new(VecDeque::new())),
            receive_queue: Arc::new(RwLock::new(VecDeque::new())),
            last_time_sent: Arc::new(AsyncMutex::new(now)),
            frame_grouping: Arc::new(AsyncMutex::new(Duration::from_millis(frame_grouping_ms))),
            retry_time: Duration::from_millis(retry_timeout_ms),
            num_retries: Arc::new(AsyncMutex::new(0)),
            retry_metrics_time: Arc::new(AsyncMutex::new(now)),
            latencies: Arc::new(AsyncMutex::new([0.0; CONNECTION_LATENCY_HISTORY_LENGTH])),
            average_latency_ms: Arc::new(AsyncMutex::new(0.0)),
            pending_ack_times: Arc::new(AsyncMutex::new(HashMap::new())),
            is_quitting: Arc::new(AsyncMutex::new(false)),
            quit_time: Arc::new(AsyncMutex::new(None)),
            reliability,
            security,
            created_at: Utc::now(),
            last_keepalive: Arc::new(RwLock::new(now)),
            last_activity: Arc::new(RwLock::new(now)),
            shutdown_tx,
            task_handles: Vec::new(),
            command_context: None,
        })
    }

    pub fn set_command_context(&mut self, context: CommandProcessorContext) {
        self.command_context = Some(context);
    }

    /// Start the connection
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!(
            "Starting connection for player {} at {}",
            self.player_id, self.remote_addr
        );

        {
            let mut state = self.state.write().await;
            *state = ConnectionState::Connecting;
        }

        // Start background tasks
        self.start_background_tasks().await?;

        // Send initial connection handshake
        self.send_handshake().await?;

        Ok(())
    }

    /// Start background processing tasks
    async fn start_background_tasks(&mut self) -> NetworkResult<()> {
        let shutdown_tx = self.shutdown_tx.clone();

        // Message processing task
        {
            let connection_id = self.id;
            let send_queue = self.send_queue.clone();
            let transport = self.transport.clone();
            let remote_addr = self.remote_addr;
            let protocol = self.protocol;
            let stats = self.stats.clone();
            let reliability = self.reliability.clone();
            let security = self.security.clone();
            let encryption_enabled = self.config.enable_encryption;
            let player_id = self.player_id;
            let last_time_sent = self.last_time_sent.clone();
            let frame_grouping = self.frame_grouping.clone();
            let retry_time = self.retry_time;
            let num_retries = self.num_retries.clone();
            let retry_metrics_time = self.retry_metrics_time.clone();
            let pending_ack_times = self.pending_ack_times.clone();
            let is_quitting = self.is_quitting.clone();
            let quit_time = self.quit_time.clone();
            let mut shutdown_rx_clone = shutdown_tx.subscribe();

            let handle = tokio::spawn(async move {
                Self::message_processing_task(
                    connection_id,
                    send_queue,
                    transport,
                    remote_addr,
                    protocol,
                    stats,
                    reliability,
                    security,
                    encryption_enabled,
                    player_id,
                    last_time_sent,
                    frame_grouping,
                    retry_time,
                    num_retries,
                    retry_metrics_time,
                    pending_ack_times,
                    is_quitting,
                    quit_time,
                    &mut shutdown_rx_clone,
                )
                .await;
            });

            self.task_handles.push(handle);
        }

        // Keepalive task
        {
            let connection_id = self.id;
            let local_player_id = self.config.local_player_id;
            let keepalive_interval = self.config.keepalive_interval;
            let last_keepalive = self.last_keepalive.clone();
            let state = self.state.clone();
            let send_queue = self.send_queue.clone();
            let mut shutdown_rx_clone = shutdown_tx.subscribe();

            let handle = tokio::spawn(async move {
                Self::keepalive_task(
                    connection_id,
                    local_player_id,
                    keepalive_interval,
                    last_keepalive,
                    state,
                    send_queue,
                    &mut shutdown_rx_clone,
                )
                .await;
            });

            self.task_handles.push(handle);
        }

        // Reliability task (if enabled)
        if let Some(reliability) = self.reliability.clone() {
            let connection_id = self.id;
            let send_queue = self.send_queue.clone();
            let stats = self.stats.clone();
            let mut shutdown_rx_clone = shutdown_tx.subscribe();

            let handle = tokio::spawn(async move {
                Self::reliability_task(
                    connection_id,
                    reliability,
                    send_queue,
                    stats,
                    &mut shutdown_rx_clone,
                )
                .await;
            });

            self.task_handles.push(handle);
        }

        Ok(())
    }

    /// Send handshake message
    async fn send_handshake(&self) -> NetworkResult<()> {
        let handshake = NetCommand::new(
            NetCommandType::KeepAlive, // Use KeepAlive for connection establishment
            self.config.local_player_id,
            0,
            crate::commands::CommandPayload::Generic(b"HANDSHAKE".to_vec()),
        );

        self.send_command(handshake).await
    }

    /// Send a command through this connection
    pub async fn send_command(&self, mut command: NetCommand) -> NetworkResult<()> {
        if *self.is_quitting.lock().await {
            return Ok(());
        }

        let should_wrap = match bincode::serialized_size(&command) {
            Ok(size) => {
                size as usize > crate::commands::wrapper::MAX_WRAPPER_CHUNK_SIZE
                    && command.command_type != NetCommandType::Wrapper
            }
            Err(_) => false,
        };

        if should_wrap {
            // Wrapper commands carry the ACKs; avoid waiting on the wrapped command ID.
            command.flags.needs_ack = false;
        } else if let Some(layer) = &self.reliability {
            if command.needs_acknowledgment() {
                layer.send_reliable(command.clone()).await?;
            }
        }

        // Add to send queue
        {
            let mut queue = self.send_queue.write().await;

            // Check queue size
            if queue.len() >= self.config.max_send_queue {
                queue.pop_front();
                warn!(
                    "Send queue overflow for player {}, dropping oldest command",
                    self.player_id
                );
            }

            queue.push_back(command);
        }

        Ok(())
    }

    /// Receive a command from this connection
    pub async fn receive_command(&self) -> Option<NetCommand> {
        let mut queue = self.receive_queue.write().await;
        queue.pop_front()
    }

    /// Determine whether both send and receive queues are empty.
    pub async fn queues_empty(&self) -> bool {
        let send_empty = { self.send_queue.read().await.is_empty() };
        if !send_empty {
            return false;
        }
        self.receive_queue.read().await.is_empty()
    }

    /// Fetch lightweight statistics used for heuristics.
    pub async fn quick_stats(&self) -> ConnectionStats {
        self.get_stats().await
    }

    /// Process incoming transport message
    pub async fn process_incoming_message(&self, message: TransportMessage) -> NetworkResult<()> {
        {
            let mut last_activity = self.last_activity.write().await;
            *last_activity = NetworkInstant::now();
        }

        let payload = match encryption::decode_envelope(&message.data)? {
            encryption::Envelope::Plain(data) => data.to_vec(),
            encryption::Envelope::Encrypted {
                key_id,
                nonce,
                payload,
            } => {
                let sec = self.security.as_ref().ok_or_else(|| {
                    NetworkError::security("received encrypted payload without security manager")
                })?;

                let packet = EncryptedPacket {
                    key_id,
                    nonce,
                    payload: payload.to_vec(),
                };

                let provider = sec.encryption_provider();
                if key_id == 0 {
                    let session_key = sec.secure_session_key(self.player_id).await?;
                    provider.decrypt_with_session(&packet, &session_key).await?
                } else {
                    provider.decrypt(&packet).await?
                }
            }
        };

        let command: NetCommand = bincode::deserialize(&payload)
            .map_err(|e| NetworkError::generic(format!("failed to deserialize command: {}", e)))?;

        let is_ack = matches!(
            command.command_type,
            NetCommandType::AckBoth | NetCommandType::AckStage1 | NetCommandType::AckStage2
        );

        if let Some(layer) = &self.reliability {
            if is_ack {
                layer.process_acknowledgment(&command).await?;
                self.sync_reliability_stats().await;

                if let CommandPayload::Ack(data) = &command.payload {
                    let mut pending = self.pending_ack_times.lock().await;
                    if let Some(sent_at) = pending.remove(&data.command_id) {
                        let latency_ms = sent_at.elapsed().as_secs_f32() * 1000.0;
                        let index = (data.command_id.as_u128() as usize)
                            % CONNECTION_LATENCY_HISTORY_LENGTH;
                        let mut latencies = self.latencies.lock().await;
                        let mut avg = self.average_latency_ms.lock().await;
                        *avg -= latencies[index] / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
                        *avg += latency_ms / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
                        latencies[index] = latency_ms;

                        let mut stats = self.stats.write().await;
                        stats.current_latency_ms = latency_ms as f64;
                        stats.average_rtt_ms = *avg as f64;
                    }
                }
            } else {
                let ready_commands = layer.process_incoming(command).await?;
                if !ready_commands.is_empty() {
                    let mut commands_to_enqueue = Vec::new();
                    for cmd in &ready_commands {
                        if let Some(ctx) = &self.command_context {
                            let result = ctx.handle_command(self.player_id, cmd).await;
                            for response in result.responses {
                                if let Err(err) = self.send_command(response).await {
                                    warn!(
                                        "Failed to send response command to player {}: {}",
                                        self.player_id, err
                                    );
                                }
                            }
                            commands_to_enqueue.extend(result.additional_commands);
                        }
                        if cmd.command_type != NetCommandType::Wrapper {
                            commands_to_enqueue.push(cmd.clone());
                        }
                    }
                    if !commands_to_enqueue.is_empty() {
                        self.enqueue_incoming_commands(commands_to_enqueue).await;
                    }
                }
                self.sync_reliability_stats().await;
            }
        } else {
            if is_ack {
                if let CommandPayload::Ack(data) = &command.payload {
                    let mut pending = self.pending_ack_times.lock().await;
                    if let Some(sent_at) = pending.remove(&data.command_id) {
                        let latency_ms = sent_at.elapsed().as_secs_f32() * 1000.0;
                        let index = (data.command_id.as_u128() as usize)
                            % CONNECTION_LATENCY_HISTORY_LENGTH;
                        let mut latencies = self.latencies.lock().await;
                        let mut avg = self.average_latency_ms.lock().await;
                        *avg -= latencies[index] / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
                        *avg += latency_ms / CONNECTION_LATENCY_HISTORY_LENGTH as f32;
                        latencies[index] = latency_ms;

                        let mut stats = self.stats.write().await;
                        stats.current_latency_ms = latency_ms as f64;
                        stats.average_rtt_ms = *avg as f64;
                    }
                }
            } else {
                if let Some(ctx) = &self.command_context {
                    let result = ctx.handle_command(self.player_id, &command).await;
                    for response in result.responses {
                        if let Err(err) = self.send_command(response).await {
                            warn!(
                                "Failed to send response command to player {}: {}",
                                self.player_id, err
                            );
                        }
                    }
                    let mut commands_to_enqueue = Vec::new();
                    if command.command_type != NetCommandType::Wrapper {
                        commands_to_enqueue.push(command.clone());
                    }
                    commands_to_enqueue.extend(result.additional_commands);
                    if !commands_to_enqueue.is_empty() {
                        self.enqueue_incoming_commands(commands_to_enqueue).await;
                    }
                } else {
                    if command.command_type != NetCommandType::Wrapper {
                        self.enqueue_incoming_commands(vec![command.clone()]).await;
                    }
                }

                if command.needs_acknowledgment() {
                    let ack = NetCommand::ack(
                        self.config.local_player_id,
                        NetCommandType::AckBoth,
                        command.id,
                    );
                    self.send_command(ack).await?;
                }
            }
        }

        {
            let mut stats = self.stats.write().await;
            stats.packets_received += 1;
            stats.bytes_received += message.data.len() as u64;
            stats.last_activity = Utc::now();
        }

        Ok(())
    }

    async fn enqueue_incoming_commands(&self, commands: Vec<NetCommand>) {
        if commands.is_empty() {
            return;
        }

        let mut queue = self.receive_queue.write().await;
        let mut dropped = 0usize;

        for command in commands {
            if queue.len() >= self.config.max_receive_queue {
                queue.pop_front();
                dropped += 1;
            }
            queue.push_back(command);
        }

        if dropped > 0 {
            warn!(
                "Receive queue overflow for player {} (dropped {} commands)",
                self.player_id, dropped
            );
        }
    }

    async fn sync_reliability_stats(&self) {
        if let Some(layer) = &self.reliability {
            let layer_stats = layer.get_stats().await;
            let mut stats = self.stats.write().await;
            stats.average_rtt_ms = layer_stats.average_rtt_ms;
            stats.current_latency_ms = layer_stats.average_rtt_ms;
            stats.packets_lost = layer_stats.messages_failed;
        }
    }

    /// Get connection state
    pub async fn get_state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Set connection state
    pub async fn set_state(&self, new_state: ConnectionState) {
        let mut state = self.state.write().await;
        if *state != new_state {
            debug!(
                "Connection {} state change: {} -> {}",
                self.id, *state, new_state
            );
            *state = new_state;
        }
    }

    /// Check if connection is active
    pub async fn is_active(&self) -> bool {
        match self.get_state().await {
            ConnectionState::Connected
            | ConnectionState::Authenticated
            | ConnectionState::InGame => true,
            _ => false,
        }
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let mut stats = self.stats.read().await.clone();
        stats.uptime = Utc::now()
            .signed_duration_since(self.created_at)
            .to_std()
            .unwrap_or_default();
        stats
    }

    /// Get connection info
    pub fn get_info(&self) -> ConnectionInfo {
        ConnectionInfo {
            id: self.id,
            player_id: self.player_id,
            remote_addr: self.remote_addr,
            protocol: self.protocol,
            created_at: self.created_at,
        }
    }

    /// Return the local player identifier used for outbound messages on this link.
    pub fn local_player_id(&self) -> u8 {
        self.config.local_player_id
    }

    /// Set minimum time between packet sends (C++ SetFrameGrouping).
    pub async fn set_frame_grouping_ms(&self, frame_grouping_ms: u64) {
        let mut grouping = self.frame_grouping.lock().await;
        *grouping = Duration::from_millis(frame_grouping_ms);
    }

    /// Gracefully disconnect
    pub async fn disconnect(&self) -> NetworkResult<()> {
        info!(
            "Disconnecting player {} at {}",
            self.player_id, self.remote_addr
        );

        {
            let mut quitting = self.is_quitting.lock().await;
            *quitting = true;
            let mut quit_time = self.quit_time.lock().await;
            *quit_time = Some(NetworkInstant::now());
        }

        self.set_state(ConnectionState::Disconnecting).await;

        // Send disconnect notification
        let disconnect_cmd = NetCommand::new(
            NetCommandType::DisconnectEnd,
            self.config.local_player_id,
            0,
            crate::commands::CommandPayload::Generic(b"DISCONNECT".to_vec()),
        );

        let _ = self.send_command(disconnect_cmd).await;

        // Shutdown background tasks
        let _ = self.shutdown_tx.send(()); // broadcast::Sender::send() returns number of receivers

        self.set_state(ConnectionState::Disconnected).await;

        Ok(())
    }

    /// Mark this connection as quitting (C++ parity).
    pub async fn set_quitting(&self) {
        let mut quitting = self.is_quitting.lock().await;
        *quitting = true;
        let mut quit_time = self.quit_time.lock().await;
        *quit_time = Some(NetworkInstant::now());
    }

    /// Check if connection is marked as quitting.
    pub async fn is_quitting(&self) -> bool {
        *self.is_quitting.lock().await
    }

    /// Background task for processing outgoing messages with backpressure handling
    async fn message_processing_task(
        connection_id: Uuid,
        send_queue: Arc<RwLock<VecDeque<NetCommand>>>,
        transport: Arc<Transport>,
        remote_addr: SocketAddr,
        protocol: TransportProtocol,
        stats: Arc<RwLock<ConnectionStats>>,
        reliability: Option<Arc<ReliabilityLayer>>,
        security: Option<Arc<SecurityManager>>,
        encryption_enabled: bool,
        remote_player_id: u8,
        last_time_sent: Arc<AsyncMutex<NetworkInstant>>,
        frame_grouping: Arc<AsyncMutex<Duration>>,
        retry_time: Duration,
        num_retries: Arc<AsyncMutex<u32>>,
        retry_metrics_time: Arc<AsyncMutex<NetworkInstant>>,
        pending_ack_times: Arc<AsyncMutex<HashMap<Uuid, NetworkInstant>>>,
        is_quitting: Arc<AsyncMutex<bool>>,
        quit_time: Arc<AsyncMutex<Option<NetworkInstant>>>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        debug!(
            "Starting message processing task for connection {}",
            connection_id
        );

        let mut interval = tokio::time::interval(Duration::from_millis(1));
        let mut batch_size = 5u32; // Adaptive batch size
        let mut consecutive_empty_ticks = 0u32;

        // Backpressure monitoring
        let mut send_failures = 0u32;
        let mut last_failure_time = NetworkInstant::now();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let now = NetworkInstant::now();

                    // Quit flush handling (C++ parity)
                    {
                        let quitting = *is_quitting.lock().await;
                        if quitting {
                            let quit_at = *quit_time.lock().await;
                            if let Some(quit_at) = quit_at {
                                if quit_at.elapsed() > Duration::from_millis(MAX_QUIT_FLUSH_TIME_MS) {
                                    let mut queue = send_queue.write().await;
                                    queue.clear();
                                    continue;
                                }
                            }
                        }
                    }

                    // Frame grouping throttle (C++ parity)
                    {
                        let last = *last_time_sent.lock().await;
                        let grouping = *frame_grouping.lock().await;
                        if grouping > Duration::from_millis(0) && last.elapsed() < grouping {
                            continue;
                        }
                    }

                    // Retry metrics (C++ parity: 10s window)
                    {
                        let mut last_metrics = retry_metrics_time.lock().await;
                        if last_metrics.elapsed() > Duration::from_secs(10) {
                            *last_metrics = now;
                            let mut retries = num_retries.lock().await;
                            *retries = 0;
                        }
                    }

                    // Get current queue size for backpressure monitoring
                    let queue_size = {
                        let queue = send_queue.read().await;
                        queue.len()
                    };

                    // Adaptive batching based on queue pressure
                    if queue_size > 50 {
                        batch_size = (batch_size * 2).min(20); // Increase batch size
                    } else if queue_size == 0 {
                        consecutive_empty_ticks += 1;
                        if consecutive_empty_ticks > 100 {
                            batch_size = (batch_size / 2).max(1); // Decrease batch size
                            consecutive_empty_ticks = 0;
                        }
                    } else {
                        consecutive_empty_ticks = 0;
                    }

                    // Backpressure handling - slow down if too many send failures
                    if send_failures > 5 {
                        let since_last_failure = last_failure_time.elapsed();
                        if since_last_failure < Duration::from_secs(1) {
                            // Back off exponentially
                            let backoff_ms = (send_failures * 10).min(1000);
                            tokio::time::sleep(Duration::from_millis(backoff_ms as u64)).await;
                        } else {
                            // Reset failure count after successful quiet period
                            send_failures = 0;
                        }
                    }

                    // Process outgoing messages in batches
                    let commands_to_send = {
                        let mut queue = send_queue.write().await;
                        let mut commands = Vec::with_capacity(batch_size as usize);

                        // Extract batch of commands
                        for _ in 0..batch_size {
                            if let Some(command) = queue.pop_front() {
                                let should_send = if command.needs_acknowledgment() {
                                    let mut pending = pending_ack_times.lock().await;
                                    if let Some(last_sent) = pending.get(&command.id).copied() {
                                        if last_sent.elapsed() < retry_time {
                                            queue.push_back(command);
                                            continue; // Skip to next iteration
                                        } else {
                                            let mut retries = num_retries.lock().await;
                                            *retries += 1;
                                            true
                                        }
                                    } else {
                                        true
                                    }
                                } else {
                                    true
                                };

                                if should_send {
                                    commands.push(command);
                                }
                            } else {
                                break;
                            }
                        }

                        commands
                    };

                    if !commands_to_send.is_empty() {
                        let mut batch_success = 0usize;
                        let mut batch_failures = 0usize;
                        let mut batch_messages = Vec::with_capacity(commands_to_send.len());

                        for command in commands_to_send {
                            match bincode::serialize(&command) {
                                Ok(serialized) => {
                                    if serialized.len() > crate::commands::wrapper::MAX_WRAPPER_CHUNK_SIZE
                                        && command.command_type != NetCommandType::Wrapper
                                    {
                                        let wrapped_id = (command.id.as_u128() & 0xFFFF) as u16;
                                        match crate::commands::wrapper::WrapperCommand::split_message(
                                            wrapped_id,
                                            serialized,
                                        ) {
                                            Ok(chunks) => {
                                                for chunk in chunks {
                                                    let wrapper_command = NetCommand::new(
                                                        NetCommandType::Wrapper,
                                                        command.player_id,
                                                        command.execution_frame,
                                                        CommandPayload::Wrapper(chunk),
                                                    );
                                                    if let Some(layer) = &reliability {
                                                        if wrapper_command.needs_acknowledgment() {
                                                            if let Err(err) =
                                                                layer.send_reliable(wrapper_command.clone()).await
                                                            {
                                                                warn!(
                                                                    "Failed to mark wrapper command reliable for connection {}: {}",
                                                                    connection_id, err
                                                                );
                                                            }
                                                        }
                                                    }
                                                    match bincode::serialize(&wrapper_command) {
                                                        Ok(wrapper_bytes) => {
                                                            let envelope = if encryption_enabled {
                                                                if let Some(sec) = security.as_ref() {
                                                                    let sec = Arc::clone(sec);
                                                                    match sec.secure_session_key(remote_player_id).await {
                                                                        Ok(session_key) => {
                                                                            let provider = sec.encryption_provider();
                                                                            match provider.encrypt(&wrapper_bytes, Some(session_key)).await {
                                                                                Ok(packet) => encryption::encode_encrypted_envelope(&packet),
                                                                                Err(err) => {
                                                                                    warn!(
                                                                                        "Encryption failed for connection {} (player {}): {}",
                                                                                        connection_id,
                                                                                        remote_player_id,
                                                                                        err
                                                                                    );
                                                                                    encryption::encode_plain_envelope(&wrapper_bytes)
                                                                                }
                                                                            }
                                                                        }
                                                                        Err(_) => encryption::encode_plain_envelope(&wrapper_bytes),
                                                                    }
                                                                } else {
                                                                    encryption::encode_plain_envelope(&wrapper_bytes)
                                                                }
                                                            } else {
                                                                encryption::encode_plain_envelope(&wrapper_bytes)
                                                            };

                                                            let message = TransportMessage::new(envelope, protocol)
                                                                .with_destination(remote_addr);
                                                            batch_messages.push((message, wrapper_command));
                                                        }
                                                        Err(e) => {
                                                            error!(
                                                                "Failed to serialize wrapper command for connection {}: {}",
                                                                connection_id,
                                                                e
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                warn!(
                                                    "Failed to split wrapper command for connection {}: {}",
                                                    connection_id, err
                                                );
                                            }
                                        }
                                        continue;
                                    }

                                    let envelope = if encryption_enabled {
                                        if let Some(sec) = security.as_ref() {
                                            let sec = Arc::clone(sec);
                                            match sec.secure_session_key(remote_player_id).await {
                                                Ok(session_key) => {
                                                    let provider = sec.encryption_provider();
                                                    match provider.encrypt(&serialized, Some(session_key)).await {
                                                        Ok(packet) => encryption::encode_encrypted_envelope(&packet),
                                                        Err(err) => {
                                                            warn!(
                                                                "Encryption failed for connection {} (player {}): {}",
                                                                connection_id,
                                                                remote_player_id,
                                                                err
                                                            );
                                                            encryption::encode_plain_envelope(&serialized)
                                                        }
                                                    }
                                                }
                                                Err(_) => encryption::encode_plain_envelope(&serialized),
                                            }
                                        } else {
                                            encryption::encode_plain_envelope(&serialized)
                                        }
                                    } else {
                                        encryption::encode_plain_envelope(&serialized)
                                    };

                                    let message = TransportMessage::new(envelope, protocol)
                                        .with_destination(remote_addr);
                                    batch_messages.push((message, command));
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to serialize command for connection {}: {}",
                                        connection_id,
                                        e
                                    );
                                }
                            }
                        }

                        let mut any_sent = false;
                        for (message, command) in batch_messages {
                            let payload_len = message.data.len();
                            match transport.send_message(message).await {
                                Ok(()) => {
                                    any_sent = true;
                                    batch_success += 1;
                                    {
                                        let mut stats_guard = stats.write().await;
                                        stats_guard.packets_sent += 1;
                                        stats_guard.bytes_sent += payload_len as u64;
                                        stats_guard.last_activity = Utc::now();
                                    }

                                    if command.needs_acknowledgment() {
                                        let mut pending = pending_ack_times.lock().await;
                                        pending.insert(command.id, now);
                                    }

                                    if let Some(layer) = &reliability {
                                        if command.needs_acknowledgment() {
                                            layer.mark_sent(command.id).await;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to send message for connection {}: {}",
                                        connection_id,
                                        e
                                    );
                                    batch_failures += 1;
                                    send_failures += 1;
                                    last_failure_time = NetworkInstant::now();

                                    let mut queue = send_queue.write().await;
                                    queue.push_front(command);
                                }
                            }
                        }

                        if any_sent {
                            let mut last = last_time_sent.lock().await;
                            *last = now;
                        }

                        if batch_failures > 0 {
                            warn!(
                                "Connection {} batch send: {} failures out of {} messages",
                                connection_id,
                                batch_failures,
                                batch_failures + batch_success
                            );
                        } else {
                            trace!(
                                "Connection {} sent batch of {} messages successfully",
                                connection_id,
                                batch_success
                            );
                        }
                    }

                }
                _ = shutdown_rx.recv() => {
                    debug!("Message processing task shutting down for connection {}", connection_id);
                    break;
                }
            }
        }
    }

    /// Background task for sending keepalive messages
    async fn keepalive_task(
        connection_id: Uuid,
        player_id: u8,
        interval: Duration,
        last_keepalive: Arc<RwLock<NetworkInstant>>,
        state: Arc<RwLock<ConnectionState>>,
        send_queue: Arc<RwLock<VecDeque<NetCommand>>>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        debug!("Starting keepalive task for connection {}", connection_id);

        let mut timer = tokio::time::interval(interval);

        loop {
            tokio::select! {
                    _ = timer.tick() => {
                        // Check if we need to send keepalive
                        let should_send = {
                            let last = last_keepalive.read().await;
                            let state_val = *state.read().await;

                            last.elapsed() >= interval &&
                            matches!(state_val, ConnectionState::Connected | ConnectionState::Authenticated | ConnectionState::InGame)
                        };

                        if should_send {
                            // Create and queue keepalive command
            let keepalive = NetCommand::keep_alive(player_id);

                            {
                                let mut queue = send_queue.write().await;
                                queue.push_back(keepalive);
                            }

                            {
                                let mut last = last_keepalive.write().await;
                                *last = NetworkInstant::now();
                            }

                            trace!("Queued keepalive for connection {}", connection_id);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Keepalive task shutting down for connection {}", connection_id);
                        break;
                    }
                }
        }
    }

    /// Background task for handling reliable message retries
    async fn reliability_task(
        connection_id: Uuid,
        reliability: Arc<ReliabilityLayer>,
        send_queue: Arc<RwLock<VecDeque<NetCommand>>>,
        stats: Arc<RwLock<ConnectionStats>>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        debug!("Starting reliability task for connection {}", connection_id);

        let mut interval = tokio::time::interval(Duration::from_millis(50));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let mut outbound = Vec::new();

                    let mut control = reliability.drain_control_queue().await;
                    if !control.is_empty() {
                        outbound.append(&mut control);
                    }

                    let retries = reliability.process_retransmission().await;
                    if !retries.is_empty() {
                        outbound.extend(retries);
                    }

                    if !outbound.is_empty() {
                        let mut queue = send_queue.write().await;
                        for command in outbound {
                            queue.push_back(command);
                        }
                    }

                    let layer_stats = reliability.get_stats().await;
                    {
                        let mut stats_guard = stats.write().await;
                        stats_guard.average_rtt_ms = layer_stats.average_rtt_ms;
                        stats_guard.current_latency_ms = layer_stats.average_rtt_ms;
                        stats_guard.packets_lost = layer_stats.messages_failed;
                    }
                }
                _ = shutdown_rx.recv() => {
                    debug!("Reliability task shutting down for connection {}", connection_id);
                    break;
                }
            }
        }
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("id", &self.id)
            .field("player_id", &self.player_id)
            .field("remote_addr", &self.remote_addr)
            .field("protocol", &self.protocol)
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Clean up background tasks
        for handle in &self.task_handles {
            handle.abort();
        }
    }
}

/// Connection information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectionInfo {
    /// Unique identifier
    pub id: Uuid,
    /// Player ID
    pub player_id: u8,
    /// Remote address
    pub remote_addr: SocketAddr,
    /// Transport protocol
    pub protocol: TransportProtocol,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Connection manager for handling multiple connections
pub struct ConnectionManager {
    transport: Arc<Transport>,
    config: ConnectionConfig,
    connections: Arc<RwLock<HashMap<u8, Arc<Connection>>>>,
    address_map: Arc<RwLock<HashMap<SocketAddr, u8>>>,
    file_transfers: Arc<RwLock<HashMap<Uuid, ManagedTransfer>>>,
    message_routing_task: Option<JoinHandle<()>>,
    security: Option<Arc<SecurityManager>>,
    shutdown_tx: broadcast::Sender<()>,
    command_context: Option<CommandProcessorContext>,
}

impl ConnectionManager {
    /// Create a connection manager with an internally owned transport
    pub async fn new() -> NetworkResult<Self> {
        let transport = Arc::new(Transport::new().await?);
        Self::new_with_transport(transport).await
    }

    /// Create a connection manager that reuses an existing transport
    pub async fn new_with_transport(transport: Arc<Transport>) -> NetworkResult<Self> {
        let (shutdown_tx, _) = broadcast::channel(16);

        let mut manager = Self {
            transport,
            config: ConnectionConfig::default(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            address_map: Arc::new(RwLock::new(HashMap::new())),
            file_transfers: Arc::new(RwLock::new(HashMap::new())),
            message_routing_task: None,
            security: None,
            shutdown_tx,
            command_context: None,
        };

        manager.start_message_routing().await?;
        Ok(manager)
    }

    /// Configure the local endpoint preferences applied to subsequent connections.
    pub fn configure_local_endpoint(
        &mut self,
        local_player_id: u8,
        enable_compression: bool,
        enable_encryption: bool,
    ) {
        self.config.local_player_id = local_player_id;
        self.config.enable_compression = enable_compression;
        self.config.enable_encryption = enable_encryption;
    }

    /// Attach a shared security manager used for encryption and session tracking.
    pub fn set_command_context(&mut self, context: CommandProcessorContext) {
        self.command_context = Some(context);
    }

    pub fn set_security_manager(&mut self, security: Arc<SecurityManager>) {
        self.security = Some(security);
    }

    /// Add a new connection for the specified player.
    pub async fn add_connection(
        &self,
        player_id: u8,
        remote_addr: SocketAddr,
        protocol: TransportProtocol,
    ) -> NetworkResult<()> {
        if protocol != TransportProtocol::Quic && protocol != TransportProtocol::Udp {
            return Err(NetworkError::transport(
                "Only QUIC and UDP connections are supported",
            ));
        }

        {
            let connections = self.connections.read().await;
            if connections.contains_key(&player_id) {
                return Err(NetworkError::connection(format!(
                    "player {} already connected",
                    player_id
                )));
            }
        }

        // Ensure we have a QUIC connection ready before scheduling commands
        self.transport.connect(remote_addr).await?;

        let mut connection = Connection::with_config(
            player_id,
            remote_addr,
            protocol,
            self.transport.clone(),
            self.config.clone(),
            self.security.clone(),
        )
        .await?;

        if let Some(ctx) = &self.command_context {
            connection.set_command_context(ctx.clone());
        }

        connection.start().await?;
        let connection = Arc::new(connection);

        {
            let mut connections = self.connections.write().await;
            connections.insert(player_id, connection.clone());
        }

        {
            let mut addresses = self.address_map.write().await;
            addresses.insert(remote_addr, player_id);
        }

        info!(
            "Added connection for player {} at {}",
            player_id, remote_addr
        );
        Ok(())
    }

    /// Remove a connection.
    pub async fn remove_connection(&self, player_id: u8) -> NetworkResult<()> {
        let connection = {
            let mut connections = self.connections.write().await;
            connections.remove(&player_id)
        };

        if let Some(connection) = connection {
            self.address_map
                .write()
                .await
                .remove(&connection.remote_addr);
            if let Err(e) = connection.disconnect().await {
                warn!("Error disconnecting player {}: {}", player_id, e);
            }
            if let Some(ctx) = &self.command_context {
                ctx.remove_player(player_id).await;
            }
            info!("Removed connection for player {}", player_id);
        }

        Ok(())
    }

    /// Fetch a connection by player identifier.
    pub async fn get_connection(&self, player_id: u8) -> Option<Arc<Connection>> {
        let connections = self.connections.read().await;
        connections.get(&player_id).cloned()
    }

    /// Return the set of currently connected player identifiers.
    pub async fn player_ids(&self) -> Vec<u8> {
        let connections = self.connections.read().await;
        let mut ids: Vec<u8> = connections.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Resolve a player identifier for the provided socket address, if one is known.
    pub async fn player_id_for_addr(&self, addr: SocketAddr) -> Option<u8> {
        let map = self.address_map.read().await;
        map.get(&addr).copied()
    }

    /// Convenience helper to resolve a player's remote address when only the
    /// manager handle is available.
    pub async fn remote_addr_for_handle(
        handle: &Arc<RwLock<Self>>,
        player_id: u8,
    ) -> Option<SocketAddr> {
        let connections_arc = {
            let guard = handle.read().await;
            guard.connections.clone()
        };
        let connections = connections_arc.read().await;
        connections
            .get(&player_id)
            .map(|connection| connection.get_info().remote_addr)
    }

    /// Return the set of currently connected player identifiers for the provided handle.
    pub async fn player_ids_for(handle: &Arc<RwLock<Self>>) -> Vec<u8> {
        let connections_arc = {
            let guard = handle.read().await;
            guard.connections.clone()
        };
        let connections = connections_arc.read().await;
        let mut ids: Vec<u8> = connections.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Broadcast a keep-alive command to all active connections associated with the handle.
    pub async fn broadcast_keepalive_for(handle: &Arc<RwLock<Self>>) {
        let connections_arc = {
            let guard = handle.read().await;
            guard.connections.clone()
        };
        let snapshot = connections_arc.read().await;
        let peers: Vec<Arc<Connection>> = snapshot.values().cloned().collect();
        drop(snapshot);

        for connection in peers {
            let info = connection.get_info();
            let keep_alive = NetCommand::keep_alive(connection.local_player_id());
            if let Err(err) = connection.send_command(keep_alive).await {
                warn!(
                    "Failed to send keep-alive to player {} at {}: {}",
                    info.player_id, info.remote_addr, err
                );
            }
        }
    }

    /// Snapshot active transfers without requiring a direct mutable reference.
    pub async fn active_transfers_for(handle: &Arc<RwLock<Self>>) -> Vec<ManagedTransfer> {
        let transfers_arc = {
            let guard = handle.read().await;
            guard.file_transfers.clone()
        };
        let transfers = transfers_arc.read().await;
        transfers.values().cloned().collect()
    }

    /// Register a new transfer with the connection manager so higher-level systems
    /// can surface progress in telemetry or UI layers.
    pub async fn record_transfer_started(&self, progress: TransferProgress) {
        trace!(
            "Registering transfer {} {:?} ({} bytes)",
            progress.transfer_id,
            progress.direction,
            progress.metadata.file_size
        );
        let mut participants = Vec::new();
        if let Some(addr) = progress.peer {
            if let Some(player_id) = self.player_id_for_addr(addr).await {
                participants.push(player_id);
            }
        }

        let mut completion_status = HashMap::new();
        for participant in &participants {
            completion_status.insert(*participant, false);
        }

        let mut transfers = self.file_transfers.write().await;
        transfers.insert(
            progress.transfer_id,
            ManagedTransfer {
                progress,
                participants,
                completion_status,
                command_id: None,
                failure_reason: None,
            },
        );
    }

    /// Update progress information for a running transfer.
    pub async fn record_transfer_progress(&self, progress: TransferProgress) {
        trace!(
            "Transfer {} progress: {}/{}",
            progress.transfer_id,
            progress.bytes_transferred,
            progress.metadata.file_size
        );
        let mut transfers = self.file_transfers.write().await;
        if let Some(record) = transfers.get_mut(&progress.transfer_id) {
            record.progress = progress;
        } else {
            transfers.insert(
                progress.transfer_id,
                ManagedTransfer {
                    progress,
                    participants: Vec::new(),
                    completion_status: HashMap::new(),
                    command_id: None,
                    failure_reason: None,
                },
            );
        }
    }

    /// Mark a transfer as completed successfully.
    pub async fn record_transfer_completed(&self, progress: TransferProgress) {
        trace!("Transfer {} completed", progress.transfer_id);
        self.record_transfer_progress(progress.clone()).await;

        let mut transfers = self.file_transfers.write().await;
        if let Some(record) = transfers.get_mut(&progress.transfer_id) {
            record.progress.complete = true;
            for status in record.completion_status.values_mut() {
                *status = true;
            }
        }
    }

    /// Mark a transfer as failed and capture the failure reason.
    pub async fn record_transfer_failed(&self, progress: TransferProgress, reason: &str) {
        warn!(
            "Transfer {} {:?} failed: {}",
            progress.transfer_id, progress.direction, reason
        );
        let mut transfers = self.file_transfers.write().await;
        match transfers.get_mut(&progress.transfer_id) {
            Some(record) => {
                record.progress = progress;
                record.failure_reason = Some(reason.to_string());
                record.progress.complete = true;
            }
            None => {
                transfers.insert(
                    progress.transfer_id,
                    ManagedTransfer {
                        progress,
                        participants: Vec::new(),
                        completion_status: HashMap::new(),
                        command_id: None,
                        failure_reason: Some(reason.to_string()),
                    },
                );
            }
        }
    }

    /// Associate a transfer with its announcing command identifier and target participants.
    pub async fn tag_transfer(&self, transfer_id: Uuid, command_id: u16, participants: &[u8]) {
        let mut transfers = self.file_transfers.write().await;
        if let Some(record) = transfers.get_mut(&transfer_id) {
            record.command_id = Some(command_id);
            for participant in participants {
                if !record.participants.contains(participant) {
                    record.participants.push(*participant);
                }
                record
                    .completion_status
                    .entry(*participant)
                    .or_insert(false);
            }
        } else {
            trace!(
                "Delaying tag for transfer {} -> command {} until transfer starts",
                transfer_id,
                command_id
            );
        }
    }

    /// Snapshot of currently tracked file transfers for telemetry or diagnostics.
    pub async fn active_file_transfers(&self) -> Vec<ManagedTransfer> {
        let transfers = self.file_transfers.read().await;
        transfers.values().cloned().collect()
    }

    /// Resolve metadata for a specific transfer identifier.
    pub async fn transfer_for(&self, transfer_id: Uuid) -> Option<ManagedTransfer> {
        let transfers = self.file_transfers.read().await;
        transfers.get(&transfer_id).cloned()
    }

    /// True when all send/receive queues are empty across active connections.
    pub async fn queues_empty(&self) -> bool {
        let snapshot: Vec<Arc<Connection>> = {
            let guard = self.connections.read().await;
            guard.values().cloned().collect()
        };

        for connection in snapshot {
            if !connection.queues_empty().await {
                return false;
            }
        }
        true
    }

    /// Minimum observed latency across active connections, if any.
    pub async fn min_latency_ms(&self) -> Option<f64> {
        let snapshot: Vec<Arc<Connection>> = {
            let guard = self.connections.read().await;
            guard.values().cloned().collect()
        };

        let mut min: Option<f64> = None;
        for connection in snapshot {
            let latency = connection.quick_stats().await.current_latency_ms;
            min = Some(match min {
                Some(current) => current.min(latency),
                None => latency,
            });
        }
        min
    }

    /// Broadcast a command to all active connections.
    pub async fn broadcast_command(&self, command: NetCommand) -> NetworkResult<()> {
        let connections: Vec<Arc<Connection>> = {
            let guard = self.connections.read().await;
            guard.values().cloned().collect()
        };

        for connection in connections {
            if let Err(err) = connection.send_command(command.clone()).await {
                warn!(
                    "Failed to send command to player {}: {}",
                    connection.player_id, err
                );
            }
        }

        Ok(())
    }

    /// Send a command to players specified by a bitmask (bit n -> player n).
    pub async fn send_command_to_mask(&self, command: NetCommand, mask: u32) -> NetworkResult<()> {
        let targets: Vec<Arc<Connection>> = {
            let guard = self.connections.read().await;
            guard
                .iter()
                .filter_map(|(&player_id, connection)| {
                    if mask == 0 || (mask & (1u32 << player_id)) != 0 {
                        Some(connection.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        for connection in targets {
            if let Err(err) = connection.send_command(command.clone()).await {
                warn!(
                    "Failed to send command to player {}: {}",
                    connection.player_id, err
                );
            }
        }

        Ok(())
    }

    async fn start_message_routing(&mut self) -> NetworkResult<()> {
        let transport = self.transport.clone();
        let connections = self.connections.clone();
        let address_map = self.address_map.clone();
        let config = self.config.clone();
        let security = self.security.clone();
        let command_ctx = self.command_context.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let command_ctx = command_ctx;
            let mut interval = tokio::time::interval(Duration::from_millis(1));

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        debug!("Message routing task shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        match transport.receive_messages().await {
                            Ok(messages) => {
                                for message in messages {
                                    match message.source {
                                        Some(source_addr) => {
                                            let player_id = {
                                                let addresses = address_map.read().await;
                                                addresses.get(&source_addr).copied()
                                            };

                                            if let Some(player_id) = player_id {
                                                let connection = {
                                                    let guard = connections.read().await;
                                                    guard.get(&player_id).cloned()
                                                };

                                                if let Some(connection) = connection {
                                                    address_map.write().await.insert(source_addr, player_id);
                                                    if let Err(err) = connection.process_incoming_message(message).await {
                                                        warn!("Error processing message from {}: {}", source_addr, err);
                                                    }
                                                } else {
                                                    warn!("Received packet from {} but connection {} no longer exists", source_addr, player_id);
                                                }
                                            } else {
                                                match bincode::deserialize::<NetCommand>(&message.data) {
                                                    Ok(net_command) => {
                                                        let player_id = net_command.player_id;

                                                        let connection = {
                                                            let existing = {
                                                                let guard = connections.read().await;
                                                                guard.get(&player_id).cloned()
                                                            };

                                                            if let Some(connection) = existing {
                                                                Some(connection)
                                                            } else {
                                                                match Connection::with_config(
                                                                    player_id,
                                                                    source_addr,
                                                                    TransportProtocol::Quic,
                                                                    transport.clone(),
                                                                    config.clone(),
                                                                    security.clone(),
                                                                ).await {
                                                                    Ok(mut new_connection) => {
                                                                        if let Some(ctx) = &command_ctx {
                                                                            new_connection.set_command_context(ctx.clone());
                                                                        }
                                                                        if let Err(err) = new_connection.start().await {
                                                                            warn!("Failed to start inbound connection for player {}: {}", player_id, err);
                                                                            None
                                                                        } else {
                                                                            let connection = Arc::new(new_connection);
                                                                            let mut guard = connections.write().await;
                                                                            guard.insert(player_id, connection.clone());
                                                                            Some(connection)
                                                                        }
                                                                    }
                                                                    Err(err) => {
                                                                        warn!("Failed to create inbound connection for {}: {}", source_addr, err);
                                                                        None
                                                                    }
                                                                }
                                                            }
                                                        };

                                                        if let Some(connection) = connection {
                                                            address_map.write().await.insert(source_addr, player_id);
                                                            if let Err(err) = connection.process_incoming_message(message).await {
                                                                warn!("Error processing message from {}: {}", source_addr, err);
                                                            }
                                                        }
                                                    }
                                                    Err(err) => {
                                                        warn!("Failed to decode message from {}: {}", source_addr, err);
                                                    }
                                                }
                                            }
                                        }
                                        None => {
                                            debug!("Dropped message without source ({} bytes)", message.data.len());
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Error receiving transport messages: {}", err);
                            }
                        }
                    }
                }
            }
        });

        self.message_routing_task = Some(handle);
        Ok(())
    }

    /// Perform periodic upkeep such as pruning stale connections.
    pub async fn update(&self) -> NetworkResult<()> {
        let snapshot: Vec<(u8, Arc<Connection>)> = {
            let guard = self.connections.read().await;
            guard.iter().map(|(&id, conn)| (id, conn.clone())).collect()
        };

        for (player_id, connection) in snapshot {
            let state = connection.get_state().await;
            if matches!(
                state,
                ConnectionState::Error | ConnectionState::Disconnected
            ) {
                warn!("Removing stale connection for player {}", player_id);
                self.remove_connection(player_id).await?;
            }
        }

        Ok(())
    }

    /// Gracefully shutdown all connections and stop message routing.
    pub async fn shutdown_all(&mut self) -> NetworkResult<()> {
        let _ = self.shutdown_tx.send(());

        if let Some(handle) = self.message_routing_task.take() {
            if !handle.is_finished() {
                handle.abort();
            }
        }

        let connections: Vec<Arc<Connection>> = {
            let mut guard = self.connections.write().await;
            let values = guard.values().cloned().collect();
            guard.clear();
            values
        };

        self.address_map.write().await.clear();
        self.file_transfers.write().await.clear();

        for connection in connections {
            if let Err(err) = connection.disconnect().await {
                warn!(
                    "Error disconnecting player {}: {}",
                    connection.player_id, err
                );
            }
        }

        Ok(())
    }

    /// Reset the manager to an empty state and restart message routing.
    pub async fn reset(&mut self) -> NetworkResult<()> {
        self.shutdown_all().await?;
        self.start_message_routing().await?;
        Ok(())
    }

    /// Return number of active connections.
    pub async fn active_connections(&self) -> usize {
        let snapshot: Vec<Arc<Connection>> = {
            let guard = self.connections.read().await;
            guard.values().cloned().collect()
        };

        let mut count = 0;
        for connection in snapshot {
            if connection.is_active().await {
                count += 1;
            }
        }
        count
    }

    /// Lowest player id currently connected, if any.
    pub async fn lowest_player_id(&self) -> Option<u8> {
        let connections = self.connections.read().await;
        connections.keys().copied().min()
    }
}

impl Drop for ConnectionManager {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.message_routing_task.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
impl ConnectionManager {
    /// Register a peer address for tests without performing a full QUIC handshake.
    pub async fn register_test_peer(&self, player_id: u8, addr: SocketAddr) {
        self.address_map.write().await.insert(addr, player_id);
    }
}
