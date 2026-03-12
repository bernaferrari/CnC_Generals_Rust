//! Connection pool management for efficient resource usage
//!
//! This module provides connection pooling functionality to manage
//! multiple network connections efficiently, with load balancing
//! and automatic cleanup.

use crate::connection::Connection;
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use crate::transport::{Transport, TransportProtocol};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock, Semaphore};
use tokio::time::timeout;
use tracing::{debug, info, trace, warn};

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections in pool
    pub max_connections: usize,
    /// Minimum number of idle connections to maintain
    pub min_idle: usize,
    /// Maximum number of idle connections
    pub max_idle: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Idle connection timeout
    pub idle_timeout: Duration,
    /// Connection validation interval
    pub validation_interval: Duration,
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
    /// Enable connection reuse
    pub enable_reuse: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            min_idle: 2,
            max_idle: 10,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300), // 5 minutes
            validation_interval: Duration::from_secs(60),
            max_lifetime: Duration::from_secs(3600), // 1 hour
            enable_reuse: true,
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Total connections created
    pub total_created: usize,
    /// Total connections destroyed
    pub total_destroyed: usize,
    /// Current active connections
    pub active_connections: usize,
    /// Current idle connections
    pub idle_connections: usize,
    /// Pool hits (reused connections)
    pub pool_hits: usize,
    /// Pool misses (new connections)
    pub pool_misses: usize,
    /// Failed connection attempts
    pub failed_connections: usize,
    /// Average connection age
    pub average_connection_age_ms: f64,
}

/// Pooled connection wrapper
#[derive(Debug)]
struct PooledConnection {
    connection: Arc<Connection>,
    created_at: NetworkInstant,
    last_used: NetworkInstant,
    use_count: usize,
    is_active: bool,
}

impl PooledConnection {
    fn new(connection: Arc<Connection>) -> Self {
        let now = NetworkInstant::now();
        Self {
            connection,
            created_at: now,
            last_used: now,
            use_count: 0,
            is_active: false,
        }
    }

    fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    fn idle_time(&self) -> Duration {
        self.last_used.elapsed()
    }

    fn mark_used(&mut self) {
        self.last_used = NetworkInstant::now();
        self.use_count += 1;
        self.is_active = true;
    }

    fn mark_idle(&mut self) {
        self.is_active = false;
    }

    async fn is_valid(&self) -> bool {
        self.connection.is_active().await
    }
}

/// Connection pool for managing network connections
pub struct ConnectionPool {
    /// Pool configuration
    config: PoolConfig,

    /// Transport reference
    transport: Arc<Transport>,

    /// Active connections (by remote address)
    active_connections: Arc<RwLock<HashMap<SocketAddr, PooledConnection>>>,

    /// Idle connection queue
    idle_queue: Arc<RwLock<VecDeque<PooledConnection>>>,

    /// Pool statistics
    stats: Arc<RwLock<PoolStats>>,

    /// Connection semaphore for limiting max connections
    connection_semaphore: Arc<Semaphore>,

    /// Pool usage counter
    usage_counter: AtomicUsize,

    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,

    /// Shutdown signal
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl ConnectionPool {
    /// Create new connection pool
    pub fn new(transport: Arc<Transport>) -> Self {
        Self::with_config(transport, PoolConfig::default())
    }

    /// Create connection pool with custom configuration
    pub fn with_config(transport: Arc<Transport>, config: PoolConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            connection_semaphore: Arc::new(Semaphore::new(config.max_connections)),
            config,
            transport,
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            idle_queue: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(PoolStats::default())),
            usage_counter: AtomicUsize::new(0),
            task_handles: Vec::new(),
            shutdown_tx,
        }
    }

    /// Start background pool maintenance tasks
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Starting connection pool with config: {:?}", self.config);

        let (shutdown_tx, _) = broadcast::channel(1);

        // Connection validator task
        let validator_task = {
            let active_connections = self.active_connections.clone();
            let idle_queue = self.idle_queue.clone();
            let stats = self.stats.clone();
            let config = self.config.clone();
            let mut shutdown_rx_clone = shutdown_tx.subscribe();

            tokio::spawn(async move {
                Self::connection_validator_task(
                    active_connections,
                    idle_queue,
                    stats,
                    config,
                    &mut shutdown_rx_clone,
                )
                .await;
            })
        };

        // Pool maintainer task
        let maintainer_task = {
            let idle_queue = self.idle_queue.clone();
            let stats = self.stats.clone();
            let config = self.config.clone();
            let mut shutdown_rx_clone = shutdown_tx.subscribe();

            tokio::spawn(async move {
                Self::pool_maintainer_task(idle_queue, stats, config, &mut shutdown_rx_clone).await;
            })
        };

        self.task_handles.push(validator_task);
        self.task_handles.push(maintainer_task);

        Ok(())
    }

    /// Get a connection from the pool or create a new one
    pub async fn get_connection(
        &self,
        remote_addr: SocketAddr,
        protocol: TransportProtocol,
    ) -> NetworkResult<Arc<Connection>> {
        let usage_id = self.usage_counter.fetch_add(1, Ordering::Relaxed);
        trace!("Pool request #{} for {}", usage_id, remote_addr);

        // Check if we have an active connection for this address
        {
            let mut active = self.active_connections.write().await;
            if let Some(pooled) = active.get_mut(&remote_addr) {
                if pooled.is_valid().await {
                    pooled.mark_used();

                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.pool_hits += 1;
                    }

                    trace!("Pool hit #{} for {}", usage_id, remote_addr);
                    return Ok(pooled.connection.clone());
                } else {
                    // Connection is no longer valid, remove it
                    active.remove(&remote_addr);
                }
            }
        }

        // Try to get an idle connection
        if self.config.enable_reuse {
            let mut idle_queue = self.idle_queue.write().await;

            // Look for a suitable idle connection
            for i in 0..idle_queue.len() {
                let pooled = &idle_queue[i];
                if pooled.is_valid().await && pooled.age() < self.config.max_lifetime {
                    // Found a good idle connection
                    let mut pooled = idle_queue.remove(i).unwrap();
                    pooled.mark_used();

                    // Move to active connections
                    {
                        let mut active = self.active_connections.write().await;
                        active.insert(remote_addr, pooled);
                    }

                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.pool_hits += 1;
                        stats.active_connections += 1;
                        stats.idle_connections -= 1;
                    }

                    trace!("Pool reuse #{} for {}", usage_id, remote_addr);
                    return Ok(idle_queue[i].connection.clone());
                }
            }
        }

        // Need to create a new connection
        self.create_new_connection(remote_addr, protocol, usage_id)
            .await
    }

    /// Create a new connection
    async fn create_new_connection(
        &self,
        remote_addr: SocketAddr,
        protocol: TransportProtocol,
        usage_id: usize,
    ) -> NetworkResult<Arc<Connection>> {
        // Acquire semaphore permit
        let _permit = timeout(
            self.config.connection_timeout,
            self.connection_semaphore.acquire(),
        )
        .await
        .map_err(|_| NetworkError::connection("pool connection timeout"))?
        .map_err(|_| NetworkError::connection("semaphore closed"))?;

        debug!("Creating new connection #{} to {}", usage_id, remote_addr);

        // Create the connection
        let connection = match timeout(
            self.config.connection_timeout,
            Connection::new(0, remote_addr, protocol, self.transport.clone()),
        )
        .await
        {
            Ok(Ok(conn)) => Arc::new(conn),
            Ok(Err(e)) => {
                // Update error stats
                {
                    let mut stats = self.stats.write().await;
                    stats.failed_connections += 1;
                }
                return Err(e);
            }
            Err(_) => {
                let mut stats = self.stats.write().await;
                stats.failed_connections += 1;
                return Err(NetworkError::connection("connection creation timeout"));
            }
        };

        // Create pooled wrapper
        let mut pooled = PooledConnection::new(connection.clone());
        pooled.mark_used();

        // Add to active connections
        {
            let mut active = self.active_connections.write().await;
            active.insert(remote_addr, pooled);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_created += 1;
            stats.active_connections += 1;
            stats.pool_misses += 1;
        }

        info!("Created new connection #{} to {}", usage_id, remote_addr);
        Ok(connection)
    }

    /// Return a connection to the pool
    pub async fn return_connection(&self, remote_addr: SocketAddr) -> NetworkResult<()> {
        let mut active = self.active_connections.write().await;

        if let Some(mut pooled) = active.remove(&remote_addr) {
            pooled.mark_idle();

            // Check if we should keep this connection in the idle pool
            if self.config.enable_reuse
                && pooled.age() < self.config.max_lifetime
                && pooled.is_valid().await
            {
                let mut idle_queue = self.idle_queue.write().await;

                // Only keep if we're under the idle limit
                if idle_queue.len() < self.config.max_idle {
                    idle_queue.push_back(pooled);

                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.active_connections -= 1;
                        stats.idle_connections += 1;
                    }

                    trace!("Returned connection to idle pool: {}", remote_addr);
                    return Ok(());
                }
            }

            // Connection will be dropped and destroyed
            {
                let mut stats = self.stats.write().await;
                stats.total_destroyed += 1;
                stats.active_connections -= 1;
            }

            trace!("Destroyed connection: {}", remote_addr);
        }

        Ok(())
    }

    /// Get current pool statistics
    pub async fn get_stats(&self) -> PoolStats {
        let mut stats = self.stats.read().await.clone();

        // Calculate average connection age
        let active = self.active_connections.read().await;
        let idle = self.idle_queue.read().await;

        let total_age: u64 = active
            .values()
            .chain(idle.iter())
            .map(|conn| conn.age().as_millis() as u64)
            .sum();

        let total_connections = active.len() + idle.len();
        stats.average_connection_age_ms = if total_connections > 0 {
            total_age as f64 / total_connections as f64
        } else {
            0.0
        };

        stats
    }

    /// Get pool health status
    pub async fn health_check(&self) -> PoolHealthStatus {
        let stats = self.get_stats().await;
        let config = &self.config;

        let utilization = if config.max_connections > 0 {
            stats.active_connections as f64 / config.max_connections as f64
        } else {
            0.0
        };

        let status = if utilization > 0.9 {
            PoolHealth::Critical
        } else if utilization > 0.7 {
            PoolHealth::Warning
        } else {
            PoolHealth::Healthy
        };

        PoolHealthStatus {
            health: status,
            utilization_percent: utilization * 100.0,
            active_connections: stats.active_connections,
            idle_connections: stats.idle_connections,
            max_connections: config.max_connections,
            failed_connections: stats.failed_connections,
        }
    }

    /// Force cleanup of idle connections
    pub async fn cleanup_idle(&self) -> usize {
        let mut idle_queue = self.idle_queue.write().await;
        let mut cleaned = 0;

        // Remove expired or invalid connections
        idle_queue.retain(|pooled| {
            if pooled.idle_time() > self.config.idle_timeout
                || pooled.age() > self.config.max_lifetime
            {
                cleaned += 1;
                false
            } else {
                true
            }
        });

        if cleaned > 0 {
            let mut stats = self.stats.write().await;
            stats.total_destroyed += cleaned;
            stats.idle_connections = idle_queue.len();
        }

        debug!("Cleaned {} idle connections", cleaned);
        cleaned
    }

    /// Connection validator background task
    async fn connection_validator_task(
        active_connections: Arc<RwLock<HashMap<SocketAddr, PooledConnection>>>,
        idle_queue: Arc<RwLock<VecDeque<PooledConnection>>>,
        stats: Arc<RwLock<PoolStats>>,
        config: PoolConfig,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        debug!("Starting connection validator task");

        let mut interval = tokio::time::interval(config.validation_interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    Self::validate_connections(
                        &active_connections,
                        &idle_queue,
                        &stats,
                        &config,
                    ).await;
                }
                _ = shutdown_rx.recv() => {
                    debug!("Connection validator task shutting down");
                    break;
                }
            }
        }
    }

    /// Validate all connections in the pool
    async fn validate_connections(
        active_connections: &Arc<RwLock<HashMap<SocketAddr, PooledConnection>>>,
        idle_queue: &Arc<RwLock<VecDeque<PooledConnection>>>,
        stats: &Arc<RwLock<PoolStats>>,
        config: &PoolConfig,
    ) {
        let removed_active = {
            let mut active = active_connections.write().await;
            let mut to_remove = Vec::new();

            for (addr, pooled) in active.iter() {
                if pooled.age() > config.max_lifetime || !pooled.is_valid().await {
                    to_remove.push(*addr);
                }
            }

            let removed = to_remove.len();
            for addr in &to_remove {
                active.remove(addr);
            }

            removed
        };

        let removed_idle = {
            let mut idle = idle_queue.write().await;
            let original_len = idle.len();

            idle.retain(|pooled| {
                pooled.idle_time() <= config.idle_timeout && pooled.age() <= config.max_lifetime
            });

            original_len.saturating_sub(idle.len())
        };

        // Update statistics
        if removed_active > 0 || removed_idle > 0 {
            let mut stats_guard = stats.write().await;
            stats_guard.total_destroyed += removed_active + removed_idle;
            stats_guard.active_connections -= removed_active;
            stats_guard.idle_connections -= removed_idle;

            debug!(
                "Validated connections: removed {} active, {} idle",
                removed_active, removed_idle
            );
        }
    }

    /// Pool maintainer background task
    async fn pool_maintainer_task(
        idle_queue: Arc<RwLock<VecDeque<PooledConnection>>>,
        stats: Arc<RwLock<PoolStats>>,
        config: PoolConfig,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        debug!("Starting pool maintainer task");

        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    Self::maintain_pool(&idle_queue, &stats, &config).await;
                }
                _ = shutdown_rx.recv() => {
                    debug!("Pool maintainer task shutting down");
                    break;
                }
            }
        }
    }

    /// Maintain pool size and health
    async fn maintain_pool(
        idle_queue: &Arc<RwLock<VecDeque<PooledConnection>>>,
        stats: &Arc<RwLock<PoolStats>>,
        config: &PoolConfig,
    ) {
        let idle_count = {
            let idle = idle_queue.read().await;
            idle.len()
        };

        // Log pool status periodically
        let current_stats = stats.read().await.clone();
        trace!(
            "Pool status: active={}, idle={}, hits={}, misses={}",
            current_stats.active_connections,
            idle_count,
            current_stats.pool_hits,
            current_stats.pool_misses
        );

        // Trim excess idle connections
        if idle_count > config.max_idle {
            let mut idle = idle_queue.write().await;
            let to_remove = idle_count - config.max_idle;

            for _ in 0..to_remove {
                idle.pop_front();
            }

            let mut stats_guard = stats.write().await;
            stats_guard.total_destroyed += to_remove;
            stats_guard.idle_connections = idle.len();

            debug!("Trimmed {} excess idle connections", to_remove);
        }
    }

    /// Shutdown the connection pool
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down connection pool");

        // Send shutdown signal
        if let Err(e) = self.shutdown_tx.send(()) {
            warn!("Failed to send pool shutdown signal: {}", e);
        }

        // Close all active connections
        {
            let mut active = self.active_connections.write().await;
            for (addr, pooled) in active.drain() {
                if let Err(e) = pooled.connection.disconnect().await {
                    warn!("Error disconnecting {}: {}", addr, e);
                }
            }
        }

        // Close all idle connections
        {
            let mut idle = self.idle_queue.write().await;
            while let Some(pooled) = idle.pop_front() {
                if let Err(e) = pooled.connection.disconnect().await {
                    warn!("Error disconnecting idle connection: {}", e);
                }
            }
        }

        // Wait for background tasks
        for handle in &self.task_handles {
            handle.abort();
        }

        info!("Connection pool shutdown complete");
        Ok(())
    }
}

/// Pool health status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolHealth {
    Healthy,
    Warning,
    Critical,
}

/// Detailed pool health status
#[derive(Debug, Clone)]
pub struct PoolHealthStatus {
    /// Overall health status of the pool
    pub health: PoolHealth,
    /// Current utilization as a percentage (0.0 to 100.0)
    pub utilization_percent: f64,
    /// Number of currently active connections
    pub active_connections: usize,
    /// Number of currently idle connections available for use
    pub idle_connections: usize,
    /// Maximum number of connections allowed in the pool
    pub max_connections: usize,
    /// Total number of connection attempts that have failed
    pub failed_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionConfig;
    use crate::transport::TransportProtocol;
    use rustls::crypto::ring;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::Arc;

    fn ensure_rustls_provider_installed() {
        let _ = ring::default_provider().install_default();
    }

    async fn create_test_transport() -> Arc<Transport> {
        ensure_rustls_provider_installed();
        let transport = Transport::new()
            .await
            .expect("transport initialization for tests");
        Arc::new(transport)
    }

    async fn create_dummy_connection(transport: Arc<Transport>) -> Arc<Connection> {
        let remote = SocketAddr::from((Ipv4Addr::LOCALHOST, 5000));
        let connection = Connection::with_config(
            0,
            remote,
            TransportProtocol::Quic,
            transport,
            ConnectionConfig::default(),
            None,
        )
        .await
        .expect("connection creation for tests");
        Arc::new(connection)
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let transport = create_test_transport().await;
        let pool = ConnectionPool::new(transport);

        let stats = pool.get_stats().await;
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.idle_connections, 0);
        assert_eq!(stats.total_created, 0);
    }

    #[tokio::test]
    async fn test_pool_config() {
        let mut config = PoolConfig::default();
        config.max_connections = 50;
        config.max_idle = 5;

        assert_eq!(config.max_connections, 50);
        assert_eq!(config.max_idle, 5);
        assert!(config.enable_reuse);
    }

    #[tokio::test]
    async fn test_pooled_connection_aging() {
        let transport = create_test_transport().await;
        let dummy_connection = create_dummy_connection(transport).await;
        let pooled = PooledConnection::new(dummy_connection);

        // Age should be very small initially
        assert!(pooled.age().as_millis() < 100);
        assert!(pooled.idle_time().as_millis() < 100);
        assert_eq!(pooled.use_count, 0);
    }

    #[test]
    fn test_pool_health_status() {
        let health = PoolHealthStatus {
            health: PoolHealth::Healthy,
            utilization_percent: 45.0,
            active_connections: 9,
            idle_connections: 3,
            max_connections: 20,
            failed_connections: 0,
        };

        assert_eq!(health.health, PoolHealth::Healthy);
        assert_eq!(health.utilization_percent, 45.0);
    }
}
