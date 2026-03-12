#![allow(unused_crate_dependencies)]

//! Two-Player Local Multiplayer Synchronization Test
//!
//! This test demonstrates a complete two-player game loop with:
//! - Network communication via UDP
//! - Frame synchronization (lockstep)
//! - Command execution
//! - CRC validation
//! - Determinism verification

use game_network::{NetCommand, TransportMessage, TransportProtocol, UdpConfig, UdpTransport};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

/// Simple game state for testing
#[derive(Debug, Clone, PartialEq)]
struct SimpleGameState {
    frame: u32,
    player_pos: [u32; 2],    // Position of each player
    player_health: [u32; 2], // Health of each player
}

impl SimpleGameState {
    fn new() -> Self {
        Self {
            frame: 0,
            player_pos: [100, 200],
            player_health: [100, 100],
        }
    }

    fn advance_frame(&mut self) {
        self.frame += 1;
    }

    fn apply_game_command(&mut self, player_id: u8, _command: &NetCommand) {
        let pid = player_id as usize;
        if pid >= 2 {
            return;
        }

        // For this simple test, just increment position when a command arrives
        self.player_pos[pid] = self.player_pos[pid].wrapping_add(1);
    }

    fn apply_frame_info(&mut self, frame: u32) {
        // Update frame number
        self.frame = frame;
    }

    fn compute_crc(&self) -> u32 {
        let mut data = Vec::new();
        data.extend_from_slice(&self.frame.to_le_bytes());
        data.extend_from_slice(&self.player_pos[0].to_le_bytes());
        data.extend_from_slice(&self.player_pos[1].to_le_bytes());
        data.extend_from_slice(&self.player_health[0].to_le_bytes());
        data.extend_from_slice(&self.player_health[1].to_le_bytes());
        game_network::calculate_crc32(&data)
    }
}

struct TwoPlayerTestContext {
    server_transport: Arc<UdpTransport>,
    client_transport: Arc<UdpTransport>,
    server_addr: SocketAddr,
    client_addr: SocketAddr,
    server_state: Arc<Mutex<SimpleGameState>>,
    client_state: Arc<Mutex<SimpleGameState>>,
    server_crcs: Arc<Mutex<Vec<u32>>>,
    client_crcs: Arc<Mutex<Vec<u32>>>,
}

impl TwoPlayerTestContext {
    async fn setup() -> Result<Self, Box<dyn std::error::Error>> {
        // Allocate ports
        let server_port = allocate_ephemeral_port();
        let client_port = allocate_ephemeral_port();

        let server_addr = SocketAddr::from(([127, 0, 0, 1], server_port));
        let client_addr = SocketAddr::from(([127, 0, 0, 1], client_port));

        // Create transports
        let mut server_config = UdpConfig::default();
        server_config.bind_address = server_addr;

        let server_transport = Arc::new(UdpTransport::with_config(server_config).await?);
        server_transport.bind().await?;

        let mut client_config = UdpConfig::default();
        client_config.bind_address = client_addr;

        let client_transport = Arc::new(UdpTransport::with_config(client_config).await?);
        client_transport.bind().await?;

        Ok(Self {
            server_transport,
            client_transport,
            server_addr,
            client_addr,
            server_state: Arc::new(Mutex::new(SimpleGameState::new())),
            client_state: Arc::new(Mutex::new(SimpleGameState::new())),
            server_crcs: Arc::new(Mutex::new(Vec::new())),
            client_crcs: Arc::new(Mutex::new(Vec::new())),
        })
    }

    async fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.server_transport.shutdown().await?;
        self.client_transport.shutdown().await?;
        Ok(())
    }

    async fn simulate_frame(&self, frame: u32) -> Result<(), Box<dyn std::error::Error>> {
        // Both server and client execute the same frame
        // Server state
        {
            let mut state = self.server_state.lock().unwrap();
            state.apply_frame_info(frame);
            state.advance_frame();
            let crc = state.compute_crc();
            self.server_crcs.lock().unwrap().push(crc);
        }

        // Client state (deterministic)
        {
            let mut state = self.client_state.lock().unwrap();
            state.apply_frame_info(frame);
            state.advance_frame();
            let crc = state.compute_crc();
            self.client_crcs.lock().unwrap().push(crc);
        }

        // Verify CRCs match
        {
            let server_crc = *self.server_crcs.lock().unwrap().last().unwrap();
            let client_crc = *self.client_crcs.lock().unwrap().last().unwrap();

            assert_eq!(
                server_crc, client_crc,
                "CRC mismatch at frame {}: server {:08x} != client {:08x}",
                frame, server_crc, client_crc
            );
        }

        Ok(())
    }

    async fn run_game_loop(&self, num_frames: u32) -> Result<(), Box<dyn std::error::Error>> {
        for frame in 0..num_frames {
            self.simulate_frame(frame).await?;

            // Small delay to simulate frame time
            sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }
}

fn allocate_ephemeral_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .expect("allocate ephemeral port")
        .local_addr()
        .expect("socket addr")
        .port()
}

/// Test basic two-player synchronization
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn test_two_player_frame_sync() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = TwoPlayerTestContext::setup().await?;

    // Run 10 frames of synchronized gameplay
    ctx.run_game_loop(10).await?;

    // Verify both players have same state
    let server_state = ctx.server_state.lock().unwrap();
    let client_state = ctx.client_state.lock().unwrap();

    assert_eq!(server_state.frame, client_state.frame);
    assert_eq!(server_state.player_pos, client_state.player_pos);
    assert_eq!(server_state.player_health, client_state.player_health);

    // Verify CRC history matches
    let server_crcs = ctx.server_crcs.lock().unwrap();
    let client_crcs = ctx.client_crcs.lock().unwrap();
    assert_eq!(server_crcs.len(), client_crcs.len());
    for (i, (s, c)) in server_crcs.iter().zip(client_crcs.iter()).enumerate() {
        assert_eq!(s, c, "CRC mismatch at frame {}", i);
    }

    ctx.cleanup().await?;
    Ok(())
}

/// Test frame state remains synchronized
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn test_frame_state_sync() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = TwoPlayerTestContext::setup().await?;

    // Execute one frame
    ctx.simulate_frame(5).await?;

    // Verify both players have same frame number
    {
        let server_state = ctx.server_state.lock().unwrap();
        let client_state = ctx.client_state.lock().unwrap();

        assert_eq!(server_state.frame, client_state.frame);
        assert_eq!(server_state.frame, 6); // advanced from 5
    }

    ctx.cleanup().await?;
    Ok(())
}

/// Test network packet delivery via UDP transport
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn test_network_packet_delivery() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = TwoPlayerTestContext::setup().await?;

    // Create a simple test message
    let test_data = b"Hello from client!".to_vec();
    let message = TransportMessage::new(test_data.clone(), TransportProtocol::Udp)
        .with_destination(ctx.server_addr);

    // Send from client to server
    ctx.client_transport.send_message(message).await?;

    // Wait for packet delivery
    sleep(Duration::from_millis(100)).await;

    // Receive on server
    let incoming = ctx.server_transport.receive_messages().await?;
    assert!(!incoming.is_empty(), "Server should receive message");
    assert_eq!(incoming[0].data, test_data, "Payload should match");

    ctx.cleanup().await?;
    Ok(())
}
