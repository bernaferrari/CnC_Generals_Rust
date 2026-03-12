//! Game discovery service for LAN games built on modern async primitives.
//!
//! This implementation keeps parity with the legacy C++ LAN discovery flow while
//! modernising the transport mechanics. We support broadcast discovery (for
//! backwards compatibility) and optional mDNS advertisements. Announcements are
//! deduplicated, rate limited, and surfaced through both the internal LAN API
//! messaging channel and an observer-friendly event stream.

use crate::connection::ConnectionManager;
use crate::error::{NetworkError, NetworkResult};
use crate::lan_api::crypto::LanCrypto;
use crate::lan_api::game_info::GameOptions;
use crate::lan_api::{LanBridgeEvent, LanEventSender, LanGameInfo};
use crate::security::SecurityManager;
use crate::time::NetworkInstant;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Error as IoError, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tokio::task::JoinSet;
use tokio::time::interval;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

#[cfg(feature = "mdns")]
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

const DISCOVERY_MAGIC: &[u8; 4] = b"GNZH";
const PROTOCOL_VERSION: u8 = 1;
const MAX_WIRE_SIZE: usize = 1400;

/// Discovery method configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Enable mDNS discovery
    pub enable_mdns: bool,
    /// Enable UDP broadcast discovery
    pub enable_broadcast: bool,
    /// Broadcast address for UDP discovery
    pub broadcast_addr: Ipv4Addr,
    /// mDNS service name
    pub mdns_service: String,
    /// Base port for communication
    pub base_port: u16,
    /// Interval between automatic announcement refreshes
    pub resend_interval: Duration,
    /// Maximum amount of silence tolerated before removing an entry
    pub stale_after: Duration,
}

impl DiscoveryConfig {
    /// Ensure secondary parameters have sane defaults.
    fn sanitised(self) -> Self {
        let stale_after = if self.stale_after.is_zero() {
            self.resend_interval.mul_f32(4.0)
        } else {
            self.stale_after
        };
        Self {
            stale_after,
            ..self
        }
    }
}

/// Discovery method used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    /// UDP broadcast discovery
    Broadcast,
    /// mDNS service discovery
    MDns,
}

/// Events emitted by the discovery service for observers.
#[derive(Debug, Clone)]
pub enum GameDiscoveryEvent {
    /// A game announcement was observed for the first time.
    GameUp(LanGameInfo),
    /// An existing game updated its metadata.
    GameUpdated(LanGameInfo),
    /// A game disappeared from the network.
    GameDown { game_id: Uuid },
    /// Full snapshot of the current discovery table.
    Snapshot(Vec<LanGameInfo>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WireAnnouncement {
    game_id: Uuid,
    name: String,
    port: u16,
    player_count: u8,
    max_players: u8,
    has_password: bool,
    is_public: bool,
    is_direct_connect: bool,
    version_hash: u32,
    map_crc: Option<u32>,
    options: GameOptions,
    public_host: Option<IpAddr>,
    public_port: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum WirePayload {
    Query { request_id: Uuid },
    Announcement(WireAnnouncement),
    Withdrawal { game_id: Uuid },
}

/// Public announcement metadata used by the lobby when hosting a game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameAnnouncement {
    pub game_id: Uuid,
    pub name: String,
    pub host: IpAddr,
    pub port: u16,
    pub player_count: u8,
    pub max_players: u8,
    pub has_password: bool,
    pub is_public: bool,
    pub is_direct_connect: bool,
    pub version_hash: u32,
    pub map_crc: Option<u32>,
    pub options: GameOptions,
    pub public_host: Option<IpAddr>,
    pub public_port: Option<u16>,
}

impl GameAnnouncement {
    fn to_wire(&self) -> WireAnnouncement {
        WireAnnouncement {
            game_id: self.game_id,
            name: self.name.clone(),
            port: self.port,
            player_count: self.player_count,
            max_players: self.max_players,
            has_password: self.has_password,
            is_public: self.is_public,
            is_direct_connect: self.is_direct_connect,
            version_hash: self.version_hash,
            map_crc: self.map_crc,
            options: self.options.clone(),
            public_host: self.public_host,
            public_port: self.public_port,
        }
    }

    fn from_wire(wire: WireAnnouncement, sender: SocketAddr, _method: DiscoveryMethod) -> Self {
        GameAnnouncement {
            game_id: wire.game_id,
            name: wire.name,
            host: sender.ip(),
            port: wire.port,
            player_count: wire.player_count,
            max_players: wire.max_players,
            has_password: wire.has_password,
            is_public: wire.is_public,
            is_direct_connect: wire.is_direct_connect,
            version_hash: wire.version_hash,
            map_crc: wire.map_crc,
            options: wire.options,
            public_host: wire.public_host,
            public_port: wire.public_port,
        }
    }
}

struct LocalAnnouncement {
    descriptor: GameAnnouncement,
    last_sent: NetworkInstant,
}

struct RemoteAnnouncement {
    info: LanGameInfo,
}

/// Game discovery service.
pub struct GameDiscovery {
    config: DiscoveryConfig,
    bridge_tx: LanEventSender,
    crypto: LanCrypto,
    event_tx: broadcast::Sender<GameDiscoveryEvent>,
    socket_v4: RwLock<Option<Arc<UdpSocket>>>,
    #[cfg(feature = "mdns")]
    mdns_daemon: RwLock<Option<ServiceDaemon>>,
    #[cfg(feature = "mdns")]
    mdns_service_name: String,
    local_ip: RwLock<Option<IpAddr>>,
    public_endpoint: RwLock<Option<SocketAddr>>,
    local_announcements: RwLock<HashMap<Uuid, LocalAnnouncement>>,
    remote_announcements: RwLock<HashMap<Uuid, RemoteAnnouncement>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    tasks: Mutex<JoinSet<()>>,
}

impl GameDiscovery {
    /// Create a new game discovery service. Call [`GameDiscovery::init`] before use.
    pub async fn new(config: DiscoveryConfig, bridge_tx: LanEventSender) -> NetworkResult<Self> {
        Self::with_dependencies(config, bridge_tx, None, None).await
    }

    pub async fn with_dependencies(
        config: DiscoveryConfig,
        bridge_tx: LanEventSender,
        security: Option<Arc<SecurityManager>>,
        connections: Option<Arc<RwLock<ConnectionManager>>>,
    ) -> NetworkResult<Self> {
        let sanitised = config.sanitised();
        let (event_tx, _) = broadcast::channel(256);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Ok(Self {
            #[cfg(feature = "mdns")]
            mdns_service_name: sanitised.mdns_service.clone(),
            config: sanitised,
            bridge_tx,
            crypto: LanCrypto::new(security, connections),
            event_tx,
            socket_v4: RwLock::new(None),
            #[cfg(feature = "mdns")]
            mdns_daemon: RwLock::new(None),
            local_ip: RwLock::new(None),
            public_endpoint: RwLock::new(None),
            local_announcements: RwLock::new(HashMap::new()),
            remote_announcements: RwLock::new(HashMap::new()),
            shutdown_tx,
            shutdown_rx,
            tasks: Mutex::new(JoinSet::new()),
        })
    }

    fn io_error(err: IoError) -> NetworkError {
        NetworkError::transport(format!("LAN discovery socket error: {}", err))
    }

    fn bincode_error(err: bincode::Error) -> NetworkError {
        NetworkError::transport(format!("LAN discovery serialisation error: {}", err))
    }

    /// Bind sockets and spawn background tasks.
    pub async fn init(self: &Arc<Self>) -> NetworkResult<()> {
        if self.socket_v4.read().await.is_some() {
            return Ok(());
        }

        if !self.config.enable_broadcast && !self.cfg_mdns_enabled() {
            return Err(NetworkError::configuration(
                "LAN discovery requires at least one discovery method".to_string(),
            ));
        }

        if self.config.enable_broadcast {
            let socket = UdpSocket::bind(SocketAddr::from((
                Ipv4Addr::UNSPECIFIED,
                self.config.base_port,
            )))
            .await
            .map_err(Self::io_error)?;
            socket.set_broadcast(true).map_err(Self::io_error)?;
            info!("LAN discovery listening on UDP {}", self.config.base_port);
            *self.socket_v4.write().await = Some(Arc::new(socket));
        }

        #[cfg(feature = "mdns")]
        if self.config.enable_mdns {
            let daemon = ServiceDaemon::new().map_err(|e| {
                NetworkError::transport(format!("Failed to start mDNS daemon: {}", e))
            })?;
            info!(
                "LAN discovery mDNS service initialised: {}",
                self.mdns_service_name
            );
            *self.mdns_daemon.write().await = Some(daemon);
        }

        let mut tasks = self.tasks.lock().await;
        if let Some(socket) = self.socket_v4.read().await.clone() {
            let worker = Arc::clone(self);
            tasks.spawn(async move {
                worker.recv_loop(socket).await;
            });

            let worker = Arc::clone(self);
            tasks.spawn(async move {
                worker.announce_loop().await;
            });
        }

        let worker = Arc::clone(self);
        tasks.spawn(async move {
            worker.cleanup_loop().await;
        });

        #[cfg(feature = "mdns")]
        if self.config.enable_mdns {
            if let Some(daemon) = self.mdns_daemon.read().await.clone() {
                let worker = Arc::clone(self);
                tasks.spawn(async move {
                    worker.mdns_loop(daemon).await;
                });
            }
        }

        Ok(())
    }

    fn cfg_mdns_enabled(&self) -> bool {
        #[cfg(feature = "mdns")]
        {
            self.config.enable_mdns
        }
        #[cfg(not(feature = "mdns"))]
        {
            false
        }
    }

    /// Provide an observer channel for discovery events.
    pub fn subscribe(&self) -> broadcast::Receiver<GameDiscoveryEvent> {
        self.event_tx.subscribe()
    }

    async fn recv_loop(self: Arc<Self>, socket: Arc<UdpSocket>) {
        let mut shutdown = self.shutdown_rx.clone();
        let mut buffer = vec![0u8; MAX_WIRE_SIZE];

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
                result = socket.recv_from(&mut buffer) => {
                    match result {
                        Ok((len, addr)) => {
                            if let Err(err) = self.handle_datagram(&buffer[..len], addr).await {
                                trace!("Ignoring malformed discovery datagram from {}: {}", addr, err);
                            }
                        }
                        Err(err) => {
                            if err.kind() != ErrorKind::WouldBlock {
                                warn!("Discovery receive error: {}", err);
                            }
                        }
                    }
                }
            }
        }
        debug!("Discovery receive loop terminated");
    }

    async fn handle_datagram(&self, data: &[u8], addr: SocketAddr) -> NetworkResult<()> {
        if data.len() < DISCOVERY_MAGIC.len() + 1 {
            return Err(NetworkError::transport("datagram too small".to_string()));
        }
        if &data[..DISCOVERY_MAGIC.len()] != DISCOVERY_MAGIC {
            return Err(NetworkError::transport(
                "invalid discovery header".to_string(),
            ));
        }
        let version = data[DISCOVERY_MAGIC.len()];
        if version != PROTOCOL_VERSION {
            return Err(NetworkError::transport(format!(
                "unsupported discovery protocol version {}",
                version
            )));
        }
        let plaintext = self
            .crypto
            .decode(&data[DISCOVERY_MAGIC.len() + 1..], addr)
            .await
            .map_err(|err| NetworkError::transport(err.to_string()))?;
        let payload: WirePayload = bincode::deserialize(&plaintext).map_err(Self::bincode_error)?;

        match payload {
            WirePayload::Query { .. } => {
                trace!("Discovery query from {}", addr);
                self.respond_to_query(addr).await?;
            }
            WirePayload::Announcement(wire) => {
                self.ingest_announcement(
                    GameAnnouncement::from_wire(wire, addr, DiscoveryMethod::Broadcast),
                    DiscoveryMethod::Broadcast,
                )
                .await?;
            }
            WirePayload::Withdrawal { game_id } => {
                self.ingest_withdrawal(game_id).await?;
            }
        }
        Ok(())
    }

    async fn respond_to_query(&self, target: SocketAddr) -> NetworkResult<()> {
        let socket = match self.socket_v4.read().await.clone() {
            Some(s) => s,
            None => return Ok(()),
        };

        let locals = self.local_announcements.read().await;
        for announcement in locals.values() {
            self.send_wire(
                &socket,
                target,
                WirePayload::Announcement(announcement.descriptor.to_wire()),
            )
            .await
            .map_err(Self::io_error)?;
        }
        Ok(())
    }

    async fn ingest_announcement(
        &self,
        announcement: GameAnnouncement,
        method: DiscoveryMethod,
    ) -> NetworkResult<()> {
        // Ignore announcements originating from ourselves.
        if self
            .local_announcements
            .read()
            .await
            .contains_key(&announcement.game_id)
        {
            return Ok(());
        }

        let now = Utc::now();
        let mut remote = self.remote_announcements.write().await;
        let entry = remote.entry(announcement.game_id);
        let info = Self::announcement_to_game_info(&announcement, method, now);

        match entry {
            std::collections::hash_map::Entry::Occupied(mut occupied) => {
                let previous = occupied.get_mut();
                previous.info = info.clone();
                let _ = self
                    .event_tx
                    .send(GameDiscoveryEvent::GameUpdated(info.clone()));
            }
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(RemoteAnnouncement { info: info.clone() });
                let _ = self.event_tx.send(GameDiscoveryEvent::GameUp(info.clone()));
            }
        }

        drop(remote);
        self.emit_snapshot().await;
        Ok(())
    }

    async fn ingest_withdrawal(&self, game_id: Uuid) -> NetworkResult<()> {
        let mut remote = self.remote_announcements.write().await;
        if remote.remove(&game_id).is_some() {
            let _ = self.event_tx.send(GameDiscoveryEvent::GameDown { game_id });
            drop(remote);
            self.emit_snapshot().await;
        }
        Ok(())
    }

    async fn announce_loop(self: Arc<Self>) {
        let mut shutdown = self.shutdown_rx.clone();
        let mut ticker = interval(self.config.resend_interval);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
                _ = ticker.tick() => {
                    if let Err(err) = self.flush_local_announcements().await {
                        trace!("Failed to flush local discovery announcements: {}", err);
                    }
                }
            }
        }
        debug!("Discovery announcement loop terminated");
    }

    async fn flush_local_announcements(&self) -> NetworkResult<()> {
        let socket = match self.socket_v4.read().await.clone() {
            Some(s) => s,
            None => return Ok(()),
        };

        let broadcast_target =
            SocketAddr::from((self.config.broadcast_addr, self.config.base_port));
        let mut locals = self.local_announcements.write().await;
        for announcement in locals.values_mut() {
            let should_send = announcement
                .last_sent
                .elapsed()
                .saturating_sub(self.config.resend_interval)
                .is_zero();

            if should_send {
                self.send_wire(
                    socket.as_ref(),
                    broadcast_target,
                    WirePayload::Announcement(announcement.descriptor.to_wire()),
                )
                .await
                .map_err(Self::io_error)?;
                announcement.last_sent = NetworkInstant::now();
            }
        }
        Ok(())
    }

    async fn cleanup_loop(self: Arc<Self>) {
        let mut shutdown = self.shutdown_rx.clone();
        let mut ticker = interval(self.config.stale_after / 2);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
                _ = ticker.tick() => {
                    self.prune_stale_entries().await;
                }
            }
        }
        debug!("Discovery cleanup loop terminated");
    }

    async fn prune_stale_entries(&self) {
        let threshold =
            Utc::now() - chrono::Duration::from_std(self.config.stale_after).unwrap_or_default();
        let mut removed = Vec::new();
        {
            let mut remote = self.remote_announcements.write().await;
            remote.retain(|game_id, announcement| {
                if announcement.info.last_heard < threshold {
                    removed.push(*game_id);
                    false
                } else {
                    true
                }
            });
        }

        if !removed.is_empty() {
            for game_id in &removed {
                let _ = self
                    .event_tx
                    .send(GameDiscoveryEvent::GameDown { game_id: *game_id });
            }
            self.emit_snapshot().await;
        }
    }

    #[cfg(feature = "mdns")]
    async fn mdns_loop(self: Arc<Self>, daemon: ServiceDaemon) {
        let mut shutdown = self.shutdown_rx.clone();
        let service = self.mdns_service_name.clone();
        let receiver = match daemon.browse(&service) {
            Ok(rx) => rx,
            Err(err) => {
                error!("Failed to browse mDNS services: {}", err);
                return;
            }
        };

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
                event = receiver.recv() => {
                    match event {
                        Ok(ServiceEvent::ServiceResolved(info)) => {
                            if let Some(announcement) = Self::mdns_to_announcement(&info) {
                                if let Err(err) = self.ingest_announcement(announcement, DiscoveryMethod::MDns).await {
                                    trace!("Failed to ingest mDNS announcement: {}", err);
                                }
                            }
                        }
                        Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                            let mut to_remove = Vec::new();
                            {
                                let remote = self.remote_announcements.read().await;
                                for (game_id, announcement) in remote.iter() {
                                    if announcement.info.name == fullname {
                                        to_remove.push(*game_id);
                                    }
                                }
                            }
                            for game_id in to_remove {
                                let _ = self.ingest_withdrawal(game_id).await;
                            }
                        }
                        Ok(other) => {
                            trace!("mDNS event: {:?}", other);
                        }
                        Err(err) => {
                            warn!("mDNS receiver error: {}", err);
                        }
                    }
                }
            }
        }
        debug!("Discovery mDNS loop terminated");
    }

    #[cfg(feature = "mdns")]
    fn mdns_to_announcement(info: &ServiceInfo) -> Option<GameAnnouncement> {
        let addresses = info.get_addresses();
        let host = addresses
            .iter()
            .find(|ip| ip.is_ipv4())
            .copied()
            .map(IpAddr::from)
            .or_else(|| addresses.first().copied().map(IpAddr::from))?;

        let mut player_count = 0u8;
        let mut max_players = 8u8;
        let mut has_password = false;
        let mut is_public = true;
        let mut is_direct_connect = false;
        let mut version_hash = 0u32;
        let mut map_crc = None;
        let mut options = GameOptions::default();

        for (key, value) in info.get_properties() {
            let value = std::str::from_utf8(value).unwrap_or("");
            match key.as_str() {
                "player_count" => player_count = value.parse().unwrap_or(player_count),
                "max_players" => max_players = value.parse().unwrap_or(max_players),
                "has_password" => has_password = value.parse().unwrap_or(has_password),
                "is_public" => is_public = value.parse().unwrap_or(is_public),
                "is_direct_connect" => {
                    is_direct_connect = value.parse().unwrap_or(is_direct_connect)
                }
                "version_hash" => version_hash = value.parse().unwrap_or(version_hash),
                "map_crc" => map_crc = value.parse().ok(),
                "options" => {
                    if let Ok(parsed) = GameOptions::from_string(value) {
                        options = parsed;
                    }
                }
                _ => {}
            }
        }

        Some(GameAnnouncement {
            game_id: Uuid::new_v4(),
            name: info.get_fullname().to_string(),
            host,
            port: info.get_port(),
            player_count,
            max_players,
            has_password,
            is_public,
            is_direct_connect,
            version_hash,
            map_crc,
            options,
            public_host: None,
            public_port: None,
        })
    }

    async fn send_wire(
        &self,
        socket: &UdpSocket,
        target: SocketAddr,
        payload: WirePayload,
    ) -> Result<usize, IoError> {
        let encoded =
            bincode::serialize(&payload).map_err(|err| IoError::new(ErrorKind::Other, err))?;
        let cipher = self.crypto.encode(&encoded, target).await;
        let mut buffer = Vec::with_capacity(DISCOVERY_MAGIC.len() + 1 + cipher.len());
        buffer.extend_from_slice(DISCOVERY_MAGIC);
        buffer.push(PROTOCOL_VERSION);
        buffer.extend_from_slice(&cipher);
        socket.send_to(&buffer, target).await
    }

    fn announcement_to_game_info(
        announcement: &GameAnnouncement,
        method: DiscoveryMethod,
        now: DateTime<Utc>,
    ) -> LanGameInfo {
        let mut game = LanGameInfo::new(
            announcement.name.clone(),
            announcement.host,
            announcement.port,
        );
        game.game_id = announcement.game_id;
        game.player_count = announcement.player_count.min(announcement.max_players);
        game.max_players = announcement.max_players;
        game.has_password = announcement.has_password;
        game.is_public = announcement.is_public;
        game.is_direct_connect = announcement.is_direct_connect;
        game.version_hash = announcement.version_hash;
        game.map_crc = announcement.map_crc;
        game.options = announcement.options.clone();
        game.discovery_method = method;
        game.last_heard = now;
        game.public_host = announcement.public_host;
        game.public_port = announcement.public_port;
        game
    }

    async fn emit_snapshot(&self) {
        let mut snapshot = Vec::new();
        let now = Utc::now();

        {
            let locals = self.local_announcements.read().await;
            snapshot.extend(locals.values().map(|entry| {
                Self::announcement_to_game_info(&entry.descriptor, DiscoveryMethod::Broadcast, now)
            }));
        }
        {
            let remote = self.remote_announcements.read().await;
            snapshot.extend(remote.values().map(|entry| entry.info.clone()));
        }

        let _ = self
            .bridge_tx
            .send(LanBridgeEvent::DiscoverySnapshot(snapshot.clone()));
        let _ = self.event_tx.send(GameDiscoveryEvent::Snapshot(snapshot));
    }

    /// Request a refresh of the discovery table by sending a query packet.
    pub async fn request_locations(&self) -> NetworkResult<()> {
        if !self.config.enable_broadcast {
            // mDNS is continuous – still emit the snapshot for observers.
            self.emit_snapshot().await;
            return Ok(());
        }

        let socket = match self.socket_v4.read().await.clone() {
            Some(s) => s,
            None => return Ok(()),
        };

        let target = SocketAddr::from((self.config.broadcast_addr, self.config.base_port));
        self.send_wire(
            socket.as_ref(),
            target,
            WirePayload::Query {
                request_id: Uuid::new_v4(),
            },
        )
        .await
        .map_err(Self::io_error)?;
        self.emit_snapshot().await;
        Ok(())
    }

    /// Register or update a local game announcement. Returns the game identifier.
    pub async fn publish_local(&self, mut announcement: GameAnnouncement) -> NetworkResult<Uuid> {
        if announcement.host.is_unspecified() {
            return Err(NetworkError::configuration(
                "Local announcement requires a concrete host IP".to_string(),
            ));
        }

        if announcement.public_host.is_none() || announcement.public_port.is_none() {
            if let Some(endpoint) = *self.public_endpoint.read().await {
                announcement.public_host = Some(endpoint.ip());
                announcement.public_port = Some(endpoint.port());
            }
        }

        let game_id = announcement.game_id;

        let mut locals = self.local_announcements.write().await;
        locals.insert(
            game_id,
            LocalAnnouncement {
                descriptor: announcement,
                last_sent: NetworkInstant::now()
                    .checked_sub(self.config.resend_interval)
                    .unwrap_or_else(NetworkInstant::now),
            },
        );
        drop(locals);
        self.emit_snapshot().await;
        Ok(game_id)
    }

    /// Update the public endpoint for all locally hosted announcements.
    pub async fn set_public_endpoint(&self, endpoint: Option<SocketAddr>) {
        *self.public_endpoint.write().await = endpoint;

        {
            let mut locals = self.local_announcements.write().await;
            for entry in locals.values_mut() {
                entry.descriptor.public_host = endpoint.map(|addr| addr.ip());
                entry.descriptor.public_port = endpoint.map(|addr| addr.port());
            }
        }

        let local_ids: Vec<Uuid> = {
            let locals = self.local_announcements.read().await;
            locals.keys().copied().collect()
        };

        for game_id in local_ids {
            if let Err(err) = self.refresh_local(game_id).await {
                warn!(
                    "Failed to refresh announcement after public endpoint update: {}",
                    err
                );
            }
        }

        self.emit_snapshot().await;
    }

    #[cfg(test)]
    pub(crate) async fn public_endpoint(&self) -> Option<SocketAddr> {
        *self.public_endpoint.read().await
    }

    /// Force an announcement refresh for a hosted game.
    pub async fn refresh_local(&self, game_id: Uuid) -> NetworkResult<()> {
        let socket = match self.socket_v4.read().await.clone() {
            Some(s) => s,
            None => return Ok(()),
        };
        let broadcast_target =
            SocketAddr::from((self.config.broadcast_addr, self.config.base_port));
        let locals = self.local_announcements.read().await;
        if let Some(announcement) = locals.get(&game_id) {
            self.send_wire(
                socket.as_ref(),
                broadcast_target,
                WirePayload::Announcement(announcement.descriptor.to_wire()),
            )
            .await
            .map_err(Self::io_error)?;
        }
        Ok(())
    }

    /// Withdraw a previously advertised local game.
    pub async fn retract_local(&self, game_id: Uuid) -> NetworkResult<()> {
        let mut locals = self.local_announcements.write().await;
        if locals.remove(&game_id).is_some() {
            drop(locals);
            if self.config.enable_broadcast {
                if let Some(socket) = self.socket_v4.read().await.clone() {
                    let target =
                        SocketAddr::from((self.config.broadcast_addr, self.config.base_port));
                    let _ = self
                        .send_wire(socket.as_ref(), target, WirePayload::Withdrawal { game_id })
                        .await;
                }
            }
            self.emit_snapshot().await;
        }
        Ok(())
    }

    /// Update the local interface IP so we can produce accurate self-announcements.
    pub async fn set_local_ip(&self, ip: IpAddr) {
        *self.local_ip.write().await = Some(ip);
        let mut locals = self.local_announcements.write().await;
        for entry in locals.values_mut() {
            if entry.descriptor.host.is_unspecified() {
                entry.descriptor.host = ip;
            }
        }
    }

    /// Periodic hook invoked by the wider LAN API. Background tasks already keep
    /// the discovery table live, so this is a no-op for compatibility.
    pub async fn update(&self) -> NetworkResult<()> {
        Ok(())
    }

    /// Shutdown discovery services and abort background tasks.
    pub async fn shutdown(&self) -> NetworkResult<()> {
        let _ = self.shutdown_tx.send(true);
        let mut tasks = self.tasks.lock().await;
        tasks.shutdown().await;
        while let Some(res) = tasks.join_next().await {
            if let Err(err) = res {
                warn!("Discovery task aborted: {}", err);
            }
        }
        *self.socket_v4.write().await = None;
        #[cfg(feature = "mdns")]
        {
            *self.mdns_daemon.write().await = None;
        }
        self.local_announcements.write().await.clear();
        self.remote_announcements.write().await.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lan_api::lan_event_channel;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn create_and_publish_snapshot() {
        let (tx, _rx) = lan_event_channel();
        let config = DiscoveryConfig {
            enable_mdns: false,
            enable_broadcast: false,
            broadcast_addr: Ipv4Addr::BROADCAST,
            mdns_service: "_gnzh._udp.local".to_string(),
            base_port: 9999,
            resend_interval: Duration::from_secs(5),
            stale_after: Duration::from_secs(15),
        };

        let discovery = Arc::new(GameDiscovery::new(config, tx).await.unwrap());
        let mut events = discovery.subscribe();

        let announcement = GameAnnouncement {
            game_id: Uuid::new_v4(),
            name: "UnitTest".into(),
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 9000,
            player_count: 1,
            max_players: 8,
            has_password: false,
            is_public: true,
            is_direct_connect: false,
            version_hash: 0xDEADBEEF,
            map_crc: None,
            options: GameOptions::default(),
            public_host: None,
            public_port: None,
        };

        discovery.publish_local(announcement.clone()).await.unwrap();
        if let Ok(GameDiscoveryEvent::Snapshot(games)) = events.recv().await {
            assert_eq!(games.len(), 1);
            assert_eq!(games[0].name, announcement.name);
        } else {
            panic!("no snapshot received");
        }
    }
}
