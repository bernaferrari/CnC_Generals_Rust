//! Network bridge connecting the transport layer to GameLogic.
//!
//! The `NetworkBridge` sits between the low-level transport (QUIC / UDP)
//! and the game's logic layer.  It:
//!
//! - Deserializes incoming transport messages into `crate::commands::NetCommand`
//!   values and forwards them to `GameSynchronizer`.
//! - Serializes outgoing `NetCommand` values from the synchronizer into
//!   transport messages and sends them via `Transport`.
//! - Manages the player join / leave / disconnect lifecycle during a game.
//! - Provides a channel-based API that lets GameLogic run without direct
//!   knowledge of the transport implementation.

use crate::commands::{
    CommandPayload, GameCommandData, NetCommand as GameNetCommand, NetCommandType,
};
use crate::error::{NetworkError, NetworkResult};
use crate::network_defs::{NETWORK_BASE_PORT_NUMBER, NUM_CONNECTIONS};
use crate::sync::game_sync::NetCommand as SyncNetCommand;
use crate::transport::{Transport, TransportMessage, TransportProtocol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// Bridge events (exposed to GameLogic)
// ---------------------------------------------------------------------------

/// Events emitted by the bridge for the game engine to react to.
#[derive(Debug, Clone)]
pub enum BridgeEvent {
    /// A new player has joined the game.
    PlayerJoined {
        player_id: u8,
        name: String,
        address: SocketAddr,
    },
    /// A player has left the game gracefully.
    PlayerLeft {
        player_id: u8,
        reason: PlayerLeaveReason,
    },
    /// A player disconnected unexpectedly.
    PlayerDisconnected {
        player_id: u8,
        last_frame: u32,
    },
    /// A chat message was received.
    ChatReceived {
        player_id: u8,
        message: String,
        target_mask: i32,
    },
    /// A file transfer announcement.
    FileTransferAnnounced {
        player_id: u8,
        filename: String,
        file_size: u64,
    },
    /// The game should start loading (all players ready).
    GameStartLoading,
    /// The game should begin (all players loaded).
    GameStart,
    /// A network error occurred.
    Error {
        message: String,
    },
}

/// Reason for a player leaving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerLeaveReason {
    /// Player chose to leave.
    Quit,
    /// Connection lost.
    ConnectionLost,
    /// Kicked by host.
    Kicked,
    /// Network error.
    NetworkError,
    /// Game ended.
    GameEnd,
}

// ---------------------------------------------------------------------------
// Player connection state
// ---------------------------------------------------------------------------

/// State of a connected player as tracked by the bridge.
#[derive(Debug, Clone)]
pub struct BridgePlayer {
    /// Player slot index (0-7).
    pub slot: u8,
    /// Player display name.
    pub name: String,
    /// Network address.
    pub address: SocketAddr,
    /// Whether the player has finished loading.
    pub loaded: bool,
    /// Whether the player is ready to start.
    pub ready: bool,
    /// Last received frame number from this player.
    pub last_received_frame: u32,
    /// Channel to send commands to this player.
    pub command_sender: Option<mpsc::UnboundedSender<SyncNetCommand>>,
}

// ---------------------------------------------------------------------------
// Network bridge configuration
// ---------------------------------------------------------------------------

/// Configuration for the network bridge.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Local player slot index.
    pub local_slot: u8,
    /// Whether this instance is hosting the game.
    pub is_host: bool,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// Maximum number of connection retries.
    pub max_retries: u32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            local_slot: 0,
            is_host: true,
            connect_timeout_ms: 10000,
            max_retries: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Network bridge
// ---------------------------------------------------------------------------

/// Bridge between the transport layer and game logic.
///
/// Provides a high-level, async interface for game logic to interact
/// with the network without knowing about the underlying transport
/// protocol or serialization details.
pub struct NetworkBridge {
    /// Transport layer.
    transport: Arc<Transport>,
    /// Configuration.
    config: BridgeConfig,
    /// Connected players (slot -> player info).
    players: Arc<RwLock<HashMap<u8, BridgePlayer>>>,
    /// Event channel for game logic.
    event_tx: mpsc::UnboundedSender<BridgeEvent>,
    /// Event receiver (held by game logic).
    event_rx: Mutex<Option<mpsc::UnboundedReceiver<BridgeEvent>>>,
    /// Outgoing command channel (from game logic / synchronizer to bridge).
    outgoing_tx: mpsc::UnboundedSender<GameNetCommand>,
    /// Outgoing command receiver.
    outgoing_rx: Mutex<Option<mpsc::UnboundedReceiver<GameNetCommand>>>,
    /// Running flag.
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl NetworkBridge {
    /// Create a new network bridge.
    pub fn new(transport: Arc<Transport>, config: BridgeConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel();

        Self {
            transport,
            config,
            players: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Mutex::new(Some(event_rx)),
            outgoing_tx,
            outgoing_rx: Mutex::new(Some(outgoing_rx)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Get a sender for outgoing game commands (wired to the synchronizer).
    pub fn outgoing_sender(&self) -> mpsc::UnboundedSender<GameNetCommand> {
        self.outgoing_tx.clone()
    }

    /// Take the event receiver (call once, typically at game start).
    ///
    /// Returns `None` if already taken.
    pub async fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<BridgeEvent>> {
        self.event_rx.lock().await.take()
    }

    /// Take the outgoing command receiver (called by the pump loop).
    pub async fn take_outgoing_receiver(
        &self,
    ) -> Option<mpsc::UnboundedReceiver<GameNetCommand>> {
        self.outgoing_rx.lock().await.take()
    }

    /// Connect to a remote player at the given address.
    pub async fn connect_to_player(
        &self,
        slot: u8,
        name: String,
        address: SocketAddr,
    ) -> NetworkResult<()> {
        info!("Connecting to player '{}' at {}", name, address);

        // Establish QUIC connection via transport.
        self.transport.connect(address).await.map_err(|e| {
            NetworkError::transport(format!("Failed to connect to {}: {}", address, e))
        })?;

        // Register player.
        let player = BridgePlayer {
            slot,
            name: name.clone(),
            address,
            loaded: false,
            ready: false,
            last_received_frame: 0,
            command_sender: None,
        };
        self.players.write().await.insert(slot, player.clone());

        // Notify game logic.
        let _ = self.event_tx.send(BridgeEvent::PlayerJoined {
            player_id: slot,
            name,
            address,
        });

        info!("Connected to player {} at slot {}", player.name, slot);
        Ok(())
    }

    /// Disconnect a player.
    pub async fn disconnect_player(&self, slot: u8, reason: PlayerLeaveReason) {
        if let Some(player) = self.players.write().await.remove(&slot) {
            info!(
                "Player '{}' (slot {}) disconnected: {:?}",
                player.name, slot, reason
            );

            let _ = self.event_tx.send(BridgeEvent::PlayerLeft {
                player_id: slot,
                reason,
            });
        }
    }

    /// Notify that a player has finished loading.
    pub async fn set_player_loaded(&self, slot: u8) -> NetworkResult<()> {
        let mut players = self.players.write().await;
        let player = players.get_mut(&slot).ok_or_else(|| {
            NetworkError::player(format!("slot {} not connected", slot))
        })?;
        player.loaded = true;
        Ok(())
    }

    /// Set a player's ready state.
    pub async fn set_player_ready(&self, slot: u8, ready: bool) -> NetworkResult<()> {
        let mut players = self.players.write().await;
        let player = players.get_mut(&slot).ok_or_else(|| {
            NetworkError::player(format!("slot {} not connected", slot))
        })?;
        player.ready = ready;
        Ok(())
    }

    /// Check if all connected players are loaded.
    pub async fn all_players_loaded(&self) -> bool {
        let players = self.players.read().await;
        if players.is_empty() {
            return false;
        }
        players.values().all(|p| p.loaded)
    }

    /// Check if all connected players are ready.
    pub async fn all_players_ready(&self) -> bool {
        let players = self.players.read().await;
        if players.is_empty() {
            return false;
        }
        players.values().all(|p| p.ready)
    }

    /// Get the number of connected players.
    pub async fn player_count(&self) -> usize {
        self.players.read().await.len()
    }

    /// Get player info by slot.
    pub async fn get_player(&self, slot: u8) -> Option<BridgePlayer> {
        self.players.read().await.get(&slot).cloned()
    }

    /// Get all player addresses (for the transport layer to maintain connections).
    pub async fn player_addresses(&self) -> Vec<SocketAddr> {
        let players = self.players.read().await;
        players.values().map(|p| p.address).collect()
    }

    /// Get the bridge configuration.
    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    /// Check if this instance is the host.
    pub fn is_host(&self) -> bool {
        self.config.is_host
    }

    /// Get the local player slot.
    pub fn local_slot(&self) -> u8 {
        self.config.local_slot
    }

    // -----------------------------------------------------------------------
    // Message conversion and routing
    // -----------------------------------------------------------------------

    /// Convert an incoming transport message to a `GameNetCommand`.
    ///
    /// Parses the raw bytes into a typed command.  Returns `None` if the
    /// message cannot be parsed (e.g. malformed or unknown command type).
    pub fn deserialize_message(
        &self,
        msg: &TransportMessage,
    ) -> Option<GameNetCommand> {
        if msg.data.len() < 4 {
            return None;
        }

        // First 4 bytes are the command type (i32, little-endian).
        let cmd_type_val = i32::from_le_bytes([msg.data[0], msg.data[1], msg.data[2], msg.data[3]]);
        let cmd_type = NetCommandType::from(cmd_type_val);

        if cmd_type == NetCommandType::Unknown {
            return None;
        }

        // Remaining bytes are the payload.
        let payload_bytes = &msg.data[4..];

        // Player ID from the source address (if available) or from the payload.
        let player_id = msg
            .source
            .map(|_addr| 0u8) // TODO: map address to player slot
            .unwrap_or(0);

        // Build the command based on type.
        let payload = match cmd_type {
            NetCommandType::Chat => {
                // Simple text payload after the type header.
                let message = String::from_utf8_lossy(payload_bytes).to_string();
                CommandPayload::Chat(crate::commands::ChatData {
                    message,
                    target_mask: 0xFF, // broadcast to all
                })
            }
            NetCommandType::KeepAlive => CommandPayload::KeepAlive,
            NetCommandType::LoadComplete => CommandPayload::KeepAlive,
            NetCommandType::GameCommand => {
                // Attempt bincode deserialization for game commands.
                match bincode::deserialize::<GameCommandData>(payload_bytes) {
                    Ok(game_data) => CommandPayload::GameCommand(game_data),
                    Err(_) => CommandPayload::Generic(payload_bytes.to_vec()),
                }
            }
            NetCommandType::FrameInfo => {
                match bincode::deserialize::<crate::commands::FrameInfoData>(payload_bytes) {
                    Ok(info) => CommandPayload::FrameInfo(info),
                    Err(_) => CommandPayload::Generic(payload_bytes.to_vec()),
                }
            }
            _ => CommandPayload::Generic(payload_bytes.to_vec()),
        };

        Some(GameNetCommand::new(cmd_type, player_id, 0, payload))
    }

    /// Convert a `GameNetCommand` to a transport message for sending.
    pub fn serialize_command(&self, cmd: &GameNetCommand) -> TransportMessage {
        let mut data = Vec::with_capacity(4 + 256);

        // Command type header (4 bytes LE).
        data.extend_from_slice(&cmd.command_type.as_i32().to_le_bytes());

        // Payload serialization depends on type.
        match &cmd.payload {
            CommandPayload::Chat(chat) => {
                data.extend_from_slice(chat.message.as_bytes());
            }
            CommandPayload::KeepAlive => {
                // No additional data.
            }
            CommandPayload::Generic(bytes) => {
                data.extend_from_slice(bytes);
            }
            CommandPayload::GameCommand(game_data) => {
                if let Ok(serialized) = bincode::serialize(game_data) {
                    data.extend_from_slice(&serialized);
                }
            }
            CommandPayload::FrameInfo(info) => {
                if let Ok(serialized) = bincode::serialize(info) {
                    data.extend_from_slice(&serialized);
                }
            }
            _ => {
                // Fallback: serialize the full command via bincode.
                if let Ok(serialized) = bincode::serialize(cmd) {
                    data = serialized;
                }
            }
        }

        TransportMessage::new(data, TransportProtocol::Quic)
    }

    /// Send a game command to all connected players.
    pub async fn broadcast_command(&self, cmd: &GameNetCommand) -> NetworkResult<()> {
        let msg = self.serialize_command(cmd);
        let addresses: Vec<SocketAddr> = self.player_addresses().await;

        for addr in addresses {
            let mut msg_clone = msg.clone();
            msg_clone.destination = Some(addr);
            self.transport
                .send_message(msg_clone)
                .await
                .map_err(|e| {
                    warn!("Failed to broadcast to {}: {}", addr, e);
                    e
                })
                .ok(); // Log but don't fail on single recipient error
        }

        Ok(())
    }

    /// Send a command to a specific player.
    pub async fn send_command_to(
        &self,
        cmd: &GameNetCommand,
        slot: u8,
    ) -> NetworkResult<()> {
        let address = {
            let players = self.players.read().await;
            players
                .get(&slot)
                .map(|p| p.address)
                .ok_or_else(|| NetworkError::player(format!("slot {} not connected", slot)))?
        };

        let mut msg = self.serialize_command(cmd);
        msg.destination = Some(address);
        self.transport.send_message(msg).await.map_err(|e| {
            NetworkError::transport(format!("Failed to send to slot {}: {}", slot, e))
        })?;

        Ok(())
    }

    /// Receive all pending transport messages and convert them to game commands.
    pub async fn receive_commands(&self) -> Vec<GameNetCommand> {
        let messages = match self.transport.receive_messages().await {
            Ok(msgs) => msgs,
            Err(_) => return Vec::new(),
        };

        messages
            .iter()
            .filter_map(|msg| self.deserialize_message(msg))
            .collect()
    }

    /// Process incoming messages and emit bridge events.
    ///
    /// Should be called every frame by the game loop.
    pub async fn process_incoming(&self) {
        let commands = self.receive_commands().await;

        for cmd in commands {
            match cmd.command_type {
                NetCommandType::Chat => {
                    if let CommandPayload::Chat(chat) = &cmd.payload {
                        let _ = self.event_tx.send(BridgeEvent::ChatReceived {
                            player_id: cmd.player_id,
                            message: chat.message.clone(),
                            target_mask: chat.target_mask,
                        });
                    }
                }
                NetCommandType::LoadComplete => {
                    if let Err(e) = self.set_player_loaded(cmd.player_id).await {
                        debug!("LoadComplete from unknown player: {}", e);
                    }
                }
                NetCommandType::PlayerLeave => {
                    if let CommandPayload::PlayerLeave(data) = &cmd.payload {
                        self.disconnect_player(
                            data.leaving_player_id,
                            PlayerLeaveReason::Quit,
                        )
                        .await;
                    }
                }
                NetCommandType::KeepAlive => {
                    // Update last seen frame for the player.
                    if let Some(player) =
                        self.players.write().await.get_mut(&cmd.player_id)
                    {
                        player.last_received_frame = cmd.execution_frame;
                    }
                }
                _ => {
                    // Other commands are forwarded to the synchronizer
                    // via the outgoing channel or handled directly.
                    debug!(
                        "Received command type {:?} from player {}",
                        cmd.command_type, cmd.player_id
                    );
                }
            }
        }
    }

    /// Convert a `GameNetCommand` to a `SyncNetCommand` for the synchronizer.
    ///
    /// The bridge translates the high-level game command into the compact
    /// sync-layer format.
    pub fn game_cmd_to_sync_cmd(cmd: &GameNetCommand) -> Option<SyncNetCommand> {
        let data = match &cmd.payload {
            CommandPayload::Generic(bytes) => bytes.clone(),
            CommandPayload::GameCommand(game_data) => {
                bincode::serialize(game_data).unwrap_or_default()
            }
            _ => return None, // Non-game commands don't go through the sync layer.
        };

        Some(SyncNetCommand {
            player_id: cmd.player_id,
            frame: cmd.execution_frame,
            data,
        })
    }

    /// Convert a `SyncNetCommand` back to a `GameNetCommand` for sending.
    pub fn sync_cmd_to_game_cmd(
        sync_cmd: &SyncNetCommand,
        cmd_type: NetCommandType,
    ) -> GameNetCommand {
        let payload = if let Ok(game_data) = bincode::deserialize::<GameCommandData>(&sync_cmd.data)
        {
            CommandPayload::GameCommand(game_data)
        } else {
            CommandPayload::Generic(sync_cmd.data.clone())
        };

        GameNetCommand::new(cmd_type, sync_cmd.player_id, sync_cmd.frame, payload)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_config_defaults() {
        let config = BridgeConfig::default();
        assert_eq!(config.local_slot, 0);
        assert!(config.is_host);
    }

    #[tokio::test]
    async fn test_event_channel() {
        // The bridge event channel works independently of the transport.
        // We test the plumbing by creating a bridge and using it directly.
        // Use Transport::new_for_testing which does not require certificate setup.
        let transport = Transport::new_for_testing("127.0.0.1:0".parse().unwrap());
        let bridge = NetworkBridge::new(Arc::new(transport), BridgeConfig::default());

        // Take the receiver.
        let mut rx = bridge.take_event_receiver().await.unwrap();
        assert!(bridge.take_event_receiver().await.is_none());

        // Send an event.
        bridge
            .event_tx
            .send(BridgeEvent::ChatReceived {
                player_id: 0,
                message: "hello".to_string(),
                target_mask: 0xFF,
            })
            .unwrap();

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, BridgeEvent::ChatReceived { .. }));
    }

    #[tokio::test]
    async fn test_outgoing_channel() {
        let transport = Transport::new_for_testing("127.0.0.1:0".parse().unwrap());
        let bridge = NetworkBridge::new(Arc::new(transport), BridgeConfig::default());

        let sender = bridge.outgoing_sender();
        let cmd = GameNetCommand::keep_alive(0);
        sender.send(cmd).unwrap();

        let mut rx = bridge.take_outgoing_receiver().await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.command_type, NetCommandType::KeepAlive);
    }

    #[test]
    fn test_game_cmd_to_sync_cmd() {
        let game_cmd = GameNetCommand::game_command(
            1,
            100,
            GameCommandData {
                command_type: 42,
                target_id: None,
                position: None,
                parameters: std::collections::HashMap::new(),
                checksum: 0,
            },
        );

        let sync_cmd = NetworkBridge::game_cmd_to_sync_cmd(&game_cmd).unwrap();
        assert_eq!(sync_cmd.player_id, 1);
        assert_eq!(sync_cmd.frame, 100);
    }

    #[test]
    fn test_game_cmd_to_sync_cmd_non_game() {
        let game_cmd = GameNetCommand::chat(0, "hello".to_string(), 0xFF);
        // Chat commands don't convert to sync commands.
        assert!(NetworkBridge::game_cmd_to_sync_cmd(&game_cmd).is_none());
    }

    #[test]
    fn test_sync_cmd_to_game_cmd() {
        let sync_cmd = SyncNetCommand {
            player_id: 2,
            frame: 50,
            data: vec![1, 2, 3],
        };
        let game_cmd = NetworkBridge::sync_cmd_to_game_cmd(&sync_cmd, NetCommandType::GameCommand);
        assert_eq!(game_cmd.player_id, 2);
        assert_eq!(game_cmd.execution_frame, 50);
    }
}
