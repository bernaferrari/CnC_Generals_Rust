//! Comprehensive message protocol tests
//!
//! These tests verify that the Rust implementation is binary-compatible with
//! the C++ original and that all message types serialize/deserialize correctly.

use async_trait::async_trait;
use game_network::commands::binary_format::BinaryMessageCodec;
use game_network::commands::routing::{CommandHandler, CommandRouter, LoggingHandler};
use game_network::commands::{CommandPayload, NetCommand, NetCommandType, ProgressType};
use game_network::error::NetworkResult;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Test all message types for serialization round-trip
#[tokio::test]
async fn test_all_message_types_roundtrip() {
    let codec = BinaryMessageCodec::new();

    // Test each message type
    let test_cases = vec![
        ("KeepAlive", NetCommand::keep_alive(0)),
        (
            "Chat",
            NetCommand::chat(1, "Test message".to_string(), 0xFF),
        ),
        (
            "DisconnectChat",
            NetCommand::disconnect_chat(2, "Goodbye".to_string(), 0xFF),
        ),
        (
            "Progress",
            NetCommand::progress(0, ProgressType::Loading, 75),
        ),
        ("LoadComplete", NetCommand::load_complete(1)),
        ("TimeoutStart", NetCommand::timeout_start(2)),
    ];

    for (name, command) in test_cases {
        let serialized = codec
            .serialize(&command)
            .unwrap_or_else(|_| panic!("{} serialization failed", name));

        let deserialized = codec
            .deserialize(&serialized)
            .unwrap_or_else(|_| panic!("{} deserialization failed", name));

        assert_eq!(
            command.command_type, deserialized.command_type,
            "{} command type mismatch",
            name
        );
        assert_eq!(
            command.player_id, deserialized.player_id,
            "{} player ID mismatch",
            name
        );
        assert_eq!(
            command.execution_frame, deserialized.execution_frame,
            "{} execution frame mismatch",
            name
        );

        println!(
            "✓ {} roundtrip successful ({} bytes)",
            name,
            serialized.len()
        );
    }
}

/// Test binary format validation
#[test]
fn test_binary_format_validation() {
    let codec = BinaryMessageCodec::new();

    // Test: Message too short
    let too_short = vec![1, 2, 3];
    assert!(
        codec.validate_format(&too_short).is_err(),
        "Should reject message shorter than header"
    );

    // Test: Valid header
    let valid_header = vec![
        NetCommandType::KeepAlive as u8,
        0, // player 0
        0,
        0, // command id
        0,
        0,
        0,
        0, // frame number
    ];
    assert!(
        codec.validate_format(&valid_header).is_ok(),
        "Should accept valid header"
    );

    // Test: Invalid player ID
    let invalid_player = vec![
        NetCommandType::KeepAlive as u8,
        255, // invalid player
        0,
        0, // command id
        0,
        0,
        0,
        0, // frame
    ];
    assert!(
        codec.validate_format(&invalid_player).is_err(),
        "Should reject invalid player ID"
    );

    // Test: Unknown command type
    let unknown_type = vec![
        255, // unknown type
        0,   // player 0
        0, 0, // command id
        0, 0, 0, 0, // frame
    ];
    assert!(
        codec.validate_format(&unknown_type).is_err(),
        "Should reject unknown command type"
    );
}

/// Test C++ binary compatibility for message header
#[test]
fn test_cpp_binary_compatibility_header() {
    let codec = BinaryMessageCodec::new();

    // Create a command with known values
    let mut command = NetCommand::keep_alive(3);
    command.sequence = 0x1234;
    command.execution_frame = 0x56789ABC;

    let serialized = codec.serialize(&command).unwrap();

    // Verify exact binary layout matches C++ struct:
    // struct {
    //   u8 type;
    //   u8 player_id;
    //   u16 command_id;   // little-endian
    //   u32 frame;        // little-endian
    // }

    assert_eq!(serialized[0], NetCommandType::KeepAlive as u8);
    assert_eq!(serialized[1], 3); // player_id

    // Command ID (little-endian u16)
    assert_eq!(serialized[2], 0x34);
    assert_eq!(serialized[3], 0x12);

    // Frame number (little-endian u32)
    assert_eq!(serialized[4], 0xBC);
    assert_eq!(serialized[5], 0x9A);
    assert_eq!(serialized[6], 0x78);
    assert_eq!(serialized[7], 0x56);
}

/// Test chat message UTF-16 encoding (C++ uses UnicodeString)
#[test]
fn test_chat_utf16_encoding() {
    let codec = BinaryMessageCodec::new();

    let message = "Hello, World! 你好";
    let command = NetCommand::chat(0, message.to_string(), 0xFF);

    let serialized = codec.serialize(&command).unwrap();
    let deserialized = codec.deserialize(&serialized).unwrap();

    if let CommandPayload::Chat(data) = &deserialized.payload {
        assert_eq!(data.message, message, "UTF-16 message mismatch");
        assert_eq!(data.target_mask, 0xFF);
    } else {
        panic!("Expected Chat payload");
    }
}

/// Test message routing system
#[tokio::test]
async fn test_message_routing() {
    let router = CommandRouter::new();

    // Create a test handler
    struct TestHandler {
        received: Arc<RwLock<Vec<NetCommandType>>>,
    }

    impl TestHandler {
        fn new() -> Self {
            Self {
                received: Arc::new(RwLock::new(Vec::new())),
            }
        }

        async fn get_received(&self) -> Vec<NetCommandType> {
            self.received.read().await.clone()
        }
    }

    #[async_trait]
    impl CommandHandler for TestHandler {
        async fn handle_command(&self, command: &NetCommand) -> NetworkResult<()> {
            let mut received = self.received.write().await;
            received.push(command.command_type);
            Ok(())
        }

        fn supported_types(&self) -> Vec<NetCommandType> {
            vec![
                NetCommandType::KeepAlive,
                NetCommandType::Chat,
                NetCommandType::Progress,
            ]
        }
    }

    let handler = Arc::new(TestHandler::new());
    router.register_handler(handler.clone()).await;

    // Route various commands
    router
        .route_command(&NetCommand::keep_alive(0))
        .await
        .unwrap();
    router
        .route_command(&NetCommand::chat(1, "test".to_string(), 0xFF))
        .await
        .unwrap();
    router
        .route_command(&NetCommand::progress(2, ProgressType::Loading, 50))
        .await
        .unwrap();

    let received = handler.get_received().await;
    assert_eq!(received.len(), 3);
    assert_eq!(received[0], NetCommandType::KeepAlive);
    assert_eq!(received[1], NetCommandType::Chat);
    assert_eq!(received[2], NetCommandType::Progress);

    // Check stats
    let stats = router.get_stats().await;
    assert_eq!(stats.total_routed, 3);
    assert_eq!(stats.unhandled, 0);
    assert_eq!(stats.failed, 0);
}

/// Test priority-based command queuing
#[tokio::test]
async fn test_priority_queuing() {
    let router = CommandRouter::new();

    let handler = Arc::new(LoggingHandler::new("test"));
    router.register_handler(handler).await;

    // Queue commands in reverse priority order
    let mut low_cmd = NetCommand::keep_alive(0);
    low_cmd.priority = game_network::commands::CommandPriority::Low;
    router.queue_command(low_cmd).await.unwrap();

    let mut critical_cmd = NetCommand::chat(1, "urgent".to_string(), 0xFF);
    critical_cmd.priority = game_network::commands::CommandPriority::Critical;
    router.queue_command(critical_cmd).await.unwrap();

    let mut normal_cmd = NetCommand::keep_alive(2);
    normal_cmd.priority = game_network::commands::CommandPriority::Normal;
    router.queue_command(normal_cmd).await.unwrap();

    // Process all queued commands
    let processed = router.process_queued(10).await.unwrap();
    assert_eq!(processed, 3);

    // Verify queue is empty
    assert_eq!(router.queue_depth().await, 0);
}

/// Test command validation integration
#[test]
fn test_command_validation() {
    let codec = BinaryMessageCodec::new();

    // Valid command
    let valid_cmd = NetCommand::chat(3, "Valid message".to_string(), 0xFF);
    assert!(valid_cmd.validate().is_ok());

    let serialized = codec.serialize(&valid_cmd).unwrap();
    assert!(codec.validate_format(&serialized).is_ok());

    // Invalid player ID
    let invalid_cmd = NetCommand::keep_alive(255);
    assert!(invalid_cmd.validate().is_err());
}

/// Test serialization with different frame numbers
#[test]
fn test_frame_number_serialization() {
    let codec = BinaryMessageCodec::new();

    let test_frames = vec![0u32, 1, 100, 1000, 0xFFFFFFFF];

    for frame in test_frames {
        let mut command = NetCommand::keep_alive(0);
        command.execution_frame = frame;

        let serialized = codec.serialize(&command).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();

        assert_eq!(
            deserialized.execution_frame, frame,
            "Frame number mismatch for {}",
            frame
        );
    }
}

/// Test all 30+ command types from C++ enum
#[test]
fn test_all_command_types_defined() {
    // Verify we have all command types from C++
    let all_types = vec![
        NetCommandType::AckBoth,             // 0
        NetCommandType::AckStage1,           // 1
        NetCommandType::AckStage2,           // 2
        NetCommandType::FrameInfo,           // 3
        NetCommandType::GameCommand,         // 4
        NetCommandType::PlayerLeave,         // 5
        NetCommandType::RunAheadMetrics,     // 6
        NetCommandType::RunAhead,            // 7
        NetCommandType::DestroyPlayer,       // 8
        NetCommandType::KeepAlive,           // 9
        NetCommandType::DisconnectChat,      // 10
        NetCommandType::Chat,                // 11
        NetCommandType::ManglerQuery,        // 12
        NetCommandType::ManglerResponse,     // 13
        NetCommandType::Progress,            // 14
        NetCommandType::LoadComplete,        // 15
        NetCommandType::TimeoutStart,        // 16
        NetCommandType::Wrapper,             // 17
        NetCommandType::File,                // 18
        NetCommandType::FileAnnounce,        // 19
        NetCommandType::FileProgress,        // 20
        NetCommandType::FrameResendRequest,  // 21
        NetCommandType::DisconnectStart,     // 22
        NetCommandType::DisconnectKeepAlive, // 23
        NetCommandType::DisconnectPlayer,    // 24
        NetCommandType::PacketRouterQuery,   // 25
        NetCommandType::PacketRouterAck,     // 26
        NetCommandType::DisconnectVote,      // 27
        NetCommandType::DisconnectFrame,     // 28
        NetCommandType::DisconnectScreenOff, // 29
        NetCommandType::DisconnectEnd,       // 30
    ];

    // Verify enum value matches index
    for (expected_value, cmd_type) in all_types.iter().enumerate() {
        let actual_value = *cmd_type as u8;
        assert_eq!(
            actual_value, expected_value as u8,
            "Command type {:?} has wrong enum value",
            cmd_type
        );
    }

    println!("✓ All {} command types verified", all_types.len());
}

/// Performance test: serialize/deserialize throughput
#[test]
fn test_serialization_performance() {
    let codec = BinaryMessageCodec::new();
    let iterations = 10000;

    let start = std::time::Instant::now();

    for i in 0..iterations {
        let mut command = NetCommand::chat((i % 8) as u8, format!("Message {}", i), 0xFF);
        command.execution_frame = i;

        let serialized = codec.serialize(&command).unwrap();
        let _deserialized = codec.deserialize(&serialized).unwrap();
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (iterations as f64 / elapsed.as_secs_f64()) as u64;

    println!(
        "✓ Serialization performance: {} ops/sec ({} iterations in {:?})",
        ops_per_sec, iterations, elapsed
    );

    // Should handle at least 10k messages per second
    assert!(
        ops_per_sec > 10000,
        "Serialization too slow: {} ops/sec",
        ops_per_sec
    );
}

/// Test error handling for corrupted messages
#[test]
fn test_corrupted_message_handling() {
    let codec = BinaryMessageCodec::new();

    // Create valid message
    let command = NetCommand::chat(0, "test".to_string(), 0xFF);
    let mut serialized = codec.serialize(&command).unwrap();

    // Corrupt the message
    serialized[2] = 0xFF; // Corrupt command ID byte
    serialized.truncate(serialized.len() - 2); // Truncate

    // Should fail gracefully
    let result = codec.deserialize(&serialized);
    assert!(result.is_err(), "Should reject corrupted message");
}
