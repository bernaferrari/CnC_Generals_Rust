//! GameSpy Thread Utilities
//! Background processing utilities for GameSpy services

use crate::error::NetworkResult;
use tokio::sync::mpsc;
use tracing::info;

pub struct ThreadUtils {
    sender: mpsc::Sender<ThreadMessage>,
}

pub enum ThreadMessage {
    ProcessData(Vec<u8>),
    Shutdown,
}

impl ThreadUtils {
    pub async fn new() -> NetworkResult<Self> {
        let (sender, mut receiver) = mpsc::channel(100);

        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                match msg {
                    ThreadMessage::ProcessData(data) => {
                        info!("Processing {} bytes of data", data.len());
                    }
                    ThreadMessage::Shutdown => {
                        info!("Thread shutting down");
                        break;
                    }
                }
            }
        });

        Ok(Self { sender })
    }

    pub async fn send_message(&self, message: ThreadMessage) -> NetworkResult<()> {
        self.sender
            .send(message)
            .await
            .map_err(|_| crate::error::NetworkError::generic("Failed to send thread message"))?;
        Ok(())
    }
}
