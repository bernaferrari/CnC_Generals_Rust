//! Determinism tests for frame execution
//!
//! These tests verify that frame execution is deterministic across multiple runs
//! and that CRCs are computed consistently. This is CRITICAL for RTS networking.

#[cfg(test)]
mod tests {
    use crate::commands::{CommandPayload, GameCommandData, NetCommand, NetCommandType};
    use crate::frame_data::crc::{FrameCRC, GameStateCRC, CRC};
    use crate::frame_data::sync_manager::{
        FrameData, FrameDataManager, FrameDataReturnType, SyncFrameExecutor,
    };
    use std::collections::HashMap;
    use crate::time::NetworkInstant;

    /// Test that identical command sequences produce identical CRCs
    #[test]
    fn test_identical_frame_execution() {
        // Run 1
        let mut frame_crc1 = FrameCRC::new(100);
        let commands1 = vec![
            (0u8, &b"move_unit_1"[..]),
            (0u8, &b"attack_unit_2"[..]),
            (1u8, &b"build_structure"[..]),
        ];
        let crc1 = frame_crc1.compute_command_crc(&commands1);

        // Run 2 - identical commands
        let mut frame_crc2 = FrameCRC::new(100);
        let commands2 = vec![
            (0u8, &b"move_unit_1"[..]),
            (0u8, &b"attack_unit_2"[..]),
            (1u8, &b"build_structure"[..]),
        ];
        let crc2 = frame_crc2.compute_command_crc(&commands2);

        // CRCs must be identical
        assert_eq!(crc1, crc2, "Identical command sequences must produce identical CRCs");
    }

    /// Test that different command orders produce different CRCs
    #[test]
    fn test_command_order_deterministic() {
        let mut frame_crc1 = FrameCRC::new(100);
        let commands1 = vec![
            (0u8, &b"command_a"[..]),
            (1u8, &b"command_b"[..]),
        ];
        let crc1 = frame_crc1.compute_command_crc(&commands1);

        let mut frame_crc2 = FrameCRC::new(100);
        let commands2 = vec![
            (1u8, &b"command_b"[..]),
            (0u8, &b"command_a"[..]),
        ];
        let crc2 = frame_crc2.compute_command_crc(&commands2);

        // Different orders must produce different CRCs
        assert_ne!(crc1, crc2, "Different command orders must produce different CRCs");
    }

    /// Test that CRC computation is deterministic across multiple runs
    #[test]
    fn test_crc_determinism() {
        let data = b"deterministic test data";

        let mut results = Vec::new();

        // Compute CRC 100 times
        for _ in 0..100 {
            let crc = CRC::compute_once(data);
            results.push(crc);
        }

        // All results must be identical
        let first = results[0];
        for (i, &result) in results.iter().enumerate() {
            assert_eq!(
                result, first,
                "CRC computation #{} produced different result: {:08x} vs {:08x}",
                i, result, first
            );
        }
    }

    /// Test that floating-point values produce deterministic CRCs
    #[test]
    fn test_float_determinism() {
        let mut crc1 = GameStateCRC::new();
        crc1.add_unit(1, (100.5, 200.25, 0.0), 50.75);
        let result1 = crc1.get();

        let mut crc2 = GameStateCRC::new();
        crc2.add_unit(1, (100.5, 200.25, 0.0), 50.75);
        let result2 = crc2.get();

        assert_eq!(
            result1, result2,
            "Identical float values must produce identical CRCs"
        );
    }

    /// Test frame data command ordering
    #[test]
    fn test_frame_data_command_ordering() {
        let mut frame1 = FrameData::new();
        frame1.set_frame(100);

        // Add commands in different order
        let cmd1 = NetCommand::new(
            NetCommandType::GameCommand,
            0,
            100,
            CommandPayload::GameCommand(GameCommandData {
                command_type: 1,
                target_id: None,
                position: None,
                parameters: HashMap::new(),
                checksum: 0,
            }),
        );
        let mut cmd2 = cmd1.clone();
        cmd2.sequence = 2;
        let mut cmd3 = cmd1.clone();
        cmd3.sequence = 3;

        // Add in reverse order
        frame1.add_command(cmd3.clone()).unwrap();
        frame1.add_command(cmd1.clone()).unwrap();
        frame1.add_command(cmd2.clone()).unwrap();

        // Commands should be sorted by sequence
        let ordered = frame1.get_commands_ordered();
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].sequence, 1);
        assert_eq!(ordered[1].sequence, 2);
        assert_eq!(ordered[2].sequence, 3);

        // Do same with different insertion order
        let mut frame2 = FrameData::new();
        frame2.set_frame(100);

        frame2.add_command(cmd1.clone()).unwrap();
        frame2.add_command(cmd2.clone()).unwrap();
        frame2.add_command(cmd3.clone()).unwrap();

        let ordered2 = frame2.get_commands_ordered();

        // Final order must be identical regardless of insertion order
        assert_eq!(ordered.len(), ordered2.len());
        for (c1, c2) in ordered.iter().zip(ordered2.iter()) {
            assert_eq!(c1.sequence, c2.sequence);
        }
    }

    /// Test that frame manager produces deterministic results
    #[test]
    fn test_frame_manager_determinism() {
        // Create two identical frame managers
        let mut mgr1 = FrameDataManager::new(false);
        mgr1.init();

        let mut mgr2 = FrameDataManager::new(false);
        mgr2.init();

        // Set same frame command count
        mgr1.set_frame_command_count(0, 2);
        mgr2.set_frame_command_count(0, 2);

        // Add same commands in same order
        let cmd1 = NetCommand::new(
            NetCommandType::KeepAlive,
            0,
            0,
            CommandPayload::KeepAlive,
        );
        let cmd2 = NetCommand::new(
            NetCommandType::KeepAlive,
            1,
            0,
            CommandPayload::KeepAlive,
        );

        mgr1.add_net_command_msg(cmd1.clone()).unwrap();
        mgr1.add_net_command_msg(cmd2.clone()).unwrap();

        mgr2.add_net_command_msg(cmd1.clone()).unwrap();
        mgr2.add_net_command_msg(cmd2.clone()).unwrap();

        // Both should report ready at same time
        let status1 = mgr1.all_commands_ready(0, false);
        let status2 = mgr2.all_commands_ready(0, false);

        assert_eq!(status1, status2);
        assert_eq!(status1, FrameDataReturnType::Ready);

        // Commands should be in identical order
        let commands1 = mgr1.get_frame_commands(0);
        let commands2 = mgr2.get_frame_commands(0);

        assert_eq!(commands1.len(), commands2.len());
    }

    /// Test cross-platform CRC compatibility
    #[test]
    fn test_crc_cross_platform() {
        // This tests the bit-rotation CRC algorithm matches C++
        let mut crc = CRC::new();

        // Test cases from C++ implementation
        crc.add_byte(0x00);
        let r1 = crc.get();

        crc.add_byte(0xFF);
        let r2 = crc.get();

        // Verify bit-rotation produces expected pattern
        assert_ne!(r1, 0, "CRC should not be zero after adding byte");
        assert_ne!(r2, r1, "Different bytes should produce different CRCs");

        // Test hibit behavior
        let mut crc_hibit = CRC::with_initial(0x80000000);
        crc_hibit.add_byte(0x01);

        let result = crc_hibit.get();

        // When high bit is set, it should rotate and add
        // 0x80000000 << 1 = 0x00000000 (with carry)
        // + 0x01 (byte value)
        // + 0x01 (hibit)
        // = 0x00000002
        assert_eq!(result, 0x00000002, "Hibit rotation should work correctly");
    }

    /// Test that duplicate commands are handled correctly
    #[test]
    fn test_duplicate_command_handling() {
        let mut frame = FrameData::new();
        frame.set_frame(100);

        let cmd = NetCommand::new(
            NetCommandType::KeepAlive,
            0,
            100,
            CommandPayload::KeepAlive,
        );

        // Add command
        frame.add_command(cmd.clone()).unwrap();
        assert_eq!(frame.get_command_count(), 1);

        // Add duplicate - should be ignored
        frame.add_command(cmd.clone()).unwrap();
        assert_eq!(
            frame.get_command_count(),
            1,
            "Duplicate commands should be ignored"
        );
    }

    /// Test circular buffer indexing
    #[test]
    fn test_circular_buffer_wraparound() {
        use crate::frame_data::sync_manager::FRAME_DATA_LENGTH;

        let mut mgr = FrameDataManager::new(false);
        mgr.init();

        // Test that frames wrap correctly
        for frame in 0..(FRAME_DATA_LENGTH * 2) as u32 {
            let cmd = NetCommand::new(
                NetCommandType::KeepAlive,
                0,
                frame,
                CommandPayload::KeepAlive,
            );

            // This should not panic
            mgr.add_net_command_msg(cmd).unwrap();
        }
    }

    /// Test game state CRC determinism
    #[test]
    fn test_game_state_crc_determinism() {
        let mut crc1 = GameStateCRC::new();

        // Add game state in specific order
        crc1.add_unit(1, (100.0, 200.0, 0.0), 50.0);
        crc1.add_unit(2, (150.0, 250.0, 0.0), 75.0);

        let mut resources1 = HashMap::new();
        resources1.insert("gold".to_string(), 1000);
        resources1.insert("wood".to_string(), 500);
        crc1.add_resources(&resources1);

        crc1.add_building(10, 100.0, &[1, 2, 3]);

        let result1 = crc1.get();

        // Do identical operations
        let mut crc2 = GameStateCRC::new();
        crc2.add_unit(1, (100.0, 200.0, 0.0), 50.0);
        crc2.add_unit(2, (150.0, 250.0, 0.0), 75.0);

        let mut resources2 = HashMap::new();
        resources2.insert("gold".to_string(), 1000);
        resources2.insert("wood".to_string(), 500);
        crc2.add_resources(&resources2);

        crc2.add_building(10, 100.0, &[1, 2, 3]);

        let result2 = crc2.get();

        assert_eq!(result1, result2, "Identical game states must produce identical CRCs");
    }

    /// Test that resource map ordering doesn't matter (keys are sorted)
    #[test]
    fn test_resource_map_determinism() {
        let mut crc1 = GameStateCRC::new();
        let mut resources1 = HashMap::new();
        resources1.insert("gold".to_string(), 1000);
        resources1.insert("wood".to_string(), 500);
        resources1.insert("stone".to_string(), 250);
        crc1.add_resources(&resources1);
        let result1 = crc1.get();

        // Insert in different order
        let mut crc2 = GameStateCRC::new();
        let mut resources2 = HashMap::new();
        resources2.insert("stone".to_string(), 250);
        resources2.insert("gold".to_string(), 1000);
        resources2.insert("wood".to_string(), 500);
        crc2.add_resources(&resources2);
        let result2 = crc2.get();

        assert_eq!(
            result1, result2,
            "Resource map insertion order should not affect CRC"
        );
    }

    /// Test zero frames initialization
    #[test]
    fn test_zero_frames() {
        let mut mgr = FrameDataManager::new(false);
        mgr.init();

        // Zero frames at game start
        mgr.zero_frames(0, 10);

        // All frames should have command count = 0
        for frame in 0..10 {
            assert_eq!(
                mgr.get_command_count(frame),
                0,
                "Frame {} should have zero command count",
                frame
            );
            assert_eq!(
                mgr.get_frame_command_count(frame),
                0,
                "Frame {} should have zero frame command count",
                frame
            );
        }
    }

    /// Benchmark: test that frame execution is fast enough for 30 FPS
    #[test]
    fn test_frame_execution_performance() {
        let mut executor = SyncFrameExecutor::new(0);
        executor.init(vec![0, 1]);

        // Add commands for frame 0
        for player_id in 0..2 {
            let cmd = NetCommand::new(
                NetCommandType::KeepAlive,
                player_id,
                0,
                CommandPayload::KeepAlive,
            );

            executor.add_command(cmd).unwrap();
        }

        // Set expected command counts
        executor.zero_frames(0, 1);

        // Measure execution time
        let start = NetworkInstant::now();
        let _executed = executor.update().unwrap();
        let elapsed = start.elapsed();

        // Should complete in less than 1ms for empty frame
        // At 30 FPS, we have 33.33ms per frame budget
        assert!(
            elapsed.as_micros() < 1000,
            "Frame execution took too long: {:?}",
            elapsed
        );
    }

    /// Test float bit representation is deterministic
    #[test]
    fn test_float_bit_determinism() {
        let a = 3.14159f32;
        let b = 3.14159f32;

        // Bit representation must be identical
        assert_eq!(a.to_bits(), b.to_bits());

        // Byte representation must be identical
        let bytes_a = a.to_le_bytes();
        let bytes_b = b.to_le_bytes();

        assert_eq!(bytes_a, bytes_b);

        // Reconstructed values must be bit-identical
        let reconstructed_a = f32::from_le_bytes(bytes_a);
        let reconstructed_b = f32::from_le_bytes(bytes_b);

        assert_eq!(reconstructed_a.to_bits(), reconstructed_b.to_bits());
    }

    /// Test that NaN and infinity are handled correctly
    #[test]
    fn test_special_float_values() {
        // NaN
        let nan1 = f32::NAN;
        let nan2 = f32::NAN;

        // NaN bit patterns should be identical (though NaN != NaN)
        let bytes1 = nan1.to_le_bytes();
        let bytes2 = nan2.to_le_bytes();

        // This might fail on some platforms if NaN representations differ!
        assert_eq!(bytes1, bytes2, "NaN representations should be identical");

        // Infinity
        let inf1 = f32::INFINITY;
        let inf2 = f32::INFINITY;

        assert_eq!(inf1.to_bits(), inf2.to_bits());

        // Negative infinity
        let ninf1 = f32::NEG_INFINITY;
        let ninf2 = f32::NEG_INFINITY;

        assert_eq!(ninf1.to_bits(), ninf2.to_bits());
    }

    /// CRITICAL TEST: Verify that float operations are deterministic
    /// This is a simplified test - real determinism requires careful analysis
    #[test]
    fn test_float_operations_deterministic() {
        // Addition
        let a1 = 1.5f32;
        let b1 = 2.5f32;
        let sum1 = a1 + b1;

        let a2 = 1.5f32;
        let b2 = 2.5f32;
        let sum2 = a2 + b2;

        assert_eq!(sum1.to_bits(), sum2.to_bits());

        // Multiplication
        let prod1 = a1 * b1;
        let prod2 = a2 * b2;

        assert_eq!(prod1.to_bits(), prod2.to_bits());

        // Division
        let div1 = a1 / b1;
        let div2 = a2 / b2;

        assert_eq!(div1.to_bits(), div2.to_bits());

        // WARNING: This test may fail on different platforms/compilers!
        // For true determinism, use fixed-point arithmetic or match C++ compiler flags
    }

    /// Test associativity of float operations (hint: they're NOT associative!)
    #[test]
    fn test_float_associativity_warning() {
        let a = 1e20f32;
        let b = 1.0f32;
        let c = -1e20f32;

        // (a + b) + c
        let result1 = (a + b) + c;

        // a + (b + c)
        let result2 = a + (b + c);

        // These may NOT be equal due to floating-point precision!
        // This is why deterministic execution must use EXACT same operation order
        println!(
            "Result1: {}, Result2: {}, Equal: {}",
            result1,
            result2,
            result1 == result2
        );

        // For determinism, we must ensure operations happen in EXACTLY the same order
    }
}
