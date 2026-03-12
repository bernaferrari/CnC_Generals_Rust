use super::{ChatEnvelope, ChatMessage, ChatMessageType, ChatTarget};
use crate::error::{NetworkError, NetworkResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex as SyncMutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::Message, Error as TungsteniteError},
};
use tracing::{debug, error, info, trace, warn};
use url::Url;

/// Configuration for the WebSocket chat transport.
#[derive(Debug, Clone)]
pub struct ChatTransportConfig {
    pub endpoint: Url,
    pub auth_token: Option<String>,
    pub connect_timeout: Duration,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl ChatTransportConfig {
    pub fn new(endpoint: Url) -> Self {
        Self {
            endpoint,
            auth_token: None,
            connect_timeout: Duration::from_secs(5),
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn with_backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max.max(initial);
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }
}

enum TransportCommand {
    Send(ChatEnvelope),
    Join(String),
    Leave(String),
    Shutdown,
}

/// WebSocket-backed chat transport with automatic reconnection.
pub struct WebSocketChatTransport {
    config: ChatTransportConfig,
    outbound_tx: mpsc::UnboundedSender<TransportCommand>,
    inbound_rx: SyncMutex<Option<mpsc::UnboundedReceiver<ChatMessage>>>,
    connection_task: SyncMutex<Option<JoinHandle<()>>>,
}

impl WebSocketChatTransport {
    /// Establish a WebSocket connection using the provided configuration.
    pub async fn connect(config: ChatTransportConfig) -> NetworkResult<Self> {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();

        let connection_task = tokio::spawn(Self::connection_loop(
            config.clone(),
            outbound_rx,
            inbound_tx,
        ));

        Ok(Self {
            config,
            outbound_tx,
            inbound_rx: SyncMutex::new(Some(inbound_rx)),
            connection_task: SyncMutex::new(Some(connection_task)),
        })
    }

    async fn connection_loop(
        config: ChatTransportConfig,
        mut outbound_rx: mpsc::UnboundedReceiver<TransportCommand>,
        inbound_tx: mpsc::UnboundedSender<ChatMessage>,
    ) {
        let mut pending = VecDeque::new();
        let mut backoff = config.initial_backoff;

        loop {
            // Drain any immediate commands (even before connecting).
            while let Ok(cmd) = outbound_rx.try_recv() {
                if matches!(cmd, TransportCommand::Shutdown) {
                    debug!("Chat transport received shutdown before connect");
                    return;
                }
                pending.push_back(cmd);
            }

            match timeout(
                config.connect_timeout,
                connect_async(config.endpoint.clone()),
            )
            .await
            {
                Ok(Ok((ws_stream, _))) => {
                    info!("Connected to GameSpy chat backend {}", config.endpoint);
                    backoff = config.initial_backoff;
                    let (mut writer, mut reader) = ws_stream.split();

                    if let Some(token) = &config.auth_token {
                        if let Err(err) = Self::send_command(
                            &mut writer,
                            &ChatWireCommand::Auth {
                                token: token.clone(),
                            },
                        )
                        .await
                        {
                            warn!("Failed to send auth command: {}", err);
                        }
                    }

                    // Flush any queued commands.
                    while let Some(cmd) = pending.pop_front() {
                        if matches!(cmd, TransportCommand::Shutdown) {
                            let _ = writer.close().await;
                            return;
                        }
                        if let Err(err) = Self::dispatch_command(&mut writer, &cmd).await {
                            warn!("Transport write failed: {}", err);
                            pending.push_front(cmd);
                            break;
                        }
                    }

                    if !pending.is_empty() {
                        // Connection failed while flushing; retry
                        continue;
                    }

                    // Main loop
                    loop {
                        tokio::select! {
                            cmd = outbound_rx.recv() => {
                                match cmd {
                                    Some(TransportCommand::Shutdown) => {
                                        debug!("Chat transport shutting down");
                                        let _ = writer.close().await;
                                        return;
                                    }
                                    Some(other) => {
                                        if let Err(err) = Self::dispatch_command(&mut writer, &other).await {
                                            warn!("Transport write error: {}", err);
                                            pending.push_back(other);
                                            break;
                                        }
                                    }
                                    None => {
                                        debug!("Outbound channel closed; shutting down transport");
                                        let _ = writer.close().await;
                                        return;
                                    }
                                }
                            }
                            incoming = reader.next() => {
                                match incoming {
                                    Some(Ok(Message::Text(text))) => {
                                        Self::handle_incoming(&inbound_tx, &text);
                                    }
                                    Some(Ok(Message::Binary(bin))) => {
                                        if let Ok(text) = String::from_utf8(bin) {
                                            Self::handle_incoming(&inbound_tx, &text);
                                        }
                                    }
                                    Some(Ok(Message::Close(frame))) => {
                                        debug!("Chat backend closed connection: {:?}", frame);
                                        break;
                                    }
                                    Some(Ok(Message::Ping(payload))) => {
                                        if let Err(err) = writer.send(Message::Pong(payload)).await {
                                            warn!("Failed to reply to ping: {}", err);
                                            break;
                                        }
                                    }
                                    Some(Ok(Message::Pong(_))) => {
                                        trace!("Received pong from chat backend");
                                    }
                                    Some(Ok(Message::Frame(_))) => {
                                        // Frames are handled internally by tungstenite; ignore.
                                    }
                                    Some(Err(err)) => {
                                        warn!("Chat backend read error: {}", err);
                                        break;
                                    }
                                    None => {
                                        debug!("Chat backend stream ended");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Err(err)) => {
                    warn!(
                        "Failed to connect to chat backend {}: {}",
                        config.endpoint, err
                    );
                }
                Err(_) => {
                    warn!("Timed out connecting to chat backend {}", config.endpoint);
                }
            }

            if outbound_rx.is_closed() && pending.is_empty() {
                info!("Outbound channel closed; chat transport loop exiting");
                break;
            }

            let sleep_duration = backoff.min(config.max_backoff);
            debug!(
                "Reconnecting to chat backend in {}ms",
                sleep_duration.as_millis()
            );
            sleep(sleep_duration).await;
            backoff = (backoff * 2).min(config.max_backoff);
        }
    }

    async fn dispatch_command(
        writer: &mut (impl SinkExt<Message, Error = TungsteniteError> + Unpin),
        command: &TransportCommand,
    ) -> Result<(), TungsteniteError> {
        let wire_command = match command {
            TransportCommand::Send(envelope) => ChatWireCommand::Message {
                payload: ChatWireMessage::from_envelope(envelope),
            },
            TransportCommand::Join(room) => ChatWireCommand::Join { room: room.clone() },
            TransportCommand::Leave(room) => ChatWireCommand::Leave { room: room.clone() },
            TransportCommand::Shutdown => ChatWireCommand::Shutdown,
        };

        Self::send_command(writer, &wire_command).await
    }

    async fn send_command(
        writer: &mut (impl SinkExt<Message, Error = TungsteniteError> + Unpin),
        command: &ChatWireCommand,
    ) -> Result<(), TungsteniteError> {
        let payload = serde_json::to_string(command).unwrap_or_default();
        writer.send(Message::Text(payload)).await
    }

    fn handle_incoming(inbound_tx: &mpsc::UnboundedSender<ChatMessage>, payload: &str) {
        match serde_json::from_str::<ChatWireEvent>(payload) {
            Ok(ChatWireEvent::Message {
                sender,
                message,
                room,
                kind,
                timestamp,
            }) => {
                let message_type = kind
                    .and_then(ChatWireMessageKind::into_message_type)
                    .unwrap_or(ChatMessageType::Normal);

                let ts = timestamp
                    .and_then(|stamp| DateTime::parse_from_rfc3339(&stamp).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);

                let chat_message = ChatMessage {
                    sender,
                    message,
                    room,
                    timestamp: ts,
                    message_type,
                };

                let _ = inbound_tx.send(chat_message);
            }
            Ok(ChatWireEvent::System {
                message,
                room,
                timestamp,
            }) => {
                let ts = timestamp
                    .and_then(|stamp| DateTime::parse_from_rfc3339(&stamp).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);

                let system_message = ChatMessage {
                    sender: "System".to_string(),
                    message,
                    room,
                    timestamp: ts,
                    message_type: ChatMessageType::System,
                };

                let _ = inbound_tx.send(system_message);
            }
            Err(err) => {
                warn!(
                    "Failed to parse chat backend payload '{}': {}",
                    payload, err
                );
            }
        }
    }
}

#[async_trait]
impl super::ChatTransport for WebSocketChatTransport {
    async fn send(&self, envelope: ChatEnvelope) -> NetworkResult<()> {
        self.outbound_tx
            .send(TransportCommand::Send(envelope))
            .map_err(|e| NetworkError::generic(format!("failed to enqueue chat message: {}", e)))
    }

    async fn join_room(&self, room: &str) -> NetworkResult<()> {
        self.outbound_tx
            .send(TransportCommand::Join(room.to_string()))
            .map_err(|e| NetworkError::generic(format!("failed to enqueue join: {}", e)))
    }

    async fn leave_room(&self, room: &str) -> NetworkResult<()> {
        self.outbound_tx
            .send(TransportCommand::Leave(room.to_string()))
            .map_err(|e| NetworkError::generic(format!("failed to enqueue leave: {}", e)))
    }

    fn subscribe(&self) -> Option<mpsc::UnboundedReceiver<ChatMessage>> {
        self.inbound_rx.lock().take()
    }
}

impl Drop for WebSocketChatTransport {
    fn drop(&mut self) {
        let _ = self.outbound_tx.send(TransportCommand::Shutdown);
        if let Some(handle) = self.connection_task.lock().take() {
            handle.abort();
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatWireCommand {
    Auth { token: String },
    Join { room: String },
    Leave { room: String },
    Message { payload: ChatWireMessage },
    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatWireMessage {
    sender: String,
    message: String,
    room: Option<String>,
    kind: ChatWireMessageKind,
    timestamp: String,
}

impl ChatWireMessage {
    fn from_envelope(envelope: &ChatEnvelope) -> Self {
        let kind = ChatWireMessageKind::from(envelope.message.message_type);
        let timestamp = envelope
            .message
            .timestamp
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        Self {
            sender: envelope.message.sender.clone(),
            message: envelope.message.message.clone(),
            room: envelope.message.room.clone(),
            kind,
            timestamp,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ChatWireMessageKind {
    Normal,
    Emote,
    Private,
    System,
    Owner,
}

impl ChatWireMessageKind {
    fn into_message_type(self) -> Option<ChatMessageType> {
        match self {
            ChatWireMessageKind::Normal => Some(ChatMessageType::Normal),
            ChatWireMessageKind::Emote => Some(ChatMessageType::Emote),
            ChatWireMessageKind::Private => Some(ChatMessageType::Private),
            ChatWireMessageKind::System => Some(ChatMessageType::System),
            ChatWireMessageKind::Owner => Some(ChatMessageType::Owner),
        }
    }
}

impl From<ChatMessageType> for ChatWireMessageKind {
    fn from(value: ChatMessageType) -> Self {
        match value {
            ChatMessageType::Normal => ChatWireMessageKind::Normal,
            ChatMessageType::Emote => ChatWireMessageKind::Emote,
            ChatMessageType::Private => ChatWireMessageKind::Private,
            ChatMessageType::System => ChatWireMessageKind::System,
            ChatMessageType::Owner => ChatWireMessageKind::Owner,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatWireEvent {
    Message {
        sender: String,
        message: String,
        room: Option<String>,
        kind: Option<ChatWireMessageKind>,
        timestamp: Option<String>,
    },
    System {
        message: String,
        room: Option<String>,
        timestamp: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamespy::chat::ChatTransport;
    use crate::gamespy::{ChatMessage, ChatMessageType, ChatTarget};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[tokio::test]
    async fn websocket_transport_round_trip() {
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
                                let response = json!({
                                    "type": "system",
                                    "message": "joined",
                                    "room": value["room"],
                                    "timestamp": Utc::now().to_rfc3339(),
                                });
                                ws.send(Message::Text(response.to_string())).await.unwrap();
                            }
                            Some("message") => {
                                let payload = &value["payload"];
                                let response = json!({
                                    "type": "message",
                                    "sender": payload["sender"],
                                    "message": payload["message"],
                                    "room": payload["room"],
                                    "kind": payload["kind"],
                                    "timestamp": payload["timestamp"],
                                });
                                ws.send(Message::Text(response.to_string())).await.unwrap();
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

        let mut inbound = transport.subscribe().expect("inbound receiver");

        transport.join_room("#test".into()).await.unwrap();
        let system = inbound.recv().await.unwrap();
        assert_eq!(system.message_type, ChatMessageType::System);

        let message = ChatMessage {
            sender: "Tester".to_string(),
            message: "Hello world".to_string(),
            room: Some("#test".to_string()),
            timestamp: Utc::now(),
            message_type: ChatMessageType::Normal,
        };
        transport
            .send(ChatEnvelope {
                message,
                target: ChatTarget::Room("#test".into()),
            })
            .await
            .unwrap();

        let echoed = inbound.recv().await.unwrap();
        assert_eq!(echoed.message, "Hello world");
        assert_eq!(echoed.room.as_deref(), Some("#test"));

        drop(transport);
        server.abort();
    }
}
