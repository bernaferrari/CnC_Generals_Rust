//! Mission statistics tracking system
//!
//! This module tracks various statistics during a mission such as
//! units killed, buildings destroyed, units lost, etc.

/// Maximum number of players that can be tracked
pub use crate::common::game_common::MAX_PLAYER_COUNT;

/// Mission statistics tracker
///
/// Tracks combat statistics for a player during a mission,
/// including kills and losses by category.
#[derive(Debug)]
pub struct MissionStats {
    /// Number of units killed by each player (indexed by player ID)
    units_killed: [i32; MAX_PLAYER_COUNT],
    /// Number of buildings killed by each player (indexed by player ID)
    buildings_killed: [i32; MAX_PLAYER_COUNT],
    /// Number of units this player has lost
    units_lost: i32,
    /// Number of buildings this player has lost
    buildings_lost: i32,
}

impl MissionStats {
    /// Create a new MissionStats instance
    pub fn new() -> Self {
        let mut stats = Self {
            units_killed: [0; MAX_PLAYER_COUNT],
            buildings_killed: [0; MAX_PLAYER_COUNT],
            units_lost: 0,
            buildings_lost: 0,
        };
        stats.init();
        stats
    }

    /// Initialize/reset all statistics to zero
    pub fn init(&mut self) {
        for i in 0..MAX_PLAYER_COUNT {
            self.units_killed[i] = 0;
            self.buildings_killed[i] = 0;
        }
        self.units_lost = 0;
        self.buildings_lost = 0;
    }

    /// Get the number of units killed by a specific player
    pub fn get_units_killed(&self, player_id: usize) -> i32 {
        if player_id < MAX_PLAYER_COUNT {
            self.units_killed[player_id]
        } else {
            0
        }
    }

    /// Get the number of buildings killed by a specific player
    pub fn get_buildings_killed(&self, player_id: usize) -> i32 {
        if player_id < MAX_PLAYER_COUNT {
            self.buildings_killed[player_id]
        } else {
            0
        }
    }

    /// Get the total number of units killed by all players
    pub fn get_total_units_killed(&self) -> i32 {
        self.units_killed.iter().sum()
    }

    /// Get the total number of buildings killed by all players
    pub fn get_total_buildings_killed(&self) -> i32 {
        self.buildings_killed.iter().sum()
    }

    /// Get the number of units this player has lost
    pub fn get_units_lost(&self) -> i32 {
        self.units_lost
    }

    /// Get the number of buildings this player has lost
    pub fn get_buildings_lost(&self) -> i32 {
        self.buildings_lost
    }

    /// Record that this player killed a unit belonging to another player
    pub fn record_unit_kill(&mut self, killed_player_id: usize) {
        if killed_player_id < MAX_PLAYER_COUNT {
            self.units_killed[killed_player_id] += 1;
        }
    }

    /// Record that this player killed a building belonging to another player
    pub fn record_building_kill(&mut self, killed_player_id: usize) {
        if killed_player_id < MAX_PLAYER_COUNT {
            self.buildings_killed[killed_player_id] += 1;
        }
    }

    /// Record that this player lost a unit
    pub fn record_unit_loss(&mut self) {
        self.units_lost += 1;
    }

    /// Record that this player lost a building
    pub fn record_building_loss(&mut self) {
        self.buildings_lost += 1;
    }

    /// Get kill/loss ratio for units
    pub fn get_unit_kill_loss_ratio(&self) -> f32 {
        if self.units_lost == 0 {
            if self.get_total_units_killed() > 0 {
                f32::INFINITY
            } else {
                0.0
            }
        } else {
            self.get_total_units_killed() as f32 / self.units_lost as f32
        }
    }

    /// Get kill/loss ratio for buildings
    pub fn get_building_kill_loss_ratio(&self) -> f32 {
        if self.buildings_lost == 0 {
            if self.get_total_buildings_killed() > 0 {
                f32::INFINITY
            } else {
                0.0
            }
        } else {
            self.get_total_buildings_killed() as f32 / self.buildings_lost as f32
        }
    }

    /// Get overall kill/loss ratio (units + buildings)
    pub fn get_overall_kill_loss_ratio(&self) -> f32 {
        let total_killed = self.get_total_units_killed() + self.get_total_buildings_killed();
        let total_lost = self.units_lost + self.buildings_lost;

        if total_lost == 0 {
            if total_killed > 0 {
                f32::INFINITY
            } else {
                0.0
            }
        } else {
            total_killed as f32 / total_lost as f32
        }
    }

    /// Get all unit kill statistics as a slice
    pub fn get_units_killed_stats(&self) -> &[i32; MAX_PLAYER_COUNT] {
        &self.units_killed
    }

    /// Get all building kill statistics as a slice
    pub fn get_buildings_killed_stats(&self) -> &[i32; MAX_PLAYER_COUNT] {
        &self.buildings_killed
    }

    /// Reset statistics for a specific opponent
    pub fn reset_opponent_stats(&mut self, player_id: usize) {
        if player_id < MAX_PLAYER_COUNT {
            self.units_killed[player_id] = 0;
            self.buildings_killed[player_id] = 0;
        }
    }

    /// Add stats from another MissionStats (for combining/aggregating)
    pub fn add_stats(&mut self, other: &MissionStats) {
        for i in 0..MAX_PLAYER_COUNT {
            self.units_killed[i] += other.units_killed[i];
            self.buildings_killed[i] += other.buildings_killed[i];
        }
        self.units_lost += other.units_lost;
        self.buildings_lost += other.buildings_lost;
    }
}

impl Default for MissionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MissionStats {
    fn clone(&self) -> Self {
        Self {
            units_killed: self.units_killed,
            buildings_killed: self.buildings_killed,
            units_lost: self.units_lost,
            buildings_lost: self.buildings_lost,
        }
    }
}

impl MissionStats {
    /// Serialize/deserialize for save games
    /// Matches C++ MissionStats.cpp xfer() implementation
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Version (matches C++ version = 1)
        data.extend_from_slice(&1u32.to_le_bytes());

        // Units killed array
        for i in 0..MAX_PLAYER_COUNT {
            data.extend_from_slice(&self.units_killed[i].to_le_bytes());
        }

        // Units lost
        data.extend_from_slice(&self.units_lost.to_le_bytes());

        // Buildings killed array
        for i in 0..MAX_PLAYER_COUNT {
            data.extend_from_slice(&self.buildings_killed[i].to_le_bytes());
        }

        // Buildings lost
        data.extend_from_slice(&self.buildings_lost.to_le_bytes());

        data
    }

    /// Deserialize from save game data
    /// Matches C++ MissionStats.cpp xfer() implementation
    pub fn deserialize(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 4 {
            return Err("Invalid data length");
        }

        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if version != 1 {
            return Err("Unsupported version");
        }

        let mut offset = 4;
        let mut stats = Self::new();

        // Units killed array
        for i in 0..MAX_PLAYER_COUNT {
            if offset + 4 > data.len() {
                return Err("Truncated data");
            }
            stats.units_killed[i] = i32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        // Units lost
        if offset + 4 > data.len() {
            return Err("Truncated data");
        }
        stats.units_lost = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Buildings killed array
        for i in 0..MAX_PLAYER_COUNT {
            if offset + 4 > data.len() {
                return Err("Truncated data");
            }
            stats.buildings_killed[i] = i32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        // Buildings lost
        if offset + 4 > data.len() {
            return Err("Truncated data");
        }
        stats.buildings_lost = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);

        Ok(stats)
    }

    /// Calculate CRC for network synchronization
    /// Matches C++ MissionStats.cpp crc() pattern
    pub fn calculate_crc(&self) -> u32 {
        let mut crc = 0u32;

        // CRC all unit kills
        for &count in &self.units_killed {
            crc = crc.wrapping_add(count as u32);
        }

        // CRC own losses
        crc = crc.wrapping_add(self.units_lost as u32);

        // CRC all building kills
        for &count in &self.buildings_killed {
            crc = crc.wrapping_add(count as u32);
        }

        // CRC own building losses
        crc = crc.wrapping_add(self.buildings_lost as u32);

        crc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mission_stats_init() {
        let mut stats = MissionStats::new();

        // All stats should start at 0
        assert_eq!(stats.get_total_units_killed(), 0);
        assert_eq!(stats.get_total_buildings_killed(), 0);
        assert_eq!(stats.get_units_lost(), 0);
        assert_eq!(stats.get_buildings_lost(), 0);

        for i in 0..MAX_PLAYER_COUNT {
            assert_eq!(stats.get_units_killed(i), 0);
            assert_eq!(stats.get_buildings_killed(i), 0);
        }
    }

    #[test]
    fn test_mission_stats_recording() {
        let mut stats = MissionStats::new();

        // Record some kills and losses
        stats.record_unit_kill(0);
        stats.record_unit_kill(1);
        stats.record_building_kill(0);
        stats.record_unit_loss();
        stats.record_building_loss();

        assert_eq!(stats.get_units_killed(0), 1);
        assert_eq!(stats.get_units_killed(1), 1);
        assert_eq!(stats.get_buildings_killed(0), 1);
        assert_eq!(stats.get_total_units_killed(), 2);
        assert_eq!(stats.get_total_buildings_killed(), 1);
        assert_eq!(stats.get_units_lost(), 1);
        assert_eq!(stats.get_buildings_lost(), 1);
    }

    #[test]
    fn test_kill_loss_ratios() {
        let mut stats = MissionStats::new();

        // Test infinite ratio (kills but no losses)
        stats.record_unit_kill(0);
        assert!(stats.get_unit_kill_loss_ratio().is_infinite());

        // Test normal ratio
        stats.record_unit_loss();
        assert_eq!(stats.get_unit_kill_loss_ratio(), 1.0);

        // Test zero ratio (no kills)
        let stats2 = MissionStats::new();
        assert_eq!(stats2.get_unit_kill_loss_ratio(), 0.0);
    }

    #[test]
    fn test_add_stats() {
        let mut stats1 = MissionStats::new();
        let mut stats2 = MissionStats::new();

        stats1.record_unit_kill(0);
        stats1.record_unit_loss();

        stats2.record_building_kill(1);
        stats2.record_building_loss();

        stats1.add_stats(&stats2);

        assert_eq!(stats1.get_units_killed(0), 1);
        assert_eq!(stats1.get_buildings_killed(1), 1);
        assert_eq!(stats1.get_units_lost(), 1);
        assert_eq!(stats1.get_buildings_lost(), 1);
    }

    #[test]
    fn test_bounds_checking() {
        let mut stats = MissionStats::new();

        // Test out of bounds access
        assert_eq!(stats.get_units_killed(MAX_PLAYER_COUNT), 0);
        assert_eq!(stats.get_buildings_killed(MAX_PLAYER_COUNT), 0);

        // Recording out of bounds should not crash
        stats.record_unit_kill(MAX_PLAYER_COUNT);
        stats.record_building_kill(MAX_PLAYER_COUNT);

        assert_eq!(stats.get_total_units_killed(), 0);
        assert_eq!(stats.get_total_buildings_killed(), 0);
    }

    #[test]
    fn test_serialization() {
        let mut stats = MissionStats::new();

        // Set some values
        stats.record_unit_kill(0);
        stats.record_unit_kill(1);
        stats.record_building_kill(0);
        stats.record_unit_loss();
        stats.record_building_loss();

        // Serialize
        let data = stats.serialize();

        // Deserialize
        let deserialized = MissionStats::deserialize(&data).unwrap();

        // Verify all values match
        assert_eq!(deserialized.get_units_killed(0), stats.get_units_killed(0));
        assert_eq!(deserialized.get_units_killed(1), stats.get_units_killed(1));
        assert_eq!(
            deserialized.get_buildings_killed(0),
            stats.get_buildings_killed(0)
        );
        assert_eq!(deserialized.get_units_lost(), stats.get_units_lost());
        assert_eq!(
            deserialized.get_buildings_lost(),
            stats.get_buildings_lost()
        );
    }

    #[test]
    fn test_crc_calculation() {
        let mut stats1 = MissionStats::new();
        let mut stats2 = MissionStats::new();

        // Identical stats should have identical CRC
        assert_eq!(stats1.calculate_crc(), stats2.calculate_crc());

        // Different stats should (very likely) have different CRC
        stats1.record_unit_kill(0);
        assert_ne!(stats1.calculate_crc(), stats2.calculate_crc());

        // Make stats2 identical
        stats2.record_unit_kill(0);
        assert_eq!(stats1.calculate_crc(), stats2.calculate_crc());
    }
}
