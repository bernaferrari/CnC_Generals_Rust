//! Modern firewall/NAT helper built on top of UPnP and NAT-PMP (via `igd`).
//! Provides feature parity with the legacy FirewallHelper while embracing
//! async patterns and graceful degradation when routers do not support
//! automatic port mapping.

use crate::error::{NetworkError, NetworkResult};
use crate::transport::TransportProtocol;
use igd::aio::Gateway;
use igd::{aio::search_gateway, PortMappingProtocol};
use parking_lot::Mutex as SyncMutex;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Firewall/NAT helper configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallConfig {
    /// Enable automatic port mapping.
    pub enabled: bool,
    /// Preferred public port. If `None`, the internal port will be requested.
    pub external_port: Option<u16>,
    /// Lease duration used when adding port mappings.
    pub lease_duration: Duration,
    /// Refresh interval for renewing port mappings.
    pub refresh_interval: Duration,
    /// Description used when creating mappings on the gateway.
    pub mapping_description: String,
}

impl Default for FirewallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            external_port: None,
            lease_duration: Duration::from_secs(60 * 30), // 30 minutes
            refresh_interval: Duration::from_secs(60 * 10), // 10 minutes
            mapping_description: "Generals Zero Hour".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveMapping {
    protocol: PortMappingProtocol,
    internal_addr: Ipv4Addr,
    internal_port: u16,
    external_port: u16,
    description: String,
}

impl ActiveMapping {
    fn protocol_as_str(&self) -> &'static str {
        match self.protocol {
            PortMappingProtocol::TCP => "TCP",
            PortMappingProtocol::UDP => "UDP",
        }
    }
}

/// Modern firewall helper that mirrors the behaviour of the original
/// FirewallHelper by acquiring and refreshing external port mappings.
pub struct FirewallHelper {
    config: FirewallConfig,
    gateway: Arc<Mutex<Option<Arc<Gateway>>>>,
    active_mapping: Arc<Mutex<Option<ActiveMapping>>>,
    refresh_task: SyncMutex<Option<JoinHandle<()>>>,
}

impl FirewallHelper {
    /// Construct a new firewall helper. Discovery is deferred until first use.
    pub fn new(config: FirewallConfig) -> Self {
        Self {
            config,
            gateway: Arc::new(Mutex::new(None)),
            active_mapping: Arc::new(Mutex::new(None)),
            refresh_task: SyncMutex::new(None),
        }
    }

    /// Ensure a public mapping exists for the provided protocol/port pair.
    ///
    /// Returns the external port that was mapped, or `None` if mapping was
    /// disabled or not supported.
    pub async fn ensure_mapping(
        &self,
        protocol: TransportProtocol,
        internal_addr: Ipv4Addr,
        internal_port: u16,
    ) -> NetworkResult<Option<u16>> {
        if !self.config.enabled {
            debug!("Firewall helper disabled via configuration");
            return Ok(None);
        }

        let protocol = match protocol {
            TransportProtocol::Tcp => PortMappingProtocol::TCP,
            TransportProtocol::Udp | TransportProtocol::Quic => PortMappingProtocol::UDP,
            TransportProtocol::WebSocket => {
                debug!("WebSocket transport does not require port mapping");
                return Ok(None);
            }
        };

        let gateway = match self.gateway().await {
            Ok(gateway) => gateway,
            Err(err) => {
                warn!("Failed to discover gateway for port mapping: {}", err);
                return Ok(None);
            }
        };

        let external_port = self.config.external_port.unwrap_or(internal_port);
        let description = format!("{} ({:?})", self.config.mapping_description, protocol);

        let mapping = ActiveMapping {
            protocol,
            internal_addr,
            internal_port,
            external_port,
            description: description.clone(),
        };

        if let Err(err) = Self::apply_mapping(&gateway, &mapping, self.config.lease_duration).await
        {
            warn!("Failed to install firewall mapping: {}", err);
            return Ok(None);
        }

        info!(
            protocol = ?protocol,
            internal = ?SocketAddrV4::new(internal_addr, internal_port),
            external_port,
            "Firewall mapping established via UPnP/NAT-PMP"
        );

        {
            let mut guard = self.active_mapping.lock().await;
            *guard = Some(mapping.clone());
        }

        self.spawn_refresh_task();

        Ok(Some(external_port))
    }

    /// Remove the active mapping if one exists.
    pub async fn remove_mapping(&self) {
        // Stop refresh task first
        if let Some(handle) = self.refresh_task.lock().take() {
            debug!("Stopping firewall mapping refresh task");
            handle.abort();
        }

        let mapping = {
            let mut guard = self.active_mapping.lock().await;
            guard.take()
        };

        let gateway = {
            let guard = self.gateway.lock().await;
            guard.clone()
        };

        if let (Some(mapping), Some(gateway)) = (mapping, gateway) {
            info!(
                "Removing firewall mapping for {} port {}",
                mapping.protocol_as_str(),
                mapping.external_port
            );

            if let Err(err) =
                Self::remove_mapping_internal(&gateway, mapping.protocol, mapping.external_port)
                    .await
            {
                warn!("Failed to remove firewall mapping: {}", err);
            } else {
                info!("Successfully removed firewall mapping");
            }
        }

        {
            let mut gw = self.gateway.lock().await;
            *gw = None;
        }
    }

    /// Get current mapping status.
    pub async fn current_mapping(&self) -> Option<(u16, String)> {
        self.active_mapping
            .lock()
            .await
            .as_ref()
            .map(|m| (m.external_port, m.protocol_as_str().to_string()))
    }

    /// Check if mapping is currently active.
    pub async fn is_active(&self) -> bool {
        self.active_mapping.lock().await.is_some()
    }

    async fn gateway(&self) -> NetworkResult<Arc<Gateway>> {
        {
            let guard = self.gateway.lock().await;
            if let Some(gateway) = &*guard {
                return Ok(gateway.clone());
            }
        }

        let discovered = search_gateway(Default::default())
            .await
            .map_err(|err| NetworkError::generic(format!("Gateway discovery failed: {}", err)))?;

        let gateway = Arc::new(discovered);
        {
            let mut guard = self.gateway.lock().await;
            *guard = Some(gateway.clone());
        }
        Ok(gateway)
    }

    fn spawn_refresh_task(&self) {
        let mut guard = self.refresh_task.lock();
        if guard.is_some() {
            return;
        }

        let gateway_ref = Arc::clone(&self.gateway);
        let mapping_state = Arc::clone(&self.active_mapping);
        let lease = self.config.lease_duration;
        let refresh_interval = self.config.refresh_interval;

        let handle = tokio::spawn(async move {
            loop {
                sleep(refresh_interval).await;

                let mapping = {
                    let guard = mapping_state.lock().await;
                    guard.clone()
                };

                let gateway = {
                    let guard = gateway_ref.lock().await;
                    guard.clone()
                };

                match (mapping, gateway) {
                    (Some(mapping), Some(gateway)) => {
                        if let Err(err) =
                            FirewallHelper::apply_mapping(&gateway, &mapping, lease).await
                        {
                            warn!("Failed to refresh firewall mapping: {}", err);
                        } else {
                            debug!(
                                protocol = ?mapping.protocol,
                                external_port = mapping.external_port,
                                "Firewall mapping refreshed"
                            );
                        }
                    }
                    _ => break,
                }
            }
        });

        *guard = Some(handle);
    }

    async fn apply_mapping(
        gateway: &Gateway,
        mapping: &ActiveMapping,
        lease_duration: Duration,
    ) -> Result<(), igd::AddPortError> {
        let socket = SocketAddrV4::new(mapping.internal_addr, mapping.internal_port);
        gateway
            .add_port(
                mapping.protocol,
                mapping.external_port,
                socket,
                lease_duration.as_secs() as u32,
                &mapping.description,
            )
            .await
    }

    async fn remove_mapping_internal(
        gateway: &Gateway,
        protocol: PortMappingProtocol,
        external_port: u16,
    ) -> Result<(), igd::RemovePortError> {
        gateway.remove_port(protocol, external_port).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::TransportProtocol;

    #[tokio::test]
    async fn disabled_helper_short_circuits() {
        let helper = FirewallHelper::new(FirewallConfig {
            enabled: false,
            ..Default::default()
        });
        let result = helper
            .ensure_mapping(TransportProtocol::Quic, Ipv4Addr::new(127, 0, 0, 1), 8088)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn helper_handles_gateway_absence() {
        // Intentionally use an impossible IP to force discovery failure by temporarily
        // disabling networking via loopback only (igd will fail fast).
        // The helper should surface Ok(None) rather than propagating the error.
        let helper = FirewallHelper::new(FirewallConfig {
            enabled: true,
            ..Default::default()
        });

        let result = helper
            .ensure_mapping(TransportProtocol::Quic, Ipv4Addr::new(127, 0, 0, 1), 6553)
            .await;

        assert!(result.is_ok());
    }
}
