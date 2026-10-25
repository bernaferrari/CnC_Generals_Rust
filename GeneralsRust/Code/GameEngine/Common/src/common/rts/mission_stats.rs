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
            if total_killed > 0 { f32::INFINITY } else { 0.0 }
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

impl crate::common::system::snapshot::Snapshotable for MissionStats {
    /// C++ MissionStats::crc() is intentionally empty.
    fn crc(&self, _xfer: &mut dyn crate::common::system::xfer::Xfer) -> Result<(), String> {
        Ok(())
    }

    /// C++ MissionStats::xfer() (`MissionStats.cpp:53-74`).
    fn xfer(&mut self, xfer: &mut dyn crate::common::system::xfer::Xfer) -> Result<(), String> {
        use crate::common::system::xfer::XferVersion;

        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| err.to_string())?;

        for value in &mut self.units_killed {
            xfer.xfer_int(value).map_err(|err| err.to_string())?;
        }
        xfer.xfer_int(&mut self.units_lost)
            .map_err(|err| err.to_string())?;
        for value in &mut self.buildings_killed {
            xfer.xfer_int(value).map_err(|err| err.to_string())?;
        }
        xfer.xfer_int(&mut self.buildings_lost)
            .map_err(|err| err.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
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
    fn test_snapshot_xfer_uses_cpp_wire_layout() {
        use crate::common::system::snapshot::Snapshotable;
        use crate::common::system::xfer::{Xfer, XferMode};
        use crate::common::system::xfer_load::XferLoad;
        use crate::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut stats = MissionStats::new();
        stats.units_killed[0] = 0x0102_0304;
        stats.units_lost = -7;
        stats.buildings_killed[MAX_PLAYER_COUNT - 1] = 0x1122_3344;
        stats.buildings_lost = 9;

        let mut encoded = Vec::new();
        {
            let mut xfer = XferSave::new(Cursor::new(&mut encoded), 1);
            stats.xfer(&mut xfer).unwrap();
            assert_eq!(xfer.get_xfer_mode(), XferMode::Save);
        }
        assert_eq!(encoded.len(), 1 + (2 * MAX_PLAYER_COUNT + 2) * 4);
        assert_eq!(encoded[0], 1); // C++ XferVersion is one byte.
        assert_eq!(&encoded[1..5], &0x0102_0304i32.to_le_bytes());
        assert_eq!(
            &encoded[1 + MAX_PLAYER_COUNT * 4..][..4],
            &(-7i32).to_le_bytes()
        );
        let buildings_offset = 1 + (MAX_PLAYER_COUNT + 1) * 4;
        assert_eq!(
            &encoded[buildings_offset + (MAX_PLAYER_COUNT - 1) * 4..][..4],
            &0x1122_3344i32.to_le_bytes()
        );
        assert_eq!(&encoded[encoded.len() - 4..], &9i32.to_le_bytes());

        let mut loaded = MissionStats::new();
        {
            let mut xfer = XferLoad::new(Cursor::new(&encoded), 1);
            loaded.xfer(&mut xfer).unwrap();
            assert_eq!(xfer.get_xfer_mode(), XferMode::Load);
        }
        assert_eq!(loaded.get_units_killed(0), 0x0102_0304);
        assert_eq!(loaded.get_units_lost(), -7);
        assert_eq!(
            loaded.get_buildings_killed(MAX_PLAYER_COUNT - 1),
            0x1122_3344
        );
        assert_eq!(loaded.get_buildings_lost(), 9);
    }

    #[test]
    fn test_snapshot_crc_is_empty() {
        use crate::common::system::snapshot::Snapshotable;
        use crate::common::system::xfer_crc::XferCRC;
        use crate::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut stats = MissionStats::new();
        stats.record_unit_kill(0);
        stats.record_unit_loss();
        let mut xfer = XferCRC::new(XferSave::new(Cursor::new(Vec::new()), 1));
        stats.crc(&mut xfer).unwrap();
        assert_eq!(xfer.get_crc(), 0);
    }
}
