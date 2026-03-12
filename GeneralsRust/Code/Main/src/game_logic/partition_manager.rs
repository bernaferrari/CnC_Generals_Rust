use std::collections::HashSet;

/// Minimal partition manager mirroring WW3D's map reveal behavior.
#[derive(Debug, Default)]
pub struct PartitionManager {
    revealed_players: HashSet<u32>,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            revealed_players: HashSet::new(),
        }
    }

    /// Permanently reveal the map for the specified player (observer mode).
    pub fn reveal_map_for_player(&mut self, player_id: u32) {
        if self.revealed_players.insert(player_id) {
            crate::fow_rendering::reveal_entire_map_for_player(player_id);
        }
    }

    pub fn has_revealed_map(&self, player_id: u32) -> bool {
        self.revealed_players.contains(&player_id)
    }
}
