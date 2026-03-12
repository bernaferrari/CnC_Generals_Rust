//! UPnP (Universal Plug and Play) implementation for automatic port forwarding.
//!
//! This module provides automatic NAT traversal through UPnP-enabled routers,
//! allowing games to be hosted without manual port forwarding configuration.
//!
//! ## Features
//!
//! - **SSDP Discovery**: Finds UPnP-enabled gateways on the local network
//! - **Port Mapping**: Automatically forwards ports through the router
//! - **External IP Discovery**: Retrieves the public IP address
//! - **Lease Management**: Renews port mappings before they expire
//! - **Cleanup**: Removes port mappings on shutdown
//!
//! ## Protocol Flow
//!
//! 1. M-SEARCH: Multicast UDP discovery on 239.255.255.250:1900
//! 2. SSDP Response: Gateway responds with location URL
//! 3. Device Description: Fetch XML description via HTTP
//! 4. Service Control: Parse control URL for IGD service
//! 5. SOAP Requests: AddPortMapping, GetExternalIPAddress, etc.

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::net::{IpAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{info, warn};

/// SSDP multicast address for UPnP discovery
const SSDP_MULTICAST_ADDR: &str = "239.255.255.250:1900";

/// Internet Gateway Device service type
const IGD_SERVICE_TYPE: &str = "urn:schemas-upnp-org:service:WANIPConnection:1";

/// Alternative IGD service type (for some routers)
const IGD_SERVICE_TYPE_ALT: &str = "urn:schemas-upnp-org:service:WANPPPConnection:1";

/// Configuration for UPnP NAT traversal.
#[derive(Debug, Clone)]
pub struct UPnPConfig {
    /// Enable or disable UPnP functionality
    pub enabled: bool,
    /// Timeout for M-SEARCH discovery
    pub search_timeout: Duration,
    /// Protocol description shown in router's port forwarding table
    pub protocol_description: String,
    /// Lease duration for port mappings (in seconds)
    pub lease_duration: u32,
    /// Interval to renew port mappings before they expire
    pub renewal_interval: Duration,
}

impl Default for UPnPConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            search_timeout: Duration::from_secs(3),
            protocol_description: "C&C Generals Zero Hour".to_string(),
            lease_duration: 3600,                        // 1 hour
            renewal_interval: Duration::from_secs(1800), // Renew every 30 minutes
        }
    }
}

/// Represents a UPnP gateway device.
#[derive(Debug, Clone)]
pub struct UPnPGateway {
    /// Location URL from SSDP response
    pub location_url: String,
    /// Device description URL (usually same as location)
    pub device_description_url: String,
    /// SOAP control URL for the IGD service
    pub control_url: String,
    /// Service type (WANIPConnection or WANPPPConnection)
    pub service_type: String,
    /// Base URL for resolving relative control URLs
    pub base_url: String,
}

/// Represents a port mapping on the gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortMapping {
    /// External port visible to the internet
    pub external_port: u16,
    /// Internal port on the local machine
    pub internal_port: u16,
    /// Internal IP address (local machine)
    pub internal_ip: String,
    /// Protocol: "UDP" or "TCP"
    pub protocol: String,
    /// Human-readable description
    pub description: String,
    /// Whether the mapping is enabled
    pub enabled: bool,
    /// When the mapping was created
    pub created_at: NetworkInstant,
}

impl PortMapping {
    /// Create a new port mapping.
    pub fn new(
        external_port: u16,
        internal_port: u16,
        internal_ip: String,
        protocol: String,
        description: String,
    ) -> Self {
        Self {
            external_port,
            internal_port,
            internal_ip,
            protocol,
            description,
            enabled: true,
            created_at: NetworkInstant::now(),
        }
    }

    /// Create a UDP port mapping.
    pub fn udp(
        external_port: u16,
        internal_port: u16,
        internal_ip: String,
        description: String,
    ) -> Self {
        Self::new(
            external_port,
            internal_port,
            internal_ip,
            "UDP".to_string(),
            description,
        )
    }

    /// Create a TCP port mapping.
    pub fn tcp(
        external_port: u16,
        internal_port: u16,
        internal_ip: String,
        description: String,
    ) -> Self {
        Self::new(
            external_port,
            internal_port,
            internal_ip,
            "TCP".to_string(),
            description,
        )
    }
}

/// UPnP client for automatic port forwarding.
pub struct UPnPClient {
    config: UPnPConfig,
    gateway: Arc<RwLock<Option<UPnPGateway>>>,
    port_mappings: Arc<RwLock<Vec<PortMapping>>>,
    local_ip: Arc<RwLock<Option<IpAddr>>>,
}

impl UPnPClient {
    /// Create a new UPnP client with the given configuration.
    pub fn new(config: UPnPConfig) -> Self {
        Self {
            config,
            gateway: Arc::new(RwLock::new(None)),
            port_mappings: Arc::new(RwLock::new(Vec::new())),
            local_ip: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if UPnP is available (gateway discovered).
    pub async fn is_available(&self) -> bool {
        self.gateway.read().await.is_some()
    }

    /// Discover UPnP gateway on the local network.
    ///
    /// Uses SSDP M-SEARCH to find IGD-capable routers.
    pub async fn discover_gateway(&self) -> NetworkResult<()> {
        if !self.config.enabled {
            return Err(NetworkError::nat("UPnP is disabled in configuration"));
        }

        info!("Starting UPnP gateway discovery...");

        // Perform SSDP M-SEARCH
        let location_url = self.ssdp_search().await?;

        info!(url = %location_url, "Found UPnP gateway");

        // Fetch and parse device description
        let gateway = self.fetch_device_description(&location_url).await?;

        info!(
            control_url = %gateway.control_url,
            service_type = %gateway.service_type,
            "UPnP gateway configured"
        );

        *self.gateway.write().await = Some(gateway);

        Ok(())
    }

    /// Add a port mapping to the gateway.
    pub async fn add_port_mapping(&self, mapping: PortMapping) -> NetworkResult<()> {
        let gateway = self.gateway.read().await;
        let gateway = gateway
            .as_ref()
            .ok_or_else(|| NetworkError::nat("No UPnP gateway available"))?;

        info!(
            external_port = mapping.external_port,
            internal_port = mapping.internal_port,
            protocol = %mapping.protocol,
            "Adding UPnP port mapping"
        );

        let internal_ip = if mapping.internal_ip.is_empty() {
            self.get_local_ip().await?
        } else {
            mapping
                .internal_ip
                .parse()
                .map_err(|_| NetworkError::nat("Invalid internal IP address"))?
        };

        self.soap_add_port_mapping(
            gateway,
            mapping.external_port,
            mapping.internal_port,
            &internal_ip.to_string(),
            &mapping.protocol,
            &mapping.description,
            self.config.lease_duration,
        )
        .await?;

        let mut mappings = self.port_mappings.write().await;
        mappings.push(mapping);

        Ok(())
    }

    /// Remove a port mapping from the gateway.
    pub async fn remove_port_mapping(&self, mapping: &PortMapping) -> NetworkResult<()> {
        let gateway = self.gateway.read().await;
        let gateway = gateway
            .as_ref()
            .ok_or_else(|| NetworkError::nat("No UPnP gateway available"))?;

        info!(
            external_port = mapping.external_port,
            protocol = %mapping.protocol,
            "Removing UPnP port mapping"
        );

        self.soap_delete_port_mapping(gateway, mapping.external_port, &mapping.protocol)
            .await?;

        let mut mappings = self.port_mappings.write().await;
        mappings
            .retain(|m| m.external_port != mapping.external_port || m.protocol != mapping.protocol);

        Ok(())
    }

    /// Get the external IP address from the gateway.
    pub async fn get_external_ip(&self) -> NetworkResult<IpAddr> {
        let gateway = self.gateway.read().await;
        let gateway = gateway
            .as_ref()
            .ok_or_else(|| NetworkError::nat("No UPnP gateway available"))?;

        let ip_str = self.soap_get_external_ip(gateway).await?;

        ip_str
            .parse()
            .map_err(|_| NetworkError::nat(format!("Invalid IP address from gateway: {}", ip_str)))
    }

    /// Get all current port mappings.
    pub async fn get_port_mappings(&self) -> Vec<PortMapping> {
        self.port_mappings.read().await.clone()
    }

    /// Renew a port mapping (refresh its lease).
    pub async fn renew_port_mapping(&self, mapping: &PortMapping) -> NetworkResult<()> {
        // Re-adding the same mapping effectively renews it
        self.add_port_mapping(mapping.clone()).await
    }

    /// Remove all port mappings and clean up.
    pub async fn cleanup(&self) {
        info!("Cleaning up UPnP port mappings...");

        let mappings = self.port_mappings.read().await.clone();

        for mapping in mappings {
            if let Err(err) = self.remove_port_mapping(&mapping).await {
                warn!(
                    external_port = mapping.external_port,
                    protocol = %mapping.protocol,
                    error = %err,
                    "Failed to remove port mapping"
                );
            }
        }

        info!("UPnP cleanup complete");
    }

    /// Get the local IP address.
    async fn get_local_ip(&self) -> NetworkResult<IpAddr> {
        // Check cache first
        if let Some(ip) = *self.local_ip.read().await {
            return Ok(ip);
        }

        // Discover local IP by connecting to a public address
        let socket = StdUdpSocket::bind("0.0.0.0:0")
            .map_err(|e| NetworkError::nat(format!("Failed to bind socket: {}", e)))?;

        socket
            .connect("8.8.8.8:80")
            .map_err(|e| NetworkError::nat(format!("Failed to connect: {}", e)))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| NetworkError::nat(format!("Failed to get local address: {}", e)))?;

        let ip = local_addr.ip();
        *self.local_ip.write().await = Some(ip);

        Ok(ip)
    }

    /// Perform SSDP M-SEARCH to discover gateway.
    async fn ssdp_search(&self) -> NetworkResult<String> {
        let search_msg = format!(
            "M-SEARCH * HTTP/1.1\r\n\
             HOST: {}\r\n\
             MAN: \"ssdp:discover\"\r\n\
             MX: 3\r\n\
             ST: {}\r\n\
             \r\n",
            SSDP_MULTICAST_ADDR, IGD_SERVICE_TYPE
        );

        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to bind UDP socket: {}", e)))?;

        socket
            .send_to(search_msg.as_bytes(), SSDP_MULTICAST_ADDR)
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to send M-SEARCH: {}", e)))?;

        let mut buf = [0u8; 2048];

        let result = timeout(self.config.search_timeout, socket.recv_from(&mut buf)).await;

        let (len, _) = result
            .map_err(|_| NetworkError::nat("UPnP discovery timed out"))?
            .map_err(|e| NetworkError::nat(format!("Failed to receive SSDP response: {}", e)))?;

        let response = String::from_utf8_lossy(&buf[..len]);

        // Parse LOCATION header
        for line in response.lines() {
            if line.to_uppercase().starts_with("LOCATION:") {
                let location = line[9..].trim();
                return Ok(location.to_string());
            }
        }

        Err(NetworkError::nat("No LOCATION in SSDP response"))
    }

    /// Fetch and parse device description XML.
    async fn fetch_device_description(&self, location_url: &str) -> NetworkResult<UPnPGateway> {
        // Parse base URL
        let url = location_url
            .parse::<url::Url>()
            .map_err(|e| NetworkError::nat(format!("Invalid location URL: {}", e)))?;

        let base_url = format!(
            "{}://{}:{}",
            url.scheme(),
            url.host_str().unwrap_or(""),
            url.port().unwrap_or(80)
        );

        // Fetch device description
        let response = reqwest::get(location_url)
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to fetch device description: {}", e)))?
            .text()
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to read device description: {}", e)))?;

        // Parse control URL and service type
        let (control_url, service_type) = self.parse_device_description(&response, &base_url)?;

        Ok(UPnPGateway {
            location_url: location_url.to_string(),
            device_description_url: location_url.to_string(),
            control_url,
            service_type,
            base_url,
        })
    }

    /// Parse device description XML to extract control URL.
    fn parse_device_description(
        &self,
        xml: &str,
        base_url: &str,
    ) -> NetworkResult<(String, String)> {
        // Simple XML parsing (avoiding heavy XML parser dependencies)
        // Look for WANIPConnection or WANPPPConnection service

        let service_types = [IGD_SERVICE_TYPE, IGD_SERVICE_TYPE_ALT];

        for service_type in &service_types {
            if let Some(service_start) = xml.find(service_type) {
                // Find the controlURL within this service block
                if let Some(control_start) = xml[service_start..].find("<controlURL>") {
                    let control_start = service_start + control_start + 12; // Length of "<controlURL>"
                    if let Some(control_end) = xml[control_start..].find("</controlURL>") {
                        let control_path = &xml[control_start..control_start + control_end];

                        // Resolve relative URL
                        let control_url = if control_path.starts_with("http") {
                            control_path.to_string()
                        } else {
                            format!("{}{}", base_url, control_path)
                        };

                        return Ok((control_url, service_type.to_string()));
                    }
                }
            }
        }

        Err(NetworkError::nat(
            "Could not find IGD service in device description",
        ))
    }

    /// Send SOAP AddPortMapping request.
    async fn soap_add_port_mapping(
        &self,
        gateway: &UPnPGateway,
        external_port: u16,
        internal_port: u16,
        internal_ip: &str,
        protocol: &str,
        description: &str,
        lease_duration: u32,
    ) -> NetworkResult<()> {
        let soap_body = format!(
            r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:AddPortMapping xmlns:u="{}">
      <NewRemoteHost></NewRemoteHost>
      <NewExternalPort>{}</NewExternalPort>
      <NewProtocol>{}</NewProtocol>
      <NewInternalPort>{}</NewInternalPort>
      <NewInternalClient>{}</NewInternalClient>
      <NewEnabled>1</NewEnabled>
      <NewPortMappingDescription>{}</NewPortMappingDescription>
      <NewLeaseDuration>{}</NewLeaseDuration>
    </u:AddPortMapping>
  </s:Body>
</s:Envelope>"#,
            gateway.service_type,
            external_port,
            protocol,
            internal_port,
            internal_ip,
            description,
            lease_duration
        );

        self.soap_request(gateway, "AddPortMapping", &soap_body)
            .await?;
        Ok(())
    }

    /// Send SOAP DeletePortMapping request.
    async fn soap_delete_port_mapping(
        &self,
        gateway: &UPnPGateway,
        external_port: u16,
        protocol: &str,
    ) -> NetworkResult<()> {
        let soap_body = format!(
            r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:DeletePortMapping xmlns:u="{}">
      <NewRemoteHost></NewRemoteHost>
      <NewExternalPort>{}</NewExternalPort>
      <NewProtocol>{}</NewProtocol>
    </u:DeletePortMapping>
  </s:Body>
</s:Envelope>"#,
            gateway.service_type, external_port, protocol
        );

        self.soap_request(gateway, "DeletePortMapping", &soap_body)
            .await?;
        Ok(())
    }

    /// Send SOAP GetExternalIPAddress request.
    async fn soap_get_external_ip(&self, gateway: &UPnPGateway) -> NetworkResult<String> {
        let soap_body = format!(
            r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetExternalIPAddress xmlns:u="{}">
    </u:GetExternalIPAddress>
  </s:Body>
</s:Envelope>"#,
            gateway.service_type
        );

        let response = self
            .soap_request(gateway, "GetExternalIPAddress", &soap_body)
            .await?;

        // Parse IP from response
        if let Some(ip_start) = response.find("<NewExternalIPAddress>") {
            let ip_start = ip_start + 22; // Length of tag
            if let Some(ip_end) = response[ip_start..].find("</NewExternalIPAddress>") {
                return Ok(response[ip_start..ip_start + ip_end].to_string());
            }
        }

        Err(NetworkError::nat(
            "Could not parse external IP from response",
        ))
    }

    /// Send a SOAP request to the gateway.
    async fn soap_request(
        &self,
        gateway: &UPnPGateway,
        action: &str,
        body: &str,
    ) -> NetworkResult<String> {
        let client = reqwest::Client::new();
        let soap_action = format!("\"{}#{}\"", gateway.service_type, action);

        let response = client
            .post(&gateway.control_url)
            .header("Content-Type", "text/xml; charset=\"utf-8\"")
            .header("SOAPAction", soap_action)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| NetworkError::nat(format!("SOAP request failed: {}", e)))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| NetworkError::nat(format!("Failed to read SOAP response: {}", e)))?;

        if !status.is_success() {
            // Parse SOAP fault if present
            if let Some(error) = self.parse_soap_fault(&text) {
                return Err(NetworkError::nat(format!("SOAP error: {}", error)));
            }
            return Err(NetworkError::nat(format!(
                "SOAP request failed with status {}",
                status
            )));
        }

        Ok(text)
    }

    /// Parse SOAP fault message.
    fn parse_soap_fault(&self, xml: &str) -> Option<String> {
        if let Some(fault_start) = xml.find("<faultstring>") {
            let fault_start = fault_start + 13;
            if let Some(fault_end) = xml[fault_start..].find("</faultstring>") {
                return Some(xml[fault_start..fault_start + fault_end].to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = UPnPConfig::default();
        assert!(config.enabled);
        assert_eq!(config.search_timeout, Duration::from_secs(3));
        assert_eq!(config.protocol_description, "C&C Generals Zero Hour");
        assert_eq!(config.lease_duration, 3600);
    }

    #[test]
    fn test_port_mapping_creation() {
        let mapping = PortMapping::udp(
            27015,
            27015,
            "192.168.1.100".to_string(),
            "Game Server".to_string(),
        );

        assert_eq!(mapping.external_port, 27015);
        assert_eq!(mapping.internal_port, 27015);
        assert_eq!(mapping.protocol, "UDP");
        assert!(mapping.enabled);
    }

    #[test]
    fn test_parse_device_description() {
        let client = UPnPClient::new(UPnPConfig::default());
        let xml = r#"
            <root>
                <device>
                    <serviceList>
                        <service>
                            <serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType>
                            <controlURL>/ctl/IPConn</controlURL>
                        </service>
                    </serviceList>
                </device>
            </root>
        "#;

        let result = client.parse_device_description(xml, "http://192.168.1.1:5000");
        assert!(result.is_ok());

        let (control_url, service_type) = result.unwrap();
        assert_eq!(control_url, "http://192.168.1.1:5000/ctl/IPConn");
        assert_eq!(service_type, IGD_SERVICE_TYPE);
    }

    #[test]
    fn test_parse_soap_fault() {
        let client = UPnPClient::new(UPnPConfig::default());
        let fault_xml = r#"
            <s:Envelope>
                <s:Body>
                    <s:Fault>
                        <faultcode>s:Client</faultcode>
                        <faultstring>Invalid Action</faultstring>
                    </s:Fault>
                </s:Body>
            </s:Envelope>
        "#;

        let error = client.parse_soap_fault(fault_xml);
        assert_eq!(error, Some("Invalid Action".to_string()));
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = UPnPConfig::default();
        let client = UPnPClient::new(config);

        assert!(!client.is_available().await);
        assert_eq!(client.get_port_mappings().await.len(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_empty_mappings() {
        let client = UPnPClient::new(UPnPConfig::default());

        // Cleanup with no mappings should not panic
        client.cleanup().await;

        assert_eq!(client.get_port_mappings().await.len(), 0);
    }

    // Integration tests would require a real UPnP gateway
    // These are best run manually on a network with UPnP-enabled router
}
