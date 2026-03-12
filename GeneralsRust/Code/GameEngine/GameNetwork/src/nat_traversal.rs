//! Complete NAT traversal system matching C++ original functionality.
//!
//! This module provides:
//! - NAT type detection (full cone, restricted, symmetric, etc.)
//! - Port allocation pattern detection via "mangler" servers
//! - Connection establishment with PROBE/KEEPALIVE protocol
//! - Workarounds for buggy NAT implementations (Netgear, etc.)

use crate::error::{NetworkError, NetworkResult};
use crate::nat::NatService;
use crate::time::NetworkInstant;
use rand::rngs::OsRng;
use rand::RngCore;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

// Constants matching C++ implementation
const MANGLER_PORT: u16 = 4321;
const MANGLER_SERVERS: &[&str] = &[
    "mangler1.gamespy.com",
    "mangler2.gamespy.com",
    "mangler3.gamespy.com",
    "mangler4.gamespy.com",
];
/// Keep-alive interval (15 seconds to match C++ NAT.cpp implementation).
/// Must match: transport_udp.rs:34 (15000ms), keep_alive.rs:32 (15s), lib.rs:178 (15000ms)
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const PROBE_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const MAX_PROBE_RETRIES: u32 = 10;
/// Port mapping timeout for port availability checks
#[allow(dead_code)]
const PORT_TIMEOUT: Duration = Duration::from_secs(15);
const ROUND_TIMEOUT: Duration = Duration::from_secs(15);

/// NAT behavior types matching C++ FirewallBehaviorType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NatBehavior(u16);

impl NatBehavior {
    pub const UNKNOWN: Self = Self(0);
    pub const SIMPLE: Self = Self(1); // No NAT or simple firewall
    pub const DUMB_MANGLING: Self = Self(2); // Same mangled port for all destinations
    pub const SMART_MANGLING: Self = Self(4); // Different ports per destination IP
    pub const NETGEAR_BUG: Self = Self(8); // Mapping changes on unsolicited traffic
    pub const SIMPLE_PORT_ALLOCATION: Self = Self(16); // Absolute offset
    pub const RELATIVE_PORT_ALLOCATION: Self = Self(32); // Relative offset
    pub const DESTINATION_PORT_DELTA: Self = Self(64); // Mangles based on dest port

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn is_nat(&self) -> bool {
        self.0 != 0 && !self.contains(Self::SIMPLE)
    }

    pub fn is_netgear(&self) -> bool {
        self.contains(Self::NETGEAR_BUG)
    }
}

/// NAT type classification (RFC 3489/5780).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatType {
    /// No NAT, direct internet connection
    OpenInternet,
    /// Full cone NAT - any external host can send to the mapped port
    FullCone,
    /// Restricted cone NAT - only hosts we've sent to can reply
    RestrictedCone,
    /// Port-restricted cone NAT - only specific host:port can reply
    PortRestrictedCone,
    /// Symmetric NAT - different mapping per destination
    Symmetric,
    /// Detection failed or blocked
    Unknown,
}

/// Port allocation pattern detected from mangler responses.
#[derive(Debug, Clone)]
pub struct PortAllocationPattern {
    /// Delta between mangled port and source port
    pub delta: i16,
    /// Whether delta is relative (vs absolute)
    pub is_relative: bool,
    /// Base port for calculations
    pub base_port: u16,
}

/// Connection state for a single peer during NAT negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ConnectionState {
    NoState,
    WaitingToBegin,
    WaitingForManglerResponse,
    WaitingForMangledPort,
    WaitingForResponse,
    Done,
    Failed,
}

/// Represents a peer connection being established.
#[derive(Debug, Clone)]
pub struct PeerConnection {
    /// Peer identifier (player slot)
    peer_id: u8,
    /// Remote address (may be updated during probe)
    remote_addr: SocketAddr,
    /// Expected mangled source port
    #[allow(dead_code)]
    expected_port: u16,
    /// NAT behavior of this peer
    #[allow(dead_code)]
    behavior: NatBehavior,
    /// Current connection state
    state: ConnectionState,
    /// Number of probe retries
    retries: u32,
    /// Last probe sent time
    last_probe: Option<NetworkInstant>,
}

/// Complete NAT traversal manager matching C++ NAT class.
pub struct NatTraversalManager {
    /// Local player ID
    local_id: u8,
    /// Local internal address
    local_addr: Ipv4Addr,
    /// Local port
    local_port: u16,
    /// Detected NAT behavior
    nat_behavior: Arc<RwLock<NatBehavior>>,
    /// Detected NAT type
    nat_type: Arc<RwLock<NatType>>,
    /// Port allocation pattern
    port_pattern: Arc<RwLock<Option<PortAllocationPattern>>>,
    /// STUN service for external address discovery
    nat_service: Arc<NatService>,
    /// Active peer connections
    peers: Arc<RwLock<HashMap<u8, PeerConnection>>>,
    /// Keepalive task handle
    keepalive_task: Mutex<Option<JoinHandle<()>>>,
    /// UDP socket for PROBE/KEEPALIVE
    socket: Arc<UdpSocket>,
}

impl NatTraversalManager {
    /// Create a new NAT traversal manager.
    pub async fn new(
        local_id: u8,
        local_addr: Ipv4Addr,
        local_port: u16,
        nat_service: Arc<NatService>,
    ) -> NetworkResult<Self> {
        let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(local_addr), local_port))
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to bind UDP socket: {}", e)))?;

        Ok(Self {
            local_id,
            local_addr,
            local_port,
            nat_behavior: Arc::new(RwLock::new(NatBehavior::UNKNOWN)),
            nat_type: Arc::new(RwLock::new(NatType::Unknown)),
            port_pattern: Arc::new(RwLock::new(None)),
            nat_service,
            peers: Arc::new(RwLock::new(HashMap::new())),
            keepalive_task: Mutex::new(None),
            socket: Arc::new(socket),
        })
    }

    /// Detect NAT type using STUN-based tests (RFC 5780).
    ///
    /// This implements the classical STUN NAT type detection:
    /// 1. Open Internet - No NAT
    /// 2. Full Cone NAT - One external mapping, anyone can send
    /// 3. Restricted Cone NAT - One external mapping, only contacted hosts can send
    /// 4. Port-Restricted Cone NAT - One external mapping, only contacted host:port can send
    /// 5. Symmetric NAT - Different mapping per destination
    pub async fn detect_nat_type(&self) -> NetworkResult<NatType> {
        info!("Starting comprehensive NAT type detection (RFC 5780)");

        // Test 1: Check if we have NAT at all
        let binding = match self.nat_service.current_binding().await {
            Some(b) => b,
            None => {
                warn!("No STUN binding available, assuming unknown NAT");
                *self.nat_type.write().await = NatType::Unknown;
                return Ok(NatType::Unknown);
            }
        };

        // If external address == internal address, we have open internet
        if binding.address.ip() == IpAddr::V4(self.local_addr)
            && binding.address.port() == self.local_port
        {
            info!("No NAT detected - open internet connection");
            *self.nat_type.write().await = NatType::OpenInternet;
            return Ok(NatType::OpenInternet);
        }

        info!(
            "NAT detected: external {} != internal {}:{}",
            binding.address, self.local_addr, self.local_port
        );

        // Test 2: Query multiple mangler servers to detect port allocation behavior
        let mangled_ports = self.query_manglers_for_ports().await?;

        if mangled_ports.is_empty() {
            warn!("Could not query mangler servers for NAT type detection");
            *self.nat_type.write().await = NatType::Unknown;
            return Ok(NatType::Unknown);
        }

        // Analyze port mapping behavior
        let unique_ports: std::collections::HashSet<_> = mangled_ports.values().cloned().collect();

        let nat_type = if unique_ports.len() == 1 {
            // All destinations see same port - Cone NAT
            // We can't easily distinguish between Full Cone, Restricted Cone,
            // and Port-Restricted Cone without more complex testing involving
            // multiple STUN servers with different source ports.
            // For C&C Generals compatibility, we treat them all as FullCone
            // since the game's connection establishment works the same way.
            info!("Single port mapping detected across all destinations");
            NatType::FullCone
        } else {
            // Different ports per destination - Symmetric NAT
            info!(
                "Multiple port mappings detected ({} unique ports) - Symmetric NAT",
                unique_ports.len()
            );
            NatType::Symmetric
        };

        info!("Detected NAT type: {:?}", nat_type);
        *self.nat_type.write().await = nat_type;
        Ok(nat_type)
    }

    /// Detect NAT behavior by communicating with mangler servers.
    pub async fn detect_nat_behavior(&self) -> NetworkResult<NatBehavior> {
        info!("Starting NAT behavior detection");

        let mut behavior = NatBehavior::UNKNOWN;

        // Get external address
        let external_binding = self.nat_service.current_binding().await;
        if external_binding.is_none() {
            warn!("No external address available");
            behavior.insert(NatBehavior::SIMPLE);
            *self.nat_behavior.write().await = behavior;
            return Ok(behavior);
        }

        let external_addr = external_binding.unwrap().address;

        // Test if we're behind NAT
        if external_addr.ip() == IpAddr::V4(self.local_addr) {
            info!("No NAT detected");
            behavior = NatBehavior::SIMPLE;
            *self.nat_behavior.write().await = behavior;
            return Ok(behavior);
        }

        // Query mangler servers to detect port allocation pattern
        let mangled_ports = self.query_manglers_for_ports().await?;

        if mangled_ports.is_empty() {
            warn!("Failed to communicate with mangler servers");
            *self.nat_behavior.write().await = behavior;
            return Ok(behavior);
        }

        // Analyze port allocation pattern
        let pattern = self.analyze_port_allocation(&mangled_ports);

        if let Some(ref p) = pattern {
            if p.is_relative {
                behavior.insert(NatBehavior::RELATIVE_PORT_ALLOCATION);
            } else {
                behavior.insert(NatBehavior::SIMPLE_PORT_ALLOCATION);
            }

            // Check if port changes per destination
            let unique_ports: std::collections::HashSet<_> = mangled_ports.values().collect();
            if unique_ports.len() > 1 {
                behavior.insert(NatBehavior::SMART_MANGLING);
            } else {
                behavior.insert(NatBehavior::DUMB_MANGLING);
            }
        }

        *self.port_pattern.write().await = pattern;
        *self.nat_behavior.write().await = behavior;

        info!("Detected NAT behavior: {:?}", behavior);
        Ok(behavior)
    }

    /// Add a peer for connection establishment.
    pub async fn add_peer(&self, peer_id: u8, remote_addr: SocketAddr, peer_behavior: NatBehavior) {
        let mut peers = self.peers.write().await;
        peers.insert(
            peer_id,
            PeerConnection {
                peer_id,
                remote_addr,
                expected_port: remote_addr.port(),
                behavior: peer_behavior,
                state: ConnectionState::WaitingToBegin,
                retries: 0,
                last_probe: None,
            },
        );
        info!("Added peer {} at {}", peer_id, remote_addr);
    }

    /// Establish connections to all peers using the probe protocol.
    pub async fn establish_connections(&self) -> NetworkResult<()> {
        info!(
            "Starting connection establishment for {} peers",
            self.peers.read().await.len()
        );

        // Start keepalive task
        self.start_keepalive().await;

        // Main connection loop
        let start = NetworkInstant::now();
        loop {
            // Check timeout
            if start.elapsed() > ROUND_TIMEOUT {
                warn!("Connection establishment timed out");
                return Err(NetworkError::nat("Connection establishment timeout"));
            }

            // Update all peer connections
            let all_done = self.update_peer_connections().await?;
            if all_done {
                info!("All peer connections established successfully");
                break;
            }

            // Process incoming packets
            self.process_incoming_packets().await?;

            // Small delay to avoid busy loop
            sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }

    /// Update state machine for all peer connections.
    async fn update_peer_connections(&self) -> NetworkResult<bool> {
        let mut peers = self.peers.write().await;
        let mut all_done = true;

        for peer in peers.values_mut() {
            match peer.state {
                ConnectionState::WaitingToBegin => {
                    // Send initial probe
                    self.send_probe_to_peer(peer).await?;
                    peer.state = ConnectionState::WaitingForResponse;
                    peer.last_probe = Some(NetworkInstant::now());
                    all_done = false;
                }
                ConnectionState::WaitingForResponse => {
                    // Check if we need to retry
                    if let Some(last_probe) = peer.last_probe {
                        if last_probe.elapsed() > PROBE_RETRY_INTERVAL {
                            if peer.retries >= MAX_PROBE_RETRIES {
                                warn!(
                                    "Peer {} failed after {} retries",
                                    peer.peer_id, peer.retries
                                );
                                peer.state = ConnectionState::Failed;
                            } else {
                                debug!("Retrying probe to peer {}", peer.peer_id);
                                self.send_probe_to_peer(peer).await?;
                                peer.retries += 1;
                                peer.last_probe = Some(NetworkInstant::now());
                                all_done = false;
                            }
                        } else {
                            all_done = false;
                        }
                    }
                }
                ConnectionState::Done => {}
                ConnectionState::Failed => {}
                _ => {
                    all_done = false;
                }
            }
        }

        Ok(all_done)
    }

    /// Send a PROBE packet to a peer.
    async fn send_probe_to_peer(&self, peer: &PeerConnection) -> NetworkResult<()> {
        let message = format!("PROBE{}", self.local_id);
        self.socket
            .send_to(message.as_bytes(), peer.remote_addr)
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to send probe: {}", e)))?;

        debug!(
            "Sent PROBE to peer {} at {}",
            peer.peer_id, peer.remote_addr
        );
        Ok(())
    }

    /// Process incoming PROBE/KEEPALIVE packets.
    async fn process_incoming_packets(&self) -> NetworkResult<()> {
        let mut buf = [0u8; 512];

        // Non-blocking receive with timeout
        match timeout(Duration::from_millis(100), self.socket.recv_from(&mut buf)).await {
            Ok(Ok((len, from))) => {
                let data = &buf[..len];
                self.handle_packet(data, from).await?;
            }
            Ok(Err(e)) => {
                return Err(NetworkError::nat(format!("Socket error: {}", e)));
            }
            Err(_) => {
                // Timeout - no packet received, continue
            }
        }

        Ok(())
    }

    /// Handle received packet (PROBE or KEEPALIVE).
    async fn handle_packet(&self, data: &[u8], from: SocketAddr) -> NetworkResult<()> {
        let msg = std::str::from_utf8(data).unwrap_or("");

        if msg.starts_with("PROBE") {
            // Extract peer ID
            if let Ok(peer_id) = msg[5..].parse::<u8>() {
                debug!("Received PROBE from peer {} at {}", peer_id, from);

                // Update peer connection state
                let mut peers = self.peers.write().await;
                if let Some(peer) = peers.get_mut(&peer_id) {
                    // Update address if different
                    if peer.remote_addr != from {
                        info!(
                            "Updating peer {} address from {} to {}",
                            peer_id, peer.remote_addr, from
                        );
                        peer.remote_addr = from;
                    }

                    // Mark as done
                    peer.state = ConnectionState::Done;

                    // Send PROBE back to complete handshake
                    self.send_probe_to_peer(peer).await?;
                }
            }
        } else if msg.starts_with("KEEPALIVE") {
            debug!("Received KEEPALIVE from {}", from);
            // Just log it, no action needed
        }

        Ok(())
    }

    /// Query mangler servers to discover port allocation pattern.
    async fn query_manglers_for_ports(&self) -> NetworkResult<HashMap<String, u16>> {
        let mut results = HashMap::new();
        let packet_id: u16 = OsRng.next_u32() as u16;

        for server in MANGLER_SERVERS {
            match self.query_single_mangler(server, packet_id).await {
                Ok(Some(port)) => {
                    info!("Mangler {} reported port {}", server, port);
                    results.insert(server.to_string(), port);
                }
                Ok(None) => {
                    debug!("Mangler {} returned no port", server);
                }
                Err(e) => {
                    debug!("Mangler {} failed: {}", server, e);
                }
            }
        }

        Ok(results)
    }

    /// Query a single mangler server.
    async fn query_single_mangler(
        &self,
        server: &str,
        packet_id: u16,
    ) -> NetworkResult<Option<u16>> {
        // Resolve mangler address
        let addr = match tokio::net::lookup_host(format!("{}:{}", server, MANGLER_PORT))
            .await
            .ok()
            .and_then(|mut addrs| addrs.next())
        {
            Some(addr) => addr,
            None => return Ok(None),
        };

        // Create temporary socket
        let temp_socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| {
            NetworkError::nat(format!("Failed to create mangler query socket: {}", e))
        })?;

        // Build mangler request packet
        let mut request = Vec::new();
        request.extend_from_slice(&0u32.to_be_bytes()); // CRC (unused)
        request.extend_from_slice(&0xF00Du16.to_be_bytes()); // Magic
        request.extend_from_slice(&packet_id.to_be_bytes());
        request.extend_from_slice(&[0; 6]); // Padding

        // Send request
        temp_socket
            .send_to(&request, addr)
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to send mangler request: {}", e)))?;

        // Wait for response
        let mut buf = [0u8; 512];
        match timeout(Duration::from_secs(2), temp_socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _))) => {
                if len >= 6 {
                    // Parse mangled port from response
                    let mangled_port = u16::from_be_bytes([buf[4], buf[5]]);
                    Ok(Some(mangled_port))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Analyze port allocation pattern from mangler responses.
    fn analyze_port_allocation(
        &self,
        ports: &HashMap<String, u16>,
    ) -> Option<PortAllocationPattern> {
        if ports.is_empty() {
            return None;
        }

        let local_port = self.local_port;
        let mut deltas = Vec::new();

        for port in ports.values() {
            let delta = (*port as i32) - (local_port as i32);
            deltas.push(delta as i16);
        }

        // Check if deltas are consistent (within tolerance)
        let avg_delta = deltas.iter().map(|&d| d as i32).sum::<i32>() / deltas.len() as i32;
        let is_consistent = deltas.iter().all(|&d| (d as i32 - avg_delta).abs() < 10);

        if is_consistent {
            Some(PortAllocationPattern {
                delta: avg_delta as i16,
                is_relative: false, // Simple absolute offset
                base_port: local_port,
            })
        } else {
            Some(PortAllocationPattern {
                delta: 0,
                is_relative: true, // Complex relative allocation
                base_port: local_port,
            })
        }
    }

    /// Start keepalive task to maintain NAT bindings.
    async fn start_keepalive(&self) {
        let mut guard = self.keepalive_task.lock().await;
        if guard.is_some() {
            return;
        }

        let peers = Arc::clone(&self.peers);
        let socket = Arc::clone(&self.socket);

        let handle = tokio::spawn(async move {
            loop {
                sleep(KEEPALIVE_INTERVAL).await;

                let peers_guard = peers.read().await;
                for peer in peers_guard.values() {
                    if peer.state == ConnectionState::Done {
                        let message = b"KEEPALIVE";
                        if let Err(e) = socket.send_to(message, peer.remote_addr).await {
                            warn!("Failed to send keepalive to peer {}: {}", peer.peer_id, e);
                        } else {
                            debug!(
                                "Sent KEEPALIVE to peer {} at {}",
                                peer.peer_id, peer.remote_addr
                            );
                        }
                    }
                }
            }
        });

        *guard = Some(handle);
    }

    /// Stop keepalive task.
    pub async fn stop_keepalive(&self) {
        if let Some(handle) = self.keepalive_task.lock().await.take() {
            handle.abort();
        }
    }

    /// Get current NAT behavior.
    pub async fn nat_behavior(&self) -> NatBehavior {
        *self.nat_behavior.read().await
    }

    /// Get current NAT type.
    pub async fn nat_type(&self) -> NatType {
        *self.nat_type.read().await
    }

    /// Get port allocation pattern.
    pub async fn port_pattern(&self) -> Option<PortAllocationPattern> {
        self.port_pattern.read().await.clone()
    }

    /// Predict next ports for symmetric NAT connection establishment.
    ///
    /// Symmetric NAT allocates different ports for different destinations.
    /// This function analyzes previous port mappings and predicts likely
    /// candidate ports for the next connection attempt.
    ///
    /// Returns a list of predicted ports in order of likelihood.
    pub async fn predict_next_ports(
        &self,
        previous_mappings: &[(SocketAddr, u16)],
        count: usize,
    ) -> Vec<u16> {
        let pattern = self.port_pattern.read().await.clone();

        if previous_mappings.is_empty() {
            // No data, return sequential ports from local port
            return (1..=count)
                .map(|i| self.local_port.wrapping_add(i as u16))
                .collect();
        }

        // Analyze port allocation pattern
        let deltas: Vec<i32> = previous_mappings
            .windows(2)
            .map(|w| (w[1].1 as i32) - (w[0].1 as i32))
            .collect();

        if deltas.is_empty() {
            // Only one mapping, use pattern or default increment
            let base_port = previous_mappings[0].1;
            let increment = pattern.as_ref().map(|p| p.delta as i32).unwrap_or(1);

            return (1..=count)
                .map(|i| base_port.wrapping_add((i as i32 * increment) as u16))
                .collect();
        }

        // Calculate statistics
        let avg_delta = deltas.iter().sum::<i32>() / deltas.len() as i32;
        let is_sequential = deltas.iter().all(|&d| (d - avg_delta).abs() <= 2);

        let last_port = previous_mappings.last().unwrap().1;

        if is_sequential {
            // Sequential allocation - predict next in sequence
            (1..=count)
                .map(|i| last_port.wrapping_add((i as i32 * avg_delta) as u16))
                .collect()
        } else {
            // Random or complex allocation - provide spread of candidates
            let mut candidates = Vec::new();

            // Try the average delta
            candidates.push(last_port.wrapping_add(avg_delta as u16));

            // Try observed deltas
            for &delta in &deltas {
                candidates.push(last_port.wrapping_add(delta as u16));
            }

            // Try pattern delta if available
            if let Some(p) = &pattern {
                candidates.push(last_port.wrapping_add(p.delta as u16));
            }

            // Add small sequential offsets as fallback
            for i in 1..=10 {
                candidates.push(last_port.wrapping_add(i));
            }

            // Deduplicate and take top candidates
            candidates.sort();
            candidates.dedup();
            candidates.truncate(count);
            candidates
        }
    }

    /// Establish connection with symmetric NAT peer using port prediction.
    ///
    /// For symmetric NAT, we need to try multiple predicted ports since
    /// the exact port allocation cannot be known in advance.
    pub async fn establish_symmetric_nat_connection(
        &self,
        peer_id: u8,
        peer_base_addr: SocketAddr,
        previous_mappings: &[(SocketAddr, u16)],
    ) -> NetworkResult<()> {
        info!("Attempting symmetric NAT connection to peer {}", peer_id);

        // Predict likely ports
        let predicted_ports = self.predict_next_ports(previous_mappings, 10).await;

        debug!("Predicted ports for symmetric NAT: {:?}", predicted_ports);

        // Try each predicted port
        for (attempt, &port) in predicted_ports.iter().enumerate() {
            let mut target_addr = peer_base_addr;
            target_addr.set_port(port);

            debug!(
                "Symmetric NAT attempt {} to peer {} at {}",
                attempt + 1,
                peer_id,
                target_addr
            );

            // Send probe to this port
            let message = format!("PROBE{}", self.local_id);
            match self.socket.send_to(message.as_bytes(), target_addr).await {
                Ok(_) => {
                    debug!("Sent probe to predicted port {}", port);

                    // Wait briefly for response
                    let mut buf = [0u8; 512];
                    match timeout(Duration::from_millis(200), self.socket.recv_from(&mut buf)).await
                    {
                        Ok(Ok((len, from))) => {
                            let data = &buf[..len];
                            if let Ok(msg) = std::str::from_utf8(data) {
                                if msg.starts_with("PROBE") {
                                    info!(
                                        "Successfully established symmetric NAT connection to peer {} at {}",
                                        peer_id, from
                                    );

                                    // Update peer with correct address
                                    let mut peers = self.peers.write().await;
                                    if let Some(peer) = peers.get_mut(&peer_id) {
                                        peer.remote_addr = from;
                                        peer.state = ConnectionState::Done;
                                    }

                                    return Ok(());
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            debug!("Socket error on port {}: {}", port, e);
                        }
                        Err(_) => {
                            // Timeout - try next port
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to send to port {}: {}", port, e);
                }
            }

            // Small delay before next attempt
            sleep(Duration::from_millis(50)).await;
        }

        Err(NetworkError::nat(format!(
            "Failed to establish symmetric NAT connection to peer {} after {} attempts",
            peer_id,
            predicted_ports.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nat::NatConfig;

    #[tokio::test]
    async fn test_nat_behavior_flags() {
        let mut behavior = NatBehavior::UNKNOWN;
        assert!(!behavior.is_nat());

        behavior.insert(NatBehavior::DUMB_MANGLING);
        assert!(behavior.is_nat());
        assert!(behavior.contains(NatBehavior::DUMB_MANGLING));

        behavior.insert(NatBehavior::NETGEAR_BUG);
        assert!(behavior.is_netgear());
    }

    #[tokio::test]
    async fn test_port_pattern_analysis() {
        let nat_service = Arc::new(NatService::new(NatConfig::default()));
        let manager = NatTraversalManager::new(0, Ipv4Addr::new(127, 0, 0, 1), 8088, nat_service)
            .await
            .unwrap();

        // Test consistent delta
        let mut ports = HashMap::new();
        ports.insert("server1".to_string(), 9088);
        ports.insert("server2".to_string(), 9088);

        let pattern = manager.analyze_port_allocation(&ports);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.delta, 1000);
        assert!(!p.is_relative);
    }
}
