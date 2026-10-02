#![allow(dead_code, unused_imports, unused_variables)]
//! GameSpy Ping Service
//! Handles latency measurement and server connectivity testing

use crate::error::NetworkResult;
use std::collections::HashMap;
use tracing::info;

/// Ping cache owned by the GameSpy interface. Only the owning task's methods
/// touch it, so it is a plain field with no shared lock.
pub struct PingService {
    servers: HashMap<String, u32>,
}

impl PingService {
    pub async fn new() -> NetworkResult<Self> {
        Ok(Self {
            servers: HashMap::new(),
        })
    }

    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Started ping service");
        Ok(())
    }

    pub async fn stop(&mut self) -> NetworkResult<()> {
        info!("Stopped ping service");
        Ok(())
    }

    pub async fn get_ping(&self, server: String) -> NetworkResult<u32> {
        // Simulate ping measurement
        Ok(50) // 50ms
    }
}
