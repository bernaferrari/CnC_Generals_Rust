//! Audio event system and callback management.

use crate::{error::Result, Priority};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Audio event types
#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Channel started playing
    ChannelStarted(u32),
    /// Channel stopped playing  
    ChannelStopped(u32),
    /// Channel paused
    ChannelPaused(u32),
    /// Channel resumed
    ChannelResumed(u32),
    /// Playback completed
    PlaybackComplete(u32),
    /// Audio device changed
    DeviceChanged(String),
    /// Buffer underrun occurred
    BufferUnderrun(u32),
    /// Audio error occurred
    AudioError(String),
}

/// Event priority levels
pub type EventPriority = Priority;

/// Event handler trait
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle audio event
    async fn handle_event(&self, event: AudioEvent) -> Result<()>;

    /// Get handler priority
    fn priority(&self) -> EventPriority {
        EventPriority::Normal
    }
}

/// Audio event manager
pub struct AudioEventManager {
    handlers: Vec<Arc<dyn EventHandler>>,
    event_sender: mpsc::UnboundedSender<AudioEvent>,
    event_receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<AudioEvent>>>,
}

impl AudioEventManager {
    /// Create new event manager
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            handlers: Vec::new(),
            event_sender: sender,
            event_receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
        }
    }

    /// Register event handler
    pub fn register_handler(&mut self, handler: Arc<dyn EventHandler>) {
        self.handlers.push(handler);
        // Sort handlers by priority
        self.handlers
            .sort_by(|a, b| b.priority().cmp(&a.priority()));
    }

    /// Unregister event handler
    pub fn unregister_handler(&mut self, handler: Arc<dyn EventHandler>) {
        self.handlers.retain(|h| !Arc::ptr_eq(h, &handler));
    }

    /// Send event to all registered handlers
    pub async fn send_event(&self, event: AudioEvent) -> Result<()> {
        if let Err(_) = self.event_sender.send(event) {
            return Err(crate::error::Error::Audio(
                "Event channel closed".to_string(),
            ));
        }
        Ok(())
    }

    /// Start event processing loop
    pub async fn start_processing(&self) -> Result<()> {
        let mut receiver = self.event_receiver.lock().await;
        while let Some(event) = receiver.recv().await {
            for handler in &self.handlers {
                if let Err(e) = handler.handle_event(event.clone()).await {
                    log::error!("Event handler error: {:?}", e);
                }
            }
        }
        Ok(())
    }

    /// Get number of registered handlers
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for AudioEventManager {
    fn default() -> Self {
        Self::new()
    }
}
