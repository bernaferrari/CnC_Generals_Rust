//! C++ Compatibility Serialization Tests
//!
//! These tests verify that the Rust serialization matches the C++ NetPacket format
//! using known test vectors from the C++ implementation.

use game_network::commands::cpp_compat_serialization::{
    deserialize_command_cpp_compat, serialize_command_cpp_compat, NetCommandRef,
};
use game_network::commands::{
    ChatData, CommandPayload, DisconnectVoteData, DisconnectVoteType, FileAnnouncementData,
    FileProgressData, FrameInfoData, GameCommandData, NetCommandType, PlayerLeaveData,
    ProgressData, ProgressType, RunAheadMetricsData,
};
use game_network::file_transfer::FileMetadata;
use std::collections::HashMap;

/// Test vector: KeepAlive command from C++ client
/// Expected bytes from C++ NetPacket::FillBufferWithKeepAliveCommand
#[test]
fn test_keep_alive_matches_cpp() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::KeepAlive,
        relay: 0,
        player_id: 1,
        id: 42,
        execution_frame: 100,
        payload: CommandPayload::KeepAlive,
    };

    let serialized = serialize_command_cpp_compat(&cmd);

    // Expected C++ format: T[09] F[64 00 00 00] R[00] P[01] C[2A 00] D
    assert_eq!(serialized[0], b'T');
    assert_eq!(serialized[1], 9); // NetCommandType::KeepAlive
    assert_eq!(serialized[2], b'F');
    assert_eq!(
        u32::from_le_bytes([serialized[3], serialized[4], serialized[5], serialized[6]]),
        100
    );
    assert_eq!(serialized[7], b'R');
    assert_eq!(serialized[8], 0);
    assert_eq!(serialized[9], b'P');
    assert_eq!(serialized[10], 1);
    assert_eq!(serialized[11], b'C');
    assert_eq!(u16::from_le_bytes([serialized[12], serialized[13]]), 42);
    assert_eq!(serialized[14], b'D');
    assert_eq!(serialized.len(), 15); // KeepAlive has no data after 'D'
}

/// Test vector: Chat command from C++ client
#[test]
fn test_chat_matches_cpp() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::Chat,
        relay: 0,
        player_id: 2,
        id: 10,
        execution_frame: 0,
        payload: CommandPayload::Chat(ChatData {
            message: "Hello, world!".to_string(),
            target_mask: 0xFF,
        }),
    };

    let serialized = serialize_command_cpp_compat(&cmd);

    // Verify header
    assert_eq!(serialized[0], b'T');
    assert_eq!(serialized[1], 11); // NetCommandType::Chat
    assert_eq!(serialized[2], b'F');
    assert_eq!(
        u32::from_le_bytes([serialized[3], serialized[4], serialized[5], serialized[6]]),
        0
    );
    assert_eq!(serialized[7], b'R');
    assert_eq!(serialized[8], 0);
    assert_eq!(serialized[9], b'P');
    assert_eq!(serialized[10], 2);
    assert_eq!(serialized[11], b'C');
    assert_eq!(u16::from_le_bytes([serialized[12], serialized[13]]), 10);
    assert_eq!(serialized[14], b'D');

    // Verify chat data (C++ format uses UTF-16 + i32 mask)
    let msg_len = serialized[15] as usize;
    assert_eq!(msg_len, 13);

    let utf16_start = 16;
    let utf16_end = utf16_start + msg_len * 2;
    let mut utf16 = Vec::with_capacity(msg_len);
    for i in 0..msg_len {
        let lo = serialized[utf16_start + i * 2];
        let hi = serialized[utf16_start + i * 2 + 1];
        utf16.push(u16::from_le_bytes([lo, hi]));
    }
    let message = String::from_utf16(&utf16).unwrap();
    assert_eq!(message, "Hello, world!");

    let mask_start = utf16_end;
    let target_mask = i32::from_le_bytes([
        serialized[mask_start],
        serialized[mask_start + 1],
        serialized[mask_start + 2],
        serialized[mask_start + 3],
    ]);
    assert_eq!(target_mask, 0xFF);
}

/// Test vector: FrameInfo command from C++ server
#[test]
fn test_frame_info_matches_cpp() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::FrameInfo,
        relay: 0,
        player_id: 0,
        id: 1,
        execution_frame: 200,
        payload: CommandPayload::FrameInfo(FrameInfoData {
            frame: 200,
            command_count: 0,
            checksum: 0xDEADBEEF,
        }),
    };

    let serialized = serialize_command_cpp_compat(&cmd);

    // Verify header
    assert_eq!(serialized[0], b'T');
    assert_eq!(serialized[1], 3); // NetCommandType::FrameInfo
    assert_eq!(serialized[14], b'D');

    // Verify frame data
    let frame = u32::from_le_bytes([
        serialized[15],
        serialized[16],
        serialized[17],
        serialized[18],
    ]);
    assert_eq!(frame, 200);
    let crc = u32::from_le_bytes([
        serialized[19],
        serialized[20],
        serialized[21],
        serialized[22],
    ]);
    assert_eq!(crc, 0xDEADBEEF);
}

/// Test deserializing a C++ KeepAlive packet
#[test]
fn test_deserialize_cpp_keep_alive() {
    // Simulate bytes from C++ NetPacket
    let cpp_bytes = vec![
        b'T', 9, // Type: KeepAlive
        b'F', 100, 0, 0, 0, // Frame: 100
        b'R', 0, // Relay: 0
        b'P', 1, // Player: 1
        b'C', 42, 0,    // ID: 42
        b'D', // Data (none for KeepAlive)
    ];

    let cmd = deserialize_command_cpp_compat(&cpp_bytes).unwrap();

    assert_eq!(cmd.command_type, NetCommandType::KeepAlive);
    assert_eq!(cmd.player_id, 1);
    assert_eq!(cmd.id, 42);
    assert_eq!(cmd.execution_frame, 100);
    assert_eq!(cmd.relay, 0);
    assert!(matches!(cmd.payload, CommandPayload::KeepAlive));
}

/// Test deserializing C++ packet with tags in different order
#[test]
fn test_deserialize_cpp_different_tag_order() {
    // C++ can send tags in any order
    let cpp_bytes = vec![
        b'T', 9, // Type
        b'R', 0, // Relay (before Frame)
        b'P', 3, // Player
        b'C', 99, 0, // ID
        b'F', 9, 3, 0, 0,    // Frame: 777 (0x309 in little-endian)
        b'D', // Data
    ];

    let cmd = deserialize_command_cpp_compat(&cpp_bytes).unwrap();

    assert_eq!(cmd.command_type, NetCommandType::KeepAlive);
    assert_eq!(cmd.player_id, 3);
    assert_eq!(cmd.id, 99);
    assert_eq!(cmd.execution_frame, 777);
    assert_eq!(cmd.relay, 0);
}

/// Test round-trip serialization for all command types
#[test]
fn test_round_trip_all_types() {
    let test_commands = vec![
        // KeepAlive
        NetCommandRef {
            command_type: NetCommandType::KeepAlive,
            relay: 0,
            player_id: 0,
            id: 1,
            execution_frame: 100,
            payload: CommandPayload::KeepAlive,
        },
        // FrameInfo
        NetCommandRef {
            command_type: NetCommandType::FrameInfo,
            relay: 0,
            player_id: 0,
            id: 2,
            execution_frame: 200,
            payload: CommandPayload::FrameInfo(FrameInfoData {
                frame: 200,
                command_count: 5,
                checksum: 0x12345678,
            }),
        },
        // Chat
        NetCommandRef {
            command_type: NetCommandType::Chat,
            relay: 0,
            player_id: 1,
            id: 3,
            execution_frame: 0,
            payload: CommandPayload::Chat(ChatData {
                message: "Test message".to_string(),
                target_mask: 0x0F,
            }),
        },
        // PlayerLeave
        NetCommandRef {
            command_type: NetCommandType::PlayerLeave,
            relay: 0,
            player_id: 2,
            id: 4,
            execution_frame: 300,
            payload: CommandPayload::PlayerLeave(PlayerLeaveData {
                leaving_player_id: 2,
            }),
        },
        // Progress
        NetCommandRef {
            command_type: NetCommandType::Progress,
            relay: 0,
            player_id: 0,
            id: 5,
            execution_frame: 0,
            payload: CommandPayload::Progress(ProgressData {
                progress_type: ProgressType::Loading,
                percentage: 75,
            }),
        },
        // FileProgress
        NetCommandRef {
            command_type: NetCommandType::FileProgress,
            relay: 0,
            player_id: 0,
            id: 6,
            execution_frame: 0,
            payload: CommandPayload::FileProgress(FileProgressData {
                file_id: 100,
                progress: 50,
            }),
        },
        // RunAheadMetrics
        NetCommandRef {
            command_type: NetCommandType::RunAheadMetrics,
            relay: 0,
            player_id: 0,
            id: 7,
            execution_frame: 400,
            payload: CommandPayload::RunAheadMetrics(RunAheadMetricsData {
                average_latency: 25.5,
                average_fps: 60,
                recommended_frames: 3,
            }),
        },
        // DisconnectVote
        NetCommandRef {
            command_type: NetCommandType::DisconnectVote,
            relay: 0,
            player_id: 1,
            id: 8,
            execution_frame: 500,
            payload: CommandPayload::DisconnectVote(DisconnectVoteData {
                target_slot: 2,
                vote_frame: 5000,
                vote_type: DisconnectVoteType::Kick,
            }),
        },
    ];

    for original in test_commands {
        let serialized = serialize_command_cpp_compat(&original);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        assert_eq!(original.command_type, deserialized.command_type);
        assert_eq!(original.player_id, deserialized.player_id);
        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.execution_frame, deserialized.execution_frame);
        assert_eq!(original.relay, deserialized.relay);
    }
}

/// Test GameCommand serialization with parameters
#[test]
fn test_game_command_with_parameters() {
    let mut params = HashMap::new();
    params.insert(
        "target".to_string(),
        game_network::commands::CommandParameter::ObjectId(999),
    );
    params.insert(
        "count".to_string(),
        game_network::commands::CommandParameter::Int(5),
    );

    let cmd = NetCommandRef {
        command_type: NetCommandType::GameCommand,
        relay: 0,
        player_id: 0,
        id: 50,
        execution_frame: 1000,
        payload: CommandPayload::GameCommand(GameCommandData {
            command_type: 1, // Move command
            target_id: Some(123),
            position: Some((10.0, 20.0, 30.0)),
            parameters: params,
            checksum: 0x12345678,
        }),
    };

    let serialized = serialize_command_cpp_compat(&cmd);
    let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

    assert_eq!(cmd.command_type, deserialized.command_type);

    if let CommandPayload::GameCommand(data) = deserialized.payload {
        assert_eq!(data.command_type, 1);
        assert_eq!(data.target_id, Some(123));
        assert_eq!(data.position, Some((10.0, 20.0, 30.0)));
        assert_eq!(data.checksum, 0x12345678);
        assert_eq!(data.parameters.len(), 2);
    } else {
        panic!("Expected GameCommand payload");
    }
}

/// Test FileAnnounce serialization
#[test]
fn test_file_announce() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::FileAnnounce,
        relay: 0,
        player_id: 0,
        id: 100,
        execution_frame: 0,
        payload: CommandPayload::FileAnnouncement(FileAnnouncementData {
            command_id: 1,
            player_mask: 0x03,
            metadata: FileMetadata {
                filename: "testmap.dat".to_string(),
                file_size: 1024000,
                checksum: [0xAA; 32],
                transfer_type: game_network::file_transfer::TransferType::Map,
            },
        }),
    };

    let serialized = serialize_command_cpp_compat(&cmd);
    let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

    assert_eq!(cmd.command_type, deserialized.command_type);

    if let CommandPayload::FileAnnouncement(data) = deserialized.payload {
        assert_eq!(data.command_id, 1);
        assert_eq!(data.player_mask, 0x03);
        assert_eq!(data.metadata.filename, "testmap.dat");
        // C++ FileAnnounce only transmits filename + command id + mask.
        // Size/checksum are not serialized, so they default on decode.
        assert_eq!(data.metadata.file_size, 0);
        assert_eq!(data.metadata.checksum, [0u8; 32]);
    } else {
        panic!("Expected FileAnnouncement payload");
    }
}

/// Test error handling: invalid tag
#[test]
fn test_invalid_tag() {
    let bad_bytes = vec![b'X', 0, 0, 0]; // 'X' is not a valid tag
    let result = deserialize_command_cpp_compat(&bad_bytes);
    assert!(result.is_err());
}

/// Test error handling: truncated command
#[test]
fn test_truncated_command() {
    let truncated = vec![b'T', 9, b'F']; // Missing frame data
    let result = deserialize_command_cpp_compat(&truncated);
    assert!(result.is_err());
}

/// Test error handling: empty data
#[test]
fn test_empty_data() {
    let empty: Vec<u8> = vec![];
    let result = deserialize_command_cpp_compat(&empty);
    assert!(result.is_err());
}

/// Test serialization size is reasonable
#[test]
fn test_serialization_size() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::KeepAlive,
        relay: 0,
        player_id: 0,
        id: 1,
        execution_frame: 0,
        payload: CommandPayload::KeepAlive,
    };

    let serialized = serialize_command_cpp_compat(&cmd);

    // KeepAlive should be small: T(2) + F(5) + R(2) + P(2) + C(3) + D(1) = 15 bytes
    assert_eq!(serialized.len(), 15);
    assert!(serialized.len() < 100); // Much smaller than bincode serialization
}

/// Test chat with Unicode characters
#[test]
fn test_chat_unicode() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::Chat,
        relay: 0,
        player_id: 0,
        id: 1,
        execution_frame: 0,
        payload: CommandPayload::Chat(ChatData {
            message: "Hello 世界 🎮".to_string(),
            target_mask: 0xFF,
        }),
    };

    let serialized = serialize_command_cpp_compat(&cmd);
    let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

    if let CommandPayload::Chat(data) = deserialized.payload {
        assert_eq!(data.message, "Hello 世界 🎮");
        assert_eq!(data.target_mask, 0xFF);
    } else {
        panic!("Expected Chat payload");
    }
}

/// Test command with maximum values
#[test]
fn test_maximum_values() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::FrameInfo,
        relay: 0xFF,
        player_id: 0xFF,
        id: 0xFFFF,
        execution_frame: 0xFFFFFFFF,
        payload: CommandPayload::FrameInfo(FrameInfoData {
            frame: 0xFFFFFFFF,
            command_count: 0,
            checksum: 0xFFFFFFFF,
        }),
    };

    let serialized = serialize_command_cpp_compat(&cmd);
    let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

    assert_eq!(deserialized.relay, 0xFF);
    assert_eq!(deserialized.player_id, 0xFF);
    assert_eq!(deserialized.id, 0xFFFF);
    assert_eq!(deserialized.execution_frame, 0xFFFFFFFF);
}

/// Test all parameter types in GameCommand
#[test]
fn test_all_parameter_types() {
    let mut params = HashMap::new();
    params.insert(
        "int_param".to_string(),
        game_network::commands::CommandParameter::Int(-42),
    );
    params.insert(
        "float_param".to_string(),
        game_network::commands::CommandParameter::Float(3.14),
    );
    params.insert(
        "bool_param".to_string(),
        game_network::commands::CommandParameter::Bool(true),
    );
    params.insert(
        "obj_param".to_string(),
        game_network::commands::CommandParameter::ObjectId(12345),
    );
    params.insert(
        "pos_param".to_string(),
        game_network::commands::CommandParameter::Position(1.0, 2.0, 3.0),
    );
    params.insert(
        "str_param".to_string(),
        game_network::commands::CommandParameter::String("test".to_string()),
    );

    let cmd = NetCommandRef {
        command_type: NetCommandType::GameCommand,
        relay: 0,
        player_id: 0,
        id: 1,
        execution_frame: 0,
        payload: CommandPayload::GameCommand(GameCommandData {
            command_type: 1,
            target_id: None,
            position: None,
            parameters: params,
            checksum: 0,
        }),
    };

    let serialized = serialize_command_cpp_compat(&cmd);
    let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

    if let CommandPayload::GameCommand(data) = deserialized.payload {
        assert_eq!(data.parameters.len(), 6);

        // Verify each parameter type
        if let Some(game_network::commands::CommandParameter::Int(v)) =
            data.parameters.get("int_param")
        {
            assert_eq!(*v, -42);
        } else {
            panic!("int_param not found or wrong type");
        }

        if let Some(game_network::commands::CommandParameter::Float(v)) =
            data.parameters.get("float_param")
        {
            assert!((v - 3.14).abs() < 0.001);
        } else {
            panic!("float_param not found or wrong type");
        }

        if let Some(game_network::commands::CommandParameter::Bool(v)) =
            data.parameters.get("bool_param")
        {
            assert!(*v);
        } else {
            panic!("bool_param not found or wrong type");
        }
    } else {
        panic!("Expected GameCommand payload");
    }
}

/// Benchmark helper: measure serialization performance
#[test]
fn test_serialization_performance() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::KeepAlive,
        relay: 0,
        player_id: 0,
        id: 1,
        execution_frame: 100,
        payload: CommandPayload::KeepAlive,
    };

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _ = serialize_command_cpp_compat(&cmd);
    }
    let elapsed = start.elapsed();

    println!(
        "Serialized 10000 KeepAlive commands in {:?} ({:?} per command)",
        elapsed,
        elapsed / 10000
    );

    // Should be fast (< 1ms per 10k commands on modern hardware)
    assert!(elapsed.as_millis() < 100);
}

/// Benchmark helper: measure deserialization performance
#[test]
fn test_deserialization_performance() {
    let cmd = NetCommandRef {
        command_type: NetCommandType::KeepAlive,
        relay: 0,
        player_id: 0,
        id: 1,
        execution_frame: 100,
        payload: CommandPayload::KeepAlive,
    };

    let serialized = serialize_command_cpp_compat(&cmd);

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _ = deserialize_command_cpp_compat(&serialized).unwrap();
    }
    let elapsed = start.elapsed();

    println!(
        "Deserialized 10000 KeepAlive commands in {:?} ({:?} per command)",
        elapsed,
        elapsed / 10000
    );

    // Should be fast (< 1ms per 10k commands on modern hardware)
    assert!(elapsed.as_millis() < 100);
}
