#![allow(unused_crate_dependencies)]

use game_network::observability::{
    initialize_telemetry, telemetry, HealthStatus, ObservabilityConfig,
};
use game_network::transport::{Transport, TransportConfig, TransportMessage, TransportProtocol};
use rustls::crypto::ring;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::sleep;

fn next_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .expect("allocate ephemeral port")
        .local_addr()
        .expect("socket addr")
        .port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_roundtrip_updates_metrics() -> game_network::NetworkResult<()> {
    let mut telemetry_config = ObservabilityConfig::default();
    telemetry_config.enable_metrics = false;
    telemetry_config.enable_tracing = false;
    telemetry_config.enable_console = false;
    initialize_telemetry(telemetry_config).await?;

    ring::default_provider()
        .install_default()
        .expect("install ring provider");

    let server_port = next_port();
    let server_addr = SocketAddr::from(([127, 0, 0, 1], server_port));

    let mut server_config = TransportConfig::default();
    server_config.bind_address = server_addr;
    let server = Transport::with_config(server_config).await?;
    server.bind().await?;

    let client = Transport::new().await?;
    client.connect(server_addr).await?;

    let message = TransportMessage::new(b"ping".to_vec(), TransportProtocol::Quic)
        .with_destination(server_addr);
    client.send_message(message).await?;

    sleep(Duration::from_millis(100)).await;
    server.update().await?;

    let incoming = server.receive_messages().await?;
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].data, b"ping");

    let client_metrics = client.metrics().await;
    let server_metrics = server.metrics().await;
    assert!(client_metrics.packets_sent >= 1);
    assert!(server_metrics.packets_received >= 1);

    let telemetry = telemetry().expect("telemetry initialized");
    let report = telemetry.generate_health_report().await;
    assert!(matches!(report.status, HealthStatus::Healthy));

    Ok(())
}
