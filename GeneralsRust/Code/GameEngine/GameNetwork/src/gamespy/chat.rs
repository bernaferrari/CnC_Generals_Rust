#![allow(dead_code, unused_imports, unused_variables)]
//! GameSpy Chat System
//!
//! This module implements the modernised GameSpy chat functionality including:
//! - Room and private messaging with history retention
//! - Language filtering with dynamic dictionaries
//! - Duplicate message suppression and activity tracking
//! - Transport abstraction to support real backends (WebSockets/QUIC/etc.)

use crate::error::{NetworkError, NetworkResult};
use crate::gamespy::{ChatMessage, ChatMessageType, GameSpyEvent};
use crate::time::NetworkInstant;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, error, info, instrument, trace, warn};

const DEFAULT_HISTORY_CAPACITY: usize = 2048;
const DUPLICATE_SUPPRESSION_WINDOW_MS: u64 = 750;
const BACKGROUND_HEARTBEAT_SECS: u64 = 60;

#[derive(Debug, Clone)]
struct LastSentMessage {
    normalized: String,
    target: ChatTarget,
    timestamp: NetworkInstant,
}

/// Delivery target for chat messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChatTarget {
    Global,
    Room(String),
    Private(String),
}

/// Envelope handed to the transport backend.
#[derive(Debug, Clone)]
pub struct ChatEnvelope {
    pub message: ChatMessage,
    pub target: ChatTarget,
}

#[async_trait]
pub trait ChatTransport: Send + Sync {
    async fn send(&self, envelope: ChatEnvelope) -> NetworkResult<()>;
    async fn join_room(&self, room: &str) -> NetworkResult<()>;
    async fn leave_room(&self, room: &str) -> NetworkResult<()>;
    fn subscribe(&self) -> Option<mpsc::UnboundedReceiver<ChatMessage>>;
}

struct NoopChatTransport;

impl Default for NoopChatTransport {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl ChatTransport for NoopChatTransport {
    async fn send(&self, _envelope: ChatEnvelope) -> NetworkResult<()> {
        Ok(())
    }

    async fn join_room(&self, _room: &str) -> NetworkResult<()> {
        Ok(())
    }

    async fn leave_room(&self, _room: &str) -> NetworkResult<()> {
        Ok(())
    }

    fn subscribe(&self) -> Option<mpsc::UnboundedReceiver<ChatMessage>> {
        let (_tx, rx) = mpsc::unbounded_channel();
        Some(rx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageDirection {
    Incoming,
    Outgoing,
}

/// GameSpy chat system
#[derive(Clone)]
pub struct GameSpyChat {
    event_tx: broadcast::Sender<GameSpyEvent>,
    transport: Arc<dyn ChatTransport + Send + Sync>,
    rooms: Arc<RwLock<HashMap<String, ChatRoom>>>,
    private_chats: Arc<RwLock<HashMap<String, PrivateChat>>>,
    current_room: Arc<RwLock<Option<String>>>,
    local_player_id: Arc<RwLock<String>>,
    colors: ChatColors,
    message_history: Arc<RwLock<VecDeque<ChatMessage>>>,
    language_filter: Arc<RwLock<LanguageFilter>>,
    is_connected: Arc<AtomicBool>,
    task_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    dedupe_guard: Arc<Mutex<Option<LastSentMessage>>>,
    max_history: usize,
}

impl GameSpyChat {
    /// Create new GameSpy chat system with the default loopback transport.
    pub async fn new(event_tx: broadcast::Sender<GameSpyEvent>) -> NetworkResult<Self> {
        let transport: Arc<dyn ChatTransport + Send + Sync> =
            Arc::new(NoopChatTransport::default());
        Self::with_transport(event_tx, transport).await
    }

    /// Create a chat system with a custom transport implementation.
    pub async fn with_transport(
        event_tx: broadcast::Sender<GameSpyEvent>,
        transport: Arc<dyn ChatTransport + Send + Sync>,
    ) -> NetworkResult<Self> {
        Ok(Self {
            event_tx,
            transport,
            rooms: Arc::new(RwLock::new(HashMap::new())),
            private_chats: Arc::new(RwLock::new(HashMap::new())),
            current_room: Arc::new(RwLock::new(None)),
            local_player_id: Arc::new(RwLock::new(String::new())),
            colors: ChatColors::default(),
            message_history: Arc::new(RwLock::new(VecDeque::with_capacity(
                DEFAULT_HISTORY_CAPACITY,
            ))),
            language_filter: Arc::new(RwLock::new(LanguageFilter::default())),
            is_connected: Arc::new(AtomicBool::new(false)),
            task_handles: Arc::new(Mutex::new(Vec::new())),
            dedupe_guard: Arc::new(Mutex::new(None)),
            max_history: DEFAULT_HISTORY_CAPACITY,
        })
    }

    /// Set local player ID
    pub async fn set_local_player_id(&self, player_id: String) {
        *self.local_player_id.write().await = player_id;
    }

    /// Start chat system
    #[instrument(skip(self))]
    pub async fn start(&self) -> NetworkResult<()> {
        if self.is_connected.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        info!("Starting GameSpy chat system");
        self.start_background_tasks().await?;
        info!("GameSpy chat system started");

        Ok(())
    }

    /// Stop chat system
    #[instrument(skip(self))]
    pub async fn stop(&self) -> NetworkResult<()> {
        if !self.is_connected.swap(false, Ordering::SeqCst) {
            return Ok(());
        }

        info!("Stopping GameSpy chat system");

        let mut handles = self.task_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }

        info!("GameSpy chat system stopped");
        Ok(())
    }

    /// Send a regular chat message.
    #[instrument(skip(self))]
    pub async fn send_message(&self, message: String, room: Option<String>) -> NetworkResult<()> {
        let target = match room {
            Some(room_name) => ChatTarget::Room(Self::normalize_room_name(room_name)),
            None => ChatTarget::Global,
        };
        self.dispatch_outgoing(message, target, ChatMessageType::Normal)
            .await
    }

    /// Send private message
    #[instrument(skip(self))]
    pub async fn send_private_message(
        &self,
        recipient: String,
        message: String,
    ) -> NetworkResult<()> {
        let target = ChatTarget::Private(recipient);
        self.dispatch_outgoing(message, target, ChatMessageType::Private)
            .await
    }

    /// Join chat room
    #[instrument(skip(self))]
    pub async fn join_room(&self, room_name: String) -> NetworkResult<()> {
        self.ensure_connected()?;

        let normalized = Self::normalize_room_name(room_name);
        info!("Joining chat room: {}", normalized);

        self.transport.join_room(&normalized).await?;

        {
            let mut rooms = self.rooms.write().await;
            let entry = rooms
                .entry(normalized.clone())
                .or_insert_with(|| ChatRoom::new(normalized.clone()));
            entry.record_activity();
            entry
                .users
                .insert(self.local_player_id.read().await.clone());
        }

        *self.current_room.write().await = Some(normalized.clone());

        Ok(())
    }

    /// Leave chat room
    #[instrument(skip(self))]
    pub async fn leave_room(&self, room_name: String) -> NetworkResult<()> {
        self.ensure_connected()?;

        let normalized = Self::normalize_room_name(room_name);
        info!("Leaving chat room: {}", normalized);

        self.transport.leave_room(&normalized).await?;
        self.leave_room_internal(&normalized).await?;

        Ok(())
    }

    /// Send emote message
    #[instrument(skip(self))]
    pub async fn send_emote(&self, message: String, room: Option<String>) -> NetworkResult<()> {
        let target = match room {
            Some(room_name) => ChatTarget::Room(Self::normalize_room_name(room_name)),
            None => ChatTarget::Global,
        };
        self.dispatch_outgoing(message, target, ChatMessageType::Emote)
            .await
    }

    /// Get room list
    pub async fn get_room_list(&self) -> NetworkResult<Vec<String>> {
        if !self.is_connected() {
            return Err(NetworkError::generic("Chat system not connected"));
        }

        let rooms = self.rooms.read().await;
        Ok(rooms.keys().cloned().collect())
    }

    /// Get message history
    pub async fn get_message_history(&self, count: usize) -> Vec<ChatMessage> {
        let history = self.message_history.read().await;
        history
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Get private chat history
    pub async fn get_private_chat_history(
        &self,
        participant: &str,
        count: usize,
    ) -> Vec<ChatMessage> {
        let private_chats = self.private_chats.read().await;
        if let Some(private_chat) = private_chats.get(participant) {
            private_chat
                .messages
                .iter()
                .rev()
                .take(count)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    /// Process incoming chat message from the transport/backend.
    pub async fn process_incoming_message(&self, message: ChatMessage) -> NetworkResult<()> {
        let target = if let Some(room) = &message.room {
            ChatTarget::Room(room.clone())
        } else if message.message_type == ChatMessageType::Private {
            ChatTarget::Private(message.sender.clone())
        } else {
            ChatTarget::Global
        };

        let message_clone = message.clone();
        self.record_message(&target, &message_clone, MessageDirection::Incoming)
            .await?;

        let _ = self.event_tx.send(GameSpyEvent::ChatMessage(message));
        Ok(())
    }

    async fn dispatch_outgoing(
        &self,
        text: String,
        target: ChatTarget,
        message_type: ChatMessageType,
    ) -> NetworkResult<()> {
        self.ensure_connected()?;

        let normalized = Self::normalize_message(&text);
        if normalized.is_empty() {
            return Ok(());
        }

        if self.should_suppress(&normalized, &target).await {
            warn!(?target, "Suppressing duplicate chat message");
            return Ok(());
        }

        let filtered = {
            let filter = self.language_filter.read().await;
            filter.filter_message(&normalized)
        };

        let sender = {
            let id = self.local_player_id.read().await;
            if id.is_empty() {
                return Err(NetworkError::generic("Local player ID not configured"));
            }
            id.clone()
        };

        let room = match &target {
            ChatTarget::Room(room_name) => Some(room_name.clone()),
            _ => None,
        };

        let chat_message = ChatMessage {
            sender,
            message: filtered,
            room,
            timestamp: Utc::now(),
            message_type,
        };

        self.record_message(&target, &chat_message, MessageDirection::Outgoing)
            .await?;

        self.transport
            .send(ChatEnvelope {
                message: chat_message.clone(),
                target: target.clone(),
            })
            .await?;

        let _ = self.event_tx.send(GameSpyEvent::ChatMessage(chat_message));

        Ok(())
    }

    async fn record_message(
        &self,
        target: &ChatTarget,
        message: &ChatMessage,
        direction: MessageDirection,
    ) -> NetworkResult<()> {
        self.append_history(message).await;

        match target {
            ChatTarget::Global => {}
            ChatTarget::Room(room_name) => {
                let mut rooms = self.rooms.write().await;
                let entry = rooms
                    .entry(room_name.clone())
                    .or_insert_with(|| ChatRoom::new(room_name.clone()));
                entry.record_activity();
                entry.users.insert(message.sender.clone());
            }
            ChatTarget::Private(participant) => {
                self.add_private_message(participant.clone(), message, direction)
                    .await;
            }
        }

        Ok(())
    }

    async fn append_history(&self, message: &ChatMessage) {
        let mut history = self.message_history.write().await;
        history.push_back(message.clone());
        while history.len() > self.max_history {
            history.pop_front();
        }
    }

    async fn add_private_message(
        &self,
        participant: String,
        message: &ChatMessage,
        direction: MessageDirection,
    ) {
        let mut private_chats = self.private_chats.write().await;
        let entry = private_chats
            .entry(participant.clone())
            .or_insert_with(|| PrivateChat::new(participant.clone()));

        entry.messages.push_back(message.clone());
        while entry.messages.len() > self.max_history {
            entry.messages.pop_front();
        }

        entry.last_activity = message.timestamp;
        if direction == MessageDirection::Incoming {
            entry.unread_count = entry.unread_count.saturating_add(1);
        }
    }

    async fn leave_room_internal(&self, room_name: &str) -> NetworkResult<()> {
        let local_player = self.local_player_id.read().await.clone();

        {
            let mut rooms = self.rooms.write().await;
            if let Some(room) = rooms.get_mut(room_name) {
                room.users.remove(&local_player);
                room.record_activity();
                if room.users.is_empty() {
                    rooms.remove(room_name);
                }
            }
        }

        let mut current = self.current_room.write().await;
        if current.as_deref() == Some(room_name) {
            *current = None;
        }

        info!("Left room: {}", room_name);
        Ok(())
    }

    async fn should_suppress(&self, normalized: &str, target: &ChatTarget) -> bool {
        let mut guard = self.dedupe_guard.lock().await;
        if let Some(last) = guard.as_ref() {
            if last.target == *target
                && last.normalized == normalized
                && last.timestamp.elapsed() < self.duplicate_window()
            {
                return true;
            }
        }

        *guard = Some(LastSentMessage {
            normalized: normalized.to_string(),
            target: target.clone(),
            timestamp: NetworkInstant::now(),
        });
        false
    }

    async fn start_background_tasks(&self) -> NetworkResult<()> {
        let is_connected = Arc::clone(&self.is_connected);
        let message_history = Arc::clone(&self.message_history);
        let max_history = self.max_history;
        let task = tokio::spawn(async move {
            while is_connected.load(Ordering::Relaxed) {
                sleep(Duration::from_secs(BACKGROUND_HEARTBEAT_SECS)).await;
                if !is_connected.load(Ordering::Relaxed) {
                    break;
                }

                let mut history = message_history.write().await;
                if history.len() > max_history {
                    let overflow = history.len() - max_history;
                    for _ in 0..overflow {
                        history.pop_front();
                    }
                }
                trace!(history_len = history.len(), "Chat history maintenance tick");
            }
        });

        self.task_handles.lock().await.push(task);

        if let Some(mut inbound_rx) = self.transport.subscribe() {
            let chat_clone = self.clone();
            let inbound_task = tokio::spawn(async move {
                while let Some(message) = inbound_rx.recv().await {
                    if !chat_clone.is_connected() {
                        continue;
                    }

                    if let Err(err) = chat_clone.process_incoming_message(message).await {
                        warn!("Failed to process inbound chat message: {}", err);
                    }
                }
            });
            self.task_handles.lock().await.push(inbound_task);
        }

        Ok(())
    }

    fn ensure_connected(&self) -> NetworkResult<()> {
        if self.is_connected() {
            Ok(())
        } else {
            Err(NetworkError::generic("Chat system not connected"))
        }
    }

    fn duplicate_window(&self) -> Duration {
        Duration::from_millis(DUPLICATE_SUPPRESSION_WINDOW_MS)
    }

    fn normalize_message(message: &str) -> String {
        message
            .replace('\r', " ")
            .replace('\n', " ")
            .trim()
            .to_string()
    }

    fn normalize_room_name(room: String) -> String {
        let mut cleaned = room.trim().to_string();
        if cleaned.is_empty() {
            return "#Generals".to_string();
        }
        if !cleaned.starts_with('#') {
            cleaned.insert(0, '#');
        }
        cleaned
    }
}

/// Chat room
#[derive(Debug, Clone)]
pub struct ChatRoom {
    /// Room name
    name: String,
    /// Room topic
    topic: Option<String>,
    /// Users in the room
    users: HashSet<String>,
    /// Room operators
    operators: HashSet<String>,
    /// Room owner
    owner: Option<String>,
    /// Room password (if private)
    password: Option<String>,
    /// Maximum users allowed
    max_users: Option<usize>,
    /// Room flags
    flags: ChatRoomFlags,
    /// Last activity timestamp
    last_activity: DateTime<Utc>,
}

impl ChatRoom {
    fn new(name: String) -> Self {
        Self {
            name,
            topic: None,
            users: HashSet::new(),
            operators: HashSet::new(),
            owner: None,
            password: None,
            max_users: None,
            flags: ChatRoomFlags::default(),
            last_activity: Utc::now(),
        }
    }

    fn record_activity(&mut self) {
        self.last_activity = Utc::now();
    }
}

/// Private chat session
#[derive(Debug, Clone)]
pub struct PrivateChat {
    /// Other participant
    participant: String,
    /// Message history
    messages: VecDeque<ChatMessage>,
    /// Unread message count
    unread_count: usize,
    /// Last activity timestamp
    last_activity: DateTime<Utc>,
}

impl PrivateChat {
    fn new(participant: String) -> Self {
        Self {
            participant,
            messages: VecDeque::new(),
            unread_count: 0,
            last_activity: Utc::now(),
        }
    }
}

/// Chat room flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatRoomFlags {
    /// Room is private
    pub is_private: bool,
    /// Room allows guests
    pub allow_guests: bool,
    /// Room has moderation enabled
    pub moderated: bool,
    /// Room allows voice messages
    pub voice_enabled: bool,
}

/// Chat colors configuration
#[derive(Debug, Clone)]
pub struct ChatColors {
    /// Default text color
    pub default: ChatColor,
    /// Current room color
    pub current_room: ChatColor,
    /// Chat room color
    pub room: ChatColor,
    /// Game color
    pub game: ChatColor,
    /// Player colors
    pub player_normal: ChatColor,
    pub player_owner: ChatColor,
    pub player_buddy: ChatColor,
    pub player_self: ChatColor,
    pub player_ignored: ChatColor,
    /// Chat message colors
    pub chat_normal: ChatColor,
    pub chat_emote: ChatColor,
    pub chat_private: ChatColor,
    pub chat_buddy: ChatColor,
    pub chat_self: ChatColor,
    /// System colors
    pub accept_true: ChatColor,
    pub accept_false: ChatColor,
    pub motd: ChatColor,
    pub motd_heading: ChatColor,
}

/// Chat color (RGBA)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ChatColor {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255, 255)
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0, 255)
    }

    pub fn red() -> Self {
        Self::new(255, 0, 0, 255)
    }

    pub fn green() -> Self {
        Self::new(0, 255, 0, 255)
    }

    pub fn blue() -> Self {
        Self::new(0, 0, 255, 255)
    }

    pub fn yellow() -> Self {
        Self::new(255, 255, 0, 255)
    }

    pub fn purple() -> Self {
        Self::new(128, 0, 128, 255)
    }
}

/// Language filter for chat messages
#[derive(Debug, Clone)]
pub struct LanguageFilter {
    /// Profanity words to filter
    profanity_words: HashSet<String>,
    /// Enable filtering
    enabled: bool,
    /// Replacement character for filtered words
    replacement_char: char,
}

impl Default for ChatRoomFlags {
    fn default() -> Self {
        Self {
            is_private: false,
            allow_guests: true,
            moderated: false,
            voice_enabled: false,
        }
    }
}

impl Default for ChatColors {
    fn default() -> Self {
        Self {
            default: ChatColor::white(),
            current_room: ChatColor::yellow(),
            room: ChatColor::white(),
            game: ChatColor::new(128, 128, 0, 255),
            player_normal: ChatColor::white(),
            player_owner: ChatColor::new(255, 0, 255, 255),
            player_buddy: ChatColor::new(255, 0, 128, 255),
            player_self: ChatColor::red(),
            player_ignored: ChatColor::new(128, 128, 128, 255),
            chat_normal: ChatColor::white(),
            chat_emote: ChatColor::new(255, 192, 203, 255),
            chat_private: ChatColor::new(255, 165, 0, 255),
            chat_buddy: ChatColor::new(0, 191, 255, 255),
            chat_self: ChatColor::new(255, 69, 0, 255),
            accept_true: ChatColor::green(),
            accept_false: ChatColor::red(),
            motd: ChatColor::yellow(),
            motd_heading: ChatColor::new(255, 140, 0, 255),
        }
    }
}

impl Default for LanguageFilter {
    fn default() -> Self {
        Self {
            profanity_words: HashSet::new(),
            enabled: true,
            replacement_char: '*',
        }
    }
}

impl LanguageFilter {
    /// Filter message for profanity
    pub fn filter_message(&self, message: &str) -> String {
        if !self.enabled {
            return message.to_string();
        }

        let mut filtered = message.to_string();
        for word in &self.profanity_words {
            let replacement = self.replacement_char.to_string().repeat(word.len());
            filtered = filtered.replace(word, &replacement);
        }

        filtered
    }

    /// Add profanity word
    pub fn add_profanity_word(&mut self, word: String) {
        self.profanity_words.insert(word.to_lowercase());
    }

    /// Remove profanity word
    pub fn remove_profanity_word(&mut self, word: &str) {
        self.profanity_words.remove(&word.to_lowercase());
    }

    /// Set filter enabled/disabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamespy::chat_transport::{ChatTransportConfig, WebSocketChatTransport};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::time::{timeout, Duration};
    use tokio_tungstenite::{accept_async, tungstenite::Message};
    use url::Url;

    #[tokio::test]
    async fn test_chat_creation_cycle() {
        let (tx, _) = broadcast::channel(10);
        let chat = GameSpyChat::new(tx).await.unwrap();
        assert!(!chat.is_connected());

        chat.start().await.unwrap();
        assert!(chat.is_connected());

        chat.stop().await.unwrap();
        assert!(!chat.is_connected());
    }

    #[tokio::test]
    async fn test_duplicate_suppression() {
        let (tx, mut rx) = broadcast::channel(10);
        let chat = GameSpyChat::new(tx).await.unwrap();
        chat.set_local_player_id("Tester".to_string()).await;
        chat.start().await.unwrap();

        chat.send_message("Hello world".into(), None).await.unwrap();
        chat.send_message("Hello world".into(), None).await.unwrap(); // Suppressed

        let first = rx.recv().await.unwrap();
        match first {
            GameSpyEvent::ChatMessage(msg) => assert_eq!(msg.message, "Hello world"),
            _ => panic!("unexpected event"),
        }

        // The duplicate should have been suppressed and not emitted immediately
        assert!(rx.try_recv().is_err());

        chat.stop().await.unwrap();
    }

    #[tokio::test]
    async fn websocket_transport_updates_room_state() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();

            while let Some(msg) = ws.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                        match value["type"].as_str() {
                            Some("join") => {
                                let room = value["room"].clone();
                                let response = json!({
                                    "type": "system",
                                    "message": "joined",
                                    "room": room,
                                    "timestamp": Utc::now().to_rfc3339(),
                                });
                                ws.send(Message::Text(response.to_string())).await.unwrap();

                                let remote_message = json!({
                                    "type": "message",
                                    "sender": "Opponent",
                                    "message": "Greetings",
                                    "room": room,
                                    "kind": "normal",
                                    "timestamp": Utc::now().to_rfc3339(),
                                });
                                ws.send(Message::Text(remote_message.to_string()))
                                    .await
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        });

        let url = Url::parse(&format!("ws://{addr}")).unwrap();
        let transport = WebSocketChatTransport::connect(ChatTransportConfig::new(url))
            .await
            .unwrap();
        let transport: Arc<dyn ChatTransport + Send + Sync> = Arc::new(transport);

        let (event_tx, _) = broadcast::channel(32);
        let mut events = event_tx.subscribe();
        let chat = GameSpyChat::with_transport(event_tx.clone(), transport)
            .await
            .unwrap();
        chat.set_local_player_id("Tester".to_string()).await;
        chat.start().await.unwrap();

        chat.join_room("#test".to_string()).await.unwrap();

        let system_event = timeout(Duration::from_secs(2), events.recv())
            .await
            .expect("system event timed out")
            .unwrap();
        match system_event {
            GameSpyEvent::ChatMessage(msg) => {
                assert_eq!(msg.message_type, ChatMessageType::System);
            }
            other => panic!("unexpected event: {:?}", other),
        }

        let remote_event = timeout(Duration::from_secs(2), events.recv())
            .await
            .expect("remote message timed out")
            .unwrap();
        match remote_event {
            GameSpyEvent::ChatMessage(msg) => {
                assert_eq!(msg.sender, "Opponent");
                assert_eq!(msg.message, "Greetings");
            }
            other => panic!("unexpected event: {:?}", other),
        }

        {
            let rooms = chat.rooms.read().await;
            let room = rooms.get("#test").expect("room state recorded");
            assert!(room.users.contains("Opponent"));
            assert!(room.users.contains("Tester"));
        }

        chat.stop().await.unwrap();
        server.abort();
    }

    #[test]
    fn test_language_filter() {
        let mut filter = LanguageFilter::default();
        filter.add_profanity_word("badword".to_string());

        let filtered = filter.filter_message("This is a badword test");
        assert_eq!(filtered, "This is a ******* test");
    }

    #[test]
    fn test_chat_color_creation() {
        let red = ChatColor::red();
        assert_eq!(red.r, 255);
        assert_eq!(red.g, 0);
        assert_eq!(red.b, 0);
        assert_eq!(red.a, 255);
    }
}
