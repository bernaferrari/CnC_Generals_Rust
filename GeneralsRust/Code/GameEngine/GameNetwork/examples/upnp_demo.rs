//! UPnP Port Forwarding Demo
//!
//! This example demonstrates how to use the UPnP client for automatic
//! port forwarding through NAT gateways.
//!
//! Run with: cargo run --example upnp_demo

use game_network::nat::upnp::{PortMapping, UPnPClient, UPnPConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    println!("=== C&C Generals Zero Hour - UPnP Port Forwarding Demo ===\n");

    // Create UPnP client with default configuration
    let mut config = UPnPConfig::default();
    config.search_timeout = Duration::from_secs(5);

    let client = UPnPClient::new(config);

    // Discover UPnP gateway
    println!("1. Discovering UPnP gateway on local network...");
    match client.discover_gateway().await {
        Ok(()) => {
            println!("   ✓ UPnP gateway discovered successfully!\n");
        }
        Err(e) => {
            eprintln!("   ✗ Failed to discover UPnP gateway: {}", e);
            eprintln!("   Make sure your router has UPnP enabled.");
            return Ok(());
        }
    }

    // Get external IP address
    println!("2. Retrieving external IP address...");
    match client.get_external_ip().await {
        Ok(ip) => {
            println!("   ✓ External IP: {}\n", ip);
        }
        Err(e) => {
            eprintln!("   ✗ Failed to get external IP: {}", e);
        }
    }

    // Create port mappings for game server
    println!("3. Creating port mappings for game server...");

    // Game traffic port (UDP)
    let game_port = PortMapping::udp(
        27015,
        27015,
        String::new(), // Will auto-detect local IP
        "C&C Generals - Game Traffic".to_string(),
    );

    match client.add_port_mapping(game_port.clone()).await {
        Ok(()) => {
            println!("   ✓ UDP port 27015 forwarded successfully");
        }
        Err(e) => {
            eprintln!("   ✗ Failed to forward UDP port: {}", e);
        }
    }

    // Query port (TCP)
    let query_port = PortMapping::tcp(
        27016,
        27016,
        String::new(),
        "C&C Generals - Query Port".to_string(),
    );

    match client.add_port_mapping(query_port.clone()).await {
        Ok(()) => {
            println!("   ✓ TCP port 27016 forwarded successfully\n");
        }
        Err(e) => {
            eprintln!("   ✗ Failed to forward TCP port: {}", e);
        }
    }

    // List all port mappings
    println!("4. Current port mappings:");
    let mappings = client.get_port_mappings().await;
    for mapping in &mappings {
        println!(
            "   - {} port {} -> {} ({})",
            mapping.protocol, mapping.external_port, mapping.internal_port, mapping.description
        );
    }
    println!();

    // Simulate server running
    println!("5. Server is now accessible from the internet!");
    println!("   Players can connect to your external IP on the forwarded ports.");
    println!("\n   Press Enter to clean up and exit...");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Clean up port mappings
    println!("\n6. Cleaning up port mappings...");
    client.cleanup().await;
    println!("   ✓ All port mappings removed\n");

    println!("=== Demo Complete ===");

    Ok(())
}
