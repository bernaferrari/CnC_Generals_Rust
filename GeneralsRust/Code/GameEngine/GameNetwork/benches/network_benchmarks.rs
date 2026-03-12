//! Performance benchmarks for network layer operations
//!
//! This benchmark suite measures the performance of critical network operations:
//! - CRC32 computation for various payload sizes
//! - XOR encryption/decryption throughput
//! - Packet construction and parsing
//! - Frame manager operations
//! - Command execution
//! - Network message serialization
//!
//! Run with: cargo +nightly bench --bench network_benchmarks
//!
//! Expected performance targets:
//! - CRC32: > 1 GB/s throughput
//! - XOR encryption: > 5 GB/s throughput
//! - Packet construction: < 1 microsecond for max payload
//! - Packet parsing: < 1 microsecond for max payload
//! - Frame operations: < 10 microseconds per frame
//! - Command serialization: < 500 nanoseconds per command

#![cfg_attr(feature = "nightly_bench", feature(test))]

#[cfg(feature = "nightly_bench")]
mod benches {
    extern crate test;

    use game_network::commands::{
        CommandParameter, CommandPayload, GameCommandData, NetCommand, NetCommandType,
    };
    use game_network::frame_data::{FrameCRC, FrameData, GameStateCRC, CRC};
    use game_network::net_packet::{
        CommandPacketPayload, DisconnectPayload, DisconnectReason, FrameSyncPayload,
        HandshakePayload, NetPacket, NetPacketHeader, PacketPayload, PingPayload,
    };
    use game_network::security::encryption::{
        decode_envelope, encode_encrypted_envelope, encode_plain_envelope, EncryptedPacket,
        EncryptionProvider,
    };
    use std::collections::HashMap;
    use test::Bencher;

    // ============================================================================
    // CRC COMPUTATION BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_crc_32_bytes(b: &mut Bencher) {
        let data = vec![0u8; 32];
        b.iter(|| {
            let crc = CRC::compute_once(&data);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_crc_64_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 64];
        b.iter(|| {
            let crc = CRC::compute_once(&data);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_crc_128_bytes(b: &mut Bencher) {
        let data = vec![0xAAu8; 128];
        b.iter(|| {
            let crc = CRC::compute_once(&data);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_crc_256_bytes(b: &mut Bencher) {
        let data = vec![0x55u8; 256];
        b.iter(|| {
            let crc = CRC::compute_once(&data);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_crc_470_bytes_max_payload(b: &mut Bencher) {
        // Maximum packet payload size
        let data = vec![0xFFu8; 470];
        b.iter(|| {
            let crc = CRC::compute_once(&data);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_crc_1024_bytes(b: &mut Bencher) {
        let data = vec![0x12u8; 1024];
        b.iter(|| {
            let crc = CRC::compute_once(&data);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_crc_incremental_32_bytes(b: &mut Bencher) {
        let data = vec![0xABu8; 32];
        b.iter(|| {
            let mut crc = CRC::new();
            crc.compute(&data);
            test::black_box(crc.get());
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_crc_with_initial_value(b: &mut Bencher) {
        let data = vec![0xCDu8; 256];
        let initial = 0xDEADBEEF;
        b.iter(|| {
            let crc = CRC::compute_with_initial(&data, initial);
            test::black_box(crc);
        });
    }

    // ============================================================================
    // XOR ENCRYPTION/DECRYPTION BENCHMARKS
    // ============================================================================

    fn xor_encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, &byte)| byte ^ key[i % key.len()])
            .collect()
    }

    fn xor_decrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
        // XOR is symmetric
        xor_encrypt(data, key)
    }

    #[cfg(test)]
    #[bench]
    fn bench_xor_encrypt_64_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 64];
        let key = b"SecretKey123456"; // 16 byte key
        b.iter(|| {
            let encrypted = xor_encrypt(&data, key);
            test::black_box(encrypted);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_xor_encrypt_128_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 128];
        let key = b"SecretKey123456";
        b.iter(|| {
            let encrypted = xor_encrypt(&data, key);
            test::black_box(encrypted);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_xor_encrypt_256_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 256];
        let key = b"SecretKey123456";
        b.iter(|| {
            let encrypted = xor_encrypt(&data, key);
            test::black_box(encrypted);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_xor_encrypt_470_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 470];
        let key = b"SecretKey123456";
        b.iter(|| {
            let encrypted = xor_encrypt(&data, key);
            test::black_box(encrypted);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_xor_encrypt_1024_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 1024];
        let key = b"SecretKey123456";
        b.iter(|| {
            let encrypted = xor_encrypt(&data, key);
            test::black_box(encrypted);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_xor_decrypt_256_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 256];
        let key = b"SecretKey123456";
        let encrypted = xor_encrypt(&data, key);

        b.iter(|| {
            let decrypted = xor_decrypt(&encrypted, key);
            test::black_box(decrypted);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_xor_roundtrip_470_bytes(b: &mut Bencher) {
        let data = vec![0x42u8; 470];
        let key = b"SecretKey123456";

        b.iter(|| {
            let encrypted = xor_encrypt(&data, key);
            let decrypted = xor_decrypt(&encrypted, key);
            test::black_box(decrypted);
        });
    }

    // ============================================================================
    // PACKET CONSTRUCTION BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_packet_header_creation(b: &mut Bencher) {
        b.iter(|| {
            let header = NetPacketHeader::new(12345, 67890);
            test::black_box(header);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_header_serialization(b: &mut Bencher) {
        let header = NetPacketHeader::new(PacketType::Command, 12345, 67890);
        b.iter(|| {
            let bytes = header.to_bytes();
            test::black_box(bytes);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_header_deserialization(b: &mut Bencher) {
        let header = NetPacketHeader::new(PacketType::Command, 12345, 67890);
        let bytes = header.to_bytes();

        b.iter(|| {
            let parsed = NetPacketHeader::from_bytes(&bytes).unwrap();
            test::black_box(parsed);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_header_checksum(b: &mut Bencher) {
        let header = NetPacketHeader::new(PacketType::Command, 12345, 67890);
        b.iter(|| {
            let checksum = header.calculate_checksum();
            test::black_box(checksum);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_construction_small(b: &mut Bencher) {
        b.iter(|| {
            let packet = NetPacket::ack(1234, 5678);
            test::black_box(packet);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_construction_ping(b: &mut Bencher) {
        b.iter(|| {
            let packet = NetPacket::ping(9999);
            test::black_box(packet);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_construction_command(b: &mut Bencher) {
        let commands = vec![
            NetCommand::keep_alive(0),
            NetCommand::keep_alive(1),
            NetCommand::keep_alive(2),
        ];

        b.iter(|| {
            let packet = NetPacket::command(1000, 999, commands.clone());
            test::black_box(packet);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_serialization_ack(b: &mut Bencher) {
        let packet = NetPacket::ack(1234, 5678);

        b.iter(|| {
            let bytes = packet.to_bytes().unwrap();
            test::black_box(bytes);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_serialization_ping(b: &mut Bencher) {
        let packet = NetPacket::ping(9999);

        b.iter(|| {
            let bytes = packet.to_bytes().unwrap();
            test::black_box(bytes);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_serialization_command(b: &mut Bencher) {
        let commands = vec![NetCommand::keep_alive(0), NetCommand::keep_alive(1)];
        let packet = NetPacket::command(1000, 999, commands);

        b.iter(|| {
            let bytes = packet.to_bytes().unwrap();
            test::black_box(bytes);
        });
    }

    // ============================================================================
    // PACKET PARSING BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_packet_parsing_ack(b: &mut Bencher) {
        let packet = NetPacket::ack(1234, 5678);
        let bytes = packet.to_bytes().unwrap();

        b.iter(|| {
            let parsed = NetPacket::from_bytes(&bytes).unwrap();
            test::black_box(parsed);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_parsing_ping(b: &mut Bencher) {
        let packet = NetPacket::ping(9999);
        let bytes = packet.to_bytes().unwrap();

        b.iter(|| {
            let parsed = NetPacket::from_bytes(&bytes).unwrap();
            test::black_box(parsed);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_parsing_command(b: &mut Bencher) {
        let commands = vec![NetCommand::keep_alive(0), NetCommand::keep_alive(1)];
        let packet = NetPacket::command(1000, 999, commands);
        let bytes = packet.to_bytes().unwrap();

        b.iter(|| {
            let parsed = NetPacket::from_bytes(&bytes).unwrap();
            test::black_box(parsed);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_roundtrip_ack(b: &mut Bencher) {
        b.iter(|| {
            let packet = NetPacket::ack(1234, 5678);
            let bytes = packet.to_bytes().unwrap();
            let parsed = NetPacket::from_bytes(&bytes).unwrap();
            test::black_box(parsed);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_roundtrip_ping(b: &mut Bencher) {
        b.iter(|| {
            let packet = NetPacket::ping(9999);
            let bytes = packet.to_bytes().unwrap();
            let parsed = NetPacket::from_bytes(&bytes).unwrap();
            test::black_box(parsed);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_packet_roundtrip_command(b: &mut Bencher) {
        let commands = vec![NetCommand::keep_alive(0)];

        b.iter(|| {
            let packet = NetPacket::command(1000, 999, commands.clone());
            let bytes = packet.to_bytes().unwrap();
            let parsed = NetPacket::from_bytes(&bytes).unwrap();
            test::black_box(parsed);
        });
    }

    // ============================================================================
    // FRAME MANAGER BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_frame_creation(b: &mut Bencher) {
        b.iter(|| {
            let frame = FrameData::new(1000);
            test::black_box(frame);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_add_command_single(b: &mut Bencher) {
        let frame = FrameData::new(1000);
        let command = NetCommand::keep_alive(0).with_sequence(0);

        b.iter(|| {
            let mut frame_copy = frame.clone();
            frame_copy.add_command(command.clone()).unwrap();
            test::black_box(frame_copy);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_add_command_multiple(b: &mut Bencher) {
        let commands: Vec<_> = (0..8)
            .map(|i| NetCommand::keep_alive(i as u8).with_sequence(0))
            .collect();

        b.iter(|| {
            let mut frame = FrameData::new(1000);
            for cmd in &commands {
                frame.add_command(cmd.clone()).unwrap();
            }
            test::black_box(frame);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_get_all_commands_ordered(b: &mut Bencher) {
        let mut frame = FrameData::new(1000);

        // Add commands from 8 players
        for player_id in 0..8 {
            for seq in 0..5 {
                let cmd = NetCommand::keep_alive(player_id).with_sequence(seq);
                frame.add_command(cmd).unwrap();
            }
        }

        b.iter(|| {
            let commands = frame.get_all_commands_ordered();
            test::black_box(commands);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_checksum_calculation_2_players(b: &mut Bencher) {
        let mut frame = FrameData::new(1000);

        // Add commands from 2 players
        for player_id in 0..2 {
            for seq in 0..10 {
                let cmd = NetCommand::keep_alive(player_id).with_sequence(seq);
                frame.add_command(cmd).unwrap();
            }
        }

        b.iter(|| {
            let mut frame_copy = frame.clone();
            frame_copy.mark_complete();
            test::black_box(frame_copy.checksum);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_checksum_calculation_4_players(b: &mut Bencher) {
        let mut frame = FrameData::new(1000);

        // Add commands from 4 players
        for player_id in 0..4 {
            for seq in 0..10 {
                let cmd = NetCommand::keep_alive(player_id).with_sequence(seq);
                frame.add_command(cmd).unwrap();
            }
        }

        b.iter(|| {
            let mut frame_copy = frame.clone();
            frame_copy.mark_complete();
            test::black_box(frame_copy.checksum);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_checksum_calculation_8_players(b: &mut Bencher) {
        let mut frame = FrameData::new(1000);

        // Add commands from 8 players
        for player_id in 0..8 {
            for seq in 0..10 {
                let cmd = NetCommand::keep_alive(player_id).with_sequence(seq);
                frame.add_command(cmd).unwrap();
            }
        }

        b.iter(|| {
            let mut frame_copy = frame.clone();
            frame_copy.mark_complete();
            test::black_box(frame_copy.checksum);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_checksum_validation(b: &mut Bencher) {
        let mut frame = FrameData::new(1000);

        // Add commands from 4 players
        for player_id in 0..4 {
            for seq in 0..10 {
                let cmd = NetCommand::keep_alive(player_id).with_sequence(seq);
                frame.add_command(cmd).unwrap();
            }
        }
        frame.mark_complete();

        b.iter(|| {
            let valid = frame.validate_checksum();
            test::black_box(valid);
        });
    }

    // ============================================================================
    // CRC VALIDATION BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_frame_crc_computation_2_players(b: &mut Bencher) {
        let commands = vec![
            (0u8, &b"move_unit_123_to_456"[..]),
            (1u8, &b"attack_unit_789_with_321"[..]),
        ];

        b.iter(|| {
            let mut frame_crc = FrameCRC::new(1000);
            let crc = frame_crc.compute_command_crc(&commands);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_crc_computation_4_players(b: &mut Bencher) {
        let commands = vec![
            (0u8, &b"move_unit_123_to_456"[..]),
            (1u8, &b"attack_unit_789_with_321"[..]),
            (2u8, &b"build_structure_555"[..]),
            (3u8, &b"gather_resource_999"[..]),
        ];

        b.iter(|| {
            let mut frame_crc = FrameCRC::new(1000);
            let crc = frame_crc.compute_command_crc(&commands);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_crc_computation_8_players(b: &mut Bencher) {
        let commands = vec![
            (0u8, &b"move_unit_123_to_456"[..]),
            (1u8, &b"attack_unit_789_with_321"[..]),
            (2u8, &b"build_structure_555"[..]),
            (3u8, &b"gather_resource_999"[..]),
            (4u8, &b"upgrade_tech_111"[..]),
            (5u8, &b"sell_building_222"[..]),
            (6u8, &b"repair_unit_333"[..]),
            (7u8, &b"group_units_444"[..]),
        ];

        b.iter(|| {
            let mut frame_crc = FrameCRC::new(1000);
            let crc = frame_crc.compute_command_crc(&commands);
            test::black_box(crc);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_game_state_crc_10_units(b: &mut Bencher) {
        b.iter(|| {
            let mut crc = GameStateCRC::new();
            for i in 0..10 {
                crc.add_unit(i, (100.0 * i as f32, 200.0 * i as f32, 0.0), 100.0);
            }
            let result = crc.get();
            test::black_box(result);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_game_state_crc_50_units(b: &mut Bencher) {
        b.iter(|| {
            let mut crc = GameStateCRC::new();
            for i in 0..50 {
                crc.add_unit(i, (100.0 * i as f32, 200.0 * i as f32, 0.0), 100.0);
            }
            let result = crc.get();
            test::black_box(result);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_game_state_crc_100_units(b: &mut Bencher) {
        b.iter(|| {
            let mut crc = GameStateCRC::new();
            for i in 0..100 {
                crc.add_unit(i, (100.0 * i as f32, 200.0 * i as f32, 0.0), 100.0);
            }
            let result = crc.get();
            test::black_box(result);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_game_state_crc_500_units(b: &mut Bencher) {
        b.iter(|| {
            let mut crc = GameStateCRC::new();
            for i in 0..500 {
                crc.add_unit(i, (100.0 * i as f32, 200.0 * i as f32, 0.0), 100.0);
            }
            let result = crc.get();
            test::black_box(result);
        });
    }

    // ============================================================================
    // COMMAND EXECUTION BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_command_creation_keepalive(b: &mut Bencher) {
        b.iter(|| {
            let cmd = NetCommand::keep_alive(0);
            test::black_box(cmd);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_command_creation_chat(b: &mut Bencher) {
        b.iter(|| {
            let cmd = NetCommand::chat(0, "Hello, world!".to_string(), 0xFF);
            test::black_box(cmd);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_command_creation_game_command_move(b: &mut Bencher) {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("speed".to_string(), CommandParameter::Float(1.5));

            let game_data = GameCommandData {
                command_type: 1, // Move
                target_id: None,
                position: Some((100.0, 200.0, 0.0)),
                parameters: params,
                checksum: 0,
            };

            let cmd = NetCommand::game_command(0, 1000, game_data);
            test::black_box(cmd);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_command_creation_game_command_attack(b: &mut Bencher) {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("target".to_string(), CommandParameter::ObjectId(12345));

            let game_data = GameCommandData {
                command_type: 2, // Attack
                target_id: Some(12345),
                position: None,
                parameters: params,
                checksum: 0,
            };

            let cmd = NetCommand::game_command(0, 1000, game_data);
            test::black_box(cmd);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_command_creation_game_command_build(b: &mut Bencher) {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("building_type".to_string(), CommandParameter::Int(101));
            params.insert(
                "rally_point".to_string(),
                CommandParameter::Position(150.0, 250.0, 0.0),
            );

            let game_data = GameCommandData {
                command_type: 3, // Build
                target_id: None,
                position: Some((100.0, 200.0, 0.0)),
                parameters: params,
                checksum: 0,
            };

            let cmd = NetCommand::game_command(0, 1000, game_data);
            test::black_box(cmd);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_command_creation_game_command_gather(b: &mut Bencher) {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("resource_id".to_string(), CommandParameter::ObjectId(99999));

            let game_data = GameCommandData {
                command_type: 4, // Gather
                target_id: Some(99999),
                position: Some((75.0, 125.0, 0.0)),
                parameters: params,
                checksum: 0,
            };

            let cmd = NetCommand::game_command(0, 1000, game_data);
            test::black_box(cmd);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_command_validation(b: &mut Bencher) {
        let cmd = NetCommand::keep_alive(0);

        b.iter(|| {
            let result = cmd.validate();
            test::black_box(result);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_command_with_signature(b: &mut Bencher) {
        let signature = vec![0xABu8; 64]; // Simulated 64-byte signature

        b.iter(|| {
            let cmd = NetCommand::keep_alive(0).with_signature(signature.clone());
            test::black_box(cmd);
        });
    }

    // ============================================================================
    // NETWORK MESSAGE SERIALIZATION BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_serialize_keepalive(b: &mut Bencher) {
        let cmd = NetCommand::keep_alive(0);

        b.iter(|| {
            let bytes = bincode::serialize(&cmd).unwrap();
            test::black_box(bytes);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_deserialize_keepalive(b: &mut Bencher) {
        let cmd = NetCommand::keep_alive(0);
        let bytes = bincode::serialize(&cmd).unwrap();

        b.iter(|| {
            let deserialized: NetCommand = bincode::deserialize(&bytes).unwrap();
            test::black_box(deserialized);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_serialize_chat(b: &mut Bencher) {
        let cmd = NetCommand::chat(0, "Hello, world! This is a test message.".to_string(), 0xFF);

        b.iter(|| {
            let bytes = bincode::serialize(&cmd).unwrap();
            test::black_box(bytes);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_deserialize_chat(b: &mut Bencher) {
        let cmd = NetCommand::chat(0, "Hello, world! This is a test message.".to_string(), 0xFF);
        let bytes = bincode::serialize(&cmd).unwrap();

        b.iter(|| {
            let deserialized: NetCommand = bincode::deserialize(&bytes).unwrap();
            test::black_box(deserialized);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_serialize_game_command(b: &mut Bencher) {
        let mut params = HashMap::new();
        params.insert("target".to_string(), CommandParameter::ObjectId(12345));
        params.insert("speed".to_string(), CommandParameter::Float(1.5));

        let game_data = GameCommandData {
            command_type: 2, // Attack
            target_id: Some(12345),
            position: Some((100.0, 200.0, 0.0)),
            parameters: params,
            checksum: 0xDEADBEEF,
        };

        let cmd = NetCommand::game_command(0, 1000, game_data);

        b.iter(|| {
            let bytes = bincode::serialize(&cmd).unwrap();
            test::black_box(bytes);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_deserialize_game_command(b: &mut Bencher) {
        let mut params = HashMap::new();
        params.insert("target".to_string(), CommandParameter::ObjectId(12345));
        params.insert("speed".to_string(), CommandParameter::Float(1.5));

        let game_data = GameCommandData {
            command_type: 2, // Attack
            target_id: Some(12345),
            position: Some((100.0, 200.0, 0.0)),
            parameters: params,
            checksum: 0xDEADBEEF,
        };

        let cmd = NetCommand::game_command(0, 1000, game_data);
        let bytes = bincode::serialize(&cmd).unwrap();

        b.iter(|| {
            let deserialized: NetCommand = bincode::deserialize(&bytes).unwrap();
            test::black_box(deserialized);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_serialize_all_command_types(b: &mut Bencher) {
        let commands = vec![
            NetCommand::keep_alive(0),
            NetCommand::chat(0, "Test".to_string(), 0xFF),
            NetCommand::progress(0, game_network::commands::ProgressType::Loading, 50),
            NetCommand::load_complete(0),
            NetCommand::timeout_start(0),
        ];

        b.iter(|| {
            let mut all_bytes = Vec::new();
            for cmd in &commands {
                let bytes = bincode::serialize(cmd).unwrap();
                all_bytes.extend_from_slice(&bytes);
            }
            test::black_box(all_bytes);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_deserialize_all_command_types(b: &mut Bencher) {
        let commands = vec![
            NetCommand::keep_alive(0),
            NetCommand::chat(0, "Test".to_string(), 0xFF),
            NetCommand::progress(0, game_network::commands::ProgressType::Loading, 50),
            NetCommand::load_complete(0),
            NetCommand::timeout_start(0),
        ];

        let serialized: Vec<Vec<u8>> = commands
            .iter()
            .map(|cmd| bincode::serialize(cmd).unwrap())
            .collect();

        b.iter(|| {
            let mut deserialized = Vec::new();
            for bytes in &serialized {
                let cmd: NetCommand = bincode::deserialize(bytes).unwrap();
                deserialized.push(cmd);
            }
            test::black_box(deserialized);
        });
    }

    // ============================================================================
    // ENVELOPE ENCODING/DECODING BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_encode_plain_envelope(b: &mut Bencher) {
        let payload = vec![0x42u8; 256];

        b.iter(|| {
            let envelope = encode_plain_envelope(&payload);
            test::black_box(envelope);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_decode_plain_envelope(b: &mut Bencher) {
        let payload = vec![0x42u8; 256];
        let envelope = encode_plain_envelope(&payload);

        b.iter(|| {
            let decoded = decode_envelope(&envelope).unwrap();
            test::black_box(decoded);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_encode_encrypted_envelope(b: &mut Bencher) {
        let packet = EncryptedPacket {
            key_id: 12345,
            nonce: [0xAAu8; 12],
            payload: vec![0x42u8; 256],
        };

        b.iter(|| {
            let envelope = encode_encrypted_envelope(&packet);
            test::black_box(envelope);
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_decode_encrypted_envelope(b: &mut Bencher) {
        let packet = EncryptedPacket {
            key_id: 12345,
            nonce: [0xAAu8; 12],
            payload: vec![0x42u8; 256],
        };
        let envelope = encode_encrypted_envelope(&packet);

        b.iter(|| {
            let decoded = decode_envelope(&envelope).unwrap();
            test::black_box(decoded);
        });
    }

    // ============================================================================
    // COMPREHENSIVE INTEGRATION BENCHMARKS
    // ============================================================================

    #[cfg(test)]
    #[bench]
    fn bench_full_packet_pipeline_ack(b: &mut Bencher) {
        b.iter(|| {
            // Create packet
            let packet = NetPacket::ack(1234, 5678);

            // Serialize
            let bytes = packet.to_bytes().unwrap();

            // Calculate CRC
            let crc = CRC::compute_once(&bytes);

            // Parse back
            let parsed = NetPacket::from_bytes(&bytes).unwrap();

            // Validate
            assert!(parsed.header.validate_checksum());

            test::black_box((parsed, crc));
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_full_packet_pipeline_command(b: &mut Bencher) {
        let commands = vec![
            NetCommand::keep_alive(0),
            NetCommand::keep_alive(1),
            NetCommand::keep_alive(2),
            NetCommand::keep_alive(3),
        ];

        b.iter(|| {
            // Create packet
            let packet = NetPacket::command(1000, 999, commands.clone());

            // Serialize
            let bytes = packet.to_bytes().unwrap();

            // Calculate CRC
            let crc = CRC::compute_once(&bytes);

            // Parse back
            let parsed = NetPacket::from_bytes(&bytes).unwrap();

            // Validate
            assert!(parsed.header.validate_checksum());

            test::black_box((parsed, crc));
        });
    }

    #[cfg(test)]
    #[bench]
    fn bench_frame_processing_pipeline(b: &mut Bencher) {
        b.iter(|| {
            // Create frame
            let mut frame = FrameData::new(1000);

            // Add commands from 4 players
            for player_id in 0..4 {
                for seq in 0..5 {
                    let cmd = NetCommand::keep_alive(player_id).with_sequence(seq);
                    frame.add_command(cmd).unwrap();
                }
            }

            // Mark complete (calculates checksum)
            frame.mark_complete();

            // Get ordered commands
            let commands = frame.get_all_commands_ordered();

            // Validate checksum
            assert!(frame.validate_checksum());

            test::black_box((frame, commands));
        });
    }
} // mod benches
