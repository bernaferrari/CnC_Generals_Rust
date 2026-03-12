//! NAT service providing STUN-based external address discovery.
//!
//! Provides modernised STUN-based external address discovery, background
//! refresh, and integration hooks with the transport layer.

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use crate::transport::Transport;
use rand::rngs::OsRng;
use rand::RngCore;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{lookup_host, UdpSocket};
use tokio::sync::{mpsc, watch, Mutex as AsyncMutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

const MAGIC_COOKIE: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// Configuration for NAT traversal.
#[derive(Debug, Clone)]
pub struct NatConfig {
    /// STUN servers used for external address discovery.
    pub stun_servers: Vec<String>,
    /// Timeout for a single STUN request.
    pub request_timeout: Duration,
    /// Interval between successful refresh operations.
    pub refresh_interval: Duration,
    /// Maximum refresh interval once backoff is applied after repeated failures.
    pub max_refresh_interval: Duration,
    /// Backoff factor applied after each consecutive failure.
    pub failure_backoff_factor: f32,
    /// Random jitter applied to refresh scheduling to avoid thundering herds.
    pub refresh_jitter: Duration,
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            stun_servers: vec![
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
                "stun2.l.google.com:19302".to_string(),
            ],
            request_timeout: Duration::from_secs(2),
            refresh_interval: Duration::from_secs(30),
            max_refresh_interval: Duration::from_secs(180),
            failure_backoff_factor: 2.0,
            refresh_jitter: Duration::from_millis(500),
        }
    }
}

/// Represents a currently valid NAT binding.
#[derive(Debug, Clone)]
pub struct NatBinding {
    /// Publicly reachable address discovered via STUN.
    pub address: SocketAddr,
    /// STUN server that produced the binding.
    pub server: String,
    /// Round-trip time observed for the query.
    pub round_trip_time: Duration,
    /// Instant when the binding was discovered.
    pub obtained_at: NetworkInstant,
}

impl NatBinding {
    /// Returns how long ago the binding was obtained.
    pub fn age(&self) -> Duration {
        self.obtained_at.elapsed()
    }
}

/// Handles NAT traversal, caching, background refresh and notifications.
#[derive(Debug, Clone)]
pub struct NatService {
    config: NatConfig,
    external_addr: Arc<RwLock<Option<NatBinding>>>,
    notify: watch::Sender<Option<NatBinding>>,
    refresh_task: Arc<AsyncMutex<Option<JoinHandle<()>>>>,
    consecutive_failures: Arc<AtomicU32>,
}

impl NatService {
    pub fn new(config: NatConfig) -> Self {
        let (notify, _rx) = watch::channel(None);
        Self {
            config,
            external_addr: Arc::new(RwLock::new(None)),
            notify,
            refresh_task: Arc::new(AsyncMutex::new(None)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Returns the last known external address, if any.
    pub async fn current_address(&self) -> Option<SocketAddr> {
        self.current_binding().await.map(|binding| binding.address)
    }

    /// Returns the cached NAT binding with metadata, if present.
    pub async fn current_binding(&self) -> Option<NatBinding> {
        self.external_addr.read().await.clone()
    }

    /// Subscribe to binding updates. The returned receiver immediately yields
    /// the latest known binding and subsequently pushes refresh results.
    pub fn subscribe(&self) -> watch::Receiver<Option<NatBinding>> {
        self.notify.subscribe()
    }

    /// Runs discovery and updates both the cache and the transport.
    pub async fn refresh(&self, transport: &Transport) -> NetworkResult<Option<SocketAddr>> {
        let binding = self.discover_binding().await?;
        self.apply_binding(binding, transport).await
    }

    /// Start automatic refresh in the background. Multiple invocations are
    /// ignored once a task is active.
    pub async fn start_auto_refresh(&self, transport: Arc<Transport>) {
        let mut task_guard = self.refresh_task.lock().await;
        if task_guard.is_some() {
            return;
        }

        let service = self.clone();
        let handle = tokio::spawn(async move {
            loop {
                if let Err(err) = service.refresh(&transport).await {
                    warn!("NAT refresh failed: {}", err);
                }

                let failures = service.consecutive_failures.load(Ordering::Relaxed);
                let mut interval = service.config.refresh_interval;
                if failures > 0 {
                    let factor = service
                        .config
                        .failure_backoff_factor
                        .powi((failures.min(8)) as i32);
                    let scaled = interval.mul_f32(factor);
                    interval = std::cmp::min(scaled, service.config.max_refresh_interval);
                }

                let jitter = if service.config.refresh_jitter.is_zero() {
                    Duration::ZERO
                } else {
                    let max_ms = service
                        .config
                        .refresh_jitter
                        .as_millis()
                        .min(u64::MAX as u128) as u64;
                    if max_ms == 0 {
                        Duration::ZERO
                    } else {
                        let mut rng = OsRng;
                        let roll = if max_ms == u64::MAX {
                            rng.next_u64()
                        } else {
                            rng.next_u64() % (max_ms + 1)
                        };
                        Duration::from_millis(roll.min(max_ms))
                    }
                };

                sleep(interval + jitter).await;
            }
        });

        *task_guard = Some(handle);
    }

    /// Stop the background refresh task if it is running.
    pub async fn stop_auto_refresh(&self) {
        let mut guard = self.refresh_task.lock().await;
        if let Some(handle) = guard.take() {
            handle.abort();
        }
    }

    async fn discover_binding(&self) -> NetworkResult<Option<NatBinding>> {
        if self.config.stun_servers.is_empty() {
            warn!("No STUN servers configured; skipping NAT discovery");
            return Ok(None);
        }

        let (tx, mut rx) = mpsc::unbounded_channel();
        for server in self.config.stun_servers.iter().cloned() {
            let tx = tx.clone();
            let timeout = self.config.request_timeout;
            tokio::spawn(async move {
                let started = NetworkInstant::now();
                let outcome = query_server(&server, timeout).await;
                let _ = tx.send((server, outcome, started.elapsed()));
            });
        }
        drop(tx);

        let mut failure_messages = Vec::new();

        while let Some((server, outcome, rtt)) = rx.recv().await {
            match outcome {
                Ok(Some(addr)) => {
                    return Ok(Some(NatBinding {
                        address: addr,
                        server,
                        round_trip_time: rtt,
                        obtained_at: NetworkInstant::now(),
                    }));
                }
                Ok(None) => {
                    failure_messages.push(format!("{} returned no mapping", server));
                }
                Err(err) => {
                    failure_messages.push(format!("{}", err));
                }
            }
        }

        if !failure_messages.is_empty() {
            debug!(failures = ?failure_messages, "All STUN queries failed");
        }

        Ok(None)
    }

    async fn apply_binding(
        &self,
        binding: Option<NatBinding>,
        transport: &Transport,
    ) -> NetworkResult<Option<SocketAddr>> {
        let mut guard = self.external_addr.write().await;
        let changed = match (&*guard, &binding) {
            (Some(current), Some(new)) => current.address != new.address,
            (None, Some(_)) | (Some(_), None) => true,
            (None, None) => false,
        };

        if let Some(ref new_binding) = binding {
            transport.set_public_address(Some(new_binding.address));
            self.consecutive_failures.store(0, Ordering::Relaxed);
            if changed {
                info!(
                    address = %new_binding.address,
                    server = %new_binding.server,
                    rtt_ms = new_binding.round_trip_time.as_millis(),
                    "Discovered public address via STUN"
                );
            }
        } else {
            transport.set_public_address(None);
            self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        }

        *guard = binding.clone();
        drop(guard);

        if changed {
            let _ = self.notify.send(binding.clone());
        }

        Ok(binding.map(|entry| entry.address))
    }
}

async fn query_server(
    server: &str,
    request_timeout: Duration,
) -> NetworkResult<Option<SocketAddr>> {
    let mut resolved = lookup_host(server)
        .await
        .map_err(|err| NetworkError::nat(format!("failed to resolve {}: {}", server, err)))?;

    while let Some(addr) = resolved.next() {
        match query_address(addr, request_timeout).await {
            Ok(Some(public)) => return Ok(Some(public)),
            Ok(None) => continue,
            Err(err) => {
                warn!("STUN query to {} failed: {}", addr, err);
            }
        }
    }

    Ok(None)
}

async fn query_address(
    server: SocketAddr,
    request_timeout: Duration,
) -> NetworkResult<Option<SocketAddr>> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|err| NetworkError::nat(format!("failed to bind UDP socket: {}", err)))?;

    let mut request = [0u8; 20];
    request[0..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    request[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());

    let mut rng = OsRng;
    rng.fill_bytes(&mut request[8..20]);

    socket
        .send_to(&request, server)
        .await
        .map_err(|err| NetworkError::nat(format!("failed to send STUN request: {}", err)))?;

    let mut response = [0u8; 576];
    let recv = timeout(request_timeout, socket.recv_from(&mut response))
        .await
        .map_err(|_| NetworkError::nat("STUN response timed out"))?
        .map_err(|err| NetworkError::nat(format!("failed to receive STUN response: {}", err)))?;

    let (len, _) = recv;
    parse_binding_response(&request, &response[..len])
}

fn parse_binding_response(
    request: &[u8; 20],
    response: &[u8],
) -> NetworkResult<Option<SocketAddr>> {
    if response.len() < 20 {
        return Err(NetworkError::nat("STUN response too short"));
    }

    let message_type = u16::from_be_bytes([response[0], response[1]]);
    if message_type != 0x0101 {
        return Err(NetworkError::nat(format!(
            "unexpected STUN message type {:04x}",
            message_type
        )));
    }

    if response[4..8] != MAGIC_COOKIE.to_be_bytes() {
        return Err(NetworkError::nat("invalid STUN magic cookie"));
    }

    if response[8..20] != request[8..20] {
        return Err(NetworkError::nat("transaction ID mismatch"));
    }

    let message_len = u16::from_be_bytes([response[2], response[3]]) as usize;
    if response.len() < 20 + message_len {
        return Err(NetworkError::nat("STUN response truncated"));
    }

    let mut offset = 20;
    while offset + 4 <= 20 + message_len {
        let attr_type = u16::from_be_bytes([response[offset], response[offset + 1]]);
        let attr_len = u16::from_be_bytes([response[offset + 2], response[offset + 3]]) as usize;
        let value_start = offset + 4;
        let padded_len = ((attr_len + 3) / 4) * 4;

        if value_start + attr_len > response.len() {
            break;
        }

        if attr_type == XOR_MAPPED_ADDRESS {
            return parse_xor_mapped_address(&response[value_start..value_start + attr_len]);
        }

        offset = value_start + padded_len;
    }

    Ok(None)
}

fn parse_xor_mapped_address(data: &[u8]) -> NetworkResult<Option<SocketAddr>> {
    if data.len() < 8 {
        return Err(NetworkError::nat("XOR-MAPPED-ADDRESS attribute too short"));
    }

    let family = data[1];
    if family != 0x01 {
        return Ok(None);
    }

    let x_port = u16::from_be_bytes([data[2], data[3]]);
    let port = x_port ^ ((MAGIC_COOKIE >> 16) as u16);

    let mut ip_bytes = [0u8; 4];
    let cookie_bytes = MAGIC_COOKIE.to_be_bytes();
    for i in 0..4 {
        ip_bytes[i] = data[4 + i] ^ cookie_bytes[i];
    }

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip_bytes)), port);
    Ok(Some(addr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::net::UdpSocket;

    const TEST_PUBLIC_IP: Ipv4Addr = Ipv4Addr::new(198, 51, 100, 7);
    const TEST_PUBLIC_PORT: u16 = 62000;

    #[test]
    fn parses_valid_binding_response() {
        let mut request = [0u8; 20];
        request[0..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
        request[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
        request[8..20].copy_from_slice(&[1u8; 12]);

        let mut response = Vec::new();
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&request[8..20]);
        response.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.push(0x00);
        response.push(0x01);
        let port = 3478 ^ ((MAGIC_COOKIE >> 16) as u16);
        response.extend_from_slice(&port.to_be_bytes());
        let ip = Ipv4Addr::new(203, 0, 113, 1);
        let cookie = MAGIC_COOKIE.to_be_bytes();
        response.push(ip.octets()[0] ^ cookie[0]);
        response.push(ip.octets()[1] ^ cookie[1]);
        response.push(ip.octets()[2] ^ cookie[2]);
        response.push(ip.octets()[3] ^ cookie[3]);

        let addr = parse_binding_response(&request, &response)
            .unwrap()
            .unwrap();
        assert_eq!(addr, SocketAddr::new(IpAddr::V4(ip), 3478));
    }

    #[tokio::test]
    async fn refresh_updates_transport_and_notifies() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let server_socket = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("failed to bind test STUN socket");
        let server_addr = server_socket.local_addr().unwrap();

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            if let Ok((_, peer)) = server_socket.recv_from(&mut buf).await {
                let mut response = Vec::with_capacity(32);
                response.extend_from_slice(&0x0101u16.to_be_bytes());
                response.extend_from_slice(&8u16.to_be_bytes());
                response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
                response.extend_from_slice(&buf[8..20]);
                response.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
                response.extend_from_slice(&8u16.to_be_bytes());
                response.push(0x00);
                response.push(0x01);
                let x_port = TEST_PUBLIC_PORT ^ ((MAGIC_COOKIE >> 16) as u16);
                response.extend_from_slice(&x_port.to_be_bytes());
                let cookie = MAGIC_COOKIE.to_be_bytes();
                for (idx, octet) in TEST_PUBLIC_IP.octets().iter().enumerate() {
                    response.push(octet ^ cookie[idx]);
                }

                let _ = server_socket.send_to(&response, peer).await;
            }
        });

        let mut config = NatConfig::default();
        config.stun_servers = vec![server_addr.to_string()];
        config.request_timeout = Duration::from_millis(500);
        config.refresh_interval = Duration::from_secs(30);

        let service = NatService::new(config);
        let transport = Arc::new(Transport::new().await.expect("transport init failed"));
        let mut updates = service.subscribe();

        let public = service
            .refresh(&transport)
            .await
            .expect("refresh failed")
            .expect("no public address returned");
        let expected_addr = SocketAddr::new(IpAddr::V4(TEST_PUBLIC_IP), TEST_PUBLIC_PORT);
        assert_eq!(public, expected_addr);
        assert_eq!(transport.public_address(), Some(expected_addr));

        tokio::time::timeout(Duration::from_secs(1), updates.changed())
            .await
            .expect("binding update timed out")
            .expect("binding channel closed");
        let binding = updates
            .borrow()
            .clone()
            .expect("binding missing after refresh");
        assert_eq!(binding.address, expected_addr);
        assert_eq!(binding.server, server_addr.to_string());
        assert!(binding.round_trip_time <= Duration::from_secs(1));
    }
}
