//! Network addressing utilities

use std::net::{SocketAddr, ToSocketAddrs};

/// Addressing utilities
pub struct AddressingUtils;

impl AddressingUtils {
    /// Parse a human friendly address into a socket address.
    ///
    /// The function accepts:
    /// - Fully qualified `ip:port` pairs (IPv4 or IPv6)
    /// - Hostnames with explicit ports
    /// - Bare hostnames or IPs; these will default to the GameNetwork base port
    pub fn parse_address(addr: &str) -> Option<SocketAddr> {
        let trimmed = addr.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Direct `SocketAddr` parsing (covers IPv6 in brackets and IPv4).
        if let Ok(sock) = trimmed.parse::<SocketAddr>() {
            return Some(sock);
        }

        // For bare host/IP without a port, fall back to the game default port.
        let needs_port = !trimmed.contains(':');
        let query = if needs_port {
            format!("{}:{}", trimmed, crate::config::BASE_PORT)
        } else {
            trimmed.to_string()
        };

        // Resolve using the system's resolver.
        query
            .to_socket_addrs()
            .ok()
            .and_then(|mut iter| iter.next())
    }
}

impl Default for AddressingUtils {
    fn default() -> Self {
        Self
    }
}
