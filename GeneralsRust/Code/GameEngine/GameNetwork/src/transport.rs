//! Transport layer built on QUIC.
//!
//! The original GameNetwork stack implemented a bespoke UDP transport with many
//! bespoke reliability features. For the modern port we consolidate the traffic
//! on top of QUIC, allowing us to retain a datagram-oriented API while gaining
//! congestion control, encryption, and stream support out of the box.

use crate::error::{NetworkError, NetworkResult};
use crate::observability::telemetry;
use crate::time::NetworkInstant;
use parking_lot::RwLock as SyncRwLock;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{ClientConfig, Connection, Endpoint, SendStream, ServerConfig};
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{broadcast, Notify, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Transport protocol types. Only QUIC is implemented end-to-end today, but we
/// retain the legacy variants so existing call-sites continue to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportProtocol {
    Tcp,
    Udp,
    WebSocket,
    Quic,
}

/// Snapshot of aggregate transport counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportMetrics {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl TransportProtocol {
    pub fn is_reliable(self) -> bool {
        matches!(
            self,
            TransportProtocol::Tcp | TransportProtocol::WebSocket | TransportProtocol::Quic
        )
    }

    pub fn is_ordered(self) -> bool {
        matches!(
            self,
            TransportProtocol::Tcp | TransportProtocol::WebSocket | TransportProtocol::Quic
        )
    }
}

/// Transport message container mirroring the legacy API surface.
#[derive(Debug, Clone)]
pub struct TransportMessage {
    pub data: Vec<u8>,
    pub protocol: TransportProtocol,
    pub source: Option<SocketAddr>,
    pub destination: Option<SocketAddr>,
    pub timestamp: NetworkInstant,
}

impl TransportMessage {
    /// Create a new transport message
    pub fn new(data: Vec<u8>, protocol: TransportProtocol) -> Self {
        Self {
            data,
            protocol,
            source: None,
            destination: None,
            timestamp: NetworkInstant::now(),
        }
    }

    /// Attach a destination address to the message
    pub fn with_destination(mut self, addr: SocketAddr) -> Self {
        self.destination = Some(addr);
        self
    }

    /// Attach a source address to the message
    pub fn with_source(mut self, addr: SocketAddr) -> Self {
        self.source = Some(addr);
        self
    }
}

/// Transport configuration shared across client/server roles.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Address used when binding as a server or announcing local endpoint
    pub bind_address: SocketAddr,
    /// Protocol – only QUIC is currently supported
    pub protocol: TransportProtocol,
    /// SNI/server name used during QUIC handshakes
    pub server_name: String,
    /// QUIC keep-alive ping interval
    pub keep_alive_interval: Duration,
    /// Maximum idle timeout before a connection is closed
    pub max_idle_timeout: Duration,
    /// Legacy maximum packet size hint
    pub max_packet_size: usize,
    /// Legacy send buffer size hint
    pub send_buffer_size: usize,
    /// Legacy receive buffer size hint
    pub receive_buffer_size: usize,
    /// Legacy broadcast flag retained for compatibility
    pub enable_broadcast: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            bind_address: SocketAddr::from(([127, 0, 0, 1], 8088)),
            protocol: TransportProtocol::Quic,
            server_name: "generals-zero-hour".to_string(),
            keep_alive_interval: Duration::from_secs(15),
            max_idle_timeout: Duration::from_secs(120),
            max_packet_size: 1400,
            send_buffer_size: 65536,
            receive_buffer_size: 65536,
            enable_broadcast: false,
        }
    }
}

/// Lazily generated self-signed certificate shared by all transport instances.
struct SharedCertificate {
    cert_chain: Vec<CertificateDer<'static>>,
    private_key: Vec<u8>,
}

static SHARED_CERTIFICATE: OnceLock<Result<SharedCertificate, NetworkError>> = OnceLock::new();

fn shared_certificate() -> NetworkResult<&'static SharedCertificate> {
    SHARED_CERTIFICATE
        .get_or_init(|| {
            let cert = generate_simple_self_signed(vec![
                "generals-zero-hour".to_string(),
                "localhost".to_string(),
            ])
            .map_err(|err| {
                NetworkError::transport(format!("Failed to generate certificate: {}", err))
            })?;

            let cert_der = CertificateDer::from(cert.serialize_der().map_err(|err| {
                NetworkError::transport(format!("Failed to serialize certificate: {}", err))
            })?);
            let key_der = cert.serialize_private_key_der();

            Ok(SharedCertificate {
                cert_chain: vec![cert_der],
                private_key: key_der,
            })
        })
        .as_ref()
        .map_err(|err| NetworkError::transport(err.to_string()))
}

fn build_transport_configs(
    config: &TransportConfig,
) -> NetworkResult<(ServerConfig, Arc<ClientConfig>)> {
    let shared = shared_certificate()?;

    let mut transport_config = quinn::TransportConfig::default();
    transport_config.keep_alive_interval(Some(config.keep_alive_interval));
    if let Ok(idle) = quinn::IdleTimeout::try_from(config.max_idle_timeout) {
        transport_config.max_idle_timeout(Some(idle));
    }
    let transport_config = Arc::new(transport_config);

    let mut rustls_server = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            shared.cert_chain.clone(),
            PrivatePkcs8KeyDer::from(shared.private_key.clone()).into(),
        )
        .map_err(|err| {
            NetworkError::transport(format!("Failed to build server TLS config: {}", err))
        })?;
    rustls_server.alpn_protocols = vec![b"gnzh/1".to_vec()];

    let quic_server = QuicServerConfig::try_from(rustls_server).map_err(|err| {
        NetworkError::transport(format!("Failed to convert server TLS config: {}", err))
    })?;
    let mut server_config = ServerConfig::with_crypto(Arc::new(quic_server));
    server_config.transport_config(transport_config.clone());

    let mut root_store = rustls::RootCertStore::empty();
    for cert in &shared.cert_chain {
        root_store.add(cert.clone()).map_err(|err| {
            NetworkError::transport(format!("Failed to add root certificate: {}", err))
        })?;
    }

    let mut rustls_client = rustls::ClientConfig::builder()
        .with_root_certificates(Arc::new(root_store))
        .with_no_client_auth();
    rustls_client.alpn_protocols = vec![b"gnzh/1".to_vec()];

    let quic_client = QuicClientConfig::try_from(rustls_client).map_err(|err| {
        NetworkError::transport(format!("Failed to convert client TLS config: {}", err))
    })?;
    let mut client_config = ClientConfig::new(Arc::new(quic_client));
    client_config.transport_config(transport_config);

    Ok((server_config, Arc::new(client_config)))
}

/// QUIC transport implementation.
pub struct Transport {
    config: SyncRwLock<TransportConfig>,
    runtime: RwLock<TransportRuntime>,
    client_endpoint: Arc<Endpoint>,
    connection_tasks: Arc<RwLock<HashMap<SocketAddr, Vec<JoinHandle<()>>>>>,
    connections: Arc<RwLock<HashMap<SocketAddr, Connection>>>,
    packets_sent: Arc<RwLock<u64>>,
    packets_received: Arc<RwLock<u64>>,
    bytes_sent: Arc<RwLock<u64>>,
    bytes_received: Arc<RwLock<u64>>,
    outbound_messages: Arc<RwLock<Vec<TransportMessage>>>,
    inbound_messages: Arc<RwLock<Vec<TransportMessage>>>,
    active_connections: Arc<RwLock<HashMap<SocketAddr, NetworkInstant>>>,
    connection_births: Arc<RwLock<HashMap<SocketAddr, NetworkInstant>>>,
    shutdown_tx: broadcast::Sender<()>,
    incoming_uni_streams: Arc<RwLock<VecDeque<(SocketAddr, quinn::RecvStream)>>>,
    incoming_uni_notify: Arc<Notify>,
    public_address: SyncRwLock<Option<SocketAddr>>,
    client_config: Arc<ClientConfig>,
}

struct TransportRuntime {
    endpoint: Option<Arc<Endpoint>>,
    incoming_task: Option<JoinHandle<()>>,
    is_bound: bool,
    server_config: Option<ServerConfig>,
}

impl Transport {
    /// Create a new transport with default configuration
    pub async fn new() -> NetworkResult<Self> {
        Self::with_config(TransportConfig::default()).await
    }

    /// Create transport with custom configuration
    pub async fn with_config(config: TransportConfig) -> NetworkResult<Self> {
        info!("Creating transport with protocol: {:?}", config.protocol);
        if config.protocol != TransportProtocol::Quic {
            return Err(NetworkError::transport(
                "Only QUIC transport is supported in this build",
            ));
        }

        let (server_config, client_config) = build_transport_configs(&config)?;
        let (shutdown_tx, _) = broadcast::channel(4);

        let client_addr: SocketAddr = SocketAddr::from(([0, 0, 0, 0], 0));
        let mut client_endpoint = Endpoint::client(client_addr).map_err(|err| {
            NetworkError::transport(format!("Failed to create client endpoint: {}", err))
        })?;
        client_endpoint.set_default_client_config((*client_config).clone());

        Ok(Self {
            config: SyncRwLock::new(config),
            runtime: RwLock::new(TransportRuntime {
                endpoint: None,
                incoming_task: None,
                is_bound: false,
                server_config: Some(server_config),
            }),
            client_endpoint: Arc::new(client_endpoint),
            connection_tasks: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            packets_sent: Arc::new(RwLock::new(0)),
            packets_received: Arc::new(RwLock::new(0)),
            bytes_sent: Arc::new(RwLock::new(0)),
            bytes_received: Arc::new(RwLock::new(0)),
            outbound_messages: Arc::new(RwLock::new(Vec::new())),
            inbound_messages: Arc::new(RwLock::new(Vec::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            connection_births: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            incoming_uni_streams: Arc::new(RwLock::new(VecDeque::new())),
            incoming_uni_notify: Arc::new(Notify::new()),
            public_address: SyncRwLock::new(None),
            client_config,
        })
    }

    /// Create a transport for testing purposes
    ///
    /// This method creates a Transport instance suitable for unit tests
    /// without requiring actual network binding or external dependencies.
    ///
    /// # Arguments
    /// * `bind_addr` - Address to use in the configuration
    ///
    /// # Returns
    /// An Arc-wrapped Transport instance ready for testing
    #[cfg(test)]
    pub fn new_for_testing(bind_addr: SocketAddr) -> Arc<Self> {
        let config = TransportConfig {
            bind_address: bind_addr,
            protocol: TransportProtocol::Quic,
            server_name: "test-server".to_string(),
            keep_alive_interval: Duration::from_secs(15),
            max_idle_timeout: Duration::from_secs(120),
            max_packet_size: 1400,
            send_buffer_size: 65536,
            receive_buffer_size: 65536,
            enable_broadcast: false,
        };

        // For testing, we create a minimal transport without actual QUIC setup
        let (server_config, client_config) = build_transport_configs(&config).unwrap();
        let (shutdown_tx, _) = broadcast::channel(4);

        let client_addr: SocketAddr = SocketAddr::from(([0, 0, 0, 0], 0));
        let mut client_endpoint = Endpoint::client(client_addr).unwrap();
        client_endpoint.set_default_client_config((*client_config).clone());

        Arc::new(Self {
            config: SyncRwLock::new(config),
            runtime: RwLock::new(TransportRuntime {
                endpoint: None,
                incoming_task: None,
                is_bound: false,
                server_config: Some(server_config),
            }),
            client_endpoint: Arc::new(client_endpoint),
            connection_tasks: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            packets_sent: Arc::new(RwLock::new(0)),
            packets_received: Arc::new(RwLock::new(0)),
            bytes_sent: Arc::new(RwLock::new(0)),
            bytes_received: Arc::new(RwLock::new(0)),
            outbound_messages: Arc::new(RwLock::new(Vec::new())),
            inbound_messages: Arc::new(RwLock::new(Vec::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            connection_births: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            incoming_uni_streams: Arc::new(RwLock::new(VecDeque::new())),
            incoming_uni_notify: Arc::new(Notify::new()),
            public_address: SyncRwLock::new(None),
            client_config,
        })
    }

    /// Bind to the configured local address and start accepting connections.
    pub async fn bind(&self) -> NetworkResult<()> {
        let config_snapshot = self.config.read().clone();

        let server_config = {
            let mut runtime = self.runtime.write().await;
            if runtime.is_bound {
                return Ok(());
            }
            if runtime.server_config.is_none() {
                let (server_config, _) = build_transport_configs(&config_snapshot)?;
                runtime.server_config = Some(server_config);
            }
            runtime
                .server_config
                .take()
                .ok_or_else(|| NetworkError::transport("Server configuration unavailable"))?
        };

        let mut endpoint =
            Endpoint::server(server_config, config_snapshot.bind_address).map_err(|err| {
                NetworkError::transport(format!("Failed to bind QUIC endpoint: {}", err))
            })?;
        endpoint.set_default_client_config((*self.client_config).clone());
        let endpoint = Arc::new(endpoint);

        let incoming_handle = self.spawn_incoming_processor(endpoint.clone());

        {
            let mut runtime = self.runtime.write().await;
            runtime.endpoint = Some(endpoint);
            runtime.incoming_task = Some(incoming_handle);
            runtime.is_bound = true;
        }
        info!("QUIC transport bound to {}", config_snapshot.bind_address);
        Ok(())
    }

    fn spawn_incoming_processor(&self, endpoint: Arc<Endpoint>) -> JoinHandle<()> {
        let inbound_messages = self.inbound_messages.clone();
        let packets_received = self.packets_received.clone();
        let bytes_received = self.bytes_received.clone();
        let active_connections = self.active_connections.clone();
        let connection_births = self.connection_births.clone();
        let connections = self.connections.clone();
        let connection_tasks = self.connection_tasks.clone();
        let incoming_uni_streams = self.incoming_uni_streams.clone();
        let incoming_uni_notify = self.incoming_uni_notify.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut shutdown_rx = shutdown_rx;
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                    incoming_conn = endpoint.accept() => {
                        match incoming_conn {
                            Some(connecting) => match connecting.await {
                                Ok(connection) => {
                                    register_connection(
                                        connection,
                                        inbound_messages.clone(),
                                        packets_received.clone(),
                                        bytes_received.clone(),
                                        active_connections.clone(),
                                        connection_births.clone(),
                                        connections.clone(),
                                        connection_tasks.clone(),
                                        incoming_uni_streams.clone(),
                                        incoming_uni_notify.clone(),
                                        shutdown_rx.resubscribe(),
                                    ).await;
                                }
                                Err(err) => {
                                    warn!("Incoming QUIC connection failed: {}", err);
                                }
                            },
                            None => break,
                        }
                    }
                }
            }
        });
        handle
    }

    async fn get_or_connect(&self, addr: SocketAddr) -> NetworkResult<Connection> {
        if let Some(existing) = self.connections.read().await.get(&addr) {
            return Ok(existing.clone());
        }

        let config_snapshot = self.config.read().clone();

        let endpoint = {
            let runtime = self.runtime.read().await;
            runtime
                .endpoint
                .clone()
                .unwrap_or_else(|| self.client_endpoint.clone())
        };

        let connecting = endpoint
            .connect(addr, &config_snapshot.server_name)
            .map_err(|err| {
                NetworkError::transport(format!("Failed to initiate QUIC connection: {}", err))
            })?;

        let connection = connecting
            .await
            .map_err(|err| NetworkError::transport(format!("QUIC handshake failed: {}", err)))?;

        let connection_clone = connection.clone();
        register_connection(
            connection_clone,
            self.inbound_messages.clone(),
            self.packets_received.clone(),
            self.bytes_received.clone(),
            self.active_connections.clone(),
            self.connection_births.clone(),
            self.connections.clone(),
            self.connection_tasks.clone(),
            self.incoming_uni_streams.clone(),
            self.incoming_uni_notify.clone(),
            self.shutdown_tx.subscribe(),
        )
        .await;

        Ok(connection)
    }

    /// Open a unidirectional send stream to the specified peer.
    pub async fn open_uni_stream(&self, addr: SocketAddr) -> NetworkResult<SendStream> {
        let connection = self.get_or_connect(addr).await?;
        connection.open_uni().await.map_err(|err| {
            NetworkError::transport(format!("Failed to open QUIC uni-stream: {}", err))
        })
    }

    /// Await an incoming unidirectional receive stream.
    pub async fn recv_uni_stream(&self) -> NetworkResult<(SocketAddr, quinn::RecvStream)> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        loop {
            if let Some(item) = {
                let mut guard = self.incoming_uni_streams.write().await;
                guard.pop_front()
            } {
                return Ok(item);
            }

            tokio::select! {
                _ = self.incoming_uni_notify.notified() => {},
                _ = shutdown_rx.recv() => {
                    return Err(NetworkError::transport("Transport shutting down"));
                }
            }
        }
    }

    /// Send a message through the transport.
    pub async fn send_message(&self, message: TransportMessage) -> NetworkResult<()> {
        if message.protocol != TransportProtocol::Quic {
            return Err(NetworkError::transport(
                "Only QUIC transport is implemented",
            ));
        }

        let destination = message
            .destination
            .ok_or_else(|| NetworkError::transport("Destination address required for QUIC send"))?;

        let connection = self.get_or_connect(destination).await?;
        let payload_len = message.data.len() as u64;
        connection
            .send_datagram(message.data.clone().into())
            .map_err(|err| {
                NetworkError::transport(format!("Failed to send QUIC datagram: {}", err))
            })?;

        {
            let mut outbound = self.outbound_messages.write().await;
            outbound.push(message);
        }

        {
            let mut sent = self.packets_sent.write().await;
            *sent += 1;
            let mut bytes = self.bytes_sent.write().await;
            *bytes += payload_len;
        }

        if let Some(telemetry) = telemetry() {
            telemetry.record_packet_sent(payload_len as usize);
        }

        Ok(())
    }

    /// Establish or reuse a QUIC connection to the provided address.
    pub async fn connect(&self, addr: SocketAddr) -> NetworkResult<()> {
        self.get_or_connect(addr).await.map(|_| ())
    }

    /// Receive pending messages.
    pub async fn receive_messages(&self) -> NetworkResult<Vec<TransportMessage>> {
        let mut inbound = self.inbound_messages.write().await;
        Ok(inbound.drain(..).collect())
    }

    /// Update transport state.
    pub async fn update(&self) -> NetworkResult<()> {
        let mut finished = Vec::new();
        {
            let tasks = self.connection_tasks.read().await;
            for (addr, handles) in tasks.iter() {
                if handles.iter().all(|handle| handle.is_finished()) {
                    finished.push(*addr);
                }
            }
        }

        if !finished.is_empty() {
            let mut removed = Vec::with_capacity(finished.len());
            {
                let mut task_map = self.connection_tasks.write().await;
                for addr in &finished {
                    if let Some(handles) = task_map.remove(addr) {
                        removed.push((*addr, handles));
                    }
                }
            }

            for (addr, handles) in removed {
                for handle in handles {
                    if !handle.is_finished() {
                        handle.abort();
                    }
                }

                self.connections.write().await.remove(&addr);
                let duration = {
                    let mut births = self.connection_births.write().await;
                    births.remove(&addr).map(|started| started.elapsed())
                };
                let remaining = {
                    let mut active = self.active_connections.write().await;
                    active.remove(&addr);
                    active.len()
                };
                if let Some(telemetry) = telemetry() {
                    let observed = duration.unwrap_or_else(|| Duration::from_secs(0));
                    telemetry.record_connection_closed(observed);
                    telemetry.set_active_connections(remaining);
                }
            }
        }

        // Drop idle connections if they exceeded the configured timeout
        let now = NetworkInstant::now();
        let idle_timeout = self.config.read().max_idle_timeout;
        let mut to_close = Vec::new();
        {
            let active = self.active_connections.read().await;
            for (addr, last_seen) in active.iter() {
                if now.duration_since(*last_seen) > idle_timeout {
                    to_close.push(*addr);
                }
            }
        }

        for addr in to_close {
            if let Some(conn) = self.connections.write().await.remove(&addr) {
                conn.close(0u32.into(), b"idle timeout");
            }
            if let Some(handles) = self.connection_tasks.write().await.remove(&addr) {
                for handle in handles {
                    handle.abort();
                }
            }

            let duration = {
                let mut births = self.connection_births.write().await;
                births.remove(&addr).map(|started| started.elapsed())
            };
            let remaining = {
                let mut active = self.active_connections.write().await;
                active.remove(&addr);
                active.len()
            };

            if let Some(telemetry) = telemetry() {
                let observed = duration.unwrap_or_else(|| Duration::from_secs(0));
                telemetry.record_connection_closed(observed);
                telemetry.set_active_connections(remaining);
            }
        }

        Ok(())
    }

    /// Shutdown the transport and terminate all background tasks.
    pub async fn shutdown(&self) -> NetworkResult<()> {
        let _ = self.shutdown_tx.send(());

        let (incoming_handle, endpoint_handle) = {
            let mut runtime = self.runtime.write().await;
            let incoming = runtime.incoming_task.take();
            let endpoint = runtime.endpoint.take();
            runtime.is_bound = false;
            (incoming, endpoint)
        };

        if let Some(handle) = incoming_handle {
            if !handle.is_finished() {
                handle.abort();
            }
        }

        let mut tasks = self.connection_tasks.write().await;
        for (_, handles) in tasks.drain() {
            for handle in handles {
                if !handle.is_finished() {
                    handle.abort();
                }
            }
        }

        if let Some(endpoint) = endpoint_handle {
            endpoint.close(0u32.into(), b"transport shutdown");
        }
        self.client_endpoint
            .close(0u32.into(), b"transport shutdown");

        self.incoming_uni_notify.notify_waiters();
        self.incoming_uni_streams.write().await.clear();

        self.connections.write().await.clear();
        self.active_connections.write().await.clear();
        self.inbound_messages.write().await.clear();
        self.outbound_messages.write().await.clear();

        Ok(())
    }

    /// Get packets sent count (async)
    pub async fn packets_sent(&self) -> u64 {
        *self.packets_sent.read().await
    }

    /// Get packets received count (async)
    pub async fn packets_received(&self) -> u64 {
        *self.packets_received.read().await
    }

    /// Get bytes sent count (async)
    pub async fn bytes_sent(&self) -> u64 {
        *self.bytes_sent.read().await
    }

    /// Get bytes received count (async)
    pub async fn bytes_received(&self) -> u64 {
        *self.bytes_received.read().await
    }

    /// Snapshot aggregate metrics.
    pub async fn metrics(&self) -> TransportMetrics {
        TransportMetrics {
            packets_sent: *self.packets_sent.read().await,
            packets_received: *self.packets_received.read().await,
            bytes_sent: *self.bytes_sent.read().await,
            bytes_received: *self.bytes_received.read().await,
        }
    }

    /// Determine whether the transport is bound to a socket.
    pub async fn is_bound(&self) -> bool {
        self.runtime.read().await.is_bound
    }

    /// Check if transport is bound and ready
    pub fn is_ready(&self) -> bool {
        self.runtime
            .try_read()
            .map(|rt| rt.is_bound)
            .unwrap_or(false)
    }

    /// Get transport configuration snapshot
    pub fn config(&self) -> TransportConfig {
        self.config.read().clone()
    }

    /// Update bind address for subsequent bind attempts.
    pub fn set_bind_address(&self, bind_address: SocketAddr) -> NetworkResult<()> {
        if self.is_ready() {
            return Err(NetworkError::transport(
                "Cannot change bind address after transport is bound",
            ));
        }

        self.config.write().bind_address = bind_address;
        Ok(())
    }

    /// Set the discovered public address (e.g., from NAT traversal).
    pub fn set_public_address(&self, address: Option<SocketAddr>) {
        *self.public_address.write() = address;
    }

    /// Retrieve the public address if known.
    pub fn public_address(&self) -> Option<SocketAddr> {
        *self.public_address.read()
    }
}

async fn register_connection(
    connection: Connection,
    inbound_messages: Arc<RwLock<Vec<TransportMessage>>>,
    packets_received: Arc<RwLock<u64>>,
    bytes_received: Arc<RwLock<u64>>,
    active_connections: Arc<RwLock<HashMap<SocketAddr, NetworkInstant>>>,
    connection_births: Arc<RwLock<HashMap<SocketAddr, NetworkInstant>>>,
    connections: Arc<RwLock<HashMap<SocketAddr, Connection>>>,
    connection_tasks: Arc<RwLock<HashMap<SocketAddr, Vec<JoinHandle<()>>>>>,
    incoming_uni_streams: Arc<RwLock<VecDeque<(SocketAddr, quinn::RecvStream)>>>,
    incoming_uni_notify: Arc<Notify>,
    shutdown_rx: broadcast::Receiver<()>,
) {
    let remote_addr = connection.remote_address();
    info!("Established QUIC connection with {}", remote_addr);

    let handshake_started = NetworkInstant::now();

    {
        let mut map = connections.write().await;
        map.insert(remote_addr, connection.clone());
    }

    let active_count = {
        let mut active = active_connections.write().await;
        active.insert(remote_addr, handshake_started);
        active.len()
    };

    {
        let mut births = connection_births.write().await;
        births.insert(remote_addr, handshake_started);
    }

    if let Some(telemetry) = telemetry() {
        telemetry.record_connection_established();
        telemetry.set_active_connections(active_count);
    }

    let inbound_messages_clone = inbound_messages.clone();
    let packets_received_clone = packets_received.clone();
    let bytes_received_clone = bytes_received.clone();
    let active_connections_clone = active_connections.clone();
    let connection_births_clone = connection_births.clone();
    let connections_clone = connections.clone();
    let incoming_uni_streams_clone = incoming_uni_streams.clone();
    let incoming_uni_notify_clone = incoming_uni_notify.clone();
    let active_connections_stream = active_connections.clone();

    let mut datagram_shutdown = shutdown_rx.resubscribe();
    let mut stream_shutdown = shutdown_rx;

    let datagram_connection = connection.clone();
    let datagram_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = datagram_shutdown.recv() => {
                    break;
                }
                result = datagram_connection.read_datagram() => {
                    match result {
                        Ok(data) => {
                            {
                                let mut queue = inbound_messages_clone.write().await;
                                queue.push(
                                    TransportMessage::new(data.to_vec(), TransportProtocol::Quic)
                                        .with_source(remote_addr),
                                );
                            }

                            *packets_received_clone.write().await += 1;
                            *bytes_received_clone.write().await += data.len() as u64;
                            if let Some(telemetry) = telemetry() {
                                telemetry.record_packet_received(data.len(), Duration::from_secs(0));
                            }
                            active_connections_clone
                                .write()
                                .await
                                .insert(remote_addr, NetworkInstant::now());
                        }
                        Err(err) => {
                            warn!("Datagram receive error from {}: {}", remote_addr, err);
                            break;
                        }
                    }
                }
            }
        }

        let connection_duration = {
            let mut births = connection_births_clone.write().await;
            births.remove(&remote_addr).map(|started| started.elapsed())
        };
        let active_remaining = {
            let mut active = active_connections_clone.write().await;
            active.remove(&remote_addr);
            active.len()
        };
        connections_clone.write().await.remove(&remote_addr);
        if let Some(telemetry) = telemetry() {
            let duration = connection_duration.unwrap_or_else(|| Duration::from_secs(0));
            telemetry.record_connection_closed(duration);
            telemetry.set_active_connections(active_remaining);
        }
        debug!("Connection with {} closed", remote_addr);
    });

    let uni_connection = connection.clone();
    let stream_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = stream_shutdown.recv() => {
                    break;
                }
                result = uni_connection.accept_uni() => {
                    match result {
                        Ok(recv_stream) => {
                            {
                                let mut queue = incoming_uni_streams_clone.write().await;
                                queue.push_back((remote_addr, recv_stream));
                            }
                            incoming_uni_notify_clone.notify_one();
                            active_connections_stream
                                .write()
                                .await
                                .insert(remote_addr, NetworkInstant::now());
                        }
                        Err(err) => {
                            warn!("Uni-stream receive error from {}: {}", remote_addr, err);
                            break;
                        }
                    }
                }
            }
        }
    });

    if let Some(existing) = connection_tasks
        .write()
        .await
        .insert(remote_addr, vec![datagram_task, stream_task])
    {
        for handle in existing {
            if !handle.is_finished() {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transport_creation() {
        let transport = Transport::new().await.unwrap();
        assert!(!transport.is_ready());
        assert_eq!(transport.packets_sent().await, 0);
        assert_eq!(transport.packets_received().await, 0);
    }

    #[tokio::test]
    async fn test_quic_send_receive_loopback() {
        let mut server_config = TransportConfig::default();
        server_config.bind_address = SocketAddr::from(([127, 0, 0, 1], 19001));

        let server = Transport::with_config(server_config.clone()).await.unwrap();
        server.bind().await.unwrap();

        let client = Transport::with_config(TransportConfig::default())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        let message = TransportMessage::new(b"hello".to_vec(), TransportProtocol::Quic)
            .with_destination(server_config.bind_address);
        client.send_message(message).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        let received = server.receive_messages().await.unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].data, b"hello");
    }
}
