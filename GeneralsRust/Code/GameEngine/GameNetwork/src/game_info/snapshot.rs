// FILE: game_info/snapshot.rs
// Port of SkirmishGameInfo snapshot/xfer functionality
// Matches C++ SkirmishGameInfo::xfer and SkirmishGameInfo::crc

use super::*;
use serde::{Deserialize, Serialize};

/// Snapshot version for SkirmishGameInfo (matches C++ version 4)
pub const SKIRMISH_GAME_INFO_VERSION: u32 = 4;

/// SkirmishGameInfo - game info with snapshot support for save/load
/// Matches C++ SkirmishGameInfo class
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkirmishGameInfo {
    // Note: Can't use #[serde(flatten)] with bincode as it creates dynamic maps
    game_info: GameInfo,
}

impl SkirmishGameInfo {
    /// Create a new SkirmishGameInfo
    pub fn new() -> Self {
        Self {
            game_info: GameInfo::new(),
        }
    }

    /// Get mutable reference to underlying GameInfo
    pub fn game_info_mut(&mut self) -> &mut GameInfo {
        &mut self.game_info
    }

    /// Get immutable reference to underlying GameInfo
    pub fn game_info(&self) -> &GameInfo {
        &self.game_info
    }

    /// Serialize to bytes (matches C++ xfer in save mode)
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("Serialization error: {}", e))
    }

    /// Deserialize from bytes (matches C++ xfer in load mode)
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        bincode::deserialize(data).map_err(|e| format!("Deserialization error: {}", e))
    }

    /// Calculate CRC for snapshot (matches C++ crc method - currently empty)
    pub fn calculate_crc(&self) -> u32 {
        // C++ implementation is empty, so we return 0
        // This may be filled in later for save game validation
        0
    }

    /// Load post-process (matches C++ loadPostProcess - currently empty)
    pub fn load_post_process(&mut self) {
        // C++ implementation is empty
        // This may be filled in later for post-load fixups
    }
}

impl Default for SkirmishGameInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable snapshot of GameSlot for save/load
/// Matches C++ GameSlot xfer structure exactly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSlotSnapshot {
    state: u8, // SlotState as u8
    name: String,
    is_accepted: bool,
    is_muted: bool,
    color: i32,
    start_pos: i32,
    player_template: i32,
    team_number: i32,
    orig_color: i32,
    orig_start_pos: i32,
    orig_player_template: i32,
}

impl From<&GameSlot> for GameSlotSnapshot {
    fn from(slot: &GameSlot) -> Self {
        Self {
            state: slot.get_state() as u8,
            name: slot.get_name().to_string(),
            is_accepted: slot.is_accepted(),
            is_muted: slot.is_muted(),
            color: slot.get_color(),
            start_pos: slot.get_start_pos(),
            player_template: slot.get_player_template(),
            team_number: slot.get_team_number(),
            orig_color: slot.get_original_color(),
            orig_start_pos: slot.get_original_start_pos(),
            orig_player_template: slot.get_original_player_template(),
        }
    }
}

impl GameSlotSnapshot {
    /// Convert snapshot back to GameSlot
    pub fn to_game_slot(&self) -> GameSlot {
        let mut slot = GameSlot::new();

        let state = match self.state {
            0 => SlotState::Open,
            1 => SlotState::Closed,
            2 => SlotState::EasyAI,
            3 => SlotState::MedAI,
            4 => SlotState::BrutalAI,
            5 => SlotState::Player,
            _ => SlotState::Closed,
        };

        slot.set_state(state, self.name.clone(), 0);

        if self.is_accepted {
            slot.set_accept();
        }

        slot.mute(self.is_muted);

        // Set original info first
        slot.set_player_template(self.orig_player_template);
        slot.set_start_pos(self.orig_start_pos);
        slot.set_color(self.orig_color);
        slot.save_off_original_info();

        // Then set current info
        slot.set_team_number(self.team_number);
        slot.set_color(self.color);
        slot.set_start_pos(self.start_pos);
        slot.set_player_template(self.player_template);

        slot
    }
}

/// Serializable snapshot of GameInfo for save/load
/// Matches C++ GameInfo xfer structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfoSnapshot {
    version: u32,
    preorder_mask: i32,
    crc_interval: i32,
    in_game: bool,
    in_progress: bool,
    surrendered: bool,
    game_id: i32,
    slots: Vec<GameSlotSnapshot>,
    local_ip: u32,
    map_name: String,
    map_crc: u32,
    map_size: u32,
    map_mask: i32,
    seed: i32,
    superweapon_restriction: u16,
    starting_cash: u32,
}

impl From<&GameInfo> for GameInfoSnapshot {
    fn from(info: &GameInfo) -> Self {
        let slots: Vec<GameSlotSnapshot> = (0..MAX_SLOTS)
            .map(|i| {
                info.get_slot(i)
                    .map(GameSlotSnapshot::from)
                    .unwrap_or_else(|| GameSlotSnapshot::from(&GameSlot::new()))
            })
            .collect();

        Self {
            version: SKIRMISH_GAME_INFO_VERSION,
            preorder_mask: info.preorder_mask,
            crc_interval: info.get_crc_interval(),
            in_game: info.is_in_game(),
            in_progress: info.is_game_in_progress(),
            surrendered: info.have_we_surrendered(),
            game_id: info.get_game_id(),
            slots,
            local_ip: info.get_local_ip(),
            map_name: info.get_map().to_string(),
            map_crc: info.get_map_crc(),
            map_size: info.get_map_size(),
            map_mask: info.get_map_contents_mask(),
            seed: info.get_seed(),
            superweapon_restriction: info.get_superweapon_restriction(),
            starting_cash: info.get_starting_cash().count_money(),
        }
    }
}

impl GameInfoSnapshot {
    /// Convert snapshot back to GameInfo
    pub fn to_game_info(&self) -> GameInfo {
        let mut info = GameInfo::new();

        info.preorder_mask = self.preorder_mask;
        info.set_crc_interval(self.crc_interval);
        info.set_in_game();
        if self.in_progress {
            info.set_game_in_progress(true);
        }
        if self.surrendered {
            info.mark_as_surrendered();
        }
        info.game_id = self.game_id;
        info.set_local_ip(self.local_ip);
        info.set_map(self.map_name.clone());
        info.set_map_crc(self.map_crc);
        info.set_map_size(self.map_size);
        info.set_map_contents_mask(self.map_mask);
        info.set_seed(self.seed);
        info.set_superweapon_restriction(self.superweapon_restriction);

        let mut starting_cash = Money::new(0);
        starting_cash.init();
        starting_cash.deposit(self.starting_cash);
        info.set_starting_cash(starting_cash);

        // Restore slots
        for (i, slot_snapshot) in self.slots.iter().enumerate() {
            if i < MAX_SLOTS {
                let slot = slot_snapshot.to_game_slot();
                info.set_slot(i, slot);
            }
        }

        info
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("Serialization error: {}", e))
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        bincode::deserialize(data).map_err(|e| format!("Deserialization error: {}", e))
    }

    /// Check if version is compatible
    pub fn is_version_compatible(&self) -> bool {
        self.version <= SKIRMISH_GAME_INFO_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skirmish_game_info_serialization() {
        let mut info = SkirmishGameInfo::new();
        info.game_info_mut().set_seed(12345);
        info.game_info_mut().set_map("TestMap.map".to_string());

        let bytes = info.to_bytes().unwrap();
        let info2 = SkirmishGameInfo::from_bytes(&bytes).unwrap();

        assert_eq!(info2.game_info().get_seed(), 12345);
        assert_eq!(info2.game_info().get_map(), "TestMap.map");
    }

    #[test]
    fn test_game_slot_snapshot() {
        let mut slot = GameSlot::new();
        slot.set_state(SlotState::Player, "TestPlayer".to_string(), 0x12345678);
        slot.set_color(5);
        slot.set_player_template(2);

        let snapshot = GameSlotSnapshot::from(&slot);
        let restored = snapshot.to_game_slot();

        assert_eq!(restored.get_name(), "TestPlayer");
        assert_eq!(restored.get_color(), 5);
        assert_eq!(restored.get_player_template(), 2);
    }

    #[test]
    fn test_game_info_snapshot() {
        let mut info = GameInfo::new();
        info.set_seed(99999);
        info.set_map("MyMap.map".to_string());
        info.set_map_crc(0xDEADBEEF);

        let snapshot = GameInfoSnapshot::from(&info);
        let restored = snapshot.to_game_info();

        assert_eq!(restored.get_seed(), 99999);
        assert_eq!(restored.get_map(), "MyMap.map");
        assert_eq!(restored.get_map_crc(), 0xDEADBEEF);
    }

    #[test]
    fn test_version_compatibility() {
        let snapshot = GameInfoSnapshot {
            version: SKIRMISH_GAME_INFO_VERSION,
            preorder_mask: 0,
            crc_interval: 100,
            in_game: false,
            in_progress: false,
            surrendered: false,
            game_id: 0,
            slots: vec![],
            local_ip: 0,
            map_name: String::new(),
            map_crc: 0,
            map_size: 0,
            map_mask: 0,
            seed: 0,
            superweapon_restriction: 0,
            starting_cash: 10000,
        };

        assert!(snapshot.is_version_compatible());
    }
}
