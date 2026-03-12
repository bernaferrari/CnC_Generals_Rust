//! Internal event bus used to connect the LAN subsystems (`GameDiscovery`,
//! `LanLobby`, `LanChat`) with the high level [`LanApi`].
//!
//! This module deliberately exposes only strongly typed senders/receivers so
//! public consumers can compose custom networking stacks without relying on
//! private types. The bus itself is built on top of Tokio's unbounded channel
//! which fits the fan-out pattern required by the original C++ implementation.

use std::fmt;
use std::net::SocketAddr;

use tokio::sync::mpsc::{self, error::TryRecvError};

use super::chat::ChatMessage;
use super::lobby::LobbyEvent;
use super::messages::LanMessage;
use super::LanGameInfo;

/// Event emitted by LAN subsystems for consumption by [`LanApi`].
#[derive(Debug, Clone)]
pub enum LanBridgeEvent {
    /// Raw LAN message received from the network alongside the sender
    /// endpoint. The high-level API turns this into the legacy LAN callbacks.
    NetworkMessage(LanMessage, SocketAddr),
    /// Snapshot of all discovered games.
    DiscoverySnapshot(Vec<LanGameInfo>),
    /// Event emitted by the lobby layer (player joins, accept status, etc.).
    LobbyEvent(LobbyEvent),
    /// Chat traffic observed on the LAN channel.
    ChatEvent(ChatMessage),
    /// Graceful shutdown request for the background task.
    Shutdown,
}

/// Error returned when an event cannot be delivered to the bus consumer.
#[derive(Debug)]
pub struct LanEventSendError(mpsc::error::SendError<LanBridgeEvent>);

impl fmt::Display for LanEventSendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LAN event channel closed: {}", self.0)
    }
}

impl std::error::Error for LanEventSendError {}

/// Cloneable sender handle used by subsystems to push events into the bus.
#[derive(Clone)]
pub struct LanEventSender {
    inner: mpsc::UnboundedSender<LanBridgeEvent>,
}

impl LanEventSender {
    /// Push an event onto the bus.
    pub fn send(&self, event: LanBridgeEvent) -> Result<(), LanEventSendError> {
        self.inner.send(event).map_err(LanEventSendError)
    }

    /// Returns `true` if there are no active receivers.
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

/// Receiver side of the bus consumed by [`LanApi`]'s background task.
pub struct LanEventReceiver {
    inner: mpsc::UnboundedReceiver<LanBridgeEvent>,
}

impl LanEventReceiver {
    /// Receive the next event emitted by a subsystem.
    pub async fn recv(&mut self) -> Option<LanBridgeEvent> {
        self.inner.recv().await
    }

    /// Stop receiving new events while draining buffered ones.
    pub fn close(&mut self) {
        self.inner.close();
    }

    /// Attempt to fetch the next event without waiting.
    pub fn try_recv(&mut self) -> Result<LanBridgeEvent, TryRecvError> {
        self.inner.try_recv()
    }
}

/// Construct a fresh sender/receiver pair for the LAN event bus.
pub fn lan_event_channel() -> (LanEventSender, LanEventReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    (LanEventSender { inner: tx }, LanEventReceiver { inner: rx })
}
