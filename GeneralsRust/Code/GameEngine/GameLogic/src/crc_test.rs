//! CRC System Test - Verifies deterministic CRC calculation for multiplayer synchronization
//!
//! This test module ensures that the CRC system produces identical results for
//! identical game states and different results for different game states.
//! This is CRITICAL for multiplayer synchronization.

#[cfg(test)]
mod tests {
    use crate::helpers::{set_game_logic_random_seed, get_game_logic_random_seed_crc};
    use crc32fast::Hasher;
    
    /// Test CRC32 basic functionality - ensures CRC32 library works correctly
    #[test]
    fn test_crc32_basic() {
        let data = b"Hello, World!";
        let crc1 = crc32fast::hash(data);
        let crc2 = crc32fast::hash(data);
        
        assert_eq!(crc1, crc2, "Same data should produce same CRC");
        
        let different_data = b"Hello, Rust!";
        let crc3 = crc32fast::hash(different_data);
        
        assert_ne!(crc1, crc3, "Different data should produce different CRC");
        
        println!("✓ CRC32 basic test passed: Same: {:08X}, Different: {:08X}", crc1, crc3);
    }
    
    /// Test frame number CRC contribution
    #[test] 
    fn test_frame_number_crc() {
        let mut hasher1 = Hasher::new();
        hasher1.update(b"MARKER:GameFrame");
        hasher1.update(&0u32.to_le_bytes());
        let frame_0_crc = hasher1.finalize();
        
        let mut hasher2 = Hasher::new();
        hasher2.update(b"MARKER:GameFrame");
        hasher2.update(&1u32.to_le_bytes());
        let frame_1_crc = hasher2.finalize();
        
        assert_ne!(frame_0_crc, frame_1_crc, "Different frame numbers should produce different CRCs");
        
        println!("✓ Frame number CRC test passed: Frame 0: {:08X}, Frame 1: {:08X}", 
                 frame_0_crc, frame_1_crc);
    }
    
    /// Test position data CRC contribution
    #[test]
    fn test_position_crc() {
        let mut hasher1 = Hasher::new();
        hasher1.update(b"MARKER:Objects");
        hasher1.update(&100.0f32.to_le_bytes());  // x position
        hasher1.update(&200.0f32.to_le_bytes());  // y position  
        hasher1.update(&0.0f32.to_le_bytes());    // z position
        let pos1_crc = hasher1.finalize();
        
        let mut hasher2 = Hasher::new();
        hasher2.update(b"MARKER:Objects");
        hasher2.update(&100.1f32.to_le_bytes());  // Slightly different x position
        hasher2.update(&200.0f32.to_le_bytes());  // y position
        hasher2.update(&0.0f32.to_le_bytes());    // z position  
        let pos2_crc = hasher2.finalize();
        
        assert_ne!(pos1_crc, pos2_crc, "Different positions should produce different CRCs");
        
        println!("✓ Position CRC test passed: Pos1: {:08X}, Pos2: {:08X}", pos1_crc, pos2_crc);
    }
    
    /// Test random seed CRC function directly
    #[test]
    fn test_random_seed_crc_function() {
        // Test that same seed produces same CRC
        let seed = [0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210];
        
        set_game_logic_random_seed(seed);
        let crc1 = get_game_logic_random_seed_crc();
        
        set_game_logic_random_seed(seed);
        let crc2 = get_game_logic_random_seed_crc();
        
        assert_eq!(crc1, crc2, "Same random seed should produce same CRC");
        
        // Test that different seed produces different CRC
        let different_seed = [0x87654321, 0x0FEDCBA9, 0xFDB97531, 0x0ECA8642, 0x89ABCDEF, 0x01234567];
        set_game_logic_random_seed(different_seed);
        let crc3 = get_game_logic_random_seed_crc();
        
        assert_ne!(crc1, crc3, "Different random seed should produce different CRC");
        
        println!("✓ Random seed CRC function test passed: Same: {:08X}, Different: {:08X}", 
                 crc1, crc3);
    }
}