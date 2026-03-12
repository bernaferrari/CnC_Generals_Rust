//! Game Server with UPnP Integration
//!
//! This example demonstrates how to integrate UPnP port forwarding
//! into a C&C Generals Zero Hour game server.
//!
//! Run with: cargo run --example game_server_with_upnp

use game_network::nat::upnp::{PortMapping, UPnPClient, UPnPConfig};
use game_network::nat::{NatConfig, NatService};
use game_network::transport::Transport;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    println!("=== C&C Generals Zero Hour - Game Server ===\n");

    // Create transport layer
    let transport = Arc::new(Transport::new().await?);
    let local_port = 27015; // Default game port
    println!("1. Transport initialized\n");

    // Initialize STUN-based NAT discovery
    println!("2. Discovering public address via STUN...");
    let nat_config = NatConfig::default();
    let nat_service = NatService::new(nat_config);

    let stun_address = match nat_service.refresh(&transport).await? {
        Some(addr) => {
            println!("   ✓ STUN discovered: {}\n", addr);
            Some(addr)
        }
        None => {
            println!("   ✗ STUN discovery failed (may be on local network)\n");
            None
        }
    };

    // Initialize UPnP for automatic port forwarding
    println!("3. Setting up UPnP port forwarding...");
    let mut upnp_config = UPnPConfig::default();
    upnp_config.search_timeout = Duration::from_secs(5);

    let upnp_client = Arc::new(UPnPClient::new(upnp_config));

    let upnp_success = match upnp_client.discover_gateway().await {
        Ok(()) => {
            println!("   ✓ UPnP gateway discovered");

            // Get external IP from UPnP
            if let Ok(external_ip) = upnp_client.get_external_ip().await {
                println!("   ✓ External IP: {}", external_ip);
            }

            // Forward game port (UDP)
            let game_port = PortMapping::udp(
                local_port,
                local_port,
                String::new(), // Auto-detect local IP
                "C&C Generals - Game Server".to_string(),
            );

            match upnp_client.add_port_mapping(game_port).await {
                Ok(()) => {
                    println!("   ✓ UDP port {} forwarded via UPnP", local_port);
                    true
                }
                Err(e) => {
                    println!("   ✗ Failed to forward port via UPnP: {}", e);
                    false
                }
            }
        }
        Err(e) => {
            println!("   ✗ UPnP not available: {}", e);
            println!("   (Players may need manual port forwarding)");
            false
        }
    };

    println!();

    // Start STUN refresh in background
    nat_service.start_auto_refresh(transport.clone()).await;

    // Display server information
    println!("4. Server Status:");
    println!("   Game Port:      {}", local_port);

    if let Some(stun_addr) = stun_address {
        println!("   Public Address: {} (via STUN)", stun_addr);
    }

    if upnp_success {
        println!("   Port Forwarding: Enabled (UPnP)");
        println!(
            "   Players can connect to your external IP on port {}",
            local_port
        );
    } else {
        println!("   Port Forwarding: Manual required");
        println!("   Forward UDP port {} on your router", local_port);
    }

    println!("\n5. Server is running!");
    println!("   Press Ctrl+C to shutdown...\n");

    // Wait for shutdown signal
    let upnp_client_clone = upnp_client.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        println!("\n\n6. Shutting down...");

        // Clean up UPnP port mappings
        if upnp_success {
            println!("   Removing UPnP port mappings...");
            upnp_client_clone.cleanup().await;
        }

        println!("   ✓ Cleanup complete");
        std::process::exit(0);
    });

    // Simulate server loop
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;

        // In a real server, you would:
        // - Accept player connections
        // - Process game commands
        // - Synchronize game state
        // - Handle player disconnections
    }
}
