//! Enhanced connection manager with frame synchronization
//!
//! This module provides a comprehensive connection management system that handles
//! multiple player connections with frame-based synchronization, command queueing,
//! and reliable message delivery.

use crate::commands::NetCommand;
use crate::connection::pool::{ConnectionPool, PoolConfig};
use crate::connection::reliability::{ReliabilityConfig, ReliabilityLayer};
use crate::connection::state::{ConnectionStateMachine, DetailedConnectionState, TransitionReason};
use crate::connection::{Connection, ConnectionConfig};
use crate::error::{NetworkError, NetworkResult};
use crate::file_transfer::TransferDirection;
use crate::transport::{Transport, TransportProtocol};

use crate::time::NetworkInstant;
#[cfg(feature = "performance")]
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Frame synchronization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameSyncConfig {
    /// Target frames per second
    pub target_fps: u32,
    /// Frame execution timeout
    pub frame_timeout: Duration,
    /// Maximum frames to buffer ahead
    pub max_frame_buffer: u32,
    /// Minimum players required for frame execution
    pub min_players_for_execution: u32,
    /// Maximum frame drift before resync
    pub max_frame_drift: u32,
    /// Enable frame prediction
    pub enable_prediction: bool,
    /// Command batching size per frame
    pub commands_per_frame: usize,
}

impl Default for FrameSyncConfig {
    fn default() -> Self {
        Self {
            target_fps: 30,
            frame_timeout: Duration::from_millis(100),
            max_frame_buffer: 10,
            min_players_for_execution: 1,
            max_frame_drift: 5,
            enable_prediction: true,
            commands_per_frame: 64,
        }
    }
}

/// Per-player frame information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerFrameInfo {
    /// Player ID
    pub player_id: u8,
    /// Current frame number
    pub current_frame: u64,
    /// Commands pending for this frame
    pub pending_commands: Vec<NetCommand>,
    /// Last acknowledged frame
    pub last_ack_frame: u64,
    /// Frame execution latency
    pub frame_latency_ms: f64,
    /// Is player ready for next frame
    pub ready_for_next: bool,
}

/// Frame synchronization state
#[derive(Debug, Clone)]
pub struct FrameSync {
    /// Current global frame number
    current_frame: u64,
    /// Per-player frame information
    player_frames: HashMap<u8, PlayerFrameInfo>,
    /// Buffered commands by frame
    frame_buffer: HashMap<u64, Vec<NetCommand>>,
    /// Frame execution history
    execution_history: VecDeque<u64>,
    /// Configuration
    config: FrameSyncConfig,
}

impl FrameSync {
    fn new(config: FrameSyncConfig) -> Self {
        Self {
            current_frame: 0,
            player_frames: HashMap::new(),
            frame_buffer: HashMap::new(),
            execution_history: VecDeque::new(),
            config,
        }
    }

    /// Add a player to frame synchronization
    fn add_player(&mut self, player_id: u8) {
        self.player_frames.insert(
            player_id,
            PlayerFrameInfo {
                player_id,
                current_frame: self.current_frame,
                pending_commands: Vec::new(),
                last_ack_frame: 0,
                frame_latency_ms: 0.0,
                ready_for_next: true,
            },
        );
        debug!(
            "Added player {} to frame sync at frame {}",
            player_id, self.current_frame
        );
    }

    /// Remove a player from frame synchronization
    fn remove_player(&mut self, player_id: u8) {
        self.player_frames.remove(&player_id);
        debug!("Removed player {} from frame sync", player_id);
    }

    /// Add command to frame buffer
    fn add_command(&mut self, frame: u64, command: NetCommand) {
        let commands = self.frame_buffer.entry(frame).or_insert_with(Vec::new);
        commands.push(command);

        // Limit buffer size to prevent memory growth
        if commands.len() > self.config.commands_per_frame {
            warn!(
                "Frame {} command buffer overflow, dropping oldest commands",
                frame
            );
            commands.drain(0..commands.len() - self.config.commands_per_frame);
        }
    }

    /// Check if frame is ready for execution
    fn is_frame_ready(&self, frame: u64) -> bool {
        if self.player_frames.is_empty() {
            return false;
        }

        // Check if minimum number of players are ready
        let ready_players = self
            .player_frames
            .values()
            .filter(|info| info.current_frame >= frame)
            .count();

        ready_players >= self.config.min_players_for_execution as usize
    }

    /// Get commands for frame execution
    fn get_frame_commands(&mut self, frame: u64) -> Vec<NetCommand> {
        self.frame_buffer.remove(&frame).unwrap_or_default()
    }

    /// Advance to next frame
    fn advance_frame(&mut self) -> u64 {
        self.current_frame += 1;

        // Clean old frame buffer entries
        let cutoff = self
            .current_frame
            .saturating_sub(self.config.max_frame_buffer as u64);
        self.frame_buffer.retain(|&frame, _| frame >= cutoff);

        // Update execution history
        self.execution_history.push_back(self.current_frame);
        if self.execution_history.len() > 100 {
            self.execution_history.pop_front();
        }

        self.current_frame
    }

    /// Update player frame info
    fn update_player_frame(&mut self, player_id: u8, frame: u64) {
        if let Some(info) = self.player_frames.get_mut(&player_id) {
            info.current_frame = frame;
            info.ready_for_next = true;
        }
    }

    /// Get synchronization statistics
    fn get_sync_stats(&self) -> FrameSyncStats {
        let min_frame = self
            .player_frames
            .values()
            .map(|info| info.current_frame)
            .min()
            .unwrap_or(self.current_frame);

        let max_frame = self
            .player_frames
            .values()
            .map(|info| info.current_frame)
            .max()
            .unwrap_or(self.current_frame);

        let avg_latency = if !self.player_frames.is_empty() {
            self.player_frames
                .values()
                .map(|info| info.frame_latency_ms)
                .sum::<f64>()
                / self.player_frames.len() as f64
        } else {
            0.0
        };

        FrameSyncStats {
            current_frame: self.current_frame,
            min_player_frame: min_frame,
            max_player_frame: max_frame,
            frame_drift: max_frame.saturating_sub(min_frame),
            average_latency_ms: avg_latency,
            buffered_frames: self.frame_buffer.len(),
            active_players: self.player_frames.len(),
        }
    }
}

/// Frame synchronization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameSyncStats {
    pub current_frame: u64,
    pub min_player_frame: u64,
    pub max_player_frame: u64,
    pub frame_drift: u64,
    pub average_latency_ms: f64,
    pub buffered_frames: usize,
    pub active_players: usize,
}

/// File transfer state tracking
#[derive(Debug, Clone)]
pub struct FileTransferState {
    /// Transfer ID
    pub transfer_id: Uuid,
    /// File name
    pub filename: String,
    /// Total file size
    pub total_size: u64,
    /// Bytes transferred
    pub transferred: u64,
    /// Transfer start time
    pub started_at: NetworkInstant,
    /// Last time we received an update for this transfer
    pub last_update: NetworkInstant,
    /// Direction of transfer relative to the local peer
    pub direction: TransferDirection,
    /// Remote peer socket address, if known
    pub peer: Option<SocketAddr>,
    /// Players participating in transfer
    pub participants: Vec<u8>,
    /// Transfer completion status per player
    pub completion_status: HashMap<u8, bool>,
}

/// Disconnect vote tracking
#[derive(Debug, Clone)]
pub struct DisconnectVote {
    /// Player being voted to disconnect
    pub target_player: u8,
    /// Players who voted
    pub voters: Vec<u8>,
    /// Vote start time
    pub started_at: NetworkInstant,
    /// Vote timeout
    pub timeout: Duration,
    /// Required votes for disconnect
    pub required_votes: usize,
}

/// Enhanced connection manager with full game networking features
pub struct EnhancedConnectionManager {
    /// Transport layer
    transport: Arc<Transport>,

    /// Connection pool for efficient resource management
    connection_pool: ConnectionPool,

    /// Active connections
    #[cfg(feature = "performance")]
    connections: Arc<DashMap<u8, Arc<Connection>>>,
    #[cfg(not(feature = "performance"))]
    connections: Arc<RwLock<HashMap<u8, Arc<Connection>>>>,

    /// Connection state machines
    connection_states: Arc<RwLock<HashMap<u8, ConnectionStateMachine>>>,

    /// Reliability layer
    reliability: ReliabilityLayer,

    /// Frame synchronization
    frame_sync: Arc<RwLock<FrameSync>>,

    /// Configuration
    config: ConnectionConfig,
    frame_config: FrameSyncConfig,

    /// File transfers
    active_transfers: Arc<RwLock<HashMap<Uuid, FileTransferState>>>,

    /// Disconnect votes
    disconnect_votes: Arc<RwLock<HashMap<u8, DisconnectVote>>>,

    /// Statistics
    global_stats: Arc<RwLock<ManagerStats>>,

    /// Message routing and processing
    command_queue: Arc<RwLock<VecDeque<(u8, NetCommand)>>>,
    frame_timer: Option<JoinHandle<()>>,
    message_processor: Option<JoinHandle<()>>,

    /// Control channels
    shutdown_tx: broadcast::Sender<()>,
    command_tx: mpsc::Sender<NetCommand>,
    command_rx: Arc<RwLock<mpsc::Receiver<NetCommand>>>,

    /// Semaphores for rate limiting
    connection_semaphore: Arc<Semaphore>,
    command_semaphore: Arc<Semaphore>,

    /// Frame counter
    frame_counter: AtomicU64,
    last_frame_time: Arc<RwLock<NetworkInstant>>,
}

/// Manager-wide statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManagerStats {
    /// Total connections created
    pub total_connections: u64,
    /// Current active connections
    pub active_connections: u32,
    /// Total messages processed
    pub messages_processed: u64,
    /// Total frames executed
    pub frames_executed: u64,
    /// Average frame time
    pub average_frame_time_ms: f64,
    /// Commands per second
    pub commands_per_second: f64,
    /// Total file transfers
    pub file_transfers: u64,
    /// Total disconnect votes
    pub disconnect_votes: u64,
}

impl EnhancedConnectionManager {
    /// Create new enhanced connection manager
    pub async fn new(transport: Arc<Transport>) -> NetworkResult<Self> {
        Self::with_configs(
            transport,
            ConnectionConfig::default(),
            FrameSyncConfig::default(),
            PoolConfig::default(),
        )
        .await
    }

    /// Create with custom configurations
    pub async fn with_configs(
        transport: Arc<Transport>,
        config: ConnectionConfig,
        frame_config: FrameSyncConfig,
        pool_config: PoolConfig,
    ) -> NetworkResult<Self> {
        info!("Creating enhanced connection manager");

        let (shutdown_tx, _) = broadcast::channel(16);
        let (command_tx, command_rx) = mpsc::channel(10000);

        let connection_pool = ConnectionPool::with_config(transport.clone(), pool_config);
        let reliability = ReliabilityLayer::with_config(ReliabilityConfig::default());

        let manager = Self {
            transport,
            connection_pool,
            #[cfg(feature = "performance")]
            connections: Arc::new(DashMap::new()),
            #[cfg(not(feature = "performance"))]
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_states: Arc::new(RwLock::new(HashMap::new())),
            reliability,
            frame_sync: Arc::new(RwLock::new(FrameSync::new(frame_config.clone()))),
            config,
            frame_config,
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            disconnect_votes: Arc::new(RwLock::new(HashMap::new())),
            global_stats: Arc::new(RwLock::new(ManagerStats::default())),
            command_queue: Arc::new(RwLock::new(VecDeque::new())),
            frame_timer: None,
            message_processor: None,
            shutdown_tx,
            command_tx,
            command_rx: Arc::new(RwLock::new(command_rx)),
            connection_semaphore: Arc::new(Semaphore::new(8)), // Max 8 players
            command_semaphore: Arc::new(Semaphore::new(1000)),
            frame_counter: AtomicU64::new(0),
            last_frame_time: Arc::new(RwLock::new(NetworkInstant::now())),
        };

        Ok(manager)
    }

    /// Start the connection manager
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Starting enhanced connection manager");

        // Start connection pool
        // Note: We need to make connection_pool mutable for this
        // In a real implementation, we'd need to restructure the ownership

        // Start frame synchronization timer
        self.start_frame_timer().await?;

        // Start message processor
        self.start_message_processor().await?;

        info!("Enhanced connection manager started successfully");
        Ok(())
    }

    /// Add a new connection
    pub async fn add_connection(
        &self,
        player_id: u8,
        remote_addr: SocketAddr,
        protocol: TransportProtocol,
    ) -> NetworkResult<()> {
        // Acquire connection semaphore
        let _permit = self
            .connection_semaphore
            .acquire()
            .await
            .map_err(|_| NetworkError::connection("connection semaphore closed"))?;

        info!(
            "Adding connection for player {} at {}",
            player_id, remote_addr
        );

        // Check if player already connected
        #[cfg(feature = "performance")]
        let already_exists = self.connections.contains_key(&player_id);
        #[cfg(not(feature = "performance"))]
        let already_exists = {
            let connections = self.connections.read().await;
            connections.contains_key(&player_id)
        };

        if already_exists {
            return Err(NetworkError::connection(format!(
                "player {} already connected",
                player_id
            )));
        }

        // Create connection using pool
        let connection_arc = self
            .connection_pool
            .get_connection(remote_addr, protocol)
            .await?;

        // Initialize state machine
        {
            let mut states = self.connection_states.write().await;
            let mut state_machine = ConnectionStateMachine::new();
            state_machine.transition_to(
                DetailedConnectionState::ConnectingInitiate,
                TransitionReason::UserAction,
            )?;
            states.insert(player_id, state_machine);
        }

        // Add to frame sync
        {
            let mut frame_sync = self.frame_sync.write().await;
            frame_sync.add_player(player_id);
        }

        // Store connection
        #[cfg(feature = "performance")]
        self.connections.insert(player_id, connection_arc);
        #[cfg(not(feature = "performance"))]
        {
            let mut connections = self.connections.write().await;
            connections.insert(player_id, connection_arc);
        }

        // Update statistics
        {
            let mut stats = self.global_stats.write().await;
            stats.total_connections += 1;
            stats.active_connections += 1;
        }

        info!("Successfully added connection for player {}", player_id);
        Ok(())
    }

    /// Remove a connection
    pub async fn remove_connection(&self, player_id: u8) -> NetworkResult<()> {
        info!("Removing connection for player {}", player_id);

        // Get and remove connection
        #[cfg(feature = "performance")]
        let connection = self.connections.remove(&player_id).map(|(_, v)| v);
        #[cfg(not(feature = "performance"))]
        let connection = {
            let mut connections = self.connections.write().await;
            connections.remove(&player_id)
        };

        if let Some(conn) = connection {
            // Graceful disconnect
            if let Err(e) = conn.disconnect().await {
                warn!(
                    "Error during graceful disconnect for player {}: {}",
                    player_id, e
                );
            }

            // Return to pool
            let info = conn.get_info();
            if let Err(e) = self
                .connection_pool
                .return_connection(info.remote_addr)
                .await
            {
                warn!("Error returning connection to pool: {}", e);
            }
        }

        // Remove from frame sync
        {
            let mut frame_sync = self.frame_sync.write().await;
            frame_sync.remove_player(player_id);
        }

        // Remove state machine
        {
            let mut states = self.connection_states.write().await;
            states.remove(&player_id);
        }

        // Update statistics
        {
            let mut stats = self.global_stats.write().await;
            stats.active_connections = stats.active_connections.saturating_sub(1);
        }

        info!("Successfully removed connection for player {}", player_id);
        Ok(())
    }

    /// Send command to specific player
    pub async fn send_command(&self, player_id: u8, command: NetCommand) -> NetworkResult<()> {
        #[cfg(feature = "performance")]
        let connection = self.connections.get(&player_id);
        #[cfg(not(feature = "performance"))]
        let connection = {
            let connections = self.connections.read().await;
            connections.get(&player_id).cloned()
        };

        if let Some(conn) = connection {
            #[cfg(feature = "performance")]
            let conn = conn.value().clone();

            conn.send_command(command).await?;
        } else {
            return Err(NetworkError::connection(format!(
                "player {} not connected",
                player_id
            )));
        }

        Ok(())
    }

    /// Broadcast command to all players
    pub async fn broadcast_command(&self, command: NetCommand) -> NetworkResult<()> {
        let mut errors = Vec::new();

        #[cfg(feature = "performance")]
        {
            for entry in self.connections.iter() {
                let player_id = *entry.key();
                let connection = entry.value().clone();

                if let Err(e) = connection.send_command(command.clone()).await {
                    errors.push((player_id, e));
                }
            }
        }
        #[cfg(not(feature = "performance"))]
        {
            let connections = self.connections.read().await;
            for (&player_id, connection) in connections.iter() {
                if let Err(e) = connection.send_command(command.clone()).await {
                    errors.push((player_id, e));
                }
            }
        }

        if !errors.is_empty() {
            warn!("Broadcast errors: {:?}", errors);
        }

        Ok(())
    }

    /// Process frame synchronization
    async fn process_frame_sync(&self) -> NetworkResult<()> {
        let current_frame = self.frame_counter.fetch_add(1, Ordering::Relaxed);

        // Get commands for current frame
        let commands = {
            let mut frame_sync = self.frame_sync.write().await;
            if frame_sync.is_frame_ready(current_frame) {
                frame_sync.get_frame_commands(current_frame)
            } else {
                Vec::new()
            }
        };

        if !commands.is_empty() {
            debug!(
                "Executing frame {} with {} commands",
                current_frame,
                commands.len()
            );

            // Process commands for this frame
            for command in commands {
                // Add to command queue for processing
                {
                    let mut queue = self.command_queue.write().await;
                    queue.push_back((command.player_id, command));
                }
            }

            // Update frame sync
            {
                let mut frame_sync = self.frame_sync.write().await;
                frame_sync.advance_frame();
            }

            // Update statistics
            {
                let mut stats = self.global_stats.write().await;
                stats.frames_executed += 1;

                let frame_time = {
                    let mut last_time = self.last_frame_time.write().await;
                    let now = NetworkInstant::now();
                    let elapsed = now.duration_since(*last_time).as_millis() as f64;
                    *last_time = now;
                    elapsed
                };

                // Update average frame time
                if stats.average_frame_time_ms == 0.0 {
                    stats.average_frame_time_ms = frame_time;
                } else {
                    stats.average_frame_time_ms =
                        stats.average_frame_time_ms * 0.9 + frame_time * 0.1;
                }
            }
        }

        Ok(())
    }

    /// Start frame synchronization timer
    async fn start_frame_timer(&mut self) -> NetworkResult<()> {
        let frame_config = self.frame_config.clone();
        let frame_sync = self.frame_sync.clone();
        let frame_counter = Arc::new(AtomicU64::new(0));
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let frame_duration = Duration::from_millis(1000 / frame_config.target_fps as u64);
            let mut interval = interval(frame_duration);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let current_frame = frame_counter.fetch_add(1, Ordering::Relaxed);

                        // Check frame readiness and process
                        {
                            let frame_sync_guard = frame_sync.read().await;
                            if frame_sync_guard.is_frame_ready(current_frame) {
                                trace!("Frame {} ready for execution", current_frame);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Frame timer shutting down");
                        break;
                    }
                }
            }
        });

        self.frame_timer = Some(handle);
        Ok(())
    }

    /// Start message processor
    async fn start_message_processor(&mut self) -> NetworkResult<()> {
        let command_queue = self.command_queue.clone();
        let global_stats = self.global_stats.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(1));
            let mut last_command_count = 0u64;
            let mut last_stats_update = NetworkInstant::now();

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Process queued commands
                        let commands_to_process = {
                            let mut queue = command_queue.write().await;
                            let mut commands = Vec::new();

                            // Process up to 100 commands per tick to avoid blocking
                            for _ in 0..100 {
                                if let Some((player_id, command)) = queue.pop_front() {
                                    commands.push((player_id, command));
                                } else {
                                    break;
                                }
                            }

                            commands
                        };

                        if !commands_to_process.is_empty() {
                            let commands_count = commands_to_process.len();
                            // Process commands
                            for (player_id, command) in commands_to_process {
                                trace!("Processing command from player {}: {:?}", player_id, command.command_type);
                                // In real implementation, this would execute game logic
                            }

                            // Update statistics
                            {
                                let mut stats = global_stats.write().await;
                                stats.messages_processed += commands_count as u64;

                                // Update commands per second periodically
                                let now = NetworkInstant::now();
                                if now.duration_since(last_stats_update).as_secs() >= 1 {
                                    let commands_this_period = stats.messages_processed - last_command_count;
                                    let elapsed = now.duration_since(last_stats_update).as_secs_f64();
                                    stats.commands_per_second = commands_this_period as f64 / elapsed;

                                    last_command_count = stats.messages_processed;
                                    last_stats_update = now;
                                }
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Message processor shutting down");
                        break;
                    }
                }
            }
        });

        self.message_processor = Some(handle);
        Ok(())
    }

    /// Get comprehensive manager statistics
    pub async fn get_stats(&self) -> ManagerStats {
        let mut stats = self.global_stats.read().await.clone();

        // Update real-time stats
        #[cfg(feature = "performance")]
        {
            stats.active_connections = self.connections.len() as u32;
        }
        #[cfg(not(feature = "performance"))]
        {
            let connections = self.connections.read().await;
            stats.active_connections = connections.len() as u32;
        }

        stats
    }

    /// Get frame synchronization statistics
    pub async fn get_frame_sync_stats(&self) -> FrameSyncStats {
        let frame_sync = self.frame_sync.read().await;
        frame_sync.get_sync_stats()
    }

    /// Shutdown the connection manager
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down enhanced connection manager");

        // Send shutdown signal
        if let Err(e) = self.shutdown_tx.send(()) {
            warn!("Failed to send shutdown signal: {}", e);
        }

        // Wait for background tasks
        if let Some(handle) = self.frame_timer.take() {
            handle.abort();
            let _ = handle.await;
        }

        if let Some(handle) = self.message_processor.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Disconnect all connections
        #[cfg(feature = "performance")]
        {
            for entry in self.connections.iter() {
                let connection = entry.value();
                if let Err(e) = connection.disconnect().await {
                    warn!("Error disconnecting player {}: {}", entry.key(), e);
                }
            }
            self.connections.clear();
        }
        #[cfg(not(feature = "performance"))]
        {
            let mut connections = self.connections.write().await;
            for (player_id, connection) in connections.drain() {
                if let Err(e) = connection.disconnect().await {
                    warn!("Error disconnecting player {}: {}", player_id, e);
                }
            }
        }

        info!("Enhanced connection manager shutdown complete");
        Ok(())
    }

    // =========================================================================
    // C++ Parity Methods - ConnectionManager.cpp behavioral compatibility
    // =========================================================================

    /// Check if a player is connected
    /// Matches C++ ConnectionManager::isPlayerConnected()
    pub async fn is_player_connected(&self, player_id: u8) -> bool {
        #[cfg(feature = "performance")]
        {
            if player_id as usize == self.config.local_slot {
                return true;
            }
            if let Some(conn) = self.connections.get(&player_id) {
                return !conn.is_quitting().await;
            }
            false
        }
        #[cfg(not(feature = "performance"))]
        {
            let connections = self.connections.read().await;
            if player_id as usize == self.config.local_slot {
                return true;
            }
            if let Some(c) = connections.get(&player_id) {
                return !c.is_quitting().await;
            }
            false
        }
    }

    /// Zero out command counts for the given frames
    /// Used at game start since there won't be commands for first few frames due to runahead
    /// Matches C++ ConnectionManager::zeroFrames()
    pub async fn zero_frames(&self, starting_frame: u32, num_frames: u32) {
        let mut frame_sync = self.frame_sync.write().await;
        for frame in starting_frame..(starting_frame + num_frames) {
            frame_sync.frame_buffer.remove(&(frame as u64));
        }
    }

    /// Destroy any game messages left over due to run ahead
    /// Matches C++ ConnectionManager::destroyGameMessages()
    pub async fn destroy_game_messages(&self) {
        let mut frame_sync = self.frame_sync.write().await;
        frame_sync.frame_buffer.clear();
    }

    /// Send local command with relay mask
    /// Matches C++ ConnectionManager::sendLocalCommand()
    pub async fn send_local_command(
        &self,
        command: NetCommand,
        relay_mask: u8,
    ) -> NetworkResult<()> {
        // Send to all players in the relay mask
        for player_id in 0..8u8 {
            if (relay_mask & (1 << player_id)) != 0 {
                self.send_command(player_id, command.clone()).await?;
            }
        }
        Ok(())
    }

    /// Check if all commands for a frame are ready
    /// Matches C++ ConnectionManager::allCommandsReady()
    pub async fn all_commands_ready(&self, frame: u32) -> bool {
        let frame_sync = self.frame_sync.read().await;
        frame_sync.is_frame_ready(frame as u64)
    }

    /// Get frame command list for a specific frame
    /// Matches C++ ConnectionManager::getFrameCommandList()
    pub async fn get_frame_command_list(&self, frame: u32) -> Vec<NetCommand> {
        let mut frame_sync = self.frame_sync.write().await;
        frame_sync
            .frame_buffer
            .remove(&(frame as u64))
            .unwrap_or_default()
    }

    /// Process a network command
    /// Returns true if the command was processed and should not be relayed
    /// Matches C++ ConnectionManager::processNetCommand()
    pub async fn process_net_command(&self, command: &NetCommand) -> NetworkResult<bool> {
        use crate::commands::NetCommandType;

        // Handle ACK commands
        if matches!(
            command.command_type,
            NetCommandType::AckStage1 | NetCommandType::AckStage2 | NetCommandType::AckBoth
        ) {
            self.process_ack(command).await?;
            return Ok(false); // Should still relay
        }

        // Check if player is still connected
        if !self.is_player_connected(command.player_id).await {
            return Ok(true); // Don't relay, player left
        }

        // Handle wrapper commands
        if command.command_type == NetCommandType::Wrapper {
            self.process_wrapper(command).await?;
            return Ok(false);
        }

        // Handle frame info
        if command.command_type == NetCommandType::FrameInfo {
            self.process_frame_info(command).await?;
            return Ok(false);
        }

        // Handle progress commands
        if command.command_type == NetCommandType::Progress {
            self.process_progress(command).await?;
            return Ok(true);
        }

        // Handle keep-alive
        if command.command_type == NetCommandType::KeepAlive {
            return Ok(true);
        }

        // Handle run-ahead metrics
        if command.command_type == NetCommandType::RunAheadMetrics {
            self.process_run_ahead_metrics(command).await?;
            return Ok(true);
        }

        // Handle chat
        if command.command_type == NetCommandType::Chat {
            self.process_chat(command).await?;
            return Ok(false);
        }

        // Handle disconnect chat
        if command.command_type == NetCommandType::DisconnectChat {
            self.process_disconnect_chat(command).await?;
            return Ok(true);
        }

        // Handle load complete
        if command.command_type == NetCommandType::LoadComplete {
            self.process_load_complete(command).await?;
            return Ok(false);
        }

        // Handle file transfer commands
        if command.command_type == NetCommandType::File {
            self.process_file(command).await?;
            return Ok(false);
        }

        if command.command_type == NetCommandType::FileAnnounce {
            self.process_file_announce(command).await?;
            return Ok(false);
        }

        if command.command_type == NetCommandType::FileProgress {
            self.process_file_progress(command).await?;
            return Ok(false);
        }

        if command.command_type == NetCommandType::FrameResendRequest {
            self.process_frame_resend_request(command).await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Process acknowledgment command
    async fn process_ack(&self, _command: &NetCommand) -> NetworkResult<()> {
        // Ack processing - remove from connection's command list
        // This is handled by the reliability layer
        Ok(())
    }

    /// Process frame info command
    async fn process_frame_info(&self, command: &NetCommand) -> NetworkResult<()> {
        let mut frame_sync = self.frame_sync.write().await;
        if let Some(info) = frame_sync.player_frames.get_mut(&command.player_id) {
            info.last_ack_frame = command.execution_frame as u64;
            info.ready_for_next = true;
        }
        Ok(())
    }

    /// Process progress command
    async fn process_progress(&self, command: &NetCommand) -> NetworkResult<()> {
        use crate::commands::CommandPayload;
        if let CommandPayload::Progress(data) = &command.payload {
            debug!(
                "Progress from player {}: {}%",
                command.player_id, data.percentage
            );
        }
        Ok(())
    }

    /// Process run-ahead metrics command
    async fn process_run_ahead_metrics(&self, command: &NetCommand) -> NetworkResult<()> {
        // Store latency and FPS averages for this player
        debug!("Run-ahead metrics from player {}", command.player_id);
        Ok(())
    }

    /// Process chat command
    async fn process_chat(&self, command: &NetCommand) -> NetworkResult<()> {
        use crate::commands::CommandPayload;
        if let CommandPayload::Chat(data) = &command.payload {
            debug!("Chat from player {}: {}", command.player_id, data.message);
            // In full implementation, this would display in-game chat
        }
        Ok(())
    }

    /// Process disconnect chat command
    async fn process_disconnect_chat(&self, command: &NetCommand) -> NetworkResult<()> {
        use crate::commands::CommandPayload;
        if let CommandPayload::Chat(data) = &command.payload {
            debug!(
                "Disconnect chat from player {}: {}",
                command.player_id, data.message
            );
        }
        Ok(())
    }

    /// Process load complete command
    async fn process_load_complete(&self, command: &NetCommand) -> NetworkResult<()> {
        debug!("Load complete from player {}", command.player_id);
        Ok(())
    }

    /// Process wrapper command (for large commands split across packets)
    async fn process_wrapper(&self, _command: &NetCommand) -> NetworkResult<()> {
        // Wrapper commands are handled by the wrapper reassembler
        Ok(())
    }

    /// Process file transfer command
    async fn process_file(&self, command: &NetCommand) -> NetworkResult<()> {
        use crate::commands::CommandPayload;
        if let CommandPayload::FileTransfer(data) = &command.payload {
            debug!(
                "File transfer from player {}: {} ({} bytes)",
                command.player_id,
                data.filename,
                data.data.len()
            );

            // Store in active transfers
            let transfer_id = uuid::Uuid::new_v4();
            let state = FileTransferState {
                transfer_id,
                filename: data.filename.clone(),
                total_size: data.data.len() as u64,
                transferred: data.data.len() as u64,
                started_at: NetworkInstant::now(),
                last_update: NetworkInstant::now(),
                direction: TransferDirection::Download,
                peer: None,
                participants: vec![command.player_id],
                completion_status: [(command.player_id, true)].into_iter().collect(),
            };

            let mut transfers = self.active_transfers.write().await;
            transfers.insert(transfer_id, state);
        }
        Ok(())
    }

    /// Process file announcement command
    async fn process_file_announce(&self, command: &NetCommand) -> NetworkResult<()> {
        use crate::commands::CommandPayload;
        if let CommandPayload::FileAnnouncement(data) = &command.payload {
            debug!(
                "File announcement from player {}: {} ({} bytes)",
                command.player_id, data.metadata.filename, data.metadata.file_size
            );
        }
        Ok(())
    }

    /// Process file progress command
    async fn process_file_progress(&self, command: &NetCommand) -> NetworkResult<()> {
        use crate::commands::CommandPayload;
        if let CommandPayload::FileProgress(data) = &command.payload {
            debug!(
                "File progress from player {}: file {} at {}%",
                command.player_id, data.file_id, data.progress
            );
        }
        Ok(())
    }

    /// Process frame resend request
    async fn process_frame_resend_request(&self, command: &NetCommand) -> NetworkResult<()> {
        debug!("Frame resend request from player {}", command.player_id);
        // Would resend frame data to requesting player
        Ok(())
    }

    /// Vote for player disconnect
    /// Matches C++ ConnectionManager::voteForPlayerDisconnect()
    pub async fn vote_for_player_disconnect(&self, target_player: u8) -> NetworkResult<()> {
        let mut votes = self.disconnect_votes.write().await;

        let vote = votes
            .entry(target_player)
            .or_insert_with(|| DisconnectVote {
                target_player,
                voters: Vec::new(),
                started_at: NetworkInstant::now(),
                timeout: Duration::from_secs(30),
                required_votes: 2, // Need majority
            });

        vote.voters.push(self.config.local_slot as u8);

        // Check if we have enough votes
        if vote.voters.len() >= vote.required_votes {
            info!("Enough votes to disconnect player {}", target_player);
        }

        Ok(())
    }

    /// Disconnect a player
    /// Matches C++ ConnectionManager::disconnectPlayer()
    pub async fn disconnect_player(&self, player_id: u8) -> NetworkResult<()> {
        info!("Disconnecting player {}", player_id);
        self.remove_connection(player_id).await
    }

    /// Quit the game (disconnect everyone)
    /// Matches C++ ConnectionManager::quitGame()
    pub async fn quit_game(&mut self) -> NetworkResult<()> {
        info!("Quitting game");
        self.shutdown().await
    }

    /// Get the local player ID
    /// Matches C++ ConnectionManager::getLocalPlayerID()
    pub fn get_local_player_id(&self) -> u8 {
        self.config.local_slot as u8
    }

    /// Check if this instance is the packet router
    /// Matches C++ ConnectionManager::isPacketRouter()
    pub fn is_packet_router(&self) -> bool {
        self.config.is_packet_router
    }

    /// Get packet arrival cushion (for timing calculations)
    /// Matches C++ ConnectionManager::getPacketArrivalCushion()
    pub async fn get_packet_arrival_cushion(&self) -> u32 {
        let frame_sync = self.frame_sync.read().await;
        let stats = frame_sync.get_sync_stats();
        // Return frame drift as cushion
        stats.frame_drift as u32
    }

    /// Update frame grouping
    /// Matches C++ ConnectionManager::setFrameGrouping()
    pub fn set_frame_grouping(&mut self, frame_grouping_ms: u64) {
        self.frame_config.frame_timeout = Duration::from_millis(frame_grouping_ms);
    }

    /// Send file to players
    /// Matches C++ ConnectionManager::sendFile()
    pub async fn send_file(
        &self,
        path: &str,
        player_mask: u8,
        command_id: u16,
    ) -> NetworkResult<()> {
        use crate::commands::{CommandPayload, FileTransferData};

        let payload = CommandPayload::FileTransfer(FileTransferData {
            filename: path.to_string(),
            data: Vec::new(), // Would contain actual file data
            file_id: command_id as u32,
            chunk_number: 0,
            total_chunks: 1,
            checksum: 0,
        });

        let command = NetCommand::new(
            crate::commands::NetCommandType::File,
            self.config.local_slot as u8,
            0,
            payload,
        );

        self.send_local_command(command, player_mask).await
    }

    /// Send file announcement
    /// Matches C++ ConnectionManager::sendFileAnnounce()
    pub async fn send_file_announce(&self, path: &str, player_mask: u8) -> NetworkResult<u16> {
        use crate::commands::{CommandPayload, FileAnnouncementData};
        use crate::file_transfer::{FileMetadata, TransferType};

        let command_id = crate::net_command_messages::generate_next_command_id();

        let metadata = FileMetadata {
            filename: path.to_string(),
            file_size: 0, // Would read actual file size
            checksum: [0u8; 32],
            transfer_type: TransferType::Map,
        };

        let payload = CommandPayload::FileAnnouncement(FileAnnouncementData {
            metadata,
            command_id,
            player_mask,
        });

        let command = NetCommand::new(
            crate::commands::NetCommandType::FileAnnounce,
            self.config.local_slot as u8,
            0,
            payload,
        );

        self.send_local_command(command, player_mask).await?;
        Ok(command_id)
    }

    /// Get file transfer progress
    /// Matches C++ ConnectionManager::getFileTransferProgress()
    pub async fn get_file_transfer_progress(&self, player_id: u8, _path: &str) -> i32 {
        let transfers = self.active_transfers.read().await;
        for (_, state) in transfers.iter() {
            if state.participants.contains(&player_id) {
                return ((state.transferred as f64 / state.total_size as f64) * 100.0) as i32;
            }
        }
        0
    }

    /// Check if all queues are empty
    /// Matches C++ ConnectionManager::areAllQueuesEmpty()
    pub async fn are_all_queues_empty(&self) -> bool {
        let queue = self.command_queue.read().await;
        queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_frame_sync_creation() {
        let config = FrameSyncConfig::default();
        let mut frame_sync = FrameSync::new(config);

        assert_eq!(frame_sync.current_frame, 0);
        assert!(frame_sync.player_frames.is_empty());

        frame_sync.add_player(0);
        assert_eq!(frame_sync.player_frames.len(), 1);

        let stats = frame_sync.get_sync_stats();
        assert_eq!(stats.active_players, 1);
    }

    #[tokio::test]
    async fn test_frame_advancement() {
        let config = FrameSyncConfig::default();
        let mut frame_sync = FrameSync::new(config);

        frame_sync.add_player(0);
        frame_sync.add_player(1);

        let frame1 = frame_sync.advance_frame();
        assert_eq!(frame1, 1);

        let frame2 = frame_sync.advance_frame();
        assert_eq!(frame2, 2);

        let stats = frame_sync.get_sync_stats();
        assert_eq!(stats.current_frame, 2);
    }

    #[tokio::test]
    #[ignore] // Requires proper transport setup
    async fn test_enhanced_manager_creation() {
        // This test would require a proper transport implementation
        // For now, it's ignored as it depends on network setup
    }
}
