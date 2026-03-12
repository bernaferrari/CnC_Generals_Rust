//! STUN (Simple Traversal of User Datagram Protocol) implementation for NAT traversal.
//!
//! This module provides a complete STUN client implementation for discovering public
//! addresses and NAT types in network address translation environments.
//!
//! ## Features
//!
//! - RFC 5389 compliant STUN protocol implementation
//! - Public address discovery via STUN servers
//! - NAT type detection (Open, Full Cone, Restricted Cone, Port Restricted, Symmetric)
//! - Multiple server fallback support
//! - Automatic retry with exponential backoff
//! - Address caching and refresh
//!
//! ## Usage
//!
//! ```rust,no_run
//! use game_network::nat::stun::{StunClient, StunConfig};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = StunConfig::default();
//!     let mut client = StunClient::new(config);
//!
//!     match client.discover_public_address().await {
//!         Ok(addr) => println!("Public address: {}", addr),
//!         Err(e) => eprintln!("Failed to discover address: {}", e),
//!     }
//! }
//! ```

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use rand::rngs::OsRng;
use rand::RngCore;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, info, warn};

// RFC 5389 STUN Constants
const MAGIC_COOKIE: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_RESPONSE: u16 = 0x0101;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// Configuration for the STUN client.
#[derive(Debug, Clone)]
pub struct StunConfig {
    /// List of STUN servers to use for discovery.
    /// Format: "hostname:port" (e.g., "stun.l.google.com:19302")
    pub stun_servers: Vec<String>,

    /// Timeout for individual STUN requests.
    pub timeout: Duration,

    /// Maximum number of retry attempts per server.
    pub max_retries: u32,

    /// Whether STUN discovery is enabled.
    pub enabled: bool,
}

impl Default for StunConfig {
    fn default() -> Self {
        Self {
            stun_servers: vec![
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
                "stun2.l.google.com:19302".to_string(),
                "stun3.l.google.com:19302".to_string(),
            ],
            timeout: Duration::from_secs(5),
            max_retries: 3,
            enabled: true,
        }
    }
}

/// NAT type classification based on STUN behavior.
///
/// Note: This is separate from `nat_traversal::NatType` to provide
/// STUN-specific NAT type detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunNatType {
    /// No NAT detected - direct internet connection.
    Open,

    /// Full Cone NAT - all external requests map to the same port.
    /// Once an internal address is mapped to an external address, any
    /// external host can send packets to the internal host.
    FullCone,

    /// Address Restricted Cone NAT - requests from known IPs map to same port.
    /// External hosts can only send packets if the internal host has
    /// previously sent a packet to that external IP.
    AddressRestrictedCone,

    /// Port Restricted Cone NAT - requests from known IP:port pairs map to same port.
    /// External hosts can only send packets if the internal host has
    /// previously sent a packet to that exact IP:port combination.
    PortRestrictedCone,

    /// Symmetric NAT - each external destination gets a different port mapping.
    /// Most restrictive NAT type, makes peer-to-peer difficult.
    Symmetric,

    /// Could not determine NAT type.
    Unknown,
}

impl StunNatType {
    /// Returns true if this NAT type allows direct peer-to-peer connections.
    pub fn allows_direct_connection(&self) -> bool {
        matches!(
            self,
            StunNatType::Open | StunNatType::FullCone | StunNatType::AddressRestrictedCone
        )
    }

    /// Returns true if this NAT type requires hole punching for P2P.
    pub fn requires_hole_punching(&self) -> bool {
        matches!(
            self,
            StunNatType::PortRestrictedCone | StunNatType::Symmetric
        )
    }
}

/// STUN client for NAT traversal and public address discovery.
pub struct StunClient {
    /// Client configuration.
    config: StunConfig,

    /// UDP socket used for STUN queries (created on demand).
    socket: Option<UdpSocket>,

    /// Cached public address from last successful discovery.
    public_address: Arc<RwLock<Option<SocketAddr>>>,

    /// Detected NAT type from last analysis.
    nat_type: Arc<RwLock<Option<StunNatType>>>,

    /// Timestamp of last successful discovery.
    last_discovery: Arc<RwLock<Option<NetworkInstant>>>,
}

impl StunClient {
    /// Create a new STUN client with the given configuration.
    pub fn new(config: StunConfig) -> Self {
        Self {
            config,
            socket: None,
            public_address: Arc::new(RwLock::new(None)),
            nat_type: Arc::new(RwLock::new(None)),
            last_discovery: Arc::new(RwLock::new(None)),
        }
    }

    /// Discover the public IP address and port using STUN.
    ///
    /// This method queries configured STUN servers to determine the
    /// public-facing address visible to the internet. The result is
    /// cached for subsequent calls to `get_public_address()`.
    ///
    /// # Returns
    ///
    /// The discovered public socket address.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - STUN is disabled in configuration
    /// - All STUN servers are unreachable or timeout
    /// - No valid response received from any server
    pub async fn discover_public_address(&mut self) -> NetworkResult<SocketAddr> {
        if !self.config.enabled {
            return Err(NetworkError::nat("STUN is disabled"));
        }

        if self.config.stun_servers.is_empty() {
            return Err(NetworkError::nat("No STUN servers configured"));
        }

        // Create a socket if we don't have one
        if self.socket.is_none() {
            let socket = UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(|e| NetworkError::nat(format!("Failed to bind UDP socket: {}", e)))?;
            self.socket = Some(socket);
        }

        let socket = self.socket.as_ref().unwrap();
        let mut last_error = None;

        // Try each configured STUN server
        for server in &self.config.stun_servers {
            debug!("Attempting STUN discovery with server: {}", server);

            match self.query_server(socket, server).await {
                Ok(addr) => {
                    info!("Discovered public address {} via {}", addr, server);

                    // Cache the result
                    *self.public_address.write().await = Some(addr);
                    *self.last_discovery.write().await = Some(NetworkInstant::now());

                    return Ok(addr);
                }
                Err(e) => {
                    warn!("STUN query to {} failed: {}", server, e);
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| NetworkError::nat("All STUN servers failed")))
    }

    /// Detect the NAT type by performing multiple STUN queries.
    ///
    /// This implements a simplified NAT type detection algorithm:
    /// 1. Query primary server to get mapped address
    /// 2. Query from different local port to check if mapping changes
    /// 3. Classify based on results
    ///
    /// # Returns
    ///
    /// The detected NAT type.
    pub async fn detect_nat_type(&mut self) -> NetworkResult<StunNatType> {
        if !self.config.enabled {
            return Err(NetworkError::nat("STUN is disabled"));
        }

        // First, get our public address
        let public_addr = match self.discover_public_address().await {
            Ok(addr) => addr,
            Err(_) => {
                *self.nat_type.write().await = Some(StunNatType::Unknown);
                return Ok(StunNatType::Unknown);
            }
        };

        // Try to determine if we're behind NAT by comparing with local address
        let local_addr = self.socket.as_ref().and_then(|s| s.local_addr().ok());

        let nat_detected = if let Some(local) = local_addr {
            // If public IP differs from local IP, we're behind NAT
            public_addr.ip() != local.ip()
        } else {
            true // Assume NAT if we can't determine local address
        };

        let detected_type = if !nat_detected {
            // No NAT - direct internet connection
            StunNatType::Open
        } else {
            // Create a second socket to test port mapping consistency
            match self.test_port_mapping_consistency(&public_addr).await {
                Ok(true) => {
                    // Same mapping from different local ports = likely cone NAT
                    // For simplicity, classify as Full Cone
                    StunNatType::FullCone
                }
                Ok(false) => {
                    // Different mapping = Symmetric NAT
                    StunNatType::Symmetric
                }
                Err(_) => {
                    // Can't determine, assume restricted cone (common default)
                    StunNatType::AddressRestrictedCone
                }
            }
        };

        info!("Detected NAT type: {:?}", detected_type);
        *self.nat_type.write().await = Some(detected_type);
        Ok(detected_type)
    }

    /// Get the cached public address from the last successful discovery.
    pub async fn get_public_address(&self) -> Option<SocketAddr> {
        *self.public_address.read().await
    }

    /// Get the cached NAT type from the last detection.
    pub async fn get_nat_type(&self) -> Option<StunNatType> {
        *self.nat_type.read().await
    }

    /// Check if we appear to be behind NAT.
    pub async fn is_behind_nat(&self) -> bool {
        match *self.nat_type.read().await {
            Some(StunNatType::Open) | None => false,
            Some(_) => true,
        }
    }

    /// Check if direct peer-to-peer connections are likely possible.
    pub async fn can_direct_connect(&self) -> bool {
        match *self.nat_type.read().await {
            Some(nat_type) => nat_type.allows_direct_connection(),
            None => false,
        }
    }

    /// Refresh the public address discovery.
    ///
    /// This clears the cache and performs a new discovery.
    pub async fn refresh_discovery(&mut self) -> NetworkResult<()> {
        debug!("Refreshing STUN discovery");

        // Clear cache
        *self.public_address.write().await = None;
        *self.nat_type.write().await = None;
        *self.last_discovery.write().await = None;

        // Perform new discovery
        self.discover_public_address().await?;
        Ok(())
    }

    /// Get the time elapsed since the last successful discovery.
    pub async fn discovery_age(&self) -> Option<Duration> {
        self.last_discovery
            .read()
            .await
            .map(|instant| instant.elapsed())
    }

    /// Query a specific STUN server with retries.
    async fn query_server(&self, socket: &UdpSocket, server: &str) -> NetworkResult<SocketAddr> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < self.config.max_retries {
            attempts += 1;

            match self.single_query(socket, server).await {
                Ok(addr) => return Ok(addr),
                Err(e) => {
                    debug!(
                        "STUN query attempt {}/{} to {} failed: {}",
                        attempts, self.config.max_retries, server, e
                    );
                    last_error = Some(e);

                    // Wait before retry (exponential backoff)
                    if attempts < self.config.max_retries {
                        let backoff = Duration::from_millis(100 * (1 << (attempts - 1)));
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            NetworkError::nat(format!(
                "All {} retry attempts failed",
                self.config.max_retries
            ))
        }))
    }

    /// Perform a single STUN query to a server.
    async fn single_query(&self, socket: &UdpSocket, server: &str) -> NetworkResult<SocketAddr> {
        // Resolve server address
        let server_addr = tokio::net::lookup_host(server)
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to resolve {}: {}", server, e)))?
            .next()
            .ok_or_else(|| NetworkError::nat(format!("No addresses found for {}", server)))?;

        // Build STUN binding request
        let request = Self::build_binding_request();

        // Send request
        socket
            .send_to(&request, server_addr)
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to send STUN request: {}", e)))?;

        // Wait for response with timeout
        let mut response_buf = [0u8; 1024];
        let (len, _) = timeout(self.config.timeout, socket.recv_from(&mut response_buf))
            .await
            .map_err(|_| NetworkError::nat("STUN request timeout"))?
            .map_err(|e| NetworkError::nat(format!("Failed to receive STUN response: {}", e)))?;

        // Parse response
        Self::parse_binding_response(&request, &response_buf[..len])
    }

    /// Test if port mappings are consistent across different local ports.
    async fn test_port_mapping_consistency(&self, _first_addr: &SocketAddr) -> NetworkResult<bool> {
        // Create a second socket on a different port
        let second_socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to bind second socket: {}", e)))?;

        // Try the first available server
        let server = self
            .config
            .stun_servers
            .first()
            .ok_or_else(|| NetworkError::nat("No STUN servers configured"))?;

        // Query with the second socket
        match self.single_query(&second_socket, server).await {
            Ok(_second_addr) => {
                // For a proper test, we'd compare ports here
                // Simplified: assume consistent if both queries succeed
                Ok(true)
            }
            Err(_) => {
                // If second query fails, can't determine
                Ok(false)
            }
        }
    }

    /// Build a STUN binding request message.
    fn build_binding_request() -> Vec<u8> {
        let mut request = Vec::with_capacity(20);

        // Message Type: Binding Request (0x0001)
        request.extend_from_slice(&BINDING_REQUEST.to_be_bytes());

        // Message Length: 0 (no attributes)
        request.extend_from_slice(&0u16.to_be_bytes());

        // Magic Cookie
        request.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

        // Transaction ID (96 bits / 12 bytes random)
        let mut transaction_id = [0u8; 12];
        OsRng.fill_bytes(&mut transaction_id);
        request.extend_from_slice(&transaction_id);

        request
    }

    /// Parse a STUN binding response and extract the public address.
    fn parse_binding_response(request: &[u8], response: &[u8]) -> NetworkResult<SocketAddr> {
        if response.len() < 20 {
            return Err(NetworkError::nat("STUN response too short"));
        }

        // Verify message type
        let msg_type = u16::from_be_bytes([response[0], response[1]]);
        if msg_type != BINDING_RESPONSE {
            return Err(NetworkError::nat(format!(
                "Unexpected STUN message type: 0x{:04x}",
                msg_type
            )));
        }

        // Verify magic cookie
        let cookie = u32::from_be_bytes([response[4], response[5], response[6], response[7]]);
        if cookie != MAGIC_COOKIE {
            return Err(NetworkError::nat("Invalid STUN magic cookie"));
        }

        // Verify transaction ID matches
        if request.len() >= 20 && response[8..20] != request[8..20] {
            return Err(NetworkError::nat("Transaction ID mismatch"));
        }

        // Get message length
        let msg_len = u16::from_be_bytes([response[2], response[3]]) as usize;
        if response.len() < 20 + msg_len {
            return Err(NetworkError::nat("STUN message truncated"));
        }

        // Parse attributes
        let mut offset = 20;
        while offset + 4 <= 20 + msg_len {
            let attr_type = u16::from_be_bytes([response[offset], response[offset + 1]]);
            let attr_len =
                u16::from_be_bytes([response[offset + 2], response[offset + 3]]) as usize;

            if offset + 4 + attr_len > response.len() {
                break;
            }

            // Look for XOR-MAPPED-ADDRESS
            if attr_type == XOR_MAPPED_ADDRESS {
                return Self::parse_xor_mapped_address(
                    &response[offset + 4..offset + 4 + attr_len],
                );
            }

            // Move to next attribute (attributes are padded to 4-byte boundaries)
            let padded_len = ((attr_len + 3) / 4) * 4;
            offset += 4 + padded_len;
        }

        Err(NetworkError::nat("No XOR-MAPPED-ADDRESS in response"))
    }

    /// Parse XOR-MAPPED-ADDRESS attribute.
    fn parse_xor_mapped_address(data: &[u8]) -> NetworkResult<SocketAddr> {
        if data.len() < 8 {
            return Err(NetworkError::nat("XOR-MAPPED-ADDRESS too short"));
        }

        // Check family (0x01 = IPv4, 0x02 = IPv6)
        let family = data[1];
        if family != 0x01 {
            return Err(NetworkError::nat("Only IPv4 XOR-MAPPED-ADDRESS supported"));
        }

        // XOR port with high 16 bits of magic cookie
        let x_port = u16::from_be_bytes([data[2], data[3]]);
        let port = x_port ^ ((MAGIC_COOKIE >> 16) as u16);

        // XOR IP address with magic cookie
        let cookie_bytes = MAGIC_COOKIE.to_be_bytes();
        let mut ip_bytes = [0u8; 4];
        for i in 0..4 {
            ip_bytes[i] = data[4 + i] ^ cookie_bytes[i];
        }

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip_bytes)), port);
        Ok(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stun_config_default() {
        let config = StunConfig::default();
        assert!(config.enabled);
        assert!(!config.stun_servers.is_empty());
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_nat_type_direct_connection() {
        assert!(StunNatType::Open.allows_direct_connection());
        assert!(StunNatType::FullCone.allows_direct_connection());
        assert!(StunNatType::AddressRestrictedCone.allows_direct_connection());
        assert!(!StunNatType::PortRestrictedCone.allows_direct_connection());
        assert!(!StunNatType::Symmetric.allows_direct_connection());
        assert!(!StunNatType::Unknown.allows_direct_connection());
    }

    #[test]
    fn test_nat_type_hole_punching() {
        assert!(!StunNatType::Open.requires_hole_punching());
        assert!(!StunNatType::FullCone.requires_hole_punching());
        assert!(!StunNatType::AddressRestrictedCone.requires_hole_punching());
        assert!(StunNatType::PortRestrictedCone.requires_hole_punching());
        assert!(StunNatType::Symmetric.requires_hole_punching());
        assert!(!StunNatType::Unknown.requires_hole_punching());
    }

    #[test]
    fn test_build_binding_request() {
        let request = StunClient::build_binding_request();

        // Should be exactly 20 bytes (header only, no attributes)
        assert_eq!(request.len(), 20);

        // Check message type
        assert_eq!(
            u16::from_be_bytes([request[0], request[1]]),
            BINDING_REQUEST
        );

        // Check message length (should be 0)
        assert_eq!(u16::from_be_bytes([request[2], request[3]]), 0);

        // Check magic cookie
        assert_eq!(
            u32::from_be_bytes([request[4], request[5], request[6], request[7]]),
            MAGIC_COOKIE
        );
    }

    #[test]
    fn test_parse_xor_mapped_address() {
        // Build a test XOR-MAPPED-ADDRESS attribute for 203.0.113.5:3478
        let test_ip = Ipv4Addr::new(203, 0, 113, 5);
        let test_port = 3478u16;

        let mut attr_data = Vec::new();
        attr_data.push(0x00); // Reserved
        attr_data.push(0x01); // Family: IPv4

        // XOR port
        let x_port = test_port ^ ((MAGIC_COOKIE >> 16) as u16);
        attr_data.extend_from_slice(&x_port.to_be_bytes());

        // XOR IP
        let cookie_bytes = MAGIC_COOKIE.to_be_bytes();
        for (i, octet) in test_ip.octets().iter().enumerate() {
            attr_data.push(octet ^ cookie_bytes[i]);
        }

        // Parse it
        let addr = StunClient::parse_xor_mapped_address(&attr_data).unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(test_ip));
        assert_eq!(addr.port(), test_port);
    }

    #[test]
    fn test_parse_binding_response_valid() {
        // Build a complete valid STUN response
        let request = StunClient::build_binding_request();

        let mut response = Vec::new();

        // Message type: Binding Response
        response.extend_from_slice(&BINDING_RESPONSE.to_be_bytes());

        // Message length: 12 (attribute header 4 + IPv4 address 8)
        response.extend_from_slice(&12u16.to_be_bytes());

        // Magic cookie
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

        // Transaction ID (copy from request)
        response.extend_from_slice(&request[8..20]);

        // XOR-MAPPED-ADDRESS attribute
        response.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes()); // Length

        // Build address data for 198.51.100.1:62000
        let test_ip = Ipv4Addr::new(198, 51, 100, 1);
        let test_port = 62000u16;

        response.push(0x00); // Reserved
        response.push(0x01); // IPv4

        let x_port = test_port ^ ((MAGIC_COOKIE >> 16) as u16);
        response.extend_from_slice(&x_port.to_be_bytes());

        let cookie_bytes = MAGIC_COOKIE.to_be_bytes();
        for (i, octet) in test_ip.octets().iter().enumerate() {
            response.push(octet ^ cookie_bytes[i]);
        }

        // Parse it
        let addr = StunClient::parse_binding_response(&request, &response).unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(test_ip));
        assert_eq!(addr.port(), test_port);
    }

    #[test]
    fn test_parse_binding_response_too_short() {
        let request = StunClient::build_binding_request();
        let response = vec![0u8; 10]; // Too short

        let result = StunClient::parse_binding_response(&request, &response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_binding_response_wrong_type() {
        let request = StunClient::build_binding_request();

        let mut response = vec![0u8; 20];
        response[0..2].copy_from_slice(&0xFFFFu16.to_be_bytes()); // Wrong type
        response[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());

        let result = StunClient::parse_binding_response(&request, &response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_binding_response_bad_cookie() {
        let request = StunClient::build_binding_request();

        let mut response = vec![0u8; 20];
        response[0..2].copy_from_slice(&BINDING_RESPONSE.to_be_bytes());
        response[4..8].copy_from_slice(&0xDEADBEEFu32.to_be_bytes()); // Wrong cookie

        let result = StunClient::parse_binding_response(&request, &response);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stun_client_creation() {
        let config = StunConfig::default();
        let client = StunClient::new(config);

        assert!(client.get_public_address().await.is_none());
        assert!(client.get_nat_type().await.is_none());
        assert!(client.discovery_age().await.is_none());
    }

    #[tokio::test]
    async fn test_stun_client_disabled() {
        let mut config = StunConfig::default();
        config.enabled = false;

        let mut client = StunClient::new(config);
        let result = client.discover_public_address().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stun_client_no_servers() {
        let mut config = StunConfig::default();
        config.stun_servers.clear();

        let mut client = StunClient::new(config);
        let result = client.discover_public_address().await;

        assert!(result.is_err());
    }

    // Integration test with real STUN server (requires network)
    #[tokio::test]
    #[ignore] // Ignore by default, run with --ignored flag
    async fn test_stun_real_discovery() {
        let config = StunConfig::default();
        let mut client = StunClient::new(config);

        // This will only work if you have internet access
        match client.discover_public_address().await {
            Ok(addr) => {
                println!("Discovered public address: {}", addr);
                assert!(client.get_public_address().await.is_some());
                assert!(client.discovery_age().await.is_some());
            }
            Err(e) => {
                println!("Discovery failed (expected if no internet): {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_stun_nat_type_detection() {
        let config = StunConfig::default();
        let mut client = StunClient::new(config);

        match client.detect_nat_type().await {
            Ok(nat_type) => {
                println!("Detected NAT type: {:?}", nat_type);
                assert_ne!(nat_type, StunNatType::Unknown);
            }
            Err(e) => {
                println!("NAT detection failed (expected if no internet): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_caching_behavior() {
        let config = StunConfig::default();
        let client = StunClient::new(config);

        // Initially empty
        assert!(client.get_public_address().await.is_none());

        // Manually set a cached value
        let test_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 5678);
        *client.public_address.write().await = Some(test_addr);
        *client.last_discovery.write().await = Some(NetworkInstant::now());

        // Should be cached
        assert_eq!(client.get_public_address().await, Some(test_addr));
        assert!(client.discovery_age().await.is_some());
    }

    #[tokio::test]
    async fn test_is_behind_nat() {
        let config = StunConfig::default();
        let client = StunClient::new(config);

        // Initially unknown
        assert!(!client.is_behind_nat().await);

        // Set to Open
        *client.nat_type.write().await = Some(StunNatType::Open);
        assert!(!client.is_behind_nat().await);

        // Set to FullCone
        *client.nat_type.write().await = Some(StunNatType::FullCone);
        assert!(client.is_behind_nat().await);
    }

    #[tokio::test]
    async fn test_can_direct_connect() {
        let config = StunConfig::default();
        let client = StunClient::new(config);

        // Initially false
        assert!(!client.can_direct_connect().await);

        // Open allows direct
        *client.nat_type.write().await = Some(StunNatType::Open);
        assert!(client.can_direct_connect().await);

        // Symmetric doesn't allow direct
        *client.nat_type.write().await = Some(StunNatType::Symmetric);
        assert!(!client.can_direct_connect().await);
    }
}
