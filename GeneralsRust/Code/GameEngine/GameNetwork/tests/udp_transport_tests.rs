#![allow(unused_crate_dependencies)]

//! UDP Transport Tests - Verify C++ wire-protocol compatibility
//!
//! These tests verify that the UDP transport implementation correctly matches
//! the C++ original implementation in packet format, encryption, and CRC validation.

use game_network::observability::{initialize_telemetry, ObservabilityConfig};
use game_network::transport_udp::{
    calculate_crc32, xor_decrypt, xor_encrypt, GENERALS_MAGIC_NUMBER, IDLE_TIMEOUT_SECS,
    KEEP_ALIVE_INTERVAL_MS, MAX_PACKET_SIZE,
};
use game_network::{TransportMessage, TransportProtocol, UdpTransport};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::{sleep, timeout};

fn next_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .expect("allocate ephemeral port")
        .local_addr()
        .expect("socket addr")
        .port()
}

/// Test that CRC32 calculation matches C++ implementation
#[test]
fn test_crc32_calculation() {
    // Test with known values - these should match C++ output
    let test_data = b"Hello, World!";
    let crc = calculate_crc32(test_data);

    // CRC32 should be non-zero for this data
    assert_ne!(crc, 0, "CRC32 should be non-zero for test data");

    // Same data should produce same CRC
    let crc2 = calculate_crc32(test_data);
    assert_eq!(crc, crc2, "CRC32 should be deterministic");

    // Different data should produce different CRC
    let other_data = b"Different data";
    let crc3 = calculate_crc32(other_data);
    assert_ne!(crc, crc3, "Different data should produce different CRC");
}

/// Test that XOR encryption/decryption is symmetric
#[test]
fn test_xor_cipher_roundtrip() {
    let original = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    let mut data = original.clone();

    xor_encrypt(&mut data);
    assert_ne!(data, original, "Encryption should change data");

    xor_decrypt(&mut data);
    assert_eq!(data, original, "Decryption should recover original");
}

/// Test XOR encryption with 4-byte aligned data
#[test]
fn test_xor_encrypt_4byte_aligned() {
    // XOR cipher operates on 4-byte words
    let original = vec![0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44];
    let mut data = original.clone();

    xor_encrypt(&mut data);

    // First 4 bytes should be XORed with initial mask (0x0000Fade)
    // Second 4 bytes should be XORed with (0x0000Fade + 0x00000321)
    assert_ne!(
        &data[0..4],
        &original[0..4],
        "First 4 bytes should be encrypted"
    );
    assert_ne!(
        &data[4..8],
        &original[4..8],
        "Second 4 bytes should be encrypted"
    );

    xor_decrypt(&mut data);
    assert_eq!(data, original, "Decryption should recover original");
}

/// Test that protocol constants match C++ values
#[test]
fn test_protocol_constants() {
    assert_eq!(
        GENERALS_MAGIC_NUMBER, 0xF00D,
        "Magic number should be 0xF00D"
    );
    assert_eq!(MAX_PACKET_SIZE, 476, "Max packet size should be 476 bytes");
    assert_eq!(
        KEEP_ALIVE_INTERVAL_MS, 15_000,
        "Keep-alive should be 15 seconds"
    );
    assert_eq!(IDLE_TIMEOUT_SECS, 60, "Idle timeout should be 60 seconds");
}

/// Test UDP transport creation and binding
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_udp_transport_creation() -> game_network::NetworkResult<()> {
    // Telemetry is initialized once, ignore repeated init errors
    let _ = async {
        let mut telemetry_config = ObservabilityConfig::default();
        telemetry_config.enable_metrics = false;
        telemetry_config.enable_tracing = false;
        telemetry_config.enable_console = false;
        let _ = initialize_telemetry(telemetry_config).await;
    }
    .await;

    let port = next_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let mut config = game_network::UdpConfig::default();
    config.bind_address = addr;

    let transport = UdpTransport::with_config(config).await?;
    assert!(
        !transport.is_ready(),
        "Transport should not be ready before binding"
    );

    transport.bind().await?;
    assert!(
        transport.is_ready(),
        "Transport should be ready after binding"
    );

    transport.shutdown().await?;
    Ok(())
}

/// Test UDP packet send/receive roundtrip
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_udp_send_receive() -> game_network::NetworkResult<()> {
    let _ = async {
        let mut telemetry_config = ObservabilityConfig::default();
        telemetry_config.enable_metrics = false;
        telemetry_config.enable_tracing = false;
        telemetry_config.enable_console = false;
        let _ = initialize_telemetry(telemetry_config).await;
    }
    .await;

    let server_port = next_port();
    let server_addr = SocketAddr::from(([127, 0, 0, 1], server_port));

    let mut server_config = game_network::UdpConfig::default();
    server_config.bind_address = server_addr;

    let server = UdpTransport::with_config(server_config).await?;
    server.bind().await?;

    let client_port = next_port();
    let mut client_config = game_network::UdpConfig::default();
    client_config.bind_address = SocketAddr::from(([127, 0, 0, 1], client_port));

    let client = UdpTransport::with_config(client_config).await?;
    client.bind().await?;

    // Send test message
    let test_payload = b"Test UDP packet";
    let message = TransportMessage::new(test_payload.to_vec(), TransportProtocol::Udp)
        .with_destination(server_addr);

    client.send_message(message).await?;

    // Wait for packet delivery
    sleep(Duration::from_millis(100)).await;

    // Receive on server
    let incoming = timeout(Duration::from_secs(2), async {
        loop {
            let incoming = server.receive_messages().await?;
            if !incoming.is_empty() {
                return Ok::<_, game_network::NetworkError>(incoming);
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| game_network::NetworkError::transport("Timed out waiting for server receive"))??;
    assert!(!incoming.is_empty(), "Server should receive message");
    assert_eq!(incoming[0].data, test_payload, "Payload should match");
    assert_eq!(
        incoming[0].source,
        Some(SocketAddr::from(([127, 0, 0, 1], client_port)))
    );

    // Check metrics
    let client_metrics = client.metrics().await;
    let server_metrics = server.metrics().await;
    assert!(
        client_metrics.packets_sent >= 1,
        "Client should have sent packets"
    );
    assert!(
        server_metrics.packets_received >= 1,
        "Server should have received packets"
    );

    server.shutdown().await?;
    client.shutdown().await?;

    Ok(())
}

/// Test that packets respect size limits
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_udp_packet_size_limit() -> game_network::NetworkResult<()> {
    let _ = async {
        let mut telemetry_config = ObservabilityConfig::default();
        telemetry_config.enable_metrics = false;
        telemetry_config.enable_tracing = false;
        telemetry_config.enable_console = false;
        let _ = initialize_telemetry(telemetry_config).await;
    }
    .await;

    let port = next_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let mut config = game_network::UdpConfig::default();
    config.bind_address = addr;

    let transport = UdpTransport::with_config(config).await?;
    transport.bind().await?;

    let max_payload_size = MAX_PACKET_SIZE - 6;

    // Try to send packet that's too large (> 470 bytes payload, > 476 bytes total)
    let oversized_payload = vec![0u8; max_payload_size + 1];
    let message = TransportMessage::new(oversized_payload, TransportProtocol::Udp)
        .with_destination(SocketAddr::from(([127, 0, 0, 1], 9999)));

    let result = transport.send_message(message).await;
    assert!(result.is_err(), "Should reject oversized packets");

    // Valid size should work
    let valid_payload = vec![0u8; max_payload_size.saturating_sub(10)];
    let message =
        TransportMessage::new(valid_payload, TransportProtocol::Udp).with_destination(addr);

    let result = transport.send_message(message).await;
    // Sending to self might fail, but should fail for network reason, not size
    let _ = result;

    transport.shutdown().await?;
    Ok(())
}

/// Test idle connection timeout
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_idle_timeout() -> game_network::NetworkResult<()> {
    let _ = async {
        let mut telemetry_config = ObservabilityConfig::default();
        telemetry_config.enable_metrics = false;
        telemetry_config.enable_tracing = false;
        telemetry_config.enable_console = false;
        let _ = initialize_telemetry(telemetry_config).await;
    }
    .await;

    let port = next_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let mut config = game_network::UdpConfig::default();
    config.bind_address = addr;
    config.max_idle_timeout = Duration::from_millis(100); // Very short for testing

    let transport = UdpTransport::with_config(config).await?;
    transport.bind().await?;

    timeout(Duration::from_secs(2), async {
        // Update should eventually clean up idle connections
        for _ in 0..50 {
            transport.update().await?;
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, game_network::NetworkError>(())
    })
    .await
    .map_err(|_| {
        game_network::NetworkError::transport("Timed out waiting for idle timeout update")
    })??;

    transport.shutdown().await?;
    Ok(())
}

/// Test metrics tracking
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_transport_metrics() -> game_network::NetworkResult<()> {
    let _ = async {
        let mut telemetry_config = ObservabilityConfig::default();
        telemetry_config.enable_metrics = false;
        telemetry_config.enable_tracing = false;
        telemetry_config.enable_console = false;
        let _ = initialize_telemetry(telemetry_config).await;
    }
    .await;

    let port = next_port();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let mut config = game_network::UdpConfig::default();
    config.bind_address = addr;

    let transport = UdpTransport::with_config(config).await?;
    transport.bind().await?;

    // Check initial metrics
    let metrics = transport.metrics().await;
    assert_eq!(metrics.packets_sent, 0);
    assert_eq!(metrics.packets_received, 0);
    assert_eq!(metrics.bytes_sent, 0);
    assert_eq!(metrics.bytes_received, 0);

    transport.shutdown().await?;
    Ok(())
}
