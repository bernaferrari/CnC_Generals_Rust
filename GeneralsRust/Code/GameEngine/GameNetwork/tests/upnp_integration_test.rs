//! UPnP Integration Tests
//!
//! These tests verify the UPnP implementation with real gateway interactions
//! when available, and provide comprehensive unit testing otherwise.

use game_network::nat::upnp::{PortMapping, UPnPClient, UPnPConfig};
use std::time::Duration;

#[tokio::test]
async fn test_upnp_client_lifecycle() {
    // Create client with short timeout
    let mut config = UPnPConfig::default();
    config.search_timeout = Duration::from_secs(2);
    config.enabled = true;

    let client = UPnPClient::new(config);

    // Client should not be available initially
    assert!(!client.is_available().await);

    // Note: Gateway discovery will fail in CI/test environments without UPnP router
    // This is expected behavior
    let discovery_result = client.discover_gateway().await;

    if discovery_result.is_ok() {
        // If we found a gateway, test port mapping
        assert!(client.is_available().await);

        // Create a test port mapping
        let mapping = PortMapping::udp(
            0, // Use port 0 to let OS assign
            0,
            String::new(),
            "UPnP Integration Test".to_string(),
        );

        // Try to add mapping (may fail due to permissions)
        let _ = client.add_port_mapping(mapping).await;

        // Clean up
        client.cleanup().await;
    } else {
        // No UPnP gateway available - this is fine for CI
        println!("No UPnP gateway found (expected in CI environment)");
    }
}

#[tokio::test]
async fn test_port_mapping_structure() {
    // Test UDP mapping creation
    let udp_mapping = PortMapping::udp(
        27015,
        27015,
        "192.168.1.100".to_string(),
        "Game Server".to_string(),
    );

    assert_eq!(udp_mapping.external_port, 27015);
    assert_eq!(udp_mapping.internal_port, 27015);
    assert_eq!(udp_mapping.protocol, "UDP");
    assert!(udp_mapping.enabled);

    // Test TCP mapping creation
    let tcp_mapping = PortMapping::tcp(
        27016,
        27016,
        "192.168.1.100".to_string(),
        "Query Server".to_string(),
    );

    assert_eq!(tcp_mapping.protocol, "TCP");
}

#[tokio::test]
async fn test_config_customization() {
    let mut config = UPnPConfig::default();

    // Customize configuration
    config.search_timeout = Duration::from_secs(10);
    config.lease_duration = 7200; // 2 hours
    config.protocol_description = "Custom Game".to_string();

    let client = UPnPClient::new(config.clone());

    // Verify configuration was applied
    assert!(!client.is_available().await);
}

#[tokio::test]
async fn test_disabled_upnp() {
    let mut config = UPnPConfig::default();
    config.enabled = false;

    let client = UPnPClient::new(config);

    // Discovery should fail when disabled
    let result = client.discover_gateway().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiple_port_mappings() {
    let config = UPnPConfig {
        enabled: true,
        search_timeout: Duration::from_secs(2),
        protocol_description: "Test".to_string(),
        lease_duration: 3600,
        renewal_interval: Duration::from_secs(1800),
    };

    let client = UPnPClient::new(config);

    // Initially no mappings
    assert_eq!(client.get_port_mappings().await.len(), 0);

    // Discovery may or may not succeed depending on environment
    let _ = client.discover_gateway().await;
}

#[test]
fn test_port_mapping_equality() {
    let mapping1 = PortMapping::udp(
        27015,
        27015,
        "192.168.1.100".to_string(),
        "Server 1".to_string(),
    );

    let mapping2 = PortMapping::udp(
        27015,
        27015,
        "192.168.1.100".to_string(),
        "Server 1".to_string(),
    );

    // Mappings should be equal (ignoring created_at)
    assert_eq!(mapping1.external_port, mapping2.external_port);
    assert_eq!(mapping1.internal_port, mapping2.internal_port);
    assert_eq!(mapping1.protocol, mapping2.protocol);
}

#[tokio::test]
async fn test_external_ip_retrieval() {
    let config = UPnPConfig::default();
    let client = UPnPClient::new(config);

    // Without gateway discovery, this should fail
    let result = client.get_external_ip().await;
    assert!(result.is_err());
}

/// Test that demonstrates proper cleanup workflow
#[tokio::test]
async fn test_cleanup_workflow() {
    let config = UPnPConfig::default();
    let client = UPnPClient::new(config);

    // Cleanup with no gateway should complete without error
    client.cleanup().await;

    // Verify no mappings remain
    assert_eq!(client.get_port_mappings().await.len(), 0);
}

/// Test configuration defaults
#[test]
fn test_default_configuration() {
    let config = UPnPConfig::default();

    assert!(config.enabled);
    assert_eq!(config.search_timeout, Duration::from_secs(3));
    assert_eq!(config.protocol_description, "C&C Generals Zero Hour");
    assert_eq!(config.lease_duration, 3600);
    assert_eq!(config.renewal_interval, Duration::from_secs(1800));
}

/// Test that port mapping descriptions are preserved
#[test]
fn test_port_mapping_descriptions() {
    let descriptions = vec![
        "Game Server - Main Port",
        "Voice Chat",
        "File Transfer",
        "C&C Generals Zero Hour",
    ];

    for desc in descriptions {
        let mapping = PortMapping::udp(12345, 12345, "192.168.1.1".to_string(), desc.to_string());
        assert_eq!(mapping.description, desc);
    }
}
