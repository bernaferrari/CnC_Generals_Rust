//! Unified transport layer supporting both QUIC and UDP transports.
//!
//! This module provides a common interface for transport-agnostic code while supporting
//! both the legacy QUIC transport and the new UDP transport that matches C++ exactly.

use crate::error::NetworkResult;
use crate::transport::{
    Transport as QuicTransport, TransportMessage, TransportMetrics, TransportProtocol,
};
use crate::transport_udp::Transport as UdpTransport;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

/// Unified transport that can delegate to either QUIC or UDP
pub enum UnifiedTransport {
    /// Original QUIC-based transport
    Quic(Arc<QuicTransport>),
    /// New UDP transport matching C++ exactly
    Udp(Arc<UdpTransport>),
}

impl UnifiedTransport {
    /// Create a new UDP transport (C++ compatible)
    pub async fn new_udp() -> NetworkResult<Self> {
        let transport = Arc::new(UdpTransport::new().await?);
        info!("Created UDP transport (C++ compatible)");
        Ok(Self::Udp(transport))
    }

    /// Create a new QUIC transport (legacy)
    pub async fn new_quic() -> NetworkResult<Self> {
        let transport = Arc::new(QuicTransport::new().await?);
        info!("Created QUIC transport (legacy)");
        Ok(Self::Quic(transport))
    }

    /// Create default transport - UDP (C++ compatible)
    pub async fn new() -> NetworkResult<Self> {
        Self::new_udp().await
    }

    /// Bind to configured address
    pub async fn bind(&self) -> NetworkResult<()> {
        match self {
            Self::Quic(t) => t.bind().await,
            Self::Udp(t) => t.bind().await,
        }
    }

    /// Send a message
    pub async fn send_message(&self, message: TransportMessage) -> NetworkResult<()> {
        match self {
            Self::Quic(t) => t.send_message(message).await,
            Self::Udp(t) => {
                // Convert message to UDP protocol if needed
                let mut msg = message;
                if msg.protocol == TransportProtocol::Quic {
                    msg.protocol = TransportProtocol::Udp;
                }
                t.send_message(msg).await
            }
        }
    }

    /// Receive pending messages
    pub async fn receive_messages(&self) -> NetworkResult<Vec<TransportMessage>> {
        match self {
            Self::Quic(t) => t.receive_messages().await,
            Self::Udp(t) => t.receive_messages().await,
        }
    }

    /// Update transport state
    pub async fn update(&self) -> NetworkResult<()> {
        match self {
            Self::Quic(t) => t.update().await,
            Self::Udp(t) => t.update().await,
        }
    }

    /// Shutdown transport
    pub async fn shutdown(&self) -> NetworkResult<()> {
        match self {
            Self::Quic(t) => t.shutdown().await,
            Self::Udp(t) => t.shutdown().await,
        }
    }

    /// Get metrics
    pub async fn metrics(&self) -> TransportMetrics {
        match self {
            Self::Quic(t) => t.metrics().await,
            Self::Udp(t) => t.metrics().await,
        }
    }

    /// Get packets sent
    pub async fn packets_sent(&self) -> u64 {
        match self {
            Self::Quic(t) => t.packets_sent().await,
            Self::Udp(t) => t.packets_sent().await,
        }
    }

    /// Get packets received
    pub async fn packets_received(&self) -> u64 {
        match self {
            Self::Quic(t) => t.packets_received().await,
            Self::Udp(t) => t.packets_received().await,
        }
    }

    /// Get bytes sent
    pub async fn bytes_sent(&self) -> u64 {
        match self {
            Self::Quic(t) => t.bytes_sent().await,
            Self::Udp(t) => t.bytes_sent().await,
        }
    }

    /// Get bytes received
    pub async fn bytes_received(&self) -> u64 {
        match self {
            Self::Quic(t) => t.bytes_received().await,
            Self::Udp(t) => t.bytes_received().await,
        }
    }

    /// Check if transport is bound
    pub async fn is_bound(&self) -> bool {
        match self {
            Self::Quic(t) => t.is_bound().await,
            Self::Udp(t) => t.is_bound().await,
        }
    }

    /// Check if transport is ready
    pub fn is_ready(&self) -> bool {
        match self {
            Self::Quic(t) => t.is_ready(),
            Self::Udp(t) => t.is_ready(),
        }
    }

    /// Get configuration
    pub fn config(&self) -> crate::transport::TransportConfig {
        match self {
            Self::Quic(t) => t.config(),
            Self::Udp(_) => {
                // Return a default config for UDP
                crate::transport::TransportConfig::default()
            }
        }
    }

    /// Set bind address
    pub fn set_bind_address(&self, bind_address: SocketAddr) -> NetworkResult<()> {
        match self {
            Self::Quic(t) => t.set_bind_address(bind_address),
            Self::Udp(t) => t.set_bind_address(bind_address),
        }
    }

    /// Set public address
    pub fn set_public_address(&self, address: Option<SocketAddr>) {
        match self {
            Self::Quic(t) => t.set_public_address(address),
            Self::Udp(t) => t.set_public_address(address),
        }
    }

    /// Get public address
    pub fn public_address(&self) -> Option<SocketAddr> {
        match self {
            Self::Quic(t) => t.public_address(),
            Self::Udp(t) => t.public_address(),
        }
    }

    /// Connect to remote address
    pub async fn connect(&self, addr: SocketAddr) -> NetworkResult<()> {
        match self {
            Self::Quic(t) => t.connect(addr).await,
            Self::Udp(_) => {
                // UDP is connectionless, but we record the address
                Ok(())
            }
        }
    }

    /// Check if using UDP transport
    pub fn is_udp(&self) -> bool {
        matches!(self, Self::Udp(_))
    }

    /// Check if using QUIC transport
    pub fn is_quic(&self) -> bool {
        matches!(self, Self::Quic(_))
    }
}
